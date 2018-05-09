#![feature(crate_in_paths)]

pub mod matching_engine;
pub mod queue_reactive;
pub mod notify;
pub mod api;

/// A price, in ticks.
pub type Price = usize;

/// An identifier which should uniquely determine a trader.
pub type TraderId = usize;

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
/// A liquidity consuming trade.
pub struct Trade {
    /// Size consumed by the trade.
    size: usize,

    // Trade timestamp.
    time: usize,

    /// Price in ticks.
    price: usize,

    /// ID of the buyer.
    buyer_id: TraderId,

    /// ID of the seller.
    seller_id: TraderId,
}