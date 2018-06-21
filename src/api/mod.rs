//! A unified API for abstracting over various exchanges.

pub mod binance;
pub mod gdax;
pub mod errors;
mod params;
mod wss;

use crate::*;
use order_book::LimitUpdate;
use futures::prelude::*;

pub use self::params::*;

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
/// See https://www.investopedia.com/terms/t/timeinforce.asp.
pub enum TimeInForce {
    GoodTilCanceled,
    ImmediateOrCancel,
    FillOrKilll,
}

trait AsStr {
    fn as_str(&self) -> &'static str;
}

impl AsStr for Side {
    fn as_str(&self) -> &'static str {
        match self {
            Side::Ask => "SELL",
            Side::Bid => "BUY",
        }
    }
}

impl AsStr for TimeInForce {
    fn as_str(&self) -> &'static str {
        match self {
            TimeInForce::GoodTilCanceled => "GTC",
            TimeInForce::FillOrKilll => "FOK",
            TimeInForce::ImmediateOrCancel => "IOC",
        }
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
/// An order to be sent through the API.
pub struct Order {
    /// Order price.
    pub price: Price,

    /// Order size.
    pub size: Size,

    /// Order side: `Bid` / buy or `Ask`/ sell.
    pub side: Side,

    /// Time in force, see https://www.investopedia.com/terms/t/timeinforce.asp.
    pub time_in_force: TimeInForce,

    /// Delay until the order becomes invalid if not treated by the server, in ms.
    /// Unusable on gdax, the exchange forces the time window to be 30s.
    pub time_window: u64,

    /// Unique id used to identify this order, stringified.
    /// Automatically generated if `None`.
    pub order_id: Option<String>,
}

impl Order {
    pub fn new(price: Price, size: Size, side: Side) -> Self {
        Order {
            price,
            size,
            side,
            time_in_force: TimeInForce::GoodTilCanceled,
            time_window: 5000,
            order_id: None,
        }
    }

    pub fn time_window(mut self, time_window: u64) -> Self {
        self.time_window = time_window;
        self
    }

    pub fn order_id(mut self, order_id: String) -> Self {
        self.order_id = Some(order_id);
        self
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
/// An order to cancel a previous order.
pub struct Cancel {
    /// Identify the order to cancel.
    pub order_id: String,

    /// Delay until the cancel order becomes invalid if not treated by the server, in ms.
    pub time_window: u64,
}

impl Cancel {
    pub fn new(order_id: String) -> Self {
        Cancel {
            order_id,
            time_window: 5000,
        }
    }

    pub fn time_window(mut self, time_window: u64) -> Self {
        self.time_window = time_window;
        self
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
/// An acknowledgment that an order has been treated by the server.
pub struct OrderAck {
    /// Timestamp at which the order was treated, in ms.
    pub timestamp: u64,

    /// ID identifiying the order.
    pub order_id: String,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
/// An acknowledgment that a cancel order has been treated by the server.
pub struct CancelAck {
    /// ID identifying the canceled order.
    pub order_id: String,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
/// A notification that some order has been updated, i.e. a trade crossed through this order.
pub struct OrderUpdate {
    /// Timestamp at which the update happened, in ms.
    pub timestamp: u64,

    /// ID identifying the order being updated.
    pub order_id: String,

    /// Size just consumed by last trade.
    pub consumed_size: Size,

    /// Total remaining size for this order (can be maintained in a standalone way
    /// using the size of the order at insertion time, `consumed_size` and `commission`).
    pub remaining_size: Size,

    /// Price at which the last trade happened.
    pub consumed_price: Price,

    /// Commission amount (warning: for binance this may not be in the same currency as
    /// the traded asset).
    pub commission: Size,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
/// A liquidity consuming order.
pub struct Trade {
    // Trade timestamp, in ms.
    pub timestamp: u64,

    /// Price in ticks.
    pub price: Price,

    /// Size consumed by the trade.
    pub size: Size,

    /// Side of the maker:
    /// * if `Ask`, then the maker was providing liquidity on the ask side,
    ///   i.e. the consumer bought to the maker
    /// * if `Bid`, then the maker was providing liquidity on the bid side,
    ///   i.e. the consumer sold to the maker
    pub maker_side: Side,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
/// A notification that some order has expired or was canceled.
pub struct OrderExpired {
    /// Expiration timestamp, in ms.
    pub timestamp: u64,

    /// Expired order.
    pub order_id: String,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
/// A notification that some order has been received by the exchange.
pub struct OrderReceived {
    /// Timestamp at which the order was received.
    pub timestamp: u64,

    /// Unique order id.
    pub order_id: String,

    /// Price at which the order was inserted.
    pub price: Price,

    /// Size at which the order was inserted.
    pub size: Price,

    /// Side of the order.
    pub side: Side,
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
/// A notification that some event happened.
pub enum Notification {
    /// A trade was executed.
    Trade(Trade),

    /// The limit order book has changed and should be updated.
    LimitUpdates(Vec<LimitUpdate>),

    OrderReceived(OrderReceived),

    /// An order has been updated.
    OrderUpdate(OrderUpdate),

    /// An order has expired or was canceled.
    OrderExpired(OrderExpired),
}

/// A trait implemented by clients of various exchanges API.
pub trait ApiClient {
    /// Type returned by the `stream` implementor, used for continuously receiving
    /// notifications.
    type Stream: Stream<Item = Notification, Error = ()> + Send + 'static;

    /// Start streaming notifications.
    fn stream(&self) -> Self::Stream;

    /// Send an order to the exchange.
    fn order(&self, order: &Order)
        -> Box<Future<Item = OrderAck, Error = errors::OrderError> + Send + 'static>;

    /// Send a cancel order to the exchange.
    fn cancel(&self, cancel: &Cancel)
        -> Box<Future<Item = CancelAck, Error = errors::CancelError> + Send + 'static>;

    /// Send a ping to the exchange.
    fn ping(&self)
        -> Box<Future<Item = (), Error = errors::Error> + Send + 'static>;

    /// Generate an order id. When possible, the result will be equal to `hint`, otherwise
    /// it is assured that all strings generated by a call to this method are distinct.
    fn new_order_id(hint: &str) -> String;
}
