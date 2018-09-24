//! A module defining error types specific to HitBTC.

use failure_derive::Fail;
use serde_derive::Deserialize;
use hyper::StatusCode;
use std::fmt;
use std::borrow::Cow;
use crate::api;

#[derive(Clone, PartialEq, Eq, Hash, Debug, Deserialize)]
pub(super) struct HitBtcRestError<'a> {
    code: i32,
    message: Cow<'a, str>, // error message can contain escaped characters
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

impl api::errors::ErrorKinded<!> for RestError {
    fn kind(&self) -> api::errors::RestErrorKind<!> {
        if self.kind == RestErrorKind::TooManyRequests {
            return api::errors::RestErrorKind::TooManyRequests;
        }

        if self.kind == RestErrorKind::Timeout {
            return api::errors::RestErrorKind::UnknownStatus;
        }

        if self.kind == RestErrorKind::InternalError
            || self.kind == RestErrorKind::ServiceUnavailable
        {
            return api::errors::RestErrorKind::OtherSide;
        }

        api::errors::RestErrorKind::InvalidRequest
    }
}

impl api::errors::ErrorKinded<api::errors::CancelErrorKind> for RestError {
    fn kind(&self) -> api::errors::RestErrorKind<api::errors::CancelErrorKind> {
        if self.kind == RestErrorKind::BadRequest && self.error_code == 20002 {
            return api::errors::RestErrorKind::Specific(
                api::errors::CancelErrorKind::UnknownOrder
            );
        }
        <Self as api::errors::ErrorKinded<!>>::kind(self).into()
    }
}

impl api::errors::ErrorKinded<api::errors::OrderErrorKind> for RestError {
    fn kind(&self) -> api::errors::RestErrorKind<api::errors::OrderErrorKind> {
        if self.kind == RestErrorKind::BadRequest && self.error_code == 20001 {
            return api::errors::RestErrorKind::Specific(
                api::errors::OrderErrorKind::InsufficientBalance
            );
        }

        if self.kind == RestErrorKind::BadRequest && self.error_code == 20008 {
            return api::errors::RestErrorKind::Specific(
                api::errors::OrderErrorKind::DuplicateOrder
            );
        }

        <Self as api::errors::ErrorKinded<!>>::kind(self).into()
    }
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

impl RestError {
    pub(super) fn from_hit_btc_error(status: StatusCode, hit_btc_error: Option<HitBtcRestError>)
        -> Self
    {
        RestError {
            kind: RestErrorKind::from_status_code(status),
            error_code: hit_btc_error.as_ref().map(|error| error.code).unwrap_or(-1),
            error_msg: hit_btc_error.as_ref().map(|error| error.message.to_string())
                .unwrap_or_else(|| "<empty>".to_owned()),
            description: hit_btc_error.and_then(
                |error| error.description.map(|desc| desc.to_owned())
            ),
        }
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
