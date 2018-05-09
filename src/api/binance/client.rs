use api::*;
use notify::{Notification, Notifier};
use std::thread;
use ws;
use crossbeam_channel::{unbounded, Sender, Receiver};
use serde_json;

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Params {
    pub symbol: String,
    pub address: String,
}

pub struct Client {
    params: Params,
    snd: Sender<ApiAction>,
    rcv: Receiver<ApiAction>,
}

impl Client {
    pub fn new(params: Params) -> Self {
        let (snd, rcv) = unbounded();
        Client {
            params,
            snd,
            rcv,
        }
    }
}

enum InternalAction {
    Notify(Notification),
}

struct Handler {
    out: ws::Sender,
    snd: Sender<InternalAction>,
}

impl<N: Notifier> ApiClient<N> for Client {
    fn sender(&self) -> Sender<ApiAction> {
        self.snd.clone()
    }

    fn stream(&mut self, mut notifier: N) {
        let (snd, internal_rcv) = unbounded();

        let params = self.params.clone();
        let thread = thread::spawn(move || {
            let address = format!(
               "{0}/stream?streams={1}@trade/{1}@depth",
                params.address,
                params.symbol
            );
            ws::connect(address, |out| Handler { out, snd: snd.clone() }).unwrap()
        });

        select_loop! {
            recv(self.rcv, msg) => (),
            recv(internal_rcv, msg) => {
                match msg {
                    InternalAction::Notify(notif) => notifier.notify(notif),
                }
            }
        };

        let _ = thread.join();
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Deserialize)]
struct BinanceTrade {
    e: String,
    E: usize,
    s: String,
    t: usize,
    p: String,
    q: String,
    b: usize,
    a: usize,
    T: usize,
    m: bool,
    M: bool,
}

impl ws::Handler for Handler {
    fn on_message(&mut self, msg: ws::Message) -> ws::Result<()> {
        if let ws::Message::Text(json) = msg {
            let mut v: serde_json::Value = serde_json::from_str(&json).unwrap();
            let stream = v["stream"].to_string();

            if stream.ends_with("trade\"") {
                let trade: BinanceTrade = serde_json::from_value(v["data"].take()).unwrap();
                self.snd.send(InternalAction::Notify(
                    Notification::Trade(Trade {
                        size: 0,//trade.q,
                        time: trade.T,
                        price: 0, //trade.p,
                        buyer_id: trade.b,
                        seller_id: trade.a,
                    })
                )).unwrap();
            } else if stream.ends_with("depth\"") {
            }
        }
        Ok(())
    }
}
