use crate::*;

pub trait Notifier {
    fn notify_trade(&mut self, trade: Trade);
    fn notify_bid_limit_update(&mut self, price: Price, size: usize);
    fn notify_ask_limit_update(&mut self, price: Price, size: usize);
}