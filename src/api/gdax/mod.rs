//! Implementation of `ApiClient` for the GDAX API.

mod wss;
mod rest;

use api::*;
use openssl::pkey::{PKey, Private};
use hyper::StatusCode;
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
    /// Malformed request, issue on the lib side or consumer side.
    BadRequest,

    #[fail(display = "unauthorized - invalid API key")]
    /// The API keys were invalid or did not have the right permissions.
    Unauthorized,

    #[fail(display = "forbidden")]
    /// Forbidden.
    Forbidden,

    #[fail(display = "not found")]
    /// Not found, issue on the consumer side (e.g. specified order id wasn't found
    /// by the server when trying to cancel an order).
    NotFound,

    #[fail(display = "too many requests")]
    /// The client broke the request rate limit set by GDAX. See GDAX API
    /// documentation for each request weight and rate limits.
    TooManyRequests,

    #[fail(display = "internal server error")]
    /// Issue on GDAX side.
    InternalError,

    #[fail(display = "unknown")]
    /// Unknown error.
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
        -> Box<Future<Item = OrderAck, Error = Error> + Send + 'static>
    {
        self.order_impl(order)
    }

    fn cancel(&self, cancel: &Cancel)
        -> Box<Future<Item = CancelAck, Error = Error> + Send + 'static>
    {
        self.cancel_impl(cancel)
    }

    fn ping(&self)
        -> Box<Future<Item = (), Error = Error> + Send + 'static>
    {
        Box::new(Ok(()).into_future())
    }
}
