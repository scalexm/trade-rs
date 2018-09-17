//! Utilities for trading on crypto-currencies exchanges. Long term goal is to
//! provide a general enough, unified API for abstracting over various exchanges,
//! hence making it easier to develop cross exchange automated trading
//! strategies.

#![feature(nll)]
#![feature(try_from)]
#![feature(never_type)]
#![feature(crate_visibility_modifier)]
#![feature(no_panic_pow)]

pub mod api;
pub mod order_book;
pub mod tick;

use serde_derive::{Serialize, Deserialize};

pub use self::tick::TickUnit;
pub use self::order_book::OrderBook;

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
/// Side of an order (bid or ask).
pub enum Side {
    /// Bid side.
    Bid,

    /// Ask side.
    Ask,
}
