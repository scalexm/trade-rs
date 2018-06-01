extern crate trade_rs;
extern crate futures;
extern crate env_logger;

use trade_rs::api::*;
use trade_rs::Tick;
use futures::prelude::*;
use std::fs::File;
use std::io::Write;

fn main() -> std::io::Result<()> {
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

    let mut trades = File::open("trades.txt")?;
    let mut depth_updates = File::open("updates.txt")?;

    let fut = client.stream().for_each(|notif| {
        match notif {
            Notification::Trade(trade) => {
                write!(
                    trades,
                    "trade,{},{},{},{},{},{:?}\n",
                    trade.time,
                    Tick::new(100).convert_ticked(trade.price).unwrap(),
                    Tick::new(1000000).convert_ticked(trade.size).unwrap(),
                    trade.consumer_order_id,
                    trade.maker_order_id,
                    trade.maker_side,
                ).unwrap();
            }
            Notification::LimitUpdates(updates) => {
                for update in updates {
                    write!(
                        depth_updates,
                        "update,{},{:?},{},{}\n",
                        update.timestamp,
                        update.side,
                        Tick::new(100).convert_ticked(update.price).unwrap(),
                        Tick::new(1000000).convert_ticked(update.size).unwrap()
                    ).unwrap();
                }
            },
            _ => (),
        };
        Ok(())
    });
    fut.wait().unwrap();

    Ok(())
}
