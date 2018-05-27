pub mod binance;

use crate::*;
use notify::Notification;
use futures::prelude::*;

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
/// See https://www.investopedia.com/terms/t/timeinforce.asp.
pub enum TimeInForce {
    GoodTilCanceled,
    ImmediateOrCancel,
    FillOrKilll,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
/// An order to be sent through the API.
pub struct Order {
    /// Order side: `Bid` / buy or `Ask`/ sell.
    pub side: Side,

    /// Order price, stringified.
    pub price: String,

    /// Order size, stringified.
    pub size: String,

    /// Time in force, see https://www.investopedia.com/terms/t/timeinforce.asp.
    pub time_in_force: TimeInForce,

    /// Delay until the order is canceled if not treated by the server.
    pub time_window: u64,

    /// Unique order id used to identify the order, stringified.
    /// Automatically generated if `None`.
    pub order_id: Option<String>,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
/// An acknowledgment that an order has been treated by the server.
pub struct OrderAck {
    /// ID identifiying the order.
    order_id: String,

    /// Time at which the order was treated.
    time: u64,
}

/// A trait implemented by clients of various exchanges API.
pub trait ApiClient {
    /// Type of the underlying `Stream` implementor.
    type Stream: Stream<Item = Notification, Error = ()>;
    //type Future: Future<Item = OrderAck, Error = Error>;

    /// Start streaming notifications.
    fn stream(&self) -> Self::Stream;

    //fn order(&self, order: Order) -> Self::Future;
}
