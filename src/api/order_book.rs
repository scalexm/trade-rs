//! A module defining an helper data structure maintaining a live order book.

use std::sync::{Arc, RwLock, RwLockReadGuard};
use std::thread;
use futures::prelude::*;
use crate::order_book::OrderBook;
use crate::api::{ApiClient, Notification};

/// A self-maintained live order book, updated each time the underlying
/// exchange stream sends an update.
pub struct LiveOrderBook {
    order_book: Arc<RwLock<OrderBook>>,
}

/// State of the order book, indicating whether the underlying stream has
/// disconnected or is still sending notifications.
pub enum BookState<'a> {
    /// Live snapshot of the order book.
    Live(RwLockReadGuard<'a, OrderBook>),

    /// The exchange stream has disconnected (due to e.g. an error or a forced
    /// disconnection), hence the order book has gone out of sync and will never
    /// be live again. A new `LiveOrderBook` must be created.
    Disconnected,
}

impl LiveOrderBook {
    /// Build a self-maintained live order book from an exchange data stream.
    pub fn new<C: ApiClient>(stream: C::Stream) -> LiveOrderBook {
        let order_book = Arc::new(RwLock::new(OrderBook::new()));
        let weak = order_book.clone();

        thread::spawn(move || {
            let weak = Arc::downgrade(&weak);
            let fut = stream.for_each(|notif| {
                if let Notification::LimitUpdates(updates) = notif {
                    if let Some(order_book) = weak.upgrade() {
                        let mut order_book = order_book.write().unwrap();
                        for update in updates {
                            order_book.update(update.into_inner());
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

        LiveOrderBook {
            order_book,
        }
    }

    /// Return the current state of the order book.
    pub fn order_book(&self) -> BookState<'_> {
        if Arc::weak_count(&self.order_book) == 0 {
            // The stream ended and released its weak reference.
            BookState::Disconnected
        } else {
            BookState::Live(self.order_book.read().unwrap())
        }
    }
}