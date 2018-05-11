use crate::*;

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
/// Side of a trade.
pub enum Side {
    Bid,
    Ask,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Notification {
    Trade(Trade),
    LimitUpdate(Side, Price, usize),
}

pub trait Notifier {
    fn notify(&mut self, notif: Notification);
}
