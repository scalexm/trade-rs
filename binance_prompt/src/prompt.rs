use trade_rs::*;
use trade_rs::api::*;
use std::collections::HashMap;
use tokio::executor::current_thread;
use tokio::runtime;
use futures::prelude::*;
use futures::sync::mpsc::{unbounded, UnboundedSender, UnboundedReceiver};
use std::thread;
use std::sync::mpsc;
use std::mem;

use cursive::Printer;
use cursive::view::View;
use cursive::vec::Vec2;

pub enum PullEvent {
    OrderAck(Result<Order, Error>),
    CancelAck(Result<Cancel, Error>),
    OrderUpdate(OrderUpdate),
    OrderExpired(String),
    OrderBook(OrderBook),
    Message(String),
}

pub enum PushEvent {
    Order(Order),
    Cancel(Cancel),
    Message(String),
}

pub struct Prompt {
    pull: mpsc::Receiver<PullEvent>,
    orders: HashMap<String, Order>,
    output: String,
    order_book: OrderBook,
}

struct PushThread<C> {
    push: Option<UnboundedReceiver<PushEvent>>,
    pull: mpsc::Sender<PullEvent>,
    client: C,
}

impl<C: ApiClient + Send + 'static> PushThread<C> {
    pub fn process_event(&self, event: PushEvent) -> Result<(), ()> {
        match event {
            PushEvent::Order(order) => {
                let cloned = self.pull.clone();
                let order_fut = self.client.order(&order).then(move |res| {
                    let res = res.map(|ack| Order {
                        order_id: Some(ack.order_id),
                        ..order
                    });
                    cloned.send(PullEvent::OrderAck(res)).unwrap();
                    Ok(())
                });
                current_thread::spawn(order_fut);
            },
            PushEvent::Cancel(cancel) => {
                let cloned = self.pull.clone();
                let cancel_fut = self.client.cancel(&cancel).then(move |res| {
                    let res = res.map(|_| cancel);
                    cloned.send(PullEvent::CancelAck(res)).unwrap();
                    Ok(())
                });
                current_thread::spawn(cancel_fut);
            },
            PushEvent::Message(msg) => {
                self.pull.send(PullEvent::Message(msg)).unwrap();
            }
        }
        Ok(())
    }

    fn run(mut self) {
        let push = mem::replace(&mut self.push, None);
        let fut = push.unwrap().for_each(move |event| {
            self.process_event(event)
        });

        let mut runtime = runtime::current_thread::Runtime::new().unwrap();
        runtime.spawn(fut);
        runtime.run().unwrap();
    }
}

struct OrderBookThread<S> {
    stream: Option<S>,
    pull: mpsc::Sender<PullEvent>,
    order_book: OrderBook,
}

impl<S: Stream<Item = Notification, Error = ()>> OrderBookThread<S> {
    fn process_notif(&mut self, notif: Notification) -> Result<(), ()> {
        match notif {
            Notification::LimitUpdates(updates) => {
                for update in updates {
                    self.order_book.update(update);
                }
                self.pull.send(PullEvent::OrderBook(self.order_book.clone())).unwrap();
            }
            Notification::OrderUpdate(update) => {
                self.pull.send(PullEvent::OrderUpdate(update)).unwrap();
            },
            Notification::OrderExpired(order_id) => {
                self.pull.send(PullEvent::OrderExpired(order_id)).unwrap();
            }
            _ => (),
        }
        Ok(())
    }

    fn run(mut self) {
        let pull = self.pull.clone();
        let stream = mem::replace(&mut self.stream, None);

        let fut = stream.unwrap().for_each(move |notif| {
            self.process_notif(notif)
        });
        current_thread::block_on_all(fut).unwrap();

        pull.send(
            PullEvent::Message("stream connection was dropped".to_string())
        ).unwrap();
    }
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

impl View for Prompt {
    fn layout(&mut self, _: Vec2) {
        self.update();
    }

    fn draw(&self, printer: &Printer) {
        let order_book = format!("{}", self.order_book);
        for (i, line) in order_book.split('\n').enumerate() {
            printer.print((0, i), line);
        }

        printer.print((0, printer.size.y - 1), &self.output);

        for (i, order) in self.orders.values().enumerate() {
            let line = format!(
                "{}: {} @ {} ({:?})",
                order.order_id.as_ref().unwrap(),
                order_book::displayable_size(order.size),
                order_book::displayable_price(order.price),
                order.side
            );
            printer.print((printer.size.x - line.len(), i), &line);
        }
    }
}
