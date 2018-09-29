use serde_derive::Deserialize;
use failure::Fail;
use futures::prelude::*;
use std::collections::HashMap;
use std::borrow::Borrow;
use hyper::Method;
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
use crate::api::timestamp::{convert_str_timestamp, Timestamped, IntoTimestamped};
use crate::api::query_string::QueryString;
use crate::api::errors::ErrorKinded;
use crate::api::symbol::{Symbol, WithSymbol};
use crate::api::hitbtc::Client;
use crate::api::hitbtc::errors::RestError;

trait AsStr {
    fn as_str(self) -> &'static str;
}

impl AsStr for Side {
    fn as_str(self) -> &'static str {
        match self {
            Side::Ask => "sell",
            Side::Bid => "buy",
        }
    }
}

impl AsStr for OrderType {
    fn as_str(self) -> &'static str {
        match self {
            OrderType::Limit => "limit",
            OrderType::LimitMaker => "limit",
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

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Deserialize)]
#[allow(non_snake_case)]
struct HitBtcOrderAck<'a> {
    clientOrderId: &'a str,
    createdAt: &'a str,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Deserialize)]
#[allow(non_snake_case)]
struct HitBtcCancelAck<'a> {
    updatedAt: &'a str,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Deserialize)]
#[allow(non_snake_case)]
struct HitBtcSymbol<'a> {
    id: &'a str,
    quantityIncrement: &'a str,
    tickSize: &'a str,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Deserialize)]
struct HitBtcBalance<'a> {
    currency: &'a str,
    available: &'a str,
    reserved: &'a str,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Deserialize)]
struct HitBtcError<'a> {
    #[serde(borrow)]
    error: crate::api::hitbtc::errors::HitBtcRestError<'a>,
}

impl Client {
    fn request<K: api::errors::ErrorKind>(
        &self,
        endpoint: &str,
        method: Method,
        query: QueryString,
    ) -> Box<
            Future<Item = hyper::Chunk, Error = api::errors::ApiError<K>>
            + Send
            + 'static
        > where RestError: ErrorKinded<K>
    {
        use hyper::Request;

        let mut request = Request::builder();

        if let Some(keys) = self.keys.as_ref() {
            request.header("Authorization", keys.auth_header.as_bytes());
        }

        let query = query.into_string();

        let address = format!(
            "{}/{}",
            self.params.rest_endpoint,
            endpoint,
        );

        request.method(method)
            .header("User-Agent", &b"hyper"[..])
            .header("Content-Type", &b"application/x-www-form-urlencoded"[..])
            .uri(&address);

        let request = match request.body(query.into()) {
            Ok(request) => request,
            Err(err) => return Box::new(
                Err(err)
                    .map_err(api::errors::RequestError::new)
                    .map_err(api::errors::ApiError::RequestError)
                    .into_future()
            )
        };

        let fut = self.http_client.request(request).and_then(|res| {
            let status = res.status();
            res.into_body().concat2().and_then(move |body| {
                Ok((status, body))
            })
        })
        .map_err(api::errors::RequestError::new)
        .map_err(api::errors::ApiError::RequestError)
        .and_then(|(status, body)| {
            if status != hyper::StatusCode::OK {
                let hit_btc_error: Option<HitBtcError<'_>> = serde_json::from_slice(&body).ok();
                let error = RestError::from_hit_btc_error(status, hit_btc_error.map(|e| e.error));
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
        query.push("timeInForce", order.time_in_force.as_str());
        query.push(
            "quantity",
            order.size.unticked(symbol.size_tick()).borrow() as &str
        );
        query.push(
            "price",
            order.price.unticked(symbol.price_tick()).borrow() as &str
        );
        if let Some(order_id) = &order.order_id {
            query.push("clientOrderId", order_id);
        }

        let fut = self.request("api/2/order", Method::POST, query).and_then(|body| {
            let ack: HitBtcOrderAck<'_> = serde_json::from_slice(&body)
                .map_err(api::errors::RequestError::new)
                .map_err(api::errors::ApiError::RequestError)?;

            let timestamp = convert_str_timestamp(ack.createdAt)
                .map_err(api::errors::RequestError::new)
                .map_err(api::errors::ApiError::RequestError)?;

            Ok(OrderAck {
                order_id: ack.clientOrderId.to_owned(),
            }.with_timestamp(timestamp))
        });
        Box::new(fut)
    }

    crate fn cancel_impl<T: Borrow<Cancel>>(&self, cancel: WithSymbol<T>)
        -> Box<Future<Item = Timestamped<CancelAck>, Error = api::errors::CancelError> + Send + 'static>
    {
        let cancel = (*cancel).borrow();
        let query = QueryString::new();

        let fut = self.request(&format!("api/2/order/{}", cancel.order_id()), Method::DELETE, query)
            .and_then(|body|
        {
            let ack: HitBtcCancelAck<'_> = serde_json::from_slice(&body)
                .map_err(api::errors::RequestError::new)
                .map_err(api::errors::ApiError::RequestError)?;

            let timestamp = convert_str_timestamp(ack.updatedAt)
                .map_err(api::errors::RequestError::new)
                .map_err(api::errors::ApiError::RequestError)?;

            Ok(CancelAck.with_timestamp(timestamp))
        });
        Box::new(fut)
    }

    crate fn balances_impl(&self)
        -> Box<Future<Item = api::Balances, Error = api::errors::Error> + Send + 'static>
    {
        let query = QueryString::new();

        let fut = self.request("api/2/trading/balance", Method::GET, query).and_then(|body|
        {
            let balances: Vec<HitBtcBalance<'_>> = serde_json::from_slice(&body)
                .map_err(api::errors::RequestError::new)
                .map_err(api::errors::ApiError::RequestError)?;

            let balances = balances.into_iter().map(|balance| {
                (balance.currency.to_owned(), api::Balance {
                    free: balance.available.to_owned(),
                    locked: balance.reserved.to_owned(),
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
        let fut = self.request("api/2/public/symbol", Method::GET, query).and_then(|body| {
            let products: Vec<HitBtcSymbol<'_>> = serde_json::from_slice(&body)
                .map_err(api::errors::RequestError::new)
                .map_err(api::errors::ApiError::RequestError)?;

            let mut symbols = HashMap::new();
            for p in products {
                let price_tick = match Tick::tick_size(p.tickSize) {
                    Some(tick) => tick,
                    None => {
                        error!("cannot read price tick for symbol `{}`", p.id);
                        continue;
                    }
                };

                let size_tick = match Tick::tick_size(p.quantityIncrement) {
                    Some(tick) => tick,
                    None => {
                        error!("cannot read size tick for symbol `{}`", p.id);
                        continue;
                    }
                };

                if let Some(symbol) = Symbol::new(p.id, price_tick, size_tick) {
                    symbols.insert(symbol.name().to_lowercase(), symbol);
                } else {
                    error!("symbol name too long: `{}`", p.id);
                }
            }
            Ok(symbols)
        });
        Box::new(fut)
    }
}
