//! Implementation of `ApiClient` for the binance API.

mod wss;
mod rest;
pub mod errors;

use api::*;
use openssl::pkey::{PKey, Private};

#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
/// A binance key pair: api key + secret key.
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

struct Keys {
    api_key: String,
    secret_key: PKey<Private>,
    listen_key: String,
}

/// A binance API client.
/// 
/// The notification stream accessed through `<Client as ApiClient>::stream` is only valid for
/// 24 hours and will automatically stop after the 24 hours mark. Just call `stream` again to
/// get a new one.
/// 
/// The listen key is only valid for 60 minutes after its creation (through `Client::new`).
/// Each `<Client as ApiClient>::ping` request will extend its validity for 60 minutes. Binance
/// recommends sending a ping every 30 minutes.
/// If the listen key becomes invalid, this client will stop forwarding the user data stream.
/// The only way to fix it will be to drop the client and create a new one.
pub struct Client {
    params: Params,
    keys: Option<Keys>,
}

impl Client {
    /// Create a new binance API client with given `params`. If `key_pair` is not
    /// `None`, this will enable performing requests to the REST API and will request
    /// a listen key for the user data stream. The request will block the thread.
    pub fn new(params: Params, key_pair: Option<KeyPair>) -> Result<Self, Error> {
        match key_pair {
            Some(pair) => {
                let secret_key = PKey::hmac(pair.secret_key.as_bytes())?;

                let mut client = Client {
                    params,
                    keys: Some(Keys {
                        api_key: pair.api_key,
                        secret_key,
                        listen_key: String::new(),
                    }),
                };

                use tokio::runtime::current_thread;
                info!("Requesting listen key");
                let listen_key = current_thread::Runtime::new()
                    .unwrap()
                    .block_on(client.get_listen_key())?;
                info!("Received listen key");

                client.keys.as_mut().unwrap().listen_key = listen_key;
                Ok(client)
            }
            None => Ok(Client {
                params,
                keys: None,
            })
        }
    }
}

impl ApiClient for Client {
    type Stream = futures::sync::mpsc::UnboundedReceiver<Notification>;

    fn stream(&self) -> Self::Stream {
        self.new_stream()
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
        self.ping_impl()
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
    fn new_order_id(hint: &str) -> String {
        hint.to_owned()
    }
}
