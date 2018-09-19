//! A module defining a simple data structure representing an order book.

pub mod display;
mod test;

use std::collections::btree_map::BTreeMap;
use serde_derive::{Serialize, Deserialize};
use crate::Side;
use crate::tick::TickUnit;

#[derive(Clone, PartialEq, Eq, Debug, Default)]
/// An order book. Internally uses two `BTreeMap`, one
/// for the bid side and another one for the ask side.
pub struct OrderBook {
    ask: BTreeMap<TickUnit, TickUnit>,
    bid: BTreeMap<TickUnit, TickUnit>,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
/// Represent a limit update of the order book.
pub struct LimitUpdate {
    /// Price of the corresponding limit.
    pub price: TickUnit,

    /// Updated size.
    pub size: TickUnit,

    /// Side of the corresponding limit.
    pub side: Side,
}

impl LimitUpdate {
    /// Return a new `LimitUpdate`.
    pub fn new(price: TickUnit, size: TickUnit, side: Side) -> Self {
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
    pub fn best_bid(&self) -> TickUnit {
        self.bid().next().map(|(price, _)| *price).unwrap_or(0)
    }

    /// Return best ask price. If the ask side is empty, return `TickUnit::max_value()`.
    /// 
    /// # Complexity
    /// `O(1)`.
    pub fn best_ask(&self) -> TickUnit {
        self.ask().next().map(|(price, _)| *price).unwrap_or(TickUnit::max_value())
    }

    /// Update the given limit with the given updated size.
    /// 
    /// # Complexity
    /// `O(log(n))` where `n` is the number of limits at the given side.
    pub fn update(&mut self, update: LimitUpdate) {
        use std::collections::btree_map::Entry;

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
    pub fn size_at_limit(&self, side: Side, price: TickUnit) -> TickUnit {
        let size = match side {
            Side::Bid => self.bid.get(&price),
            Side::Ask => self.ask.get(&price),
        };
        size.cloned().unwrap_or(0)
    }

    /// Iterator over the limits at bid, sorted by
    /// descending key.
    pub fn bid(&self) -> impl Iterator<Item = (&TickUnit, &TickUnit)> {
        self.bid.iter().rev()
    }

    /// Iterator over the limits at ask, sorted by
    /// ascending key.
    pub fn ask(&self) -> impl Iterator<Item = (&TickUnit, &TickUnit)> {
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
    pub fn diff(&self, other: &OrderBook) -> impl Iterator<Item = LimitUpdate> {
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
