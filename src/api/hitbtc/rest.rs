use serde_derive::Deserialize;
use failure::Fail;
use futures::prelude::*;
use std::collections::HashMap;
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
    ) -> impl Future<Item = hyper::Chunk, Error = api::errors::ApiError<K>> + Send + 'static
            where RestError: ErrorKinded<K>
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

        // Unwrap because it is a bug if this fails (header failed to parse or something)
        let request = request.body(query.into()).unwrap();
        self.http_client.request(request).and_then(|res| {
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
        })
    }

    crate fn order_impl(&self, order: WithSymbol<&Order>)
        -> impl Future<Item = Timestamped<OrderAck>, Error = api::errors::OrderError> + Send + 'static
    {
        use std::borrow::Borrow;

        let mut query = QueryString::new();
        let symbol = order.symbol();
        query.push_str("symbol", symbol.name());
        query.push_str("side", order.side.as_str());
        query.push_str("type", order.type_.as_str());
        query.push_str("timeInForce", order.time_in_force.as_str());
        query.push_str(
            "quantity",
            order.size.unticked(symbol.size_tick()).borrow() as &str
        );
        query.push_str(
            "price",
            order.price.unticked(symbol.price_tick()).borrow() as &str
        );
        if let Some(order_id) = &order.order_id {
            query.push_str("clientOrderId", order_id);
        }

        self.request("api/2/order", Method::POST, query).and_then(|body| {
            let ack: HitBtcOrderAck<'_> = serde_json::from_slice(&body)
                .map_err(api::errors::RequestError::new)
                .map_err(api::errors::ApiError::RequestError)?;

            let timestamp = convert_str_timestamp(ack.createdAt)
                .map_err(api::errors::RequestError::new)
                .map_err(api::errors::ApiError::RequestError)?;

            Ok(OrderAck {
                order_id: ack.clientOrderId.to_owned(),
            }.with_timestamp(timestamp))
        })
    }

    crate fn cancel_impl(&self, cancel: WithSymbol<&Cancel>)
        -> impl Future<Item = Timestamped<CancelAck>, Error = api::errors::CancelError> + Send + 'static
    {
        let endpoint = format!("api/2/order/{}", cancel.order_id());
        let query = QueryString::new();

        self.request(&endpoint, Method::DELETE, query).and_then(|body| {
            let ack: HitBtcCancelAck<'_> = serde_json::from_slice(&body)
                .map_err(api::errors::RequestError::new)
                .map_err(api::errors::ApiError::RequestError)?;

            let timestamp = convert_str_timestamp(ack.updatedAt)
                .map_err(api::errors::RequestError::new)
                .map_err(api::errors::ApiError::RequestError)?;

            Ok(CancelAck.with_timestamp(timestamp))
        })
    }

    crate fn balances_impl(&self)
        -> impl Future<Item = api::Balances, Error = api::errors::Error> + Send + 'static
    {
        let query = QueryString::new();

        self.request("api/2/trading/balance", Method::GET, query).and_then(|body| {
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
        })
    }

    crate fn get_symbols(&self)
        -> impl Future<Item = HashMap<String, Symbol>, Error = api::errors::Error> + Send + 'static
    {
        let query = QueryString::new();

        self.request("api/2/public/symbol", Method::GET, query).and_then(|body| {
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
        })
    }
}
