use api::*;
use notify::Notification;
use std::thread;
use ws;
use serde_json;
use futures::channel::mpsc::*;
use futures::prelude::*;

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Params {
    pub symbol: String,
    pub address: String,
}

pub struct Client {
    params: Params,
}

impl Client {
    pub fn new(params: Params) -> Self {
        Client {
            params,
        }
    }
}

enum InternalAction {
    Notify(Notification),
    Shutdown,
}

pub struct BinanceStream {
    rcv: UnboundedReceiver<InternalAction>,
}

impl BinanceStream {
    fn new(params: Params) -> Self {
        let (snd, rcv) = unbounded();
        thread::spawn(move || {
            let address = format!(
               "{0}/ws/{1}@trade/{1}@depth",
                params.address,
                params.symbol
            );
            
            if let Err(_err) = ws::connect(address, |out| Handler {
                out,
                snd: snd.clone(),
            })
            {
                // FIXME: log error somewhere
            }
            let _ = snd.unbounded_send(InternalAction::Shutdown);    
        });
        
        BinanceStream {
            rcv,
        }
    }
}

impl Stream for BinanceStream {
    type Item = Notification;
    type Error = Never;

    fn poll_next(&mut self, cx: &mut task::Context)
        -> Result<Async<Option<Self::Item>>, Self::Error>
    {
        let action = try_ready!(self.rcv.poll_next(cx));
        Ok(
            Async::Ready(match action.unwrap() {
                InternalAction::Notify(notif) => Some(notif),
                InternalAction::Shutdown => None,
            })
        )
    }
}

impl ApiClient for Client {
    type Stream = BinanceStream;

    fn stream(&self) -> BinanceStream {
        BinanceStream::new(self.params.clone())
    }
}

struct Handler {
    out: ws::Sender,
    snd: UnboundedSender<InternalAction>,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Deserialize)]
#[allow(non_snake_case)]
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

impl Handler {
    fn send(&mut self, action: InternalAction) {
        if let Err(..) = self.snd.unbounded_send(action) {
            let _ = self.out.shutdown();
        }
    }

    fn parse_message(&mut self, json: String) -> Result<(), serde_json::Error> {
        let v: serde_json::Value = serde_json::from_str(&json)?;
        let event = v["e"].to_string();

        if event.ends_with("trade\"") {
            let trade: BinanceTrade = serde_json::from_value(v)?;
            self.send(InternalAction::Notify(
                Notification::Trade(Trade {
                    size: 0,//trade.q,
                    time: trade.T,
                    price: 0, //trade.p,
                    buyer_id: trade.b,
                    seller_id: trade.a,
                })
            ));
        }
        Ok(())
    }
}

impl ws::Handler for Handler {
    fn on_message(&mut self, msg: ws::Message) -> ws::Result<()> {
        println!("ok");
        if let ws::Message::Text(json) = msg {
            if let Err(_err) = self.parse_message(json) {
                // FIXME: log error somewhere
            }
        }
        Ok(())
    }
}
