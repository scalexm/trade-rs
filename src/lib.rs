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
extern crate hyper;
extern crate hyper_tls;
#[macro_use] extern crate log;
extern crate num;

pub mod matching_engine;
pub mod queue_reactive;
pub mod notify;
pub mod api;
pub mod order_book;
pub mod tick;

pub use failure::Error;
pub use tick::Tick;
pub use order_book::OrderBook;

/// A price, in ticks.
pub type Price = usize;

/// An identifier which should uniquely determine a trader.
pub type TraderId = usize;

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
/// A liquidity consuming trade.
pub struct Trade {
    /// Size consumed by the trade.
    pub size: usize,

    // Trade timestamp.
    pub time: usize,

    /// Price in ticks.
    pub price: usize,

    /// ID of the buyer.
    pub buyer_id: TraderId,

    /// ID of the seller.
    pub seller_id: TraderId,
}
