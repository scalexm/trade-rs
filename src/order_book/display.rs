//! Utilities for displaying order books.

use std::fmt;
use std::cell::Cell;
use crate::order_book::OrderBook;
use crate::tick::{TickUnit, Tick};

thread_local! {
    static DISPLAY_LIMIT: Cell<usize> = Cell::new(5);
    static DISPLAY_PRICE_TICK: Cell<Option<Tick>> = Cell::new(None);
    static DISPLAY_SIZE_TICK: Cell<Option<Tick>> = Cell::new(None);
}

/// Set the thread local display limit for both sides when displaying an order book. 
pub fn set_limit(limit: usize) {
    DISPLAY_LIMIT.with(|dl| dl.set(limit));
}

/// Set the tread local tick size for displaying prices. If `None`, values are
/// displayed in tick units.
pub fn set_price_tick(maybe_tick: Option<Tick>) {
    DISPLAY_PRICE_TICK.with(|dt| dt.set(maybe_tick));
}

/// Set the tread local tick size for displaying sizes. If `None`, values are
/// displayed in tick units.
pub fn set_size_tick(maybe_tick: Option<Tick>) {
    DISPLAY_SIZE_TICK.with(|dt| dt.set(maybe_tick));
}

/// Convert a ticked value to an unticked value with the current thread local price tick.
pub fn displayable_price(ticked: TickUnit) -> String {
    match DISPLAY_PRICE_TICK.with(|dt| dt.get()) {
        Some(tick) => tick.unticked(ticked).unwrap(),
        None => format!("{}", ticked),
    }
}

/// Convert a ticked value to an unticked value with the current thread local size tick.
pub fn displayable_size(ticked: TickUnit) -> String {
    match DISPLAY_SIZE_TICK.with(|dt| dt.get()) {
        Some(tick) => tick.unticked(ticked).unwrap(),
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
