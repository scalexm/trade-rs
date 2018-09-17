//! Implementation of `ApiClient` for the GDAX API.

mod wss;
mod rest;
pub mod errors;

use openssl::pkey::{PKey, Private};
use chashmap::CHashMap;
use std::collections::HashMap;
use std::sync::Arc;
use futures::prelude::*;
use serde_derive::{Serialize, Deserialize};
use crate::api::{
    self,
    Params,
    ApiClient,
    GenerateOrderId,
    Notification,
    Order,
    OrderAck,
    Cancel,
    CancelAck,
    Balances
};
use crate::api::symbol::{Symbol, WithSymbol};
use crate::api::timestamp::{Timestamped, IntoTimestamped};

#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
/// A GDAX key pair: api key + secret key, along with a pass phrase.
pub struct KeyPair {
    api_key: String,
    secret_key: String,
    pass_phrase: String,
}

impl KeyPair {
    /// Return a new key pair along with the associated pass phrase.
    pub fn new(api_key: String, secret_key: String, pass_phrase: String) -> Self {
        KeyPair {
            api_key,
            secret_key,
            pass_phrase,
        }
    }
}

#[derive(Clone)]
struct Keys {
    api_key: String,
    secret_key: Arc<PKey<Private>>,
    pass_phrase: String,
}

/// A GDAX API client.
pub struct Client {
    params: Params,
    keys: Option<Keys>,

    // client order id => server order id
    order_ids: Arc<CHashMap<String, String>>,

    symbols: HashMap<String, Symbol>,
}

impl Client {
    /// Create a new GDAX API client with given `params`. If `key_pair` is not
    /// `None`, this will enable performing requests to the REST API and will forward
    /// the user data stream.
    pub fn new(params: Params, key_pair: Option<KeyPair>) -> Result<Self, failure::Error> {
        let keys = match key_pair {
            Some(pair) => {
                let secret_key = PKey::hmac(&base64::decode(&pair.secret_key)?)?;

                Some(Keys {
                    api_key: pair.api_key,
                    secret_key: Arc::new(secret_key),
                    pass_phrase: pair.pass_phrase,
                })
            },
            None => None,
        };

        Ok(Client {
            params,
            keys,
            order_ids: Arc::new(CHashMap::new()),
            symbols: HashMap::new(),
        })
    }
}

impl ApiClient for Client {
    type Stream = futures::sync::mpsc::UnboundedReceiver<Notification>;

    fn find_symbol(&self, symbol: &str) -> Option<Symbol> {
        self.symbols.get(&symbol.to_lowercase()).cloned()
    }

    fn stream(&self, symbol: Symbol) -> Self::Stream {
        self.new_stream(symbol)
    }

    fn order(&self, order: &Order)
        -> Box<Future<Item = Timestamped<OrderAck>, Error = api::errors::OrderError> + Send + 'static>
    {
        self.order_impl(order)
    }

    fn cancel(&self, cancel: &Cancel)
        -> Box<Future<Item = Timestamped<CancelAck>, Error = api::errors::CancelError> + Send + 'static>
    {
        self.cancel_impl(cancel)
    }

    fn ping(&self)
        -> Box<Future<Item = Timestamped<()>, Error = api::errors::Error> + Send + 'static>
    {
        Box::new(Ok(().timestamped()).into_future())
    }

    fn params(&self) -> &Params {
        &self.params
    }

    fn balances(&self)
        -> Box<Future<Item = Balances, Error = api::errors::Error> + Send + 'static>
    {
        self.balances_impl()
    }
}

impl GenerateOrderId for Client {
    fn new_order_id(_: &str) -> String {
        use uuid::Uuid;

        Uuid::new_v4().to_string()
    }
}

fn convert_str_timestamp(timestamp: &str) -> Result<u64, chrono::ParseError> {
    use chrono::{DateTime, Utc};

    let time = timestamp.parse::<DateTime<Utc>>()?;
    Ok((time.timestamp() as u64) * 1000 + u64::from(time.timestamp_subsec_millis()))
}
