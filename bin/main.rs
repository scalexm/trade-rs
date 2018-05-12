extern crate trade_rs;
extern crate futures;

use trade_rs::api::*;
use trade_rs::notify::Notification;
use trade_rs::Tick;
use futures::prelude::*;

fn main() {
    let client = binance::Client::new(binance::Params {
        symbol: "btcusdt".to_owned(),
        address: "wss://stream.binance.com:9443".to_owned(),
        price_tick: Tick::new(100),
        size_tick: Tick::new(1000000),
    });

    let fut = client.stream().for_each(|notif| {
        match notif {
            Notification::Trade(trade) => {
                println!("{:?}", trade);
            }
            _ => (),
        }
        Ok(())
    });
    futures::executor::block_on(fut).unwrap();
}
