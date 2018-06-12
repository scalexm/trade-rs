mod wss;
mod rest;

use api::*;
use openssl::pkey::{PKey, Private};

pub use api::params::*;

pub struct WithPassPhrase {
    key_pair: KeyPair,
    pass_phrase: String,
}

impl WithPassPhrase {
    pub fn new(key_pair: KeyPair, pass_phrase: String) -> Self {
        WithPassPhrase {
            key_pair,
            pass_phrase,
        }
    }
}

struct Keys {
    api_key: String,
    secret_key: PKey<Private>,
    pass_phrase: String,
}

pub struct Client {
    params: Params,
    keys: Option<Keys>,
}

impl Client {
    pub fn new(params: Params, with_pass_phrase: Option<WithPassPhrase>)
        -> Result<Self, Error>
    {
        match with_pass_phrase {
            Some(k) => {
                let secret_key = PKey::hmac(k.key_pair.secret_key.as_bytes())?;

                Ok(Client {
                    params,
                    keys: Some(Keys {
                        api_key: k.key_pair.api_key,
                        secret_key,
                        pass_phrase: k.pass_phrase,
                    })
                })
            },
            None => {
                Ok(Client {
                    params,
                    keys: None,
                })
            }
        }
    }
}

impl ApiClient for Client {
    type Stream = futures::sync::mpsc::UnboundedReceiver<Notification>;

    fn stream(&self) -> Self::Stream {
        self.new_stream()
    }

    fn order(&self, order: &Order)
        -> Box<Future<Item = OrderAck, Error = Error> + Send + 'static>
    {
        unimplemented!()
    }

    fn cancel(&self, cancel: &Cancel)
        -> Box<Future<Item = CancelAck, Error = Error> + Send + 'static>
    {
        unimplemented!()
    }

    fn ping(&self)
        -> Box<Future<Item = (), Error = Error> + Send + 'static>
    {
        unimplemented!()
    }
}
