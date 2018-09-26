//! Implementation of `ApiClient` for the GDAX API.

pub mod errors;
mod wss;
mod rest;

use openssl::pkey::{PKey, Private};
use chashmap::CHashMap;
use std::collections::HashMap;
use std::borrow::Borrow;
use std::sync::Arc;
use futures::prelude::*;
use serde_derive::{Serialize, Deserialize};
use log::debug;
use crate::api::{
    self,
    Params,
    ApiClient,
    GenerateOrderId,
    Notification,
    NotificationFlags,
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

    /// client order id => server order id
    order_ids: Arc<CHashMap<String, String>>,

    symbols: HashMap<String, Symbol>,
    http_client: hyper::Client<hyper_tls::HttpsConnector<hyper::client::HttpConnector>>,
}

impl Client {
    /// Create a new GDAX API client with given `params`. If `key_pair` is not
    /// `None`, this will enable performing requests to the REST API and will forward
    /// the user data stream.
    ///
    /// # Note
    /// This method will block, fetching the available symbols from GDAX.
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

        let http_client = hyper::Client::builder().build::<_, hyper::Body>(
            hyper_tls::HttpsConnector::new(2)?
        );

        let mut client = Client {
            params,
            keys,
            order_ids: Arc::new(CHashMap::new()),
            symbols: HashMap::new(),
            http_client,
        };

        use tokio::runtime::current_thread;
        debug!("requesting symbols");
        client.symbols = current_thread::Runtime::new()?
            .block_on(client.get_symbols())?;
        debug!("received symbols");

        Ok(client)
    }
}

impl ApiClient for Client {
    type Stream = futures::sync::mpsc::UnboundedReceiver<Notification>;

    fn find_symbol(&self, symbol: &str) -> Option<Symbol> {
        self.symbols.get(&symbol.to_lowercase()).cloned()
    }

    fn stream_with_flags(&self, symbol: Symbol, flags: NotificationFlags) -> Self::Stream {
        self.new_stream(symbol, flags)
    }

    fn order<T: Borrow<Order>>(&self, order: WithSymbol<T>)
        -> Box<Future<Item = Timestamped<OrderAck>, Error = api::errors::OrderError> + Send + 'static>
    {
        self.order_impl(order)
    }

    fn cancel<T: Borrow<Cancel>>(&self, cancel: WithSymbol<T>)
        -> Box<Future<Item = Timestamped<CancelAck>, Error = api::errors::CancelError> + Send + 'static>
    {
        self.cancel_impl(cancel)
    }

    fn ping(&self)
        -> Box<Future<Item = Timestamped<()>, Error = api::errors::Error> + Send + 'static>
    {
        Box::new(Ok(().timestamped()).into_future())
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
