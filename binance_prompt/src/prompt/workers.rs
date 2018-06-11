use trade_rs::*;
use trade_rs::api::*;
use tokio::executor::current_thread;
use tokio::runtime;
use futures::prelude::*;
use futures::sync::mpsc::UnboundedReceiver;
use std::sync::mpsc;
use std::mem;

pub(in prompt) enum PullEvent {
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

pub(in prompt) struct PushThread<C> {
    pub push: Option<UnboundedReceiver<PushEvent>>,
    pub pull: mpsc::Sender<PullEvent>,
    pub client: C,
}

impl<C: ApiClient + Send + 'static> PushThread<C> {
    fn process_event(&self, event: PushEvent) -> Result<(), ()> {
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

    pub(in prompt) fn run(mut self) {
        let push = mem::replace(&mut self.push, None);
        let fut = push.unwrap().for_each(move |event| {
            self.process_event(event)
        });

        let mut runtime = runtime::current_thread::Runtime::new().unwrap();
        runtime.spawn(fut);
        runtime.run().unwrap();
    }
}

pub(in prompt) struct OrderBookThread<S> {
    pub stream: Option<S>,
    pub pull: mpsc::Sender<PullEvent>,
    pub order_book: OrderBook,
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

    pub(in prompt) fn run(mut self) {
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
