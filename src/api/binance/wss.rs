use std::{mem, thread};
use std::sync::mpsc;
use std::borrow::Cow;
use futures::prelude::*;
use futures::sync::mpsc::{unbounded, UnboundedReceiver};
use log::{error, debug};
use failure::bail;
use serde_derive::Deserialize;
use crate::{tick, Side};
use crate::order_book::LimitUpdate;
use crate::api::{
    Notification,
    Params,
    Trade,
    OrderConfirmation,
    OrderUpdate,
    OrderExpiration,
};
use crate::api::symbol::Symbol;
use crate::api::wss;
use crate::api::timestamp::{Timestamped, IntoTimestamped};
use crate::api::binance::Client;
use crate::api::binance::errors::RestError;


impl Client {
    crate fn new_stream(&self, symbol: Symbol) -> UnboundedReceiver<Notification> {
        let params = self.params.clone();
        let listen_key = self.keys.as_ref().map(|keys| keys.listen_key.clone());
        let (snd, rcv) = unbounded();
        thread::spawn(move || {
            let mut address = format!(
               "{0}/ws/{1}@trade/{1}@depth",
                params.ws_address,
                symbol.name().to_lowercase(),
            );
            if let Some(listen_key) = listen_key {
                address += &format!("/{}", listen_key);
            }
            debug!("initiating WebSocket connection at {}", address);

            if let Err(err) = ws::connect(address.as_ref(), |out| {
                wss::Handler::new(out, snd.clone(), wss::KeepAlive::True, HandlerImpl{
                    params: params.clone(),
                    symbol,
                    book_snapshot_state: BookSnapshotState::None,
                    previous_u: None,
                })
            })
            {
                error!("WebSocket connection terminated with error: `{}`", err);
            }
        });

        rcv
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
/// Internal representation which keeps binance `u` indicator.
struct LimitUpdates {
    u: u64,
    updates: Vec<Timestamped<LimitUpdate>>,
}

type BookReceiver = mpsc::Receiver<Result<BinanceBookSnapshot<'static>, failure::Error>>;

#[derive(Debug)]
struct BookWaitingState {
    rcv: BookReceiver,
    events: Vec<LimitUpdates>,
}

#[derive(Debug)]
/// State of the book snapshot request:
/// * `None`: the request has not been made yet
/// * `Waiting(state)`: the request has started, in the meantime we have a `Receiver`
///   which will receive the snapshot, and a vector of past events which may need to be notified
///   to the `BinanceClient` consumer once the request is complete
/// * `Ok`: the request was completed already
enum BookSnapshotState {
    None,
    Waiting(BookWaitingState),
    Ok,
}

struct HandlerImpl {
    params: Params,
    symbol: Symbol,
    book_snapshot_state: BookSnapshotState,

    /// We keep track of the last `u` indicator sent by binance, this is used for checking
    /// the coherency of the ordering of the events sent by binance.
    previous_u: Option<u64>,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Deserialize)]
#[allow(non_snake_case)]
/// A JSON representation of a trade, sent by binance.
struct BinanceTrade<'a> {
    p: &'a str,
    q: &'a str,
    T: u64,
    m: bool,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Deserialize)]
/// A JSON representation of a limit update, embedded into other binance events.
struct BinanceLimitUpdate<'a> {
    #[serde(borrow)]
    price: Cow<'a, str>,
    #[serde(borrow)]
    size: Cow<'a, str>,
    _ignore: Vec<i32>,
}

impl<'a> BinanceLimitUpdate<'a> {
    pub fn owned(self) -> BinanceLimitUpdate<'static> {
        BinanceLimitUpdate {
            price: Cow::Owned(self.price.into_owned()),
            size: Cow::Owned(self.size.into_owned()),
            _ignore: vec![],
        }
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Deserialize)]
#[allow(non_snake_case)]
/// A JSON representation of an orderbook update, sent by binance.
struct BinanceDepthUpdate<'a> {
    E: u64,
    U: u64,
    u: u64,
    #[serde(borrow)]
    b: Vec<BinanceLimitUpdate<'a>>,
    #[serde(borrow)]
    a: Vec<BinanceLimitUpdate<'a>>,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Deserialize)]
#[allow(non_snake_case)]
/// A JSON representation of an orderbook snapshot, sent by binance.
struct BinanceBookSnapshot<'a> {
    lastUpdateId: u64,
    #[serde(borrow)]
    bids: Vec<BinanceLimitUpdate<'a>>,
    #[serde(borrow)]
    asks: Vec<BinanceLimitUpdate<'a>>,
}

impl<'a> BinanceBookSnapshot<'a> {
    pub fn owned(self) -> BinanceBookSnapshot<'static> {
        BinanceBookSnapshot {
            asks: self.asks.into_iter().map(|s| s.owned()).collect(),
            bids: self.bids.into_iter().map(|s| s.owned()).collect(),
            ..self
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Deserialize)]
#[allow(non_snake_case)]
/// A JSON representation of an order update, sent by binance.
struct BinanceExecutionReport<'a> {
    c: &'a str,
    C: &'a str,
    S: &'a str,
    q: &'a str,
    p: &'a str,
    x: &'a str,
    l: &'a str,
    z: &'a str,
    L: &'a str,
    n: &'a str,
    T: u64,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Deserialize)]
struct EventType<'a> {
    e: &'a str,
}

impl HandlerImpl {
    /// Utility function for converting a `BinanceLimitUpdate` into a `LimitUpdate` (with
    /// conversion in ticks and so on).
    fn convert_binance_update(&self, l: &BinanceLimitUpdate, side: Side)
        -> Result<LimitUpdate, tick::ConversionError>
    {
        Ok(
            LimitUpdate {
                side,
                price: self.symbol.price_tick().ticked(&l.price)?,
                size: self.symbol.size_tick().ticked(&l.size)?,
            }
        )
    }

    /// Parse a (should-be) JSON message sent by binance.
    fn parse_message(&mut self, json: &str) -> Result<Option<Notification>, failure::Error> {
        let event_type: EventType<'_> = serde_json::from_str(json)?;

        let notif = match event_type.e {
            "trade" => {
                let trade: BinanceTrade<'_> = serde_json::from_str(json)?;
                Some(
                    Notification::Trade(Trade {
                        size: self.symbol.size_tick().ticked(trade.q)?,
                        price: self.symbol.price_tick().ticked(trade.p)?,
                        maker_side: if trade.m { Side::Bid } else { Side::Ask },
                    }.with_timestamp(trade.T))
                )
            },

            "depthUpdate" => {
                let depth_update: BinanceDepthUpdate<'_> = serde_json::from_str(json)?;

                // The order is consistent if the previous `u + 1` is equal to current `U`.
                if let Some(previous_u) = self.previous_u {
                    if previous_u + 1 != depth_update.U {
                        panic!("previous `u + 1` and current `U` do not match");
                    }
                }
                self.previous_u = Some(depth_update.u);

                let bid = depth_update.b
                    .iter()
                    .map(|l| self.convert_binance_update(l, Side::Bid))
                    .map(|l| Ok(l?.with_timestamp(depth_update.E)));
                let ask = depth_update.a
                    .iter()
                    .map(|l| self.convert_binance_update(l, Side::Ask))
                    .map(|l| Ok(l?.with_timestamp(depth_update.E)));

                let updates =  bid.chain(ask).collect::<Result<Vec<_>, tick::ConversionError>>()?;
                if !updates.is_empty() {
                    Some(Notification::LimitUpdates(updates))
                } else {
                    None
                }
            },

            "executionReport" => {
                let report: BinanceExecutionReport<'_> = serde_json::from_str(json)?;

                match report.x {
                    "NEW" => Some(
                        Notification::OrderConfirmation(OrderConfirmation {
                            order_id: report.c.to_owned(),
                            size: self.symbol.size_tick().ticked(report.q)?,
                            price: self.symbol.price_tick().ticked(report.p)?,
                            side: match report.S {
                                "BUY" => Side::Bid,
                                "SELL" => Side::Ask,
                                other => bail!("wrong side `{}`", other),
                            },
                        }.with_timestamp(report.T))
                    ),
                    
                    "TRADE" => Some(
                        Notification::OrderUpdate(OrderUpdate {
                            order_id: report.c.to_owned(),
                            consumed_size: self.symbol.size_tick().ticked(report.l)?,

                            remaining_size: self.symbol.size_tick().ticked(report.q)?
                                - self.symbol.size_tick().ticked(report.z)?,

                            consumed_price: self.symbol.price_tick().ticked(report.L)?,
                            commission: self.symbol.commission_tick().ticked(report.n)?,
                        }.with_timestamp(report.T))
                    ),

                    "EXPIRED" => Some(
                        Notification::OrderExpiration(OrderExpiration {
                            order_id: report.c.to_owned(), // subtle: lower case `c`
                        }.with_timestamp(report.T))
                    ),

                    "CANCELED" => Some(
                        Notification::OrderExpiration(OrderExpiration {
                            order_id: report.C.to_owned(), // subtle: upper case `C`
                        }.with_timestamp(report.T))
                    ),

                    // "REJECTED" should already be handled by the REST API.
                    _ => None,
                }
            }

            _ => None,
        };
        Ok(notif)
    }

    fn process_book_snapshot(
        &self,
        snapshot: Result<BinanceBookSnapshot, failure::Error>,
        buffered_events: Vec<LimitUpdates>
    ) -> Result<Notification, failure::Error>
    {
        let snapshot = snapshot?;

        let bid = snapshot.bids
            .iter()
            .map(|l| self.convert_binance_update(l, Side::Bid))
            .map(|l| Ok(l?.timestamped()));

        let ask = snapshot.asks
            .iter()
            .map(|l| self.convert_binance_update(l, Side::Ask))
            .map(|l| Ok(l?.timestamped()));

        let buffered = buffered_events
            .into_iter()
            .filter(|update| update.u > snapshot.lastUpdateId)
            .flat_map(|update| update.updates)
            .map(Ok);

        let notif = Notification::LimitUpdates(
            bid.chain(ask).chain(buffered).collect::<Result<Vec<_>, tick::ConversionError>>()?
        );

        Ok(notif)
    }

    fn maybe_recv_book(&mut self, state: BookWaitingState)
        -> Option<Notification>
    {
        match state.rcv.try_recv() {
            Ok(book) => {
                debug!("received LOB snapshot");
                match self.process_book_snapshot(book, state.events) {
                    Ok(notif) => {
                        self.book_snapshot_state = BookSnapshotState::Ok;
                        Some(notif)
                    },
                    Err(err) => {
                        // We cannot continue without the book.
                        panic!(
                            "LOB processing encountered error: `{}`",
                            err
                        );
                    }
                }
            },

            // The snapshot request has not completed yet, we wait some more.
            Err(mpsc::TryRecvError::Empty) => {
                self.book_snapshot_state = BookSnapshotState::Waiting(state);
                None
            },

            // The only `Sender` has somehow disconnected, we won't receive
            // the book hence we cannot continue.
            Err(mpsc::TryRecvError::Disconnected) => {
                panic!("LOB sender has disconnected");
            }
        }
    }

    fn request_book_snapshot(&mut self, updates: Vec<Timestamped<LimitUpdate>>) {
        let (snd, rcv) = mpsc::channel();

        self.book_snapshot_state = BookSnapshotState::Waiting(
            BookWaitingState {
                rcv,

                // Buffer this first event we've just received.
                events: vec![LimitUpdates {
                    u: self.previous_u.unwrap(),
                    updates,
                }]
            }
        );

        let address = format!(
            "{}/api/v1/depth?symbol={}&limit=1000",
            self.params.http_address,
            self.symbol.name()
        ).parse().expect("invalid address");

        debug!("initiating LOB request at `{}`", address);

        thread::spawn(move || {
            let https = match hyper_tls::HttpsConnector::new(2) {
                Ok(https) => https,
                Err(err) => {
                    let _ = snd.send(Err(err).map_err(From::from));
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
                    let binance_error = serde_json::from_slice(&body);
                    Err(
                        RestError::from_binance_error(
                            status,
                            binance_error.ok()
                        )
                    )?;
                }

                let snapshot: BinanceBookSnapshot<'_> = serde_json::from_slice(&body)?;
                Ok(snapshot.owned())
            }).then(move |res| {
                let _ = snd.send(res);
                Ok::<(), !>(())
            });

            use tokio::runtime::current_thread;
            current_thread::block_on_all(fut).unwrap();
        });
    }
}

impl wss::HandlerImpl for HandlerImpl {
    fn on_open(&mut self, out: &ws::Sender) -> ws::Result<()> {
        out.ping(vec![])
    }

    fn on_message(&mut self, text: &str, out: &wss::NotifSender) -> Result<(), failure::Error> {
        match self.parse_message(text)? {
            // Depth update notif: behavior depends on the status of the order book snapshot.
            Some(Notification::LimitUpdates(updates)) => {
                match mem::replace(&mut self.book_snapshot_state, BookSnapshotState::Ok) {
                    // Very first limit update event received: time to ask for the book snapshot.
                    BookSnapshotState::None => self.request_book_snapshot(updates),

                    // Still waiting: buffer incoming events.
                    BookSnapshotState::Waiting(mut state) => {
                        state.events.push(LimitUpdates {
                            u: self.previous_u.unwrap(),
                            updates,
                        });

                        if let Some(notif) = self.maybe_recv_book(state) {
                            out.unbounded_send(notif).unwrap();
                        }
                    }

                    // We already received the book snapshot and notified the final consumer,
                    // we can now notify further notifications to them.
                    BookSnapshotState::Ok => out.unbounded_send(
                        Notification::LimitUpdates(updates)
                    ).unwrap(),
                }
            },

            // Other notif: just forward to the consumer.
            Some(notif) => out.unbounded_send(notif).unwrap(),

            None => (),
        }
        Ok(())
    }
}
