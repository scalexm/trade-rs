//! Utilities for trading on crypto-currencies exchanges. Long term goal is to
//! provide a general enough, unified API for abstracting over various exchanges,
//! hence making it easier to develop cross exchange automated trading strategies.

#![feature(nll)]
#![feature(try_from)]
#![feature(never_type)]
#![feature(crate_visibility_modifier)]
#![feature(no_panic_pow)]
#![warn(missing_docs)]

pub mod api;
pub mod order_book;
pub mod tick;

pub mod prelude {
    //! A prelude for crates using this library. Re-exports the most used types
    //! and traits.

    pub use crate::tick::TickUnit;
    pub use crate::api::{ApiClient, Notification, NotificationFlags};
    pub use crate::api::symbol::{Symbol, WithSymbol};
    pub use crate::api::order_book::{LiveOrderBook, BookState};
    pub use crate::Side;
}

use serde_derive::{Serialize, Deserialize};

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
/// Side of an order (bid or ask).
pub enum Side {
    /// Bid side.
    Bid,

    /// Ask side.
    Ask,
}
