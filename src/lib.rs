#![feature(crate_in_paths)]
#![feature(crate_visibility_modifier)]
#![feature(nll)]

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

pub mod matching_engine;
pub mod api;
pub mod order_book;
pub mod tick;

pub use failure::Error;

pub use tick::*;
pub use order_book::*;

/// A price, in ticks.
pub type Price = u64;

/// Size of an order / trade, in ticks.
pub type Size = u64;

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
/// Side of an order (bid or ask).
pub enum Side {
    /// Bid side.
    Bid,

    /// Ask side.
    Ask,
}

pub fn timestamp_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)
                                     .expect("time went backward");
    timestamp.as_secs() * 1000 + u64::from(timestamp.subsec_millis())
}
