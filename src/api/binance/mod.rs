mod wss;
mod rest;

use api::*;
use tick::Tick;
use openssl::pkey::{PKey, Private};
use hyper::StatusCode;

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
/// Params needed for a binance API client.
pub struct Params {
    /// Currency symbol in lower case, e.g. "trxbtc".
    pub symbol: String,

    /// WebSocket API address.
    pub ws_address: String,

    /// HTTP REST API address.
    pub http_address: String,

    /// Tick unit for prices.
    pub price_tick: Tick,

    /// Tick unit for sizes.
    pub size_tick: Tick,

    /// Tick unit for commissions.
    pub commission_tick: Tick,

    /// Binance API Key.
    pub api_key: String,

    /// Binance secrey key.
    pub secret_key: String,
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
    secret_key: PKey<Private>,
    listen_key: Option<String>,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Fail)]
pub enum RestError {
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

    #[fail(display = "error {}: {}", code, msg)]
    /// Custom error message in the response body.
    Custom {
        code: i32,
        msg: String,
    },

    #[fail(display = "unknown error")]
    /// Unkown error.
    Unknown,
}

impl RestError {
    crate fn from_status_code(code: StatusCode) -> Self {
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
            StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS => RestError::MalformedRequest,

            // 418
            StatusCode::IM_A_TEAPOT => RestError::AddressBanned,

            // 429
            StatusCode::TOO_MANY_REQUESTS => RestError::BrokeRateLimit,

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
            StatusCode::NETWORK_AUTHENTICATION_REQUIRED => RestError::BinanceInternalError,

            // 504
            StatusCode::GATEWAY_TIMEOUT => RestError::Timeout,

            _ => RestError::Unknown,
        }
    }
}

impl Client {
    /// Create a new binance API client with given `params` and request a listen key
    /// for the user data stream. The request will block the thread.
    pub fn new(params: Params) -> Result<Self, Error> {
        let secret_key = PKey::hmac(params.secret_key.as_bytes())?;

        let mut client = Client {
            secret_key,
            params,
            listen_key: None,
        };

        use tokio::runtime::current_thread;
        let key = current_thread::Runtime::new().unwrap()
                                                .block_on(client.get_listen_key())?;

        client.listen_key = Some(key);
        Ok(client)
    }
}

impl ApiClient for Client {
    type Stream = futures::sync::mpsc::UnboundedReceiver<Notification>;
    type FutureOrder = Box<Future<Item = OrderAck, Error = Error>>;
    type FutureCancel = Box<Future<Item = CancelAck, Error = Error>>;
    type FuturePing = Box<Future<Item = (), Error = Error>>;

    fn stream(&self) -> Self::Stream {
        self.new_stream()
    }

    fn order(&self, order: Order) -> Self::FutureOrder {
        self.order_impl(order)
    }

    fn cancel(&self, cancel: Cancel) -> Self::FutureCancel {
        self.cancel_impl(cancel)
    }

    fn ping(&self) -> Self::FuturePing {
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