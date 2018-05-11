use api::*;
use notify::Notification;
use std::thread;
use ws;
use serde_json;
use futures::channel::mpsc::*;
use futures::prelude::*;
use tick::*;

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
/// Params needed for a binance API client.
pub struct Params {
    /// Currency symbol in lowercase, e.g. "trxbtc".
    pub symbol: String,

    /// WebSocket server address.
    pub address: String,

    /// Tick unit for prices.
    pub price_tick: Tick,

    /// Tick unit for sizes.
    pub size_tick: Tick,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
/// A binance API client.
pub struct Client {
    params: Params,
}

impl Client {
    /// Create a new API client with given `params`.
    pub fn new(params: Params) -> Self {
        Client {
            params,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
enum InternalAction {
    Notify(Notification),
    Shutdown,
}

/// `Stream` implementor representing a binance WebSocket stream.
pub struct BinanceStream {
    rcv: UnboundedReceiver<InternalAction>,
}

impl BinanceStream {
    fn new(params: Params) -> Self {
        let (snd, rcv) = unbounded();
        let (price_tick, size_tick) = (params.price_tick, params.size_tick);
        thread::spawn(move || {
            let address = format!(
               "{0}/ws/{1}@trade/{1}@depth",
                params.address,
                params.symbol
            );
            println!("{}", address);
            
            if let Err(_err) = ws::connect(address, |out| Handler {
                out,
                snd: snd.clone(),
                price_tick,
                size_tick,
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
    price_tick: Tick,
    size_tick: Tick,
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
            // The corresponding receiver was dropped, this connection does not make sense
            // anymore.
            let _ = self.out.shutdown();
        }
    }

    fn parse_message(&mut self, json: String) -> Result<(), Error> {
        let v: serde_json::Value = serde_json::from_str(&json)?;
        let event = v["e"].to_string();

        if event.ends_with("trade\"") {
            let trade: BinanceTrade = serde_json::from_value(v)?;
            self.send(InternalAction::Notify(
                Notification::Trade(Trade {
                    size: self.size_tick.convert_unticked(&trade.q)?,
                    time: trade.T,
                    price: self.price_tick.convert_unticked(&trade.p)?,
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
        if let ws::Message::Text(json) = msg {
            if let Err(_err) = self.parse_message(json) {
                // FIXME: log error somewhere
            }
        }
        Ok(())
    }
}
