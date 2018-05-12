use crate::*;
use order_book::LimitUpdate;

#[derive(Clone, PartialEq, Eq, Debug)]
/// A notification that some event happened.
pub enum Notification {
    /// A trade was executed.
    Trade(Trade),

    /// The limit order book has changed and should be updated.
    LimitUpdates(Vec<LimitUpdate>),
}

/// A trait for receiving notifications.
pub trait Notifier {
    /// Called when a new notification is available.
    fn notify(&mut self, notif: Notification);
}
