use crate::*;
use std::fmt;
use std::collections::BTreeMap;
use std::collections::btree_map::Entry;

#[derive(Clone, PartialEq, Eq, Debug)]
/// An order book.
pub struct OrderBook {
    ask: BTreeMap<Price, usize>,
    bid: BTreeMap<Price, usize>,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
/// Side of a trade (bid or ask).
pub enum Side {
    /// Bid side.
    Bid,

    /// Ask side.
    Ask,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
/// Represent a limit update of the order book.
pub struct LimitUpdate {
    /// Side of the corresponding limit.
    pub side: Side,

    /// Price of the corresponding limit.
    pub price: Price,

    /// Updated size.
    pub size: usize,
}

impl OrderBook {
    /// Return an empty `OrderBook`.
    pub fn new() -> Self {
        OrderBook {
            ask: BTreeMap::new(),
            bid: BTreeMap::new(),
        }
    }

    /// Update the given limit with the given updated size.
    pub fn update(&mut self, update: LimitUpdate) {
        let entry = match update.side {
            Side::Bid => self.bid.entry(update.price),
            Side::Ask => self.ask.entry(update.price),
        };

        match entry {
            Entry::Occupied(mut entry) => *entry.get_mut() = update.size,
            Entry::Vacant(entry) => { entry.insert(update.size); },
        };
    }

    /// Retrieve the size of the given limit.
    /// N.B.: `&mut self` because limits are initialized lazily.
    pub fn size_at_limit(&mut self, side: Side, price: Price) -> usize {
        let entry = match side {
            Side::Bid => self.bid.entry(price),
            Side::Ask => self.ask.entry(price),
        };
        *entry.or_insert(0)
    }
}

const DISPLAY_LIMIT: usize = 5;

impl fmt::Display for OrderBook {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "--- ASK ---\n")?;
        for (&price, &size) in self.ask.iter().rev().take(DISPLAY_LIMIT) {
            if size > 0 {
                write!(f, "{}: {}\n", price, size)?;
            }
        }
        write!(f, "--- BID ---\n")?;
        for (&price, &size) in self.bid.iter().take(DISPLAY_LIMIT) {
            if size > 0 {
                write!(f, "{}: {}\n", price, size)?;
            }
        }
        Ok(())
    }
}
