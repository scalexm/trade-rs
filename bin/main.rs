extern crate trade_rs;
extern crate futures;
extern crate env_logger;

use trade_rs::api::*;
use trade_rs::Tick;
use futures::prelude::*;
use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    env_logger::init();

    let client = binance::Client::new(binance::Params {
        symbol: "btcusdt".to_owned(),
        ws_address: "wss://stream.binance.com:9443".to_owned(),
        http_address: "https://www.binance.com".to_owned(),
        price_tick: Tick::new(100),
        size_tick: Tick::new(1000000),
        commission_tick: Tick::new(100000000),
        api_key: String::new(),
        secret_key: String::new(),
    }).unwrap();

    let fut = client.stream().for_each(|notif| {
        match notif {
            Notification::Trade(trade) => {
                println!(
                    "trade,{},{},{}",
                    trade.time,
                    Tick::new(100).convert_ticked(trade.price).unwrap(),
                    Tick::new(1000000).convert_ticked(trade.size).unwrap()
                );
            }
            Notification::LimitUpdates(updates) => {
                let time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
                for update in updates {
                    println!(
                        "update,{},{:?},{},{}",
                        time.as_secs() + time.subsec_millis() as u64,
                        update.side,
                        Tick::new(100).convert_ticked(update.price).unwrap(),
                        Tick::new(1000000).convert_ticked(update.size).unwrap()
                    );
                }
            },
            _ => (),
        };
        Ok(())
    });
    fut.wait().unwrap();
}
