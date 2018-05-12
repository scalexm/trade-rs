use crate::*;

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
/// Side of a trade.
pub enum Side {
    Bid,
    Ask,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct LimitUpdate {
    pub side: Side,
    pub price: Price,
    pub size: usize,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Notification {
    Trade(Trade),
    LimitUpdates(Vec<LimitUpdate>),
}

pub trait Notifier {
    fn notify(&mut self, notif: Notification);
}
