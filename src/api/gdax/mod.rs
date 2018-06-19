//! Implementation of `ApiClient` for the GDAX API.

mod wss;
mod rest;
pub mod errors;

use api::*;
use openssl::pkey::{PKey, Private};
use base64;
use chashmap::CHashMap;
use std::sync::Arc;

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
}

impl Client {
    /// Create a new GDAX API client with given `params`. If `key_pair` is not
    /// `None`, this will enable performing requests to the REST API and will forward
    /// the user data stream.
    pub fn new(params: Params, key_pair: Option<KeyPair>) -> Result<Self, Error> {
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
        })
    }
}

impl ApiClient for Client {
    type Stream = futures::sync::mpsc::UnboundedReceiver<Notification>;

    fn stream(&self) -> Self::Stream {
        self.new_stream()
    }

    fn order(&self, order: &Order)
        -> Box<Future<Item = OrderAck, Error = api::errors::OrderError> + Send + 'static>
    {
        self.order_impl(order)
    }

    fn cancel(&self, cancel: &Cancel)
        -> Box<Future<Item = CancelAck, Error = api::errors::CancelError> + Send + 'static>
    {
        self.cancel_impl(cancel)
    }

    fn ping(&self)
        -> Box<Future<Item = (), Error = api::errors::Error> + Send + 'static>
    {
        Box::new(Ok(()).into_future())
    }
}
