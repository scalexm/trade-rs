pub mod binance;

use crate::*;
use notify::Notification;
use futures::prelude::*;

pub enum ApiAction {
    RequestOrderBookSnapshot,
}

pub trait ApiClient {
    type Stream: Stream<Item = Notification, Error = Never>;

    fn stream(&self) -> Self::Stream;
}
