//! A module defining an helper data structure maintaining a live order book.

use std::sync::{Arc, Mutex, MutexGuard};
use crate::order_book::OrderBook;
use crate::api::ApiClient;

/// A self-maintained live order book, updated in the background each time
/// the underlying exchange stream sends an update.
pub struct LiveOrderBook {
    order_book: Arc<Mutex<OrderBook>>,
}

/// State of the order book, indicating whether the underlying stream has
/// disconnected or is still sending notifications.
pub enum BookState<'a> {
    /// Live snapshot of the order book.
    Live(MutexGuard<'a, OrderBook>),

    /// The exchange stream has disconnected (due to e.g. an error or a forced
    /// disconnection), hence the order book has gone out of sync and will never
    /// be live again. A new `LiveOrderBook` must be created.
    Disconnected,
}

impl LiveOrderBook {
    /// Build a self-maintained live order book from an exchange data stream.
    ///
    /// # Note
    /// The call will block until the initial snapshot of the order book has been
    /// received.
    pub fn new<C: ApiClient>(stream: C::Stream) -> LiveOrderBook {
        use std::thread;
        use futures::prelude::*;
        use crate::api::Notification;

        let order_book = Arc::new(Mutex::new(OrderBook::new()));
        let weak = order_book.clone();

        let (sender, receiver) = std::sync::mpsc::sync_channel(0);

        thread::spawn(move || {
            let weak = Arc::downgrade(&weak);
            let mut snapshot = false;

            let fut = stream.for_each(|notif| {
                if let Notification::LimitUpdates(updates) = notif {
                    if let Some(order_book) = weak.upgrade() {
                        let mut order_book = order_book.lock().unwrap();
                        for update in updates {
                            order_book.update(update.into_inner());
                        }

                        if !snapshot {
                            sender.send(()).unwrap();
                            snapshot = true;
                        }
                    } else {
                        // The `LiveOrderBook` object was dropped.
                        return Err(());
                    }
                }
                Ok(())
            });

            use tokio::runtime::current_thread;
            let _ = current_thread::block_on_all(fut);
        });

        let _ = receiver.recv();

        LiveOrderBook {
            order_book,
        }
    }

    /// Return the current state of the order book.
    ///
    /// # Note
    /// This method may return an object holding a mutex lock: avoid keeping it
    /// alive for too long.
    pub fn order_book(&self) -> BookState<'_> {
        if Arc::weak_count(&self.order_book) == 0 {
            // The stream ended and released its weak reference.
            BookState::Disconnected
        } else {
            BookState::Live(self.order_book.lock().unwrap())
        }
    }
}
