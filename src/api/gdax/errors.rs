use std::fmt;
use hyper::StatusCode;
use api::{self, errors::ErrorKinded};

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Deserialize)]
crate struct GdaxRestError<'a> {
    message: &'a str,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Fail)]
/// An error returned by GDAX REST API.
pub struct RestError {
    /// Error kind.
    pub kind: RestErrorKind,

    /// Description of the error.
    pub error_msg: Option<String>,
}

impl ErrorKinded<api::errors::RestErrorKind<!>> for RestError {
    fn kind(&self) -> api::errors::RestErrorKind<!> {
        if self.kind == RestErrorKind::TooManyRequests {
            return api::errors::RestErrorKind::TooManyRequests;
        }

        if self.kind == RestErrorKind::Timeout {
            return api::errors::RestErrorKind::UnknownStatus;
        }

        if self.kind == RestErrorKind::InternalError {
            return api::errors::RestErrorKind::OtherSide;
        }

        if self.error_msg
            .as_ref()
            .map(|msg| msg.starts_with("request timestamp expired"))
            .unwrap_or(false)
        {
            return api::errors::RestErrorKind::OutsideTimeWindow;
        }
        
        api::errors::RestErrorKind::InvalidRequest
    }
}

impl ErrorKinded<api::errors::RestErrorKind<api::errors::CancelErrorKind>> for RestError {
    fn kind(&self) -> api::errors::RestErrorKind<api::errors::CancelErrorKind> {
        if self.kind == RestErrorKind::NotFound {
            return api::errors::RestErrorKind::Specific(
                api::errors::CancelErrorKind::UnknownOrder
            );
        }

        if self.error_msg
            .as_ref()
            .map(|msg| msg.starts_with("Order already done"))
            .unwrap_or(false)
        {
            return api::errors::RestErrorKind::Specific(
                api::errors::CancelErrorKind::UnknownOrder
            );
        }
        <Self as ErrorKinded<api::errors::RestErrorKind<!>>>::kind(self).into()
    }
}

impl ErrorKinded<api::errors::RestErrorKind<api::errors::OrderErrorKind>> for RestError {
    fn kind(&self) -> api::errors::RestErrorKind<api::errors::OrderErrorKind> {
        if self.error_msg
            .as_ref()
            .map(|msg| msg.starts_with("Insufficient funds"))
            .unwrap_or(false)
        {
            return api::errors::RestErrorKind::Specific(
                api::errors::OrderErrorKind::InsufficientBalance
            );
        }
        <Self as ErrorKinded<api::errors::RestErrorKind<!>>>::kind(self).into()
    }
}

impl RestError {
    crate fn from_gdax_error(status: StatusCode, gdax_error: Option<GdaxRestError>)
        -> Self
    {
        RestError {
            kind: RestErrorKind::from_status_code(status),
            error_msg: gdax_error.map(|error| error.message.to_owned()),
        }
    }
}

impl fmt::Display for RestError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.kind)?;
        if let Some(error_msg) = &self.error_msg {
            write!(f, ": `{}`", error_msg)?;
        }
        Ok(())
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Fail)]
/// Translate an HTTP error code to a GDAX error category.
pub enum RestErrorKind {
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
            StatusCode::NOT_FOUND => NotFound,
            StatusCode::TOO_MANY_REQUESTS => TooManyRequests,
            StatusCode::INTERNAL_SERVER_ERROR => InternalError,
            StatusCode::GATEWAY_TIMEOUT => Timeout,
            other => Unknown(other),
        }
    }
}
