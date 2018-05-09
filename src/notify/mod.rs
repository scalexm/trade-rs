use crate::*;

pub enum Side {
    Bid,
    Ask,
}

pub enum Notification {
    Trade(Trade),
    LimitUpdate(Side, Price, usize),
    OrderBookSnapshot(OrderBook),
}

pub trait Notifier {
    fn notify(&mut self, notif: Notification);
}
