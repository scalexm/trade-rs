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

    /// Binance API Key.
    pub api_key: String,

    /// Binance secrey key.
    pub secret_key: String,
}

/// A binance API client.
pub struct Client {
    params: Params,
    secret_key: PKey<Private>,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Fail)]
pub enum RestError {
    #[fail(display = "malformed request")]
    MalformedRequest,

    #[fail(display = "broke rate limit")]
    BrokeRateLimit,

    #[fail(display = "ip address was banned")]
    AddressBanned,

    #[fail(display = "server did not respond within the timeout period")]
    Timeout,

    #[fail(display = "binance internal error")]
    BinanceInternalError,

    #[fail(display = "error {}: {}", code, msg)]
    Custom {
        code: i32,
        msg: String,
    },

    #[fail(display = "unknown error")]
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
    /// Create a new API client with given `params`.
    pub fn new(params: Params) -> Self {
        Client {
            secret_key: PKey::hmac(params.secret_key.as_bytes()).unwrap(),
            params,
        }
    }
}

impl ApiClient for Client {
    type Stream = wss::BinanceStream;

    fn stream(&self) -> wss::BinanceStream {
        wss::BinanceStream::new(self.params.clone())
    }
}
