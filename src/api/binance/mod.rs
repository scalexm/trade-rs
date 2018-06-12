mod wss;
mod rest;

use api::*;
use openssl::pkey::{PKey, Private};
use hyper::StatusCode;

pub use api::params::*;

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
/// recommends to send a ping every 30 minutes.
/// If the listen key becomes invalid, this client will stop forwarding the user data stream.
/// The only way to fix it will be to drop the client and create a new one.
pub struct Client {
    params: Params,
    keys: Option<Keys>,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Deserialize)]
struct BinanceRestError {
    code: i32,
    msg: String,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Fail)]
/// An error returned by binance REST API.
pub struct RestError {
    /// Error category.
    pub category: RestErrorCategory,

    /// Internal binance error code: see API documentation.
    pub error_code: Option<i32>,

    /// Description of the error.
    pub error_msg: Option<String>,
}

impl RestError {
    fn from_binance_error(status: StatusCode, binance_error: Option<BinanceRestError>)
        -> Self
    {
        RestError {
            category: RestErrorCategory::from_status_code(status),
            error_code: binance_error.as_ref().map(|error| error.code),
            error_msg: binance_error.map(|error| error.msg),
        }
    }
}

impl std::fmt::Display for RestError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.category)?;
        if let Some(error_msg) = &self.error_msg {
            write!(f, ": `{}`", error_msg)?;
        }
        if let Some(error_code) = self.error_code {
            write!(f, " (error_code = {})", error_code)?;
        }
        Ok(())
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Fail)]
/// Translate an HTTP error code to a binance error category.
pub enum RestErrorCategory {
    #[fail(display = "malformed request")]
    /// Malformed request, issue on the lib side.
    MalformedRequest,

    #[fail(display = "broke rate limit")]
    /// The client broke the request rate limit set by binance. See binance API
    /// documentation for each request weight. A user shouldn't send any more requests
    /// after receiving such an error, or their IP address will be banned.
    BrokeRateLimit,

    #[fail(display = "ip address was banned")]
    /// IP address was banned.
    AddressBanned,

    #[fail(display = "server did not respond within the timeout period")]
    /// The server did not respond in time. The order may have been executed or may have not.
    Timeout,

    #[fail(display = "binance internal error")]
    /// Issue on binance side.
    BinanceInternalError,

    #[fail(display = "unknown error")]
    /// Unkown error.
    Unknown,
}

impl RestErrorCategory {
    fn from_status_code(code: StatusCode) -> Self {
        use self::RestErrorCategory::*;
        match code {
            StatusCode::OK => panic!("`RestError::from_status_code` with `StatusCode::Ok`"),

            // 4XX
            StatusCode::BAD_REQUEST |
            StatusCode::UNAUTHORIZED |
            StatusCode::PAYMENT_REQUIRED |
            StatusCode::FORBIDDEN |
            StatusCode::NOT_FOUND |
            StatusCode::METHOD_NOT_ALLOWED |
            StatusCode::NOT_ACCEPTABLE |
            StatusCode::PROXY_AUTHENTICATION_REQUIRED |
            StatusCode::REQUEST_TIMEOUT |
            StatusCode::CONFLICT |
            StatusCode::GONE |
            StatusCode::LENGTH_REQUIRED |
            StatusCode::PRECONDITION_FAILED |
            StatusCode::PAYLOAD_TOO_LARGE |
            StatusCode::URI_TOO_LONG |
            StatusCode::UNSUPPORTED_MEDIA_TYPE |
            StatusCode::RANGE_NOT_SATISFIABLE |
            StatusCode::EXPECTATION_FAILED |
            StatusCode::MISDIRECTED_REQUEST |
            StatusCode::UNPROCESSABLE_ENTITY |
            StatusCode::LOCKED |
            StatusCode::FAILED_DEPENDENCY |
            StatusCode::UPGRADE_REQUIRED |
            StatusCode::PRECONDITION_REQUIRED |
            StatusCode::REQUEST_HEADER_FIELDS_TOO_LARGE |
            StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS => MalformedRequest,

            // 418
            StatusCode::IM_A_TEAPOT => AddressBanned,

            // 429
            StatusCode::TOO_MANY_REQUESTS => BrokeRateLimit,

            // 5XX
            StatusCode::INTERNAL_SERVER_ERROR |
            StatusCode::NOT_IMPLEMENTED |
            StatusCode::BAD_GATEWAY |
            StatusCode::SERVICE_UNAVAILABLE |
            StatusCode::HTTP_VERSION_NOT_SUPPORTED |
            StatusCode::VARIANT_ALSO_NEGOTIATES |
            StatusCode::INSUFFICIENT_STORAGE |
            StatusCode::LOOP_DETECTED |
            StatusCode::NOT_EXTENDED |
            StatusCode::NETWORK_AUTHENTICATION_REQUIRED => BinanceInternalError,

            // 504
            StatusCode::GATEWAY_TIMEOUT => Timeout,

            _ => Unknown,
        }
    }
}

impl Client {
    /// Create a new binance API client with given `params` and request a listen key
    /// for the user data stream. The request will block the thread.
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
        self.ping_impl()
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Deserialize)]
/// Account balance for one asset.
pub struct Balance {
    /// Symbol name.
    pub asset: String,

    /// Available amount, unticked.
    pub free: String,

    /// Locked amount, unticked.
    pub locked: String,
}

#[derive(Clone, PartialEq, Eq, Debug)]
/// Account information for this client.
pub struct AccountInformation {
    pub maker_commission: u64,
    pub taker_commission: u64,
    pub buyer_commission: u64,
    pub seller_commission: u64,
    pub can_trade: bool,
    pub can_withdraw: bool,
    pub can_deposit: bool,
    pub update_timestamp: u64,
    pub balances: Vec<Balance>,
}
