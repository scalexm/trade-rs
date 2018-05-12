extern crate trade_rs;
extern crate futures;
extern crate hyper;

use trade_rs::api::*;
use trade_rs::notify::Notification;
use trade_rs::Tick;
use futures::prelude::*;

fn main() {
    let client = binance::Client::new(binance::Params {
        symbol: "btcusdt".to_owned(),
        ws_address: "wss://stream.binance.com:9443".to_owned(),
        http_address: "https://www.binance.com".to_owned(),
        price_tick: Tick::new(100),
        size_tick: Tick::new(1000000),
    });

    let fut = client.stream().for_each(|notif| {
        match notif {
            Notification::Trade(trade) => {
                println!("{:?}", trade)
            }
            Notification::LimitUpdates(updates) => {
                println!("{}", updates.len())
            },
        };
        Ok(())
    });
    futures::executor::block_on(fut).unwrap();
}
