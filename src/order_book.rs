use crate::*;
use std::fmt;
use std::collections::btree_map::{BTreeMap, Entry};
use std::cell::Cell;

#[derive(Clone, PartialEq, Eq, Debug, Default)]
/// An order book.
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

    /// Retrieve the size at the given limit.
    /// N.B.: `&mut self` because limits are initialized lazily.
    pub fn size_at_limit(&mut self, side: Side, price: Price) -> Size {
        let entry = match side {
            Side::Bid => self.bid.entry(price),
            Side::Ask => self.ask.entry(price),
        };
        *entry.or_insert(0)
    }

    /// Iterator over the non-empty limits at bid, sorted by
    /// descending key.
    pub fn bid(&self) -> impl Iterator<Item = (&Price, &Size)> {
        self.bid.iter()
                .rev()
                .filter(|(_, &size)| size > 0)
    }

    /// Iterator over the non-empty limits at ask, sorted by
    /// ascending key.
    pub fn ask(&self) -> impl Iterator<Item = (&Price, &Size)> {
        self.ask.iter()
                .filter(|(_, &size)| size > 0)
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
                                   .filter(|(_, &size)| size > 0)
                                   .take(display_limit)
        {
            writeln!(f, "{}:\t{}", displayable_price(price), displayable_size(size))?;
        }
        writeln!(f, "## BID")?;

        Ok(())
    }
}
