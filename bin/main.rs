extern crate trade_rs;
extern crate futures;

use trade_rs::api::*;
use trade_rs::notify::Notification;
use futures::prelude::*;

fn main() {
    let client = binance::Client::new(binance::Params {
        symbol: "trxeth".to_owned(),
        address: "wss://stream.binance.com:9443".to_owned(),
    });

    let fut = client.stream().for_each(|notif| {
        match notif {
            Notification::Trade(trade) => {
                println!("{}", trade.time);
            }
            _ => (),
        }
        Ok(())
    });
    futures::executor::block_on(fut).unwrap();
}
