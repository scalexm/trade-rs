//! A simple data structure representing an order book.

use crate::*;
use std::fmt;
use std::collections::btree_map::{BTreeMap, Entry};
use std::cell::Cell;

#[derive(Clone, PartialEq, Eq, Debug, Default)]
/// An order book. Internally uses two `BTreeMap`, one
/// for the bid side and another one for the ask side.
pub struct OrderBook {
    ask: BTreeMap<Price, Size>,
    bid: BTreeMap<Price, Size>,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
/// Represent a limit update of the order book.
pub struct LimitUpdate {
    /// Side of the corresponding limit.
    pub side: Side,

    /// Price of the corresponding limit.
    pub price: Price,

    /// Updated size.
    pub size: Size,

    /// Timestamp at which the update happened, in ms.
    pub timestamp: u64,
}

impl OrderBook {
    /// Return an empty `OrderBook`.
    pub fn new() -> Self {
        OrderBook {
            ask: BTreeMap::new(),
            bid: BTreeMap::new(),
        }
    }

    /// Return best bid price. If the bid side is empty, return `0`.
    /// Complexity: `O(1)`.
    pub fn best_bid(&self) -> Price {
        self.bid().next().map(|(price, _)| *price).unwrap_or(0)
    }

    /// Return best ask price. If the ask side is empty, return `Price::max_value()`.
    /// Complexity: `O(1)`.
    pub fn best_ask(&self) -> Price {
        self.ask().next().map(|(price, _)| *price).unwrap_or(Price::max_value())
    }

    /// Update the given limit with the given updated size.
    /// Complexity: `O(log(n))` where `n` is the number of limits at
    /// the given side.
    pub fn update(&mut self, update: LimitUpdate) {
        let entry = match update.side {
            Side::Bid if update.size == 0 => {
                self.bid.remove(&update.price);
                return;
            },
            Side::Ask if update.size == 0 => {
                self.ask.remove(&update.price);
                return;
            },
            Side::Bid => self.bid.entry(update.price),
            Side::Ask => self.ask.entry(update.price),
        };

        match entry {
            Entry::Occupied(mut entry) => *entry.get_mut() = update.size,
            Entry::Vacant(entry) => { entry.insert(update.size); },
        };
    }

    /// Retrieve the size at the given limit.
    /// Complexity: `O(log(n))` where `n` is the number of limits at
    /// the given side.
    pub fn size_at_limit(&self, side: Side, price: Price) -> Size {
        let size = match side {
            Side::Bid => self.bid.get(&price),
            Side::Ask => self.ask.get(&price),
        };
        size.map(|s| *s).unwrap_or(0)
    }

    /// Iterator over the limits at bid, sorted by
    /// descending key.
    pub fn bid(&self) -> impl Iterator<Item = (&Price, &Size)> {
        self.bid.iter()
                .rev()
    }

    /// Iterator over the limits at ask, sorted by
    /// ascending key.
    pub fn ask(&self) -> impl Iterator<Item = (&Price, &Size)> {
        self.ask.iter()
    }
}

thread_local! {
    static DISPLAY_LIMIT: Cell<usize> = Cell::new(5);
    static DISPLAY_PRICE_TICK: Cell<Option<Tick>> = Cell::new(None);
    static DISPLAY_SIZE_TICK: Cell<Option<Tick>> = Cell::new(None);
}

/// Set the thread local display limit for both sides when displaying an order book. 
pub fn display_limit(limit: usize) {
    DISPLAY_LIMIT.with(|dl| dl.set(limit));
}

/// Set the tread local tick size for displaying prices. If `None`, values are displayed in ticks.
pub fn display_price_tick(maybe_tick: Option<Tick>) {
    DISPLAY_PRICE_TICK.with(|dt| dt.set(maybe_tick));
}

/// Set the tread local tick size for displaying sizes. If `None`, values are displayed in ticks.
pub fn display_size_tick(maybe_tick: Option<Tick>) {
    DISPLAY_SIZE_TICK.with(|dt| dt.set(maybe_tick));
}

/// Convert a ticked `Price` in an unticked value with the current thread local price tick.
pub fn displayable_price(ticked: Price) -> String {
    match DISPLAY_PRICE_TICK.with(|dt| dt.get()) {
        Some(tick) => tick.convert_ticked(ticked).unwrap(),
        None => format!("{}", ticked),
    }
}

/// Convert a ticked `Size` in an unticked value with the current thread local size tick.
pub fn displayable_size(ticked: Size) -> String {
    match DISPLAY_SIZE_TICK.with(|dt| dt.get()) {
        Some(tick) => tick.convert_ticked(ticked).unwrap(),
        None => format!("{}", ticked),
    }
}

impl fmt::Display for OrderBook {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let display_limit = DISPLAY_LIMIT.with(|dl| dl.get());

        writeln!(f, "## ASK")?;
        let ask: Vec<_> = self.ask()
            .take(display_limit)
            .collect();
        for (&price, &size) in ask.iter().rev() {
            writeln!(f, "{}:\t{}", displayable_price(price), displayable_size(size))?;
        }

        write!(f, "\n\n")?;
        for (&price, &size) in self.bid()
                                   .take(display_limit)
        {
            writeln!(f, "{}:\t{}", displayable_price(price), displayable_size(size))?;
        }
        writeln!(f, "## BID")?;

        Ok(())
    }
}
