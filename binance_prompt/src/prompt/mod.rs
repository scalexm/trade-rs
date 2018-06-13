use trade::order_book::OrderBook;
use trade::api::{Order, ApiClient};
use std::collections::HashMap;
use futures::sync::mpsc::{unbounded, UnboundedSender};
use std::thread;
use std::sync::mpsc;

mod workers;
mod draw;

pub use self::workers::PushEvent;
use self::workers::{PullEvent, PushThread, OrderBookThread};

pub struct Prompt {
    pull: mpsc::Receiver<PullEvent>,
    orders: HashMap<String, Order>,
    output: String,
    order_book: OrderBook,
}

impl Prompt {
    pub fn new<C: ApiClient + Send + 'static>(client: C)
        -> (Self, UnboundedSender<PushEvent>)
    {
        let (pull_snd, pull_rcv) = mpsc::channel();
        let (push_snd, push_rcv) = unbounded();

        let stream = client.stream();
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
                match res {
                    Ok(order) => {
                        self.output = format!(
                            "inserted order `{}`",
                            order.order_id.as_ref().unwrap()
                        );
                        self.orders.insert(
                            order.order_id.as_ref().unwrap().clone(),
                            order
                        );
                    },
                    Err(err) => self.output = format!("{}", err),
                }
            },
            PullEvent::CancelAck(res) => {
                match res {
                    Ok(cancel) => {
                        if self.orders.remove(&cancel.order_id).is_none() {
                            self.output = format!(
                                "received `CancelAck` for unknown order `{}`",
                                cancel.order_id
                            );
                        } else {
                            self.output = format!(
                                "canceled order `{}`",
                                cancel.order_id
                            );
                        }
                    },
                    Err(err) => self.output = format!("{}", err),
                }
            },
            PullEvent::OrderUpdate(update) => {
                if let Some(order) = self.orders.get_mut(&update.order_id) {
                    order.size = update.remaining_size;
                    self.output = format!(
                        "filled order `{}` with quantity {}",
                        update.order_id,
                        update.consumed_size
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
            PullEvent::OrderExpired(order_id) => {
                if self.orders.remove(&order_id).is_none() {
                    self.output = format!(
                        "received `OrderExpired` for unknown order `{}`",
                        order_id
                    );
                } else {
                    self.output = format!("order `{}` has expired", order_id);
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
