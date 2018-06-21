//! Utilities for trading on crypto-currencies exchanges. Long term goal is to
//! provide a general enough, unified API for abstracting over various exchanges,
//! hence making it easier to develop cross exchange automated trading
//! strategies.

#![feature(crate_in_paths)]
#![feature(crate_visibility_modifier)]
#![feature(nll)]
#![feature(try_from)]
#![feature(never_type)]

extern crate ws;
extern crate futures;
extern crate serde_json;
#[macro_use] extern crate serde_derive;
#[macro_use] extern crate failure;
#[macro_use] extern crate failure_derive;
extern crate hyper;
extern crate hyper_tls;
extern crate tokio;
#[macro_use] extern crate log;
extern crate num;
extern crate openssl;
extern crate hex;
extern crate chrono;
extern crate base64;
extern crate chashmap;
extern crate uuid;

pub mod matching_engine;
pub mod api;
pub mod order_book;
pub mod tick;

pub use failure::Error;

pub use tick::Tick;
pub use order_book::OrderBook;

/// A price, in ticks.
pub type Price = u64;

/// Size of an order / trade, in ticks.
pub type Size = u64;

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
/// Side of an order (bid or ask).
pub enum Side {
    /// Bid side.
    Bid,

    /// Ask side.
    Ask,
}

/// Return UTC timestamp in milliseconds.
pub fn timestamp_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)
                                     .expect("time went backward");
    timestamp.as_secs() * 1000 + u64::from(timestamp.subsec_millis())
}
