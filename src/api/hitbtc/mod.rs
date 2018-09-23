//! Implementation of `ApiClient` for the HitBTC API.

mod rest;
mod ws;
pub mod errors;

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
    Order,
    OrderAck,
    Cancel,
    CancelAck,
    Balances,
};
use crate::api::symbol::{Symbol, WithSymbol};
use crate::api::timestamp::{Timestamped, IntoTimestamped};

#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
/// An HitBTC key pair: api key + secret key.
pub struct KeyPair {
    api_key: String,
    secret_key: String,
}

impl KeyPair {
    /// Return a new key pair.
    pub fn new(api_key: String, secret_key: String) -> Self {
        KeyPair {
            api_key,
            secret_key,
        }
    }
}

pub struct Client {
    params: Params,
    keys: Option<KeyPair>,
    symbols: HashMap<String, Symbol>,
}

impl Client {
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

    fn stream(&self, symbol: Symbol) -> Self::Stream {
        panic!()
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
