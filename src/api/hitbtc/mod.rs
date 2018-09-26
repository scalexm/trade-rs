//! Implementation of `ApiClient` for the HitBTC API.

pub mod errors;
mod rest;
mod wss;

use serde_derive::{Serialize, Deserialize};
use std::collections::HashMap;
use std::borrow::Borrow;
use log::debug;
use futures::prelude::*;
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
    Balances,
};
use crate::api::symbol::{Symbol, WithSymbol};
use crate::api::timestamp::{Timestamped, IntoTimestamped};

#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
/// An HitBTC key pair: public key + secret key.
pub struct KeyPair {
    public_key: String,
    secret_key: String,
}

impl KeyPair {
    /// Return a new key pair.
    pub fn new(public_key: String, secret_key: String) -> Self {
        KeyPair {
            public_key,
            secret_key,
        }
    }
}

/// An HitBTC API client.
pub struct Client {
    params: Params,
    keys: Option<KeyPair>,
    symbols: HashMap<String, Symbol>,
}

impl Client {
    /// Create a new HitBTC API client with given `params`. If `key_pair` is not
    /// `None`, this will enable performing requests to the REST API and will forward
    /// the user data stream.
    ///
    /// # Note
    /// This method will block, fetching the available symbols from HitBTC.
    pub fn new(params: Params, key_pair: Option<KeyPair>) -> Result<Self, failure::Error> {
        let mut client = Client {
            params,
            keys: key_pair,
            symbols: HashMap::new(),
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
    fn new_order_id(hint: &str) -> String {
        hint.to_owned()
    }
}
