extern crate trade_rs;
extern crate futures;
extern crate tokio;
extern crate cursive;
extern crate serde_json;

use std::fs::File;
use futures::prelude::*;
use tokio::executor::current_thread;
use std::thread;
use std::sync::mpsc;
use trade_rs::*;
use trade_rs::api::*;

use cursive::{Cursive, Printer};
use cursive::traits::*;
use cursive::vec::Vec2;

fn main() {
    let params = File::open("params.json").unwrap();
    let keys = File::open("keys.json").unwrap();

    let params = serde_json::from_reader(params).unwrap();
    let keys = serde_json::from_reader(keys).ok();

    let client = binance::Client::new(
        params,
        keys
    ).unwrap();

    let stream = client.stream();

    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        let mut order_book = OrderBook::new();

        let fut = stream.for_each(|notif| {
            match notif {
                Notification::LimitUpdates(updates) => {
                    for update in updates {
                        order_book.update(update);
                    }
                    tx.send(order_book.clone()).unwrap();
                },
                _ => (),
            }
            Ok(())
        });
        current_thread::block_on_all(fut).unwrap();
    });

    let mut siv = Cursive::default();

    siv.set_fps(10);
    siv.add_global_callback('q', |s| s.quit());

    siv.add_layer(OrderBookView::new(rx).full_screen());

    siv.run();
}

struct OrderBookView {
    rx: mpsc::Receiver<OrderBook>,
    order_book: Option<OrderBook>,
}

impl OrderBookView {
    fn new(rx: mpsc::Receiver<OrderBook>) -> Self {
        OrderBookView {
            rx,
            order_book: None,
        }
    }

    fn update(&mut self) {
        while let Ok(order_book) = self.rx.try_recv() {
            self.order_book = Some(order_book);
        }
    }
}

impl View for OrderBookView {
    fn layout(&mut self, _: Vec2) {
        self.update();
    }

    fn draw(&self, printer: &Printer) {
        if let Some(order_book) = &self.order_book {
            let print = format!("{}", order_book);
            printer.print((0, 0), &print);
        }
    }
}



