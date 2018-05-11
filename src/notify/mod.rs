use crate::*;

/// Side of a trade.
pub enum Side {
    Bid,
    Ask,
}

pub enum Notification {
    Trade(Trade),
    LimitUpdate(Side, Price, usize),
}

pub trait Notifier {
    fn notify(&mut self, notif: Notification);
}
