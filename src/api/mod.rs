pub mod binance;

use crate::*;
use notify::Notification;
use futures::prelude::*;

/// A trait implemented by clients of various exchanges API.
pub trait ApiClient {
    /// Type of the underlying `Stream` implementor.
    type Stream: Stream<Item = Notification, Error = Never>;

    /// Start streaming notifications.
    fn stream(&self) -> Self::Stream;
}
