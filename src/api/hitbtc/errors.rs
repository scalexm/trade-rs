//! A module defining error types specific to HitBTC.

use failure_derive::Fail;
use serde_derive::Deserialize;
use hyper::StatusCode;
use std::fmt;

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Deserialize)]
pub(super) struct HitBtcRestError<'a> {
    code: i32,
    message: &'a str,
    description: Option<&'a str>,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Fail)]
/// An error returned by HitBTC REST API.
pub struct RestError {
    /// Error kind.
    pub kind: RestErrorKind,

    /// Internal HitBTC error code: see API documentation.
    pub error_code: i32,

    /// Error message.
    pub error_msg: String,

    /// Optional description of the error.
    pub description: Option<String>,
}

impl fmt::Display for RestError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}: `{}` (", self.kind, self.error_msg)?;
        if let Some(description) = &self.description {
            write!(f, "{} ", description)?;
        }
        write!(f, "[error_code = {}])", self.error_code)?;
        Ok(())
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Fail)]
/// Translate an HTTP error code to an HitBTC error category.
pub enum RestErrorKind {
    #[fail(display = "bad request")]
    /// Malformed request, issue on the lib side or consumer side.
    BadRequest,

    #[fail(display = "unauthorized")]
    /// Authorisation required or failed.
    Unauthorized,

    #[fail(display = "forbidden")]
    /// Action is forbidden for API key.
    Forbidden,

    #[fail(display = "too many requests")]
    /// The client broke the request rate limit set by HitBTC. See HitBTC API
    /// documentation for each request weight and rate limits.
    TooManyRequests,

    #[fail(display = "internal server error")]
    /// Issue on HitBTC side.
    InternalError,

    #[fail(display = "service unavailable")]
    /// Service is down for maintenance.
    ServiceUnavailable,

    #[fail(display = "timeout")]
    /// The server did not respond in time. The order may have been executed or may have not.
    Timeout,

    #[fail(display = "unknown error, HTTP status code = {}", _0)]
    /// Unknown error.
    Unknown(StatusCode),
}

impl RestErrorKind {
    fn from_status_code(code: StatusCode) -> Self {
        use self::RestErrorKind::*;
        match code {
            StatusCode::OK => panic!("`RestErrorKind::from_status_code` with `StatusCode::Ok`"),
            StatusCode::BAD_REQUEST => BadRequest,
            StatusCode::UNAUTHORIZED => Unauthorized,
            StatusCode::FORBIDDEN => Forbidden,
            StatusCode::TOO_MANY_REQUESTS => TooManyRequests,
            StatusCode::INTERNAL_SERVER_ERROR => InternalError,
            StatusCode::SERVICE_UNAVAILABLE => ServiceUnavailable,
            StatusCode::GATEWAY_TIMEOUT => Timeout,
            other => Unknown(other),
        }
    }
}
