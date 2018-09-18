use trade::order_book::{self, OrderBook};
use trade::api::{OrderConfirmation, ApiClient};
use std::collections::HashMap;
use futures::sync::mpsc::{unbounded, UnboundedSender};
use std::thread;
use std::sync::mpsc;
use failure::Fail;

mod workers;
mod draw;

pub use self::workers::PushEvent;
use self::workers::{PullEvent, PushThread, OrderBookThread};

pub struct Prompt {
    pull: mpsc::Receiver<PullEvent>,
    orders: HashMap<String, OrderConfirmation>,
    output: String,
    order_book: OrderBook,
}

impl Prompt {
    pub fn new<C: ApiClient + Send + 'static>(client: C, symbol: &str)
        -> (Self, UnboundedSender<PushEvent>)
    {
        let (pull_snd, pull_rcv) = mpsc::channel();
        let (push_snd, push_rcv) = unbounded();

        let symbol = client.find_symbol(symbol).expect("cannot find symbol");
        order_book::display::set_price_tick(Some(symbol.price_tick()));
        order_book::display::set_size_tick(Some(symbol.size_tick()));

        let stream = client.stream(symbol);
        let order_book_thread = OrderBookThread {
            stream: Some(stream),
            pull: pull_snd.clone(),
            order_book: OrderBook::new(),
        };
        thread::spawn(move || order_book_thread.run());

        let push_thread = PushThread {
            push: Some(push_rcv),
            pull: pull_snd.clone(),
            client,
            symbol,
        };
        thread::spawn(move || push_thread.run());

        let prompt = Prompt {
            pull: pull_rcv,
            orders: HashMap::new(),
            output: String::new(),
            order_book: OrderBook::new(),
        };

        (prompt, push_snd)
    }

    fn process_event(&mut self, event: PullEvent) {
        match event {
            PullEvent::OrderAck(res) => {
                if let Some(err) = res {
                    let cause = err.cause().unwrap();
                    match cause.cause() {
                        Some(inner_cause) => {
                            self.output = format!("{} ({})", cause, inner_cause)
                        },
                        None => self.output = format!("{}", cause),
                    }
                }
            },
            PullEvent::CancelAck(res) => {
                if let Some(err) = res {
                    let cause = err.cause().unwrap();
                    match cause.cause() {
                        Some(inner_cause) => {
                            self.output = format!("{} ({})", cause, inner_cause)
                        },
                        None => self.output = format!("{}", cause),
                    }
                }
            },
            PullEvent::OrderConfirmation(order) => {
                self.output = format!(
                    "inserted order `{}`",
                    order.order_id
                );
                self.orders.insert(
                    order.order_id.clone(),
                    order
                );
            },
            PullEvent::OrderUpdate(update) => {
                if let Some(order) = self.orders.get_mut(&update.order_id) {
                    order.size = update.remaining_size;
                    self.output = format!(
                        "filled order `{}` with quantity {}",
                        update.order_id,
                        order_book::display::displayable_size(update.consumed_size)
                    );

                    if order.size == 0 {
                        self.orders.remove(&update.order_id).unwrap();
                    }
                } else {
                    self.output = format!(
                        "received `OrderUpdate` for unknown order `{}`",
                        update.order_id
                    );
                }
            },
            PullEvent::OrderExpiration(expiration) => {
                if self.orders.remove(&expiration.order_id).is_none() {
                    self.output = format!(
                        "received `OrderExpiration` for unknown order `{}`",
                        expiration.order_id
                    );
                } else {
                    self.output = format!("order `{}` has been canceled", expiration.order_id);
                }
            }
            PullEvent::OrderBook(order_book) => {
                self.order_book = order_book;
            }
            PullEvent::Message(msg) => self.output = msg,
        }
    }

    fn update(&mut self) {
        while let Ok(event) = self.pull.try_recv() {
            self.process_event(event);
        }
    }
}
