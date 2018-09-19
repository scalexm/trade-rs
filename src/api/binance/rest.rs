use std::fmt;
use openssl::{sign::Signer, hash::MessageDigest, pkey::{PKey, Private}};
use hyper::{Method, Request, Body};
use futures::prelude::*;
use std::collections::HashMap;
use std::borrow::Borrow;
use failure::Fail;
use serde_derive::Deserialize;
use log::error;
use crate::Side;
use crate::tick::Tick;
use crate::api::{
    self,
    OrderType,
    TimeInForce,
    Order,
    OrderAck,
    Cancel,
    CancelAck,
};
use crate::api::symbol::{Symbol, WithSymbol};
use crate::api::binance::Client;
use crate::api::binance::errors::{RestError, ErrorKinded};
use crate::api::timestamp::{timestamp_ms, Timestamped, IntoTimestamped};

struct QueryString {
    query: String,
}

impl QueryString {
    fn new() -> Self {
        QueryString {
            query: String::new(),
        }
    }

    fn push<P: fmt::Display>(&mut self, name: &str, arg: P) {
        use std::fmt::Write;

        if self.query.is_empty() {
            write!(&mut self.query, "{}={}", name, arg).unwrap();
        } else {
            write!(&mut self.query, "&{}={}", name, arg).unwrap();
        }
    }

    fn into_string(self) -> String {
        self.query
    }

    fn into_string_with_signature(mut self, key: &PKey<Private>) -> String {
        let mut signer = Signer::new(MessageDigest::sha256(), key).unwrap();
        signer.update(self.query.as_bytes()).unwrap();
        let signature = hex::encode(&signer.sign_to_vec().unwrap());
        self.push("signature", &signature);
        self.query
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Deserialize)]
#[allow(non_snake_case)]
struct BinanceOrderAck<'a> {
    clientOrderId: &'a str,
    transactTime: u64,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Deserialize)]
#[allow(non_snake_case)]
struct BinanceCancelAck<'a> {
    origClientOrderId: &'a str,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Deserialize)]
struct BinanceBalance<'a> {
    asset: &'a str,
    free: &'a str,
    locked: &'a str,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Deserialize)]
struct BinanceAccountInformation<'a> {
    #[serde(borrow)]
    balances: Vec<BinanceBalance<'a>>,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Deserialize)]
#[allow(non_snake_case)]
struct BinanceListenKey<'a> {
    listenKey: &'a str,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Deserialize)]
#[allow(non_snake_case)]
#[allow(non_camel_case_types)]
#[serde(tag = "filterType")]
enum BinanceFilter<'a> {
    PRICE_FILTER { tickSize: &'a str },
    LOT_SIZE { stepSize: &'a str },
    MIN_NOTIONAL,
    ICEBERG_PARTS,
    MAX_NUM_ALGO_ORDERS,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Deserialize)]
struct BinanceSymbol<'a> {
    symbol: &'a str,
    #[serde(borrow)]
    filters: Vec<BinanceFilter<'a>>,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Deserialize)]
struct BinanceExchangeInfo<'a> {
    #[serde(borrow)]
    symbols: Vec<BinanceSymbol<'a>>,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
enum Signature {
    No,
    Yes,
}

trait AsStr {
    fn as_str(self) -> &'static str;
}

impl AsStr for Side {
    fn as_str(self) -> &'static str {
        match self {
            Side::Ask => "SELL",
            Side::Bid => "BUY",
        }
    }
}

impl AsStr for OrderType {
    fn as_str(self) -> &'static str {
        match self {
            OrderType::Limit => "LIMIT",
            OrderType::LimitMaker => "LIMIT_MAKER",
        }
    }
}

impl AsStr for TimeInForce {
    fn as_str(self) -> &'static str {
        match self {
            TimeInForce::GoodTilCanceled => "GTC",
            TimeInForce::FillOrKilll => "FOK",
            TimeInForce::ImmediateOrCancel => "IOC",
        }
    }
}

impl Client {
    fn request<K: api::errors::ErrorKind>(
        &self,
        endpoint: &str,
        method: Method,
        query: QueryString,
        sig: Signature
    ) -> Box<
            Future<Item = hyper::Chunk, Error = api::errors::ApiError<K>>
            + Send
            + 'static
        > where RestError: ErrorKinded<K>
    {
        let keys = self.keys.as_ref().expect(
            "cannot perform an HTTP request without a binance key pair"
        );

        let query = match sig {
            Signature::No => query.into_string(),
            Signature::Yes => query.into_string_with_signature(&keys.secret_key),
        };

        let address = format!(
            "{}/{}{}{}",
            self.params.http_address,
            endpoint,
            if query.is_empty() { "" } else { "?" },
            query
        );

        let request = Request::builder()
            .method(method)
            .uri(&address)
            .header("X-MBX-APIKEY", keys.api_key.as_bytes())
            .body(Body::empty());
        
        let request = match request {
            Ok(request) => request,
            Err(err) => return Box::new(
                Err(err)
                    .map_err(api::errors::RequestError::new)
                    .map_err(api::errors::ApiError::RequestError)
                    .into_future()
            )
        };
        
        let https = match hyper_tls::HttpsConnector::new(2) {
            Ok(https) => https,
            Err(err) => return Box::new(
                Err(err)
                    .map_err(api::errors::RequestError::new)
                    .map_err(api::errors::ApiError::RequestError)
                    .into_future()
            ),
        };

        let client = hyper::Client::builder().build::<_, hyper::Body>(https);
        let fut = client.request(request).and_then(|res| {
            let status = res.status();
            res.into_body().concat2().and_then(move |body| {
                Ok((status, body))
            })
        })
        .map_err(api::errors::RequestError::new)
        .map_err(api::errors::ApiError::RequestError)
        .and_then(|(status, body)| {
            if status != hyper::StatusCode::OK {
                let binance_error = serde_json::from_slice(&body);
                let error = RestError::from_binance_error(status, binance_error.ok());
                let kind = error.kind();
                Err(
                    api::errors::ApiError::RestError(error.context(kind).into())
                )?;
            }
            Ok(body)
        });
        Box::new(fut)
    }

    crate fn order_impl<T: Borrow<Order>>(&self, order: WithSymbol<T>)
        -> Box<Future<Item = Timestamped<OrderAck>, Error = api::errors::OrderError> + Send + 'static>
    {
        let symbol = order.symbol();
        let order = (*order).borrow();

        let mut query = QueryString::new();
        query.push("symbol", symbol.name());
        query.push("side", order.side.as_str());
        query.push("type", order.type_.as_str());
        if order.type_ == OrderType::Limit {
            query.push("timeInForce", order.time_in_force.as_str());
        }
        query.push(
            "quantity",
            order.size.unticked(symbol.size_tick()).borrow() as &str
        );
        query.push(
            "price",
            order.price.unticked(symbol.price_tick()).borrow() as &str
        );
        if let Some(order_id) = &order.order_id {
            query.push("newClientOrderId", order_id);
        }
        query.push("recvWindow", order.time_window);
        query.push("timestamp", timestamp_ms());

        let fut = self.request("api/v3/order", Method::POST, query, Signature::Yes)
            .and_then(|body|
        {
            let ack: BinanceOrderAck = serde_json::from_slice(&body)
                .map_err(api::errors::RequestError::new)
                .map_err(api::errors::ApiError::RequestError)?;
            Ok(OrderAck {
                order_id: ack.clientOrderId.to_owned(),
            }.with_timestamp(ack.transactTime))
        });
        Box::new(fut)
    }

    crate fn cancel_impl<T: Borrow<Cancel>>(&self, cancel: WithSymbol<T>)
        -> Box<Future<Item = Timestamped<CancelAck>, Error = api::errors::CancelError> + Send + 'static>
    {
        let symbol = cancel.symbol();
        let cancel = (*cancel).borrow();

        let mut query = QueryString::new();
        query.push("symbol", symbol.name());
        query.push("origClientOrderId", &cancel.order_id);
        query.push("recvWindow", cancel.time_window);
        query.push("timestamp", timestamp_ms());

        let fut = self.request("api/v3/order", Method::DELETE, query, Signature::Yes)
            .and_then(|body|
        {
            let ack: BinanceCancelAck = serde_json::from_slice(&body)
                .map_err(api::errors::RequestError::new)
                .map_err(api::errors::ApiError::RequestError)?;
            Ok(CancelAck {
                order_id: ack.origClientOrderId.to_owned(),
            }.timestamped())
        });
        Box::new(fut)
    }

    crate fn get_listen_key(&self)
        -> Box<Future<Item = String, Error = api::errors::Error> + Send + 'static>
    {
        let query = QueryString::new();
        let fut = self.request("api/v1/userDataStream", Method::POST, query, Signature::No)
            .and_then(|body|
        {
            let key: BinanceListenKey = serde_json::from_slice(&body)
                .map_err(api::errors::RequestError::new)
                .map_err(api::errors::ApiError::RequestError)?;
            Ok(key.listenKey.to_owned())
        });
        Box::new(fut)
    }

    crate fn ping_impl(&self)
        -> Box<Future<Item = Timestamped<()>, Error = api::errors::Error> + Send + 'static>
    {
        let mut query = QueryString::new();
        query.push(
            "listenKey",
            &self.keys.as_ref().expect(
                "cannot perform an HTTP request without a binance key pair"
            ).listen_key
        );

        let fut = self.request("api/v1/userDataStream", Method::PUT, query, Signature::No)
            .and_then(|_| Ok(().timestamped()));
        Box::new(fut)
    }

    crate fn balances_impl(&self)
        -> Box<Future<Item = api::Balances, Error = api::errors::Error> + Send + 'static>
    {
        let mut query = QueryString::new();
        query.push("recvWindow", 30000);
        query.push("timestamp", timestamp_ms());

        let fut = self.request("api/v3/account", Method::GET, query, Signature::Yes)
            .and_then(|body|
        {
            let info: BinanceAccountInformation = serde_json::from_slice(&body)
                .map_err(api::errors::RequestError::new)
                .map_err(api::errors::ApiError::RequestError)?;

            let balances = info.balances.into_iter().map(|balance| {
                (balance.asset.to_owned(), api::Balance {
                    free: balance.free.to_owned(),
                    locked: balance.free.to_owned(),
                })
            }).collect();
            Ok(balances)
        });
        Box::new(fut)
    }

    crate fn get_symbols(&self)
        -> Box<Future<Item = HashMap<String, Symbol>, Error = api::errors::Error> + Send + 'static>
    {
        let query = QueryString::new();
        let fut = self.request("api/v1/exchangeInfo", Method::GET, query, Signature::No)
            .and_then(|body|
        {
            let info: BinanceExchangeInfo = serde_json::from_slice(&body)
                .map_err(api::errors::RequestError::new)
                .map_err(api::errors::ApiError::RequestError)?;

            let mut symbols = HashMap::new();
            for symbol in info.symbols.into_iter() {
                let mut price_tick = None;
                let mut size_tick = None;

                for filter in symbol.filters {
                    #[allow(non_snake_case)]
                    match filter {
                        BinanceFilter::PRICE_FILTER { tickSize } => {
                            price_tick = Tick::tick_size(tickSize);
                        }
                        BinanceFilter::LOT_SIZE { stepSize } => {
                            size_tick = Tick::tick_size(stepSize);
                        }
                        _ => (),
                    }
                }

                if price_tick.is_none() {
                    error!("cannot read price tick for symbol `{}`", symbol.symbol);
                    continue;
                }

                if size_tick.is_none() {
                    error!("cannot read size tick for symbol `{}`", symbol.symbol);
                    continue;
                }

                if let Some(symbol) = Symbol::new(
                    symbol.symbol,
                    price_tick.unwrap(),
                    size_tick.unwrap()
                )
                {
                    symbols.insert(symbol.name().to_lowercase(), symbol);
                } else {
                    error!("symbol name too long: `{}`", symbol.symbol);
                }
            }
            Ok(symbols)
        });
        Box::new(fut)
    }
}
