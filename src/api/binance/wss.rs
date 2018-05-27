use api::*;
use notify::*;
use std::thread;
use ws;
use ws::util::{Timeout, Token};
use serde_json;
use futures::sync::mpsc::*;
use futures::prelude::*;
use tick::*;
use std::mem;
use order_book::LimitUpdate;
use super::{RestError, Params};
use failure::Error;

#[derive(Clone, PartialEq, Eq, Debug)]
enum InternalAction {
    Notify(Notification),
}

#[derive(Debug)]
/// `Stream` implementor representing a binance WebSocket stream.
pub struct BinanceStream {
    rcv: UnboundedReceiver<InternalAction>,
}

impl BinanceStream {
    crate fn new(params: Params) -> Self {
        let (snd, rcv) = unbounded();
        thread::spawn(move || {
            let address = format!(
               "{0}/ws/{1}@trade/{1}@depth",
                params.ws_address,
                params.symbol.to_lowercase()
            );
            info!("Initiating WebSocket connection at {}", address);
            
            if let Err(err) = ws::connect(address, |out| Handler {
                out,
                snd: snd.clone(),
                params: params.clone(),
                timeout: None,
                book_snapshot_state: BookSnapshotState::None,
                previous_u: None,
            })
            {
                error!("WebSocket connection terminated with error `{:?}`", err);
            }   
        });
        
        BinanceStream {
            rcv,
        }
    }
}

impl Stream for BinanceStream {
    type Item = Notification;
    type Error = ();

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        let action = try_ready!(self.rcv.poll());
        Ok(
            Async::Ready(match action {
                Some(InternalAction::Notify(notif)) => Some(notif),
                None => None,
            })
        )
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
/// Internal representation which keep binance `u` indicator.
struct LimitUpdates {
    u: u64,
    updates: Vec<LimitUpdate>,
}

#[derive(Debug)]
/// State of the book snapshot request:
/// * `None`: the request has not been made yet
/// * `Waiting(rcv, passed_events)`: the request has started, in the meantime we have a `Receiver`
///   which will receive the snapshot, and a vector of past events which may need to be notified
///   to the `BinanceClient` consumer one the request is complete
/// * `Ok`: the request was completed already
enum BookSnapshotState {
    None,
    Waiting(
        Receiver<Result<BinanceBookSnapshot, Error>>,
        Vec<LimitUpdates>
    ),
    Ok,
}

/// An object handling a WebSocket API connection.
struct Handler {
    out: ws::Sender,
    snd: UnboundedSender<InternalAction>,
    params: Params,

    /// We keep a reference to the `EXPIRE` timeout so that we can cancel it when we receive
    /// something from the server.
    timeout: Option<Timeout>,

    book_snapshot_state: BookSnapshotState,

    /// We keep track of the last `u` indicator sent by binance, this is used for checking
    /// the coherency of the ordering of the events by binance.
    previous_u: Option<u64>,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Deserialize)]
#[allow(non_snake_case)]
/// A JSON representation of a trade, sent by binance.
struct BinanceTrade {
    e: String,
    E: u64,
    s: String,
    t: usize,
    p: String,
    q: String,
    b: usize,
    a: usize,
    T: u64,
    m: bool,
    M: bool,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Deserialize)]
/// A JSON representation of a limit update, embedded into other binance events.
struct BinanceLimitUpdate {
    price: String,
    size: String,
    _ignore: Vec<i32>,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Deserialize)]
#[allow(non_snake_case)]
/// A JSON representation of an orderbook update, sent by binance.
struct BinanceDepthUpdate {
    e: String,
    E: u64,
    s: String,
    U: u64,
    u: u64,
    b: Vec<BinanceLimitUpdate>,
    a: Vec<BinanceLimitUpdate>,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Deserialize)]
#[allow(non_snake_case)]
/// A JSON representation of an orderbook snapshot, sent by binance.
struct BinanceBookSnapshot {
    lastUpdateId: u64,
    bids: Vec<BinanceLimitUpdate>,
    asks: Vec<BinanceLimitUpdate>,
}

impl Handler {
    fn send(&mut self, action: InternalAction) {
        if let Err(..) = self.snd.unbounded_send(action) {
            // The corresponding receiver was dropped, this connection does not make sense
            // anymore.
            self.out.shutdown().expect("shutdown error");
        }
    }

    /// Utility function for converting a `BinanceLimitUpdate` into a `LimitUpdate` (with
    /// conversion in ticks and so on).
    fn convert_binance_update(&self, l: &BinanceLimitUpdate, side: Side)
        -> Result<LimitUpdate, ConversionError>
    {
        Ok(
            LimitUpdate {
                side,
                price: self.params.price_tick.convert_unticked(&l.price)?,
                size: self.params.size_tick.convert_unticked(&l.size)?,
            }
        )
    }

    /// Parse a (should-be) JSON message sent by binance.
    fn parse_message(&mut self, json: String) -> Result<Option<Notification>, Error> {
        let v: serde_json::Value = serde_json::from_str(&json)?;
        let event = v["e"].to_string();

        let notif = if event == r#""trade""# {
            let trade: BinanceTrade = serde_json::from_value(v)?;
            Some(
                Notification::Trade(Trade {
                    size: self.params.size_tick.convert_unticked(&trade.q)?,
                    time: trade.T,
                    price: self.params.price_tick.convert_unticked(&trade.p)?,
                    buyer_id: trade.b,
                    seller_id: trade.a,
                })
            )
        } else if event == r#""depthUpdate""# {
            let depth_update: BinanceDepthUpdate = serde_json::from_value(v)?;

            // The order is consistent if the previous `u + 1` is equal to current `U`.
            if let Some(previous_u) = self.previous_u {
                if previous_u + 1 != depth_update.U {
                    // FIXME: Maybe we should just shutdown here?
                    bail!("previous `u + 1` and current `U` do not match");
                }
            }
            self.previous_u = Some(depth_update.u);

            let bid = depth_update.b.iter().map(|l| self.convert_binance_update(l, Side::Bid));
            let ask = depth_update.a.iter().map(|l| self.convert_binance_update(l, Side::Ask));

            Some(
                Notification::LimitUpdates(
                    bid.chain(ask).collect::<Result<Vec<_>, ConversionError>>()?
                )
            )
        } else {
            None
        };

        Ok(notif)
    }

    fn process_book_snapshot(
        &mut self,
        snapshot: Result<BinanceBookSnapshot, Error>,
        passed_events: Vec<LimitUpdates>
    ) -> Result<(), Error>
    {
        let snapshot = snapshot?;
        let bid = snapshot.bids.iter().map(|l| self.convert_binance_update(l, Side::Bid));
        let ask = snapshot.asks.iter().map(|l| self.convert_binance_update(l, Side::Ask));

        let notifs = Some(
            Notification::LimitUpdates(
                bid.chain(ask).collect::<Result<Vec<_>, ConversionError>>()?
            )
        ).into_iter().chain(
            // Drop all events prior to `snapshot.lastUpdateId`.
            passed_events.into_iter()
                         .filter(|update| update.u > snapshot.lastUpdateId)
                         .map(|update| Notification::LimitUpdates(update.updates))
        );

        for notif in notifs {
            self.send(InternalAction::Notify(notif));
        }

        self.book_snapshot_state = BookSnapshotState::Ok;
        Ok(())
    }
}

const PING: Token = Token(1);
const EXPIRE: Token = Token(2);
const BOOK_SNAPSHOT: Token = Token(3);

const PING_TIMEOUT: u64 = 10_000;
const EXPIRE_TIMEOUT: u64 = 30_000;
const BOOK_SNAPSHOT_TIMEOUT: u64 = 1_000;

impl ws::Handler for Handler {
    fn on_open(&mut self, _: ws::Handshake) -> ws::Result<()> {
        self.out.ping(vec![])?;
        self.out.timeout(PING_TIMEOUT, PING)?;
        self.out.timeout(EXPIRE_TIMEOUT, EXPIRE)
    }

    fn on_timeout(&mut self, event: Token) -> ws::Result<()> {
        match event {
            PING => {
                self.out.ping(vec![])?;
                self.out.timeout(PING_TIMEOUT, PING)
            }
            EXPIRE => self.out.close(ws::CloseCode::Away),
            BOOK_SNAPSHOT => {
                match mem::replace(&mut self.book_snapshot_state, BookSnapshotState::None) {
                    // The timout is enabled only when the we are in the `Waiting` state.
                    BookSnapshotState::None |
                    BookSnapshotState::Ok => panic!("book snapshot timeout not supposed to happen"),

                    BookSnapshotState::Waiting(mut rcv, events) => {
                        match rcv.poll().unwrap() {
                            Async::Ready(Some(book)) => {
                                info!("Received LOB snapshot");
                                if let Err(err) = self.process_book_snapshot(book, events) {
                                    error!("LOB processing encountered error: {}", err);
                                    
                                    // We cannot continue without the book, we shutdown.
                                    self.out.shutdown().expect("shutdown error");
                                }
                            },

                            // The snapshot request has not completed yet, we wait some more.
                            Async::NotReady => {
                                self.book_snapshot_state = BookSnapshotState::Waiting(
                                    rcv,
                                    events
                                );
                                self.out.timeout(BOOK_SNAPSHOT_TIMEOUT, BOOK_SNAPSHOT)?
                            },

                            // The only `Sender` has somehow disconnected, we won't receive
                            // the book hence we cannot continue.
                            Async::Ready(None) => {
                                error!("LOB sender has disconnected");
                                self.out.shutdown().expect("shutdown error");
                                return Ok(());
                            }
                        }
                    },
                };
                Ok(())
            }
            _ => Err(ws::Error::new(ws::ErrorKind::Internal, "Invalid timeout token encountered!")),
        }
    }

    fn on_new_timeout(&mut self, event: Token, timeout: Timeout) -> ws::Result<()> {
        if event == EXPIRE {
            if let Some(t) = self.timeout.take() {
                self.out.cancel(t)?;
            }
            self.timeout = Some(timeout)
        }
        Ok(())
    }

    fn on_frame(&mut self, frame: ws::Frame) -> ws::Result<Option<ws::Frame>> {
        self.out.timeout(EXPIRE_TIMEOUT, EXPIRE)?;
        Ok(Some(frame))
    }

    fn on_message(&mut self, msg: ws::Message) -> ws::Result<()> {
        if let ws::Message::Text(json) = msg {
            match self.parse_message(json) {
                // Trade notif: just forward to the consumer.
                Ok(Some(notif @ Notification::Trade(..))) => {
                    self.send(InternalAction::Notify(notif))
                },

                // Depth update notif: behavior depends on the status of the order book snapshot.
                Ok(Some(Notification::LimitUpdates(updates))) => match self.book_snapshot_state {
                    // Very first limit update event received: time to ask for the book snapshot.
                    BookSnapshotState::None => {
                        #[allow(unused_mut)] // FIXME: fake warning
                        let (mut snd, rcv) = channel(1);

                        self.book_snapshot_state = BookSnapshotState::Waiting(
                            rcv,

                            // Buffer this first event we've just received.
                            vec![LimitUpdates {
                                u: self.previous_u.unwrap(),
                                updates,
                            }]
                        );

                        let address = format!(
                            "{}/api/v1/depth?symbol={}&limit=1000",
                            self.params.http_address,
                            self.params.symbol.to_uppercase()
                        ).parse().expect("invalid address");

                        info!("Initiating LOB request at {}", address);

                        thread::spawn(move || {
                            let https = match hyper_tls::HttpsConnector::new(2) {
                                Ok(https) => https,
                                Err(err) => {
                                    let _ = snd.try_send(Err(err).map_err(From::from));
                                    return;
                                }
                            };

                            let client = hyper::Client::builder().build::<_, hyper::Body>(https);
                            let fut = client.get(address).and_then(|res| {
                                let status = res.status();
                                res.into_body().concat2().and_then(move |body| {
                                    Ok((status, body))
                                })
                            }).map_err(From::from).and_then(move |(status, body)| {
                                if status != hyper::StatusCode::OK {
                                    Err(RestError::from_status_code(status))?;
                                }

                                Ok(serde_json::from_slice(&body)?)
                            }).then(move |res| {
                                let _ = snd.try_send(res);
                                Ok(())
                            });

                            hyper::rt::run(fut);
                        });

                        // We are in `Waiting` state: enable the timeout.
                        self.out.timeout(BOOK_SNAPSHOT_TIMEOUT, BOOK_SNAPSHOT)?
                    },

                    // Still waiting: buffer incoming events.
                    BookSnapshotState::Waiting(_, ref mut events) => {
                        events.push(LimitUpdates {
                            u: self.previous_u.unwrap(),
                            updates,
                        })
                    },

                    // We already received the book snapshot and notified the final consumer,
                    // we can now notify further notifications to them.
                    BookSnapshotState::Ok => {
                        self.send(InternalAction::Notify(Notification::LimitUpdates(updates)))
                    },
                },

                // Seems like the message was not conforming.
                Ok(None) => (),

                Err(err) => {
                    error!("Message parsing encountered error {:?}", err)
                }
            };
        }
        Ok(())
    }
}
