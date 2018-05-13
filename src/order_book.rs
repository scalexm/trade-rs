use crate::*;
use std::fmt;
use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use std::cell::Cell;

#[derive(Clone, PartialEq, Eq, Debug)]
/// An order book.
pub struct OrderBook {
    ask: BTreeMap<Price, u64>,
    bid: BTreeMap<Price, u64>,
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
    pub size: u64,
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
    pub fn size_at_limit(&mut self, side: Side, price: Price) -> u64 {
        let entry = match side {
            Side::Bid => self.bid.entry(price),
            Side::Ask => self.ask.entry(price),
        };
        *entry.or_insert(0)
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

fn convert_price(ticked: u64) -> String {
    match DISPLAY_PRICE_TICK.with(|dt| dt.get()) {
        Some(tick) => tick.convert_ticked(ticked).unwrap(),
        None => format!("{}", ticked),
    }
}

fn convert_size(ticked: u64) -> String {
    match DISPLAY_SIZE_TICK.with(|dt| dt.get()) {
        Some(tick) => tick.convert_ticked(ticked).unwrap(),
        None => format!("{}", ticked),
    }
}

impl fmt::Display for OrderBook {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let display_limit = DISPLAY_LIMIT.with(|dl| dl.get());

        write!(f, "--- ASK ---\n")?;
        let ask: Vec<_> = self.ask.iter()
                                  .filter(|(_, &size)| size > 0)
                                  .take(display_limit)
                                  .collect();
        for (&price, &size) in ask.iter().rev() {
            write!(f, "{}: {}\n", convert_price(price), convert_size(size))?;
        }

        write!(f, "--- BID ---\n")?;
        for (&price, &size) in self.bid.iter()
                                       .rev()
                                       .filter(|(_, &size)| size > 0)
                                       .take(display_limit)
        {
            write!(f, "{}: {}\n", convert_price(price), convert_size(size))?;
        }

        Ok(())
    }
}
