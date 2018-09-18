use trade::*;
use trade::api::{self, *};
use tokio::runtime::current_thread;
use trade::api::symbol::{Symbol, IntoWithSymbol};
use futures::prelude::*;
use futures::sync::mpsc::UnboundedReceiver;
use std::sync::mpsc;

pub enum PullEvent {
    OrderAck(Option<api::errors::OrderError>),
    CancelAck(Option<api::errors::CancelError>),
    OrderConfirmation(OrderConfirmation),
    OrderUpdate(OrderUpdate),
    OrderExpiration(OrderExpiration),
    OrderBook(OrderBook),
    Message(String),
}

pub enum PushEvent {
    Order(Order),
    Cancel(Cancel),
    Message(String),
}

pub struct PushThread<C> {
    pub push: Option<UnboundedReceiver<PushEvent>>,
    pub pull: mpsc::Sender<PullEvent>,
    pub client: C,
    pub symbol: Symbol,
}

impl<C: ApiClient + Send + 'static> PushThread<C> {
    fn process_event(&self, event: PushEvent) -> Result<(), ()> {
        match event {
            PushEvent::Order(order) => {
                let cloned = self.pull.clone();
                let order_fut = self.client.order(order.add_symbol(self.symbol)).then(move |res| {
                    cloned.send(PullEvent::OrderAck(res.err())).unwrap();
                    Ok(())
                });
                tokio::spawn(order_fut);
            },
            PushEvent::Cancel(cancel) => {
                let cloned = self.pull.clone();
                let cancel_fut = self.client.cancel(cancel.add_symbol(self.symbol)).then(move |res| {
                    cloned.send(PullEvent::CancelAck(res.err())).unwrap();
                    Ok(())
                });
                tokio::spawn(cancel_fut);
            },
            PushEvent::Message(msg) => {
                self.pull.send(PullEvent::Message(msg)).unwrap();
            }
        }
        Ok(())
    }

    pub fn run(mut self) {
        let push = self.push.take();
        let fut = push.unwrap().for_each(move |event| {
            self.process_event(event)
        });
        current_thread::block_on_all(fut).unwrap();
    }
}

pub struct OrderBookThread<S> {
    pub stream: Option<S>,
    pub pull: mpsc::Sender<PullEvent>,
    pub order_book: OrderBook,
}

impl<S: Stream<Item = Notification, Error = ()>> OrderBookThread<S> {
    fn process_notif(&mut self, notif: Notification) -> Result<(), ()> {
        match notif {
            Notification::LimitUpdates(updates) => {
                for update in updates {
                    self.order_book.update(update.into_inner());
                }
                self.pull.send(PullEvent::OrderBook(self.order_book.clone())).unwrap();
            },
            Notification::OrderConfirmation(order) => {
                self.pull.send(PullEvent::OrderConfirmation(order.into_inner())).unwrap();
            },
            Notification::OrderUpdate(update) => {
                self.pull.send(PullEvent::OrderUpdate(update.into_inner())).unwrap();
            },
            Notification::OrderExpiration(expiration) => {
                self.pull.send(PullEvent::OrderExpiration(expiration.into_inner())).unwrap();
            },
            _ => (),
        }
        Ok(())
    }

    pub fn run(mut self) {
        let pull = self.pull.clone();
        let stream = self.stream.take();

        let fut = stream.unwrap().for_each(move |notif| {
            self.process_notif(notif)
        });
        current_thread::block_on_all(fut).unwrap();

        pull.send(
            PullEvent::Message("stream connection was dropped".to_string())
        ).unwrap();
    }
}
