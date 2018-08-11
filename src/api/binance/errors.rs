use std::fmt;
use hyper::StatusCode;
use api;

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Deserialize)]
pub(super) struct BinanceRestError<'a> {
    code: i32,
    msg: &'a str,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Fail)]
/// An error returned by binance REST API.
pub struct RestError {
    /// Error kind.
    pub kind: RestErrorKind,

    /// Internal binance error code: see API documentation.
    pub error_code: Option<i32>,

    /// Description of the error.
    pub error_msg: Option<String>,
}

pub(super) trait ErrorKinded<K: api::errors::ErrorKind> {
    fn kind(&self) -> api::errors::RestErrorKind<K>;
}

impl ErrorKinded<!> for RestError {
    fn kind(&self) -> api::errors::RestErrorKind<!> {
        if self.kind == RestErrorKind::BrokeRateLimit ||
            self.kind == RestErrorKind::AddressBanned ||
            self.error_code == Some(-1003) ||
            self.error_code == Some(-1015)
        {
            return api::errors::RestErrorKind::TooManyRequests;
        }

        if self.kind == RestErrorKind::Timeout ||
            self.error_code == Some(-1007) ||
            self.error_code == Some(-1006)
        {
            return api::errors::RestErrorKind::UnknownStatus;
        }

        if self.error_code == Some(-1021) {
            return api::errors::RestErrorKind::OutsideTimeWindow;
        }

        if self.kind == RestErrorKind::InternalError ||
            self.error_code == Some(-1001)
        {
            return api::errors::RestErrorKind::OtherSide;
        }
        
        api::errors::RestErrorKind::InvalidRequest
    }
}

impl ErrorKinded<api::errors::CancelErrorKind> for RestError {
    fn kind(&self) -> api::errors::RestErrorKind<api::errors::CancelErrorKind> {
        let unknown_order =
            (self.error_code == Some(-1010) || self.error_code == Some(-2011)) &&
             self.error_msg.as_ref().map(|msg| msg.starts_with("Unknown order")).unwrap_or(false);

        if self.error_code == Some(-2013) || unknown_order {
            return api::errors::RestErrorKind::Specific(
                api::errors::CancelErrorKind::UnknownOrder
            );
        }

        <Self as ErrorKinded<!>>::kind(self).into()
    }
}

impl ErrorKinded<api::errors::OrderErrorKind> for RestError {
    fn kind(&self) -> api::errors::RestErrorKind<api::errors::OrderErrorKind> {
        let order_rejected =
            self.error_code == Some(-1010) ||
            self.error_code == Some(-2010);
        
        if order_rejected &&
            self.error_msg
                .as_ref()
                .map(|msg| msg.starts_with("Duplicate order"))
                .unwrap_or(false)
        {
            return api::errors::RestErrorKind::Specific(
                api::errors::OrderErrorKind::DuplicateOrder
            );
        }

        if order_rejected &&
            self.error_msg
                .as_ref()
                .map(|msg| msg.starts_with("Account has insufficient balance"))
                .unwrap_or(false)
        {
            return api::errors::RestErrorKind::Specific(
                api::errors::OrderErrorKind::DuplicateOrder
            );
        }

        if order_rejected &&
            self.error_msg
                .as_ref()
                .map(|msg| msg.starts_with("Order would immediately match and take"))
                .unwrap_or(false)
        {
            return api::errors::RestErrorKind::Specific(
                api::errors::OrderErrorKind::WouldTakeLiquidity
            );
        }

        <Self as ErrorKinded<!>>::kind(self).into()
    }
}

impl RestError {
    crate fn from_binance_error(status: StatusCode, binance_error: Option<BinanceRestError>)
        -> Self
    {
        RestError {
            kind: RestErrorKind::from_status_code(status),
            error_code: binance_error.as_ref().map(|error| error.code),
            error_msg: binance_error.map(|error| error.msg.to_owned()),
        }
    }
}

impl fmt::Display for RestError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.kind)?;
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
pub enum RestErrorKind {
    #[fail(display = "malformed request")]
    /// Malformed request, issue on the lib side or consumer side.
    MalformedRequest,

    #[fail(display = "broke rate limit")]
    /// The client broke the request rate limit set by binance. See binance API
    /// documentation for each request weight and rate limits.
    /// A user shouldn't send any more requests after receiving such an error, or
    /// their IP address will be banned.
    BrokeRateLimit,

    #[fail(display = "ip address was banned")]
    /// IP address was banned.
    AddressBanned,

    #[fail(display = "timeout")]
    /// The server did not respond in time. The order may have been executed or may have not.
    Timeout,

    #[fail(display = "internal error")]
    /// Issue on binance side.
    InternalError,

    #[fail(display = "unknown error, status code = {}", _0)]
    /// Unknown error.
    Unknown(StatusCode),
}

impl RestErrorKind {
    fn from_status_code(code: StatusCode) -> Self {
        use self::RestErrorKind::*;
        match code {
            StatusCode::OK => panic!("`RestErrorKind::from_status_code` with `StatusCode::Ok`"),

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
            StatusCode::NETWORK_AUTHENTICATION_REQUIRED => InternalError,

            // 504
            StatusCode::GATEWAY_TIMEOUT => Timeout,

            other => Unknown(other),
        }
    }
}
