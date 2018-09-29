use openssl::{sign::Signer, hash::MessageDigest};
use hyper::{Method, Request};
use futures::prelude::*;
use failure::Fail;
use log::{warn, debug, error};
use std::collections::HashMap;
use serde_derive::{Serialize, Deserialize};
use crate::Side;
use crate::tick::Tick;
use crate::api::{
    self,
    TimeInForce,
    OrderType,
    Order,
    OrderAck,
    Cancel,
    CancelAck,
    Balance,
    Balances
};
use crate::api::errors::ErrorKinded;
use crate::api::symbol::{Symbol, WithSymbol};
use crate::api::timestamp::{convert_str_timestamp, timestamp_ms, Timestamped, IntoTimestamped};
use crate::api::gdax::Client;
use crate::api::gdax::errors::RestError;

#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize)]
struct GdaxOrder<'a> {
    size: &'a str,
    price: &'a str,
    side: &'a str,
    product_id: &'a str,
    #[serde(borrow)]
    client_oid: Option<&'a str>,
    time_in_force: &'a str,
    post_only: bool,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Deserialize)]
struct GdaxOrderAck<'a> {
    id: &'a str,
    created_at: &'a str,
    status: &'a str,
    reject_reason: Option<&'a str>,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Deserialize)]
struct GdaxAccount<'a> {
    currency: &'a str,
    available: &'a str,
    hold: &'a str,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Deserialize)]
struct GdaxProduct<'a> {
    id: &'a str,
    base_currency: &'a str,
    quote_increment: &'a str,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Deserialize)]
struct GdaxCurrency<'a> {
    id: &'a str,
    min_size: &'a str,
}

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
        path: &str,
        method: Method,
        body: String
    ) -> impl Future<Item = hyper::Chunk, Error = api::errors::ApiError<K>> + Send + 'static
            where RestError: ErrorKinded<K>
    {
        let address = format!(
            "{}/{}",
            self.params.rest_endpoint,
            path,
        );

        let mut request = Request::builder();

        if let Some(keys) = self.keys.as_ref() {
            let timestamp = timestamp_ms() as f64 / 1000.;
            let mut signer = Signer::new(MessageDigest::sha256(), &keys.secret_key).unwrap();
            let what = format!("{}{}/{}{}", timestamp, method, path, body);
            signer.update(what.as_bytes()).unwrap();
            let signature = base64::encode(&signer.sign_to_vec().unwrap());

            request.header("CB-ACCESS-KEY", keys.api_key.as_bytes())
                .header("CB-ACCESS-SIGN", signature.as_bytes())
                .header("CB-ACCESS-TIMESTAMP", format!("{}", timestamp).as_bytes())
                .header("CB-ACCESS-PASSPHRASE", keys.pass_phrase.as_bytes());
        }

        request.method(method)
            .uri(&address)
            .header("User-Agent", &b"hyper"[..])
            .header("Content-Type", &b"application/json"[..]);
        
        // Unwrap because it is a bug if this fails (header failed to parse or something)
        let request = request.body(body.into()).unwrap();
        self.http_client.request(request).and_then(|res| {
            let status = res.status();
            res.into_body().concat2().and_then(move |body| {
                Ok((status, body))
            })
        })
        .map_err(api::errors::RequestError::new)
        .map_err(api::errors::ApiError::RequestError).and_then(|(status, body)| {
            if status != hyper::StatusCode::OK {
                let gdax_error = serde_json::from_slice(&body);
                let error = RestError::from_gdax_error(status, gdax_error.ok());
                let kind = error.kind();
                Err(
                    api::errors::ApiError::RestError(error.context(kind).into())
                )?;
            }
            Ok(body)
        })
    }

    crate fn order_impl(&self, order: WithSymbol<&Order>)
        -> impl Future<Item = Timestamped<OrderAck>, Error = api::errors::OrderError> + Send + 'static
    {
        use std::borrow::Borrow;

        // Note that GDAX only accepts custom client ids in the form of UUIDs, so there can
        // never be duplicate orders inserted in the `order_ids` map. This is actually quite
        // neat because checking for duplicate orders in a synchronized manner would have been
        // difficult otherwise.

        let symbol = order.symbol();

        let client_oid = order.order_id.clone();
        let time_in_force = order.time_in_force;

        let size = order.size.unticked(symbol.size_tick());
        let price = order.price.unticked(symbol.price_tick());

        let order = GdaxOrder {
            size: size.borrow(),
            price: price.borrow(),
            side: order.side.as_str(),
            product_id: symbol.name(),
            client_oid: client_oid.as_ref().map(|oid| oid.as_ref()),
            time_in_force: time_in_force.as_str(),
            post_only: order.type_ == OrderType::LimitMaker,
        };

        let body = serde_json::to_string(&order).expect("invalid json");

        let order_ids = self.order_ids.clone();

        self.request("orders", Method::POST, body).and_then(move |body| {
            let ack: GdaxOrderAck<'_> = serde_json::from_slice(&body)
                .map_err(api::errors::RequestError::new)
                .map_err(api::errors::ApiError::RequestError)?;

            if ack.status == "rejected" &&
                ack.reject_reason.map(|r| r.starts_with("post only")).unwrap_or(false)
            {
                Err(
                    api::errors::ApiError::RestError(
                        api::errors::RestErrorKind::Specific(
                            api::errors::OrderErrorKind::WouldTakeLiquidity
                        ).into()
                    )
                )?;
            }

            let timestamp = convert_str_timestamp(ack.created_at)
                .map_err(api::errors::RequestError::new)
                .map_err(api::errors::ApiError::RequestError)?;

            let order_id = match client_oid {
                Some(id) => id.clone(),
                None => ack.id.to_owned(),
            };
            order_ids.insert(order_id.clone(), ack.id.to_owned());
            debug!("insert order id {} (from REST)", order_id);

            Ok(OrderAck {
                order_id,
            }.with_timestamp(timestamp))
        })
    }

    crate fn cancel_impl(&self, cancel: WithSymbol<&Cancel>)
        -> Box<dyn Future<Item = Timestamped<CancelAck>, Error = api::errors::CancelError> + Send + 'static>
    {
        let endpoint = match self.order_ids.get(&cancel.order_id) {
            Some(order_id) => format!("orders/{}", *order_id),
            None => {
                warn!("called `cancel` with a not yet inserted order id");
                return Box::new(
                    Err(
                        api::errors::RestErrorKind::Specific(
                            api::errors::CancelErrorKind::UnknownOrder
                        ).into()
                    ).map_err(api::errors::ApiError::RestError).into_future()
                );
            }
        };

        let fut = self.request(&endpoint, Method::DELETE, String::new()).and_then(move |_| {
            Ok(CancelAck.timestamped())
        });
        Box::new(fut)
    }

    crate fn balances_impl(&self)
        -> impl Future<Item = Balances, Error = api::errors::Error> + Send + 'static
    {
        self.request("accounts", Method::GET, String::new()).and_then(|body| {
            let accounts: Vec<GdaxAccount<'_>> = serde_json::from_slice(&body)
                .map_err(api::errors::RequestError::new)
                .map_err(api::errors::ApiError::RequestError)?;
            
            let balances = accounts.into_iter().map(|account| {
                (account.currency.to_owned(), Balance {
                    free: account.available.to_owned(),
                    locked: account.hold.to_owned(),
                })
            }).collect();
            Ok(balances)
        })
    }

    crate fn get_symbols(&self)
        -> impl Future<Item = HashMap<String, Symbol>, Error = api::errors::Error> + Send + 'static
    {
        self.request("products", Method::GET, String::new())
            .join(self.request("currencies", Method::GET, String::new()))
            .and_then(|(body_products, body_currencies)|
        {
            let products: Vec<GdaxProduct<'_>> = serde_json::from_slice(&body_products)
                .map_err(api::errors::RequestError::new)
                .map_err(api::errors::ApiError::RequestError)?;

            let currencies: Vec<GdaxCurrency<'_>> = serde_json::from_slice(&body_currencies)
                .map_err(api::errors::RequestError::new)
                .map_err(api::errors::ApiError::RequestError)?;

            let currencies: HashMap<_, _> = currencies.into_iter()
                .map(|c| (c.id.to_owned(), c))
                .collect();

            let mut symbols = HashMap::new();
            for p in products {
                let price_tick = match Tick::tick_size(p.quote_increment) {
                    Some(tick) => tick,
                    None => {
                        error!("cannot read price tick for symbol `{}`", p.id);
                        continue;
                    }
                };

                let size_tick = match currencies.get(p.base_currency)
                    .and_then(|c| Tick::tick_size(c.min_size))
                {
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
        })
    }
}
