mod wss;
mod rest;

use api::*;
use openssl::pkey::{PKey, Private};
use hyper::StatusCode;
use base64;

pub use api::params::*;

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

struct Keys {
    api_key: String,
    secret_key: PKey<Private>,
    pass_phrase: String,
}

pub struct Client {
    params: Params,
    keys: Option<Keys>,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Deserialize)]
struct GdaxRestError<'a> {
    message: &'a str,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Fail)]
/// An error returned by GDAX REST API.
pub struct RestError {
    /// Error category.
    pub category: RestErrorCategory,

    /// Description of the error.
    pub error_msg: Option<String>,
}

impl RestError {
    fn from_gdax_error<'a>(status: StatusCode, gdax_error: Option<GdaxRestError<'a>>)
        -> Self
    {
        RestError {
            category: RestErrorCategory::from_status_code(status),
            error_msg: gdax_error.map(|error| error.message.to_owned()),
        }
    }
}

impl std::fmt::Display for RestError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.category)?;
        if let Some(error_msg) = &self.error_msg {
            write!(f, ": `{}`", error_msg)?;
        }
        Ok(())
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Fail)]
/// Translate an HTTP error code to a GDAX error category.
pub enum RestErrorCategory {
    #[fail(display = "bad request")]
    BadRequest,

    #[fail(display = "unauthorized - invalid API key")]
    Unauthorized,

    #[fail(display = "forbidden")]
    Forbidden,

    #[fail(display = "not found")]
    NotFound,

    #[fail(display = "too many requests")]
    TooManyRequests,

    #[fail(display = "internal server error")]
    InternalError,

    #[fail(display = "unknown")]
    Unknown,
}

impl RestErrorCategory {
    fn from_status_code(code: StatusCode) -> Self {
        use self::RestErrorCategory::*;
        match code {
            StatusCode::OK => panic!("`RestError::from_status_code` with `StatusCode::Ok`"),
            StatusCode::BAD_REQUEST => BadRequest,
            StatusCode::UNAUTHORIZED => Unauthorized,
            StatusCode::FORBIDDEN => Forbidden,
            StatusCode::NOT_FOUND => NotFound,
            StatusCode::TOO_MANY_REQUESTS => TooManyRequests,
            StatusCode::INTERNAL_SERVER_ERROR => InternalError,
            _ => Unknown,
        }
    }
}

impl Client {
    pub fn new(params: Params, with_pass_phrase: Option<KeyPair>)
        -> Result<Self, Error>
    {
        match with_pass_phrase {
            Some(pair) => {
                let secret_key = PKey::hmac(&base64::decode(&pair.secret_key)?)?;

                Ok(Client {
                    params,
                    keys: Some(Keys {
                        api_key: pair.api_key,
                        secret_key,
                        pass_phrase: pair.pass_phrase,
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
        self.order_impl(order)
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
