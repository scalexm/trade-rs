#![feature(crate_in_paths)]
#![feature(crate_visibility_modifier)]
#![feature(nll)]

extern crate ws;
#[macro_use] extern crate futures;
extern crate serde_json;
extern crate serde;
#[macro_use] extern crate serde_derive;
#[macro_use] extern crate failure;
#[macro_use] extern crate failure_derive;
#[macro_use] extern crate hyper;
extern crate hyper_tls;
#[macro_use] extern crate log;
extern crate num;
extern crate openssl;
extern crate hex;

pub mod matching_engine;
pub mod notify;
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

/// An identifier which should uniquely determine a trader.
pub type TraderId = usize;

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
/// Side of a trade (bid or ask).
pub enum Side {
    /// Bid side.
    Bid,

    /// Ask side.
    Ask,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
/// A liquidity consuming trade.
pub struct Trade {
    /// Size consumed by the trade.
    pub size: Size,

    // Trade timestamp.
    pub time: u64,

    /// Price in ticks.
    pub price: Price,

    /// ID of the buyer.
    pub buyer_id: TraderId,

    /// ID of the seller.
    pub seller_id: TraderId,
}
