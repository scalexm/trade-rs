//! A module defining a simple data structure representing an order book.

mod test;

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

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
/// Represent a limit update of the order book.
pub struct LimitUpdate {
    /// Price of the corresponding limit.
    pub price: Price,

    /// Updated size.
    pub size: Size,

    /// Side of the corresponding limit.
    pub side: Side,
}

impl LimitUpdate {
    pub fn new(price: Price, size: Size, side: Side) -> Self {
        LimitUpdate {
            price,
            size,
            side,
        }
    }
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
    /// 
    /// # Complexity
    /// `O(1)`.
    pub fn best_bid(&self) -> Price {
        self.bid().next().map(|(price, _)| *price).unwrap_or(0)
    }

    /// Return best ask price. If the ask side is empty, return `Price::max_value()`.
    /// 
    /// # Complexity
    /// `O(1)`.
    pub fn best_ask(&self) -> Price {
        self.ask().next().map(|(price, _)| *price).unwrap_or(Price::max_value())
    }

    /// Update the given limit with the given updated size.
    /// 
    /// # Complexity
    /// `O(log(n))` where `n` is the number of limits at the given side.
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
    /// 
    /// # Complexity
    /// `O(log(n))` where `n` is the number of limits at the given side.
    pub fn size_at_limit(&self, side: Side, price: Price) -> Size {
        let size = match side {
            Side::Bid => self.bid.get(&price),
            Side::Ask => self.ask.get(&price),
        };
        size.cloned().unwrap_or(0)
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

    /// Return an iterator over the set of limit updates to apply to `self` in
    /// order to be equal to `other`.
    /// 
    /// # Complexity
    /// `O(n + m)` where `n` is `self`'s length and `m` is `other`'s length.
    /// 
    /// # Example
    /// ```
    /// # extern crate trade;
    /// # use trade::order_book::OrderBook;
    /// # fn main() {
    /// # let mut order_book1 = OrderBook::new();
    /// # let order_book2 = OrderBook::new();
    /// for u in order_book1.diff(&order_book2) {
    ///     order_book1.update(u);
    /// }
    /// assert_eq!(order_book1, order_book2);
    /// # }
    /// ```
    pub fn diff(&self, other: &OrderBook) -> impl Iterator<Item = imitUpdate> {
        use std::collections::HashMap;

        let mut updates = Vec::new();

        let mut compute_diff = |entries: &BTreeMap<_, _>, other_entries, side| {
            let mut entries: HashMap<_, _> = entries.iter().map(|(x, y)| (*x, *y)).collect();

            for (&price, &other_size) in other_entries {
                let need_update = entries.remove(&price)
                    .map(|size| size != other_size)
                    .unwrap_or(true);

                if need_update {
                    updates.push(LimitUpdate::new(price, other_size, side));
                }
            }

            for (price, _) in entries {
                updates.push(LimitUpdate::new(price, 0, side));
            }
        };

        compute_diff(&self.bid, &other.bid, Side::Bid);
        compute_diff(&self.ask, &other.ask, Side::Ask);

        updates.into_iter()
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
