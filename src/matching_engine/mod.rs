/// A complete trading matching engine: can be used for e.g. simulations, or for implementing
/// an exchange.

mod arena;
mod test;

use std::collections::{BTreeMap, Bound};
use self::arena::{Index, Arena};
use std::mem;

/// An identifier which should uniquely determine a trader.
pub type TraderId = usize;

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
/// A limit order at some price limit of the order book.
struct BookEntry {
    /// Size of the limit order.
    size: usize,

    /// Pointer to the next order at this price limit. If `None`, then this entry
    /// is the last one at this price limit.
    next: Option<Index>,

    /// ID of the trader who owns the order.
    trader: TraderId,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
/// Pointers to begin and end of the book entries list.
struct Link {
    head: Index,
    tail: Index,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
/// A price limit in the order book.
struct PriceLimit {
    /// If `link` is `None`, the limit is empty. Else, it gives a
    /// link to the book entries list for that limit.
    link: Option<Link>,
}

/// Prices are represented by non-negative integers: the representation is therefore
/// dependent on the tick.
pub type Price = usize;

type PriceLimits = BTreeMap<Price, PriceLimit>;
type BookEntries = Arena<BookEntry>;

#[derive(Clone, Debug)]
/// A matching engine.
pub struct MatchingEngine {
    /// The various price limits, which are initialized lazily.
    price_limits: PriceLimits,

    /// A memory arena for storing book entries, independently of their actual price limit.
    entries: BookEntries,

    best_bid: Price,
    best_ask: Price,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
/// Side of an order.
pub enum Side {
    Buy,
    Sell,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
/// An order.
pub struct Order {
    /// Order price.
    price: Price,

    /// Order size, represented by a non-negative integer: the representation is therefore
    /// dependent of how much an asset can be split.
    size: usize,

    /// Order side: `Buy` or `Sell`.
    side: Side,

    /// ID of the order owner.
    trader: TraderId,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
enum ExecResult {
    Filled(Order),
    NotExecuted,
}

trait Executor {
    fn exec(&mut self, link: Link, order: Order) -> (Option<Index>, Order);

    fn exec_range<'a, I: Iterator<Item = (&'a Price, &'a mut PriceLimit)>>(
        &mut self,
        order: Order,
        range: I
    ) -> (Option<Price>, ExecResult);
}

impl Executor for BookEntries {
    /// Make an order cross through a price limit. Return the updated order (which accounts for
    /// how much the order was filled), as well as an `Index` which points to the first entry
    /// at this price limit which was not exhausted, if any.
    fn exec(&mut self, link: Link, mut order: Order) -> (Option<Index>, Order) {
        let mut maybe_index = Some(link.head);
        while let Some(index) = maybe_index {
            {
                let entry = self.get_mut(index);
                if entry.size <= order.size {
                    // This entry is exhausted by the incoming order.
                    order.size -= entry.size;
                    entry.size = 0;
                    maybe_index = entry.next;
                } else {
                    // The order has been completely filled.
                    order.size = 0;
                    entry.size -= order.size;
                    break;
                }
            }
            // If we are here, then the entry referenced by `index` has been exhausted.
            // We free it from the arena.
            self.free(index);
        }
        (maybe_index, order)
    }

    /// Make an order cross through a range of price limits. Return an `ExecResult`:
    /// * `ExecResult::Filled(limit, updated_order)` if the order was (partially) filled, with
    ///   `updated_order` accounting for how much the order was filled and `limit` being the
    ///   first price limit which was not exhausted: the best bid or best ask should then be
    ///   updated depending on the side of the order.
    /// * `ExecResult::NotExecuted` if the range was empty.
    fn exec_range<'a, I: Iterator<Item = (&'a Price, &'a mut PriceLimit)>>(
        &mut self,
        order: Order,
        range: I
    ) -> (Option<Price>, ExecResult)
    {
        let mut exec_result = ExecResult::NotExecuted;
        for (price, limit) in range {
            if let Some(link) = limit.link {
                let (maybe_index, order) = self.exec(link, order);
                exec_result = ExecResult::Filled(order);

                match maybe_index {
                    // All the indices prior to `index` were exhausted, hence we update the
                    // beginning of the entries list. Also we are sure that the order was
                    // completely filled, we can return.
                    Some(index) => {
                        limit.link.as_mut().unwrap().head = index;
                        return (Some(*price), exec_result);
                    }

                    // All the entries at this price limit were exhausted, hence we mark
                    // this price limit as empty.
                    None => limit.link = None,
                }
            }
        }
        (None, exec_result)
    }
}

impl MatchingEngine {
    pub fn new(capacity: usize) -> Self {
        MatchingEngine {
            price_limits: PriceLimits::new(),
            entries: BookEntries::new(capacity),
            best_bid: 0,
            best_ask: Price::max_value(),
        }
    }

    pub fn limit(&mut self, order: Order) -> Option<Order> {
        let (maybe_price, exec_result) = match order.side {
            Side::Buy => {
                let range = self.price_limits.range_mut(
                    (Bound::Included(self.best_ask), Bound::Included(order.price))
                );
                self.entries.exec_range(order, range)
            },
            Side::Sell => {
                let range = self.price_limits.range_mut(
                    (Bound::Included(order.price), Bound::Included(self.best_bid))
                ).rev();
                self.entries.exec_range(order, range)
            },
        };

        match exec_result {
            // The previous range was empty, i.e. the limit order is not marketable and should
            // be inserted in the order book.
            ExecResult::NotExecuted => {
                let index = self.entries.alloc(BookEntry {
                    size: order.size,
                    next: None,
                    trader: order.trader,
                });

                let price_point =
                    self.price_limits
                        .entry(order.price)
                        .or_insert_with(|| PriceLimit { link: None });

                if price_point.link.is_some() {
                    let link = price_point.link.as_mut().unwrap();
                    self.entries.get_mut(link.tail).next = Some(index);
                        link.tail = index;
                } else {
                    mem::replace(&mut price_point.link, Some(Link {
                        head: index,
                        tail: index,
                    }));
                }

                // Update the best bid / best ask consequently.
                if order.price < self.best_ask {
                    self.best_ask = order.price;
                } else if order.price > self.best_bid {
                    self.best_bid = order.price;
                }

                None
            },
            ExecResult::Filled(order) => {
                match maybe_price {
                    Some(price) => match order.side {
                        Side::Buy => self.best_ask = price,
                        Side::Sell => self.best_bid = price,
                    },

                    // The order has exhausted the whole side!
                    None => match order.side {
                        Side::Buy => self.best_ask = Price::max_value(),
                        Side::Sell => self.best_bid = 0,
                    }
                };
                Some(order)
            }
        }
    }
}
