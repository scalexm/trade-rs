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

    let client = binance::Client::new(
        binance::Params {
            symbol: binance::SymbolInfo {
                name: "btcusdt".to_owned(),
                price_tick: Tick::new(100),
                size_tick: Tick::new(1_000_000),
                commission_tick: Tick::new(100_000_000)
            },
            ws_address: "wss://stream.binance.com:9443".to_owned(),
            http_address: "https://www.binance.com".to_owned(),
        },
        None
    ).unwrap();

    let mut trades = File::create("trades.txt")?;
    let mut depth_updates = File::create("updates.txt")?;

    let fut = client.stream().for_each(|notif| {
        match notif {
            Notification::Trade(trade) => {
                writeln!(
                    trades,
                    "trade,{},{},{},{},{},{:?}",
                    trade.time,
                    Tick::new(100).convert_ticked(trade.price).unwrap(),
                    Tick::new(1_000_000).convert_ticked(trade.size).unwrap(),
                    trade.consumer_order_id,
                    trade.maker_order_id,
                    trade.maker_side,
                ).unwrap();
            }
            Notification::LimitUpdates(updates) => {
                for update in updates {
                    writeln!(
                        depth_updates,
                        "update,{},{:?},{},{}",
                        update.timestamp,
                        update.side,
                        Tick::new(100).convert_ticked(update.price).unwrap(),
                        Tick::new(1_000_000).convert_ticked(update.size).unwrap()
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
