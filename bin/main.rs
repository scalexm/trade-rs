extern crate trade_rs;
use trade_rs::api::*;
use trade_rs::notify;

struct Notifier;

impl notify::Notifier for Notifier {
    fn notify(&mut self, notif: notify::Notification) {
        match notif {
            notify::Notification::Trade(trade) => {
                println!("{}", trade.time);
            },
            _ => (),
        }
    }
}

fn main() {
    let mut client = binance::Client::new(binance::Params {
        symbol: "trxeth".to_owned(),
        address: "wss://stream.binance.com:9443".to_owned(),
    });

    client.stream(Notifier);
}
