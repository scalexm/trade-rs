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
extern crate openssl;
extern crate hex;
extern crate chrono;
extern crate base64;
extern crate chashmap;
extern crate uuid;
extern crate arrayvec;

pub mod api;
pub mod order_book;
pub mod tick;

pub use failure::Error;

pub use tick::{Tick, TickUnit};
pub use order_book::OrderBook;

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
/// Side of an order (bid or ask).
pub enum Side {
    /// Bid side.
    Bid,

    /// Ask side.
    Ask,
}
