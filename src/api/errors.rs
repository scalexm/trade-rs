//! A module defining error types returned by `ApiClient` implementors.

use failure::{Context, Backtrace};
use failure_derive::Fail;
use std::fmt;

#[doc(hidden)]
pub trait ErrorKind: private::Sealed + failure::Fail + Copy + Sized { }

mod private {
    pub trait Sealed { }
}

#[derive(Debug)]
/// An error coming from the REST API.
pub struct RestError<K: ErrorKind> {
    inner: Context<RestErrorKind<K>>,
}

impl<K: ErrorKind> failure::Fail for RestError<K> {
    fn cause(&self) -> Option<&failure::Fail> {
        use failure::Fail;
        self.inner.cause()
    }

    fn backtrace(&self) -> Option<&Backtrace> {
        use failure::Fail;
        self.inner.backtrace()
    }
}

impl<K: ErrorKind> fmt::Display for RestError<K> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.inner, f)
    }
}

impl<K: ErrorKind> RestError<K> {
    /// Return the kind of the underlying error.
    pub fn kind(&self) -> RestErrorKind<K> {
        *self.inner.get_context()
    }
}

impl<K: ErrorKind> From<RestErrorKind<K>> for RestError<K> {
    fn from(kind: RestErrorKind<K>) -> RestError<K> {
        RestError {
            inner: Context::new(kind),
        }
    }
}

impl<K: ErrorKind> From<Context<RestErrorKind<K>>> for RestError<K> {
    fn from(inner: Context<RestErrorKind<K>>) -> RestError<K> {
        RestError {
            inner,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Fail)]
/// An error kind specific to the `order` API request.
pub enum OrderErrorKind {
    #[fail(display = "insufficient balance")]
    /// Account does not have a sufficient balance for this order.
    InsufficientBalance,

    #[fail(display = "chosen order id already exists")]
    /// The client specified order id is already in use.
    DuplicateOrder,

    #[fail(display = "order would take liquidity")]
    /// The order would take liquidity but was marked as liquidity maker only.
    WouldTakeLiquidity,
}

impl private::Sealed for OrderErrorKind { }
impl ErrorKind for OrderErrorKind { }

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Fail)]
/// An error kind specific to the `cancel` API request.
pub enum CancelErrorKind {
    #[fail(display = "unknown order id")]
    /// The specified order id could not be found.
    UnknownOrder,
}

impl private::Sealed for CancelErrorKind { }
impl ErrorKind for CancelErrorKind { }

impl private::Sealed for ! { }
impl ErrorKind for ! { }

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Fail)]
/// REST error kind.
pub enum RestErrorKind<K: ErrorKind> {
    #[fail(display = "too many requests")]
    /// Too many requests were sent during a given time window, check rate limits.
    TooManyRequests,
    
    #[fail(display = "unknown execution status, could have been a success")]
    /// Execution status are unknown: e.g. timeout.
    UnknownStatus,

    #[fail(display = "invalid request")]
    /// Invalid request, issue on the lib side or consumer side.
    InvalidRequest,

    #[fail(display = "the other side encountered an error")]
    /// Issue on the exchange side.
    OtherSide,

    #[fail(display = "outside specified time window")]
    /// The request timestamp was outside of the specified time window.
    OutsideTimeWindow,

    #[fail(display = "{}", _0)]
    /// More specific error kind, depending on the request being made.
    Specific(K),
}

#[derive(Debug)]
/// An error inherent to the underlying network request.
pub struct RequestError {
    inner: Box<failure::Fail>,
}

impl RequestError {
    crate fn new<E: failure::Fail>(err: E) -> Self {
        RequestError {
            inner: Box::new(err),
        }
    }
}

impl failure::Fail for RequestError {
    fn cause(&self) -> Option<&failure::Fail> {
        self.inner.cause()
    }

    fn backtrace(&self) -> Option<&Backtrace> {
        self.inner.backtrace()
    }
}

impl fmt::Display for RequestError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.inner, f)
    }
}

#[derive(Debug, Fail)]
/// An error type returned by the implementors of `ApiClient`.
pub enum ApiError<K: ErrorKind> {
    #[fail(display = "REST API error")]
    /// An error coming from the REST API.
    RestError(#[cause] RestError<K>),

    #[fail(display = "Network request error")]
    /// An error about the underlying network request.
    RequestError(#[cause] RequestError),
}

/// Error type adding error kinds specific to `ApiClient::order`.
pub type OrderError = ApiError<OrderErrorKind>;

/// Error type adding error kinds specific to `ApiClient::cancel`.
pub type CancelError = ApiError<CancelErrorKind>;

/// Basic error type not adding any specific error kinds.
pub type Error = ApiError<!>;

impl From<RestErrorKind<!>> for RestErrorKind<CancelErrorKind> {
    fn from(err: RestErrorKind<!>) -> RestErrorKind<CancelErrorKind> {
        match err {
            RestErrorKind::TooManyRequests => RestErrorKind::TooManyRequests,
            RestErrorKind::InvalidRequest => RestErrorKind::InvalidRequest,
            RestErrorKind::UnknownStatus => RestErrorKind::UnknownStatus,
            RestErrorKind::OtherSide => RestErrorKind::OtherSide,
            RestErrorKind::OutsideTimeWindow => RestErrorKind::OutsideTimeWindow,
            RestErrorKind::Specific(x) => x,
        }
    }
}

impl From<RestErrorKind<!>> for RestErrorKind<OrderErrorKind> {
    fn from(err: RestErrorKind<!>) -> RestErrorKind<OrderErrorKind> {
        match err {
            RestErrorKind::TooManyRequests => RestErrorKind::TooManyRequests,
            RestErrorKind::InvalidRequest => RestErrorKind::InvalidRequest,
            RestErrorKind::UnknownStatus => RestErrorKind::UnknownStatus,
            RestErrorKind::OtherSide => RestErrorKind::OtherSide,
            RestErrorKind::OutsideTimeWindow => RestErrorKind::OutsideTimeWindow,
            RestErrorKind::Specific(x) => x,
        }
    }
}

crate trait ErrorKinded<K: ErrorKind> {
    fn kind(&self) -> RestErrorKind<K>;
}
