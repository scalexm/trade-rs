use crate::*;
use api::*;
use std::{mem, thread};
use ws;
use serde_json;
use futures::{prelude::*, sync::mpsc::{unbounded, UnboundedReceiver}};
use std::sync::mpsc;
use super::{Client, RestError, Params};

impl Client {
    crate fn new_stream(&self) -> UnboundedReceiver<Notification> {
        let params = self.params.clone();
        let listen_key = self.keys.as_ref().map(|keys| keys.listen_key.clone());
        let (snd, rcv) = unbounded();
        thread::spawn(move || {
            let mut address = format!(
               "{0}/ws/{1}@trade/{1}@depth",
                params.ws_address,
                params.symbol.name.to_lowercase(),
            );
            if let Some(listen_key) = listen_key {
                address += &format!("/{}", listen_key);
            }
            info!("Initiating WebSocket connection at {}", address);
            
            if let Err(err) = ws::connect(address.as_ref(), |out| {
                wss::Handler::new(out, snd.clone(), HandlerImpl{
                    params: params.clone(),
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
    updates: Vec<LimitUpdate>,
}

type BookReceiver = mpsc::Receiver<Result<BinanceBookSnapshot, Error>>;

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

/// An object handling a WebSocket API connection.
struct HandlerImpl {
    params: Params,
    book_snapshot_state: BookSnapshotState,

    /// We keep track of the last `u` indicator sent by binance, this is used for checking
    /// the coherency of the ordering of the events sent by binance.
    previous_u: Option<u64>,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Deserialize)]
#[allow(non_snake_case)]
/// A JSON representation of a trade, sent by binance.
struct BinanceTrade {
    p: String,
    q: String,
    T: u64,
    m: bool,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Deserialize)]
/// A JSON representation of a limit update, embedded into other binance events.
struct BinanceLimitUpdate {
    price: String,
    size: String,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Deserialize)]
#[allow(non_snake_case)]
/// A JSON representation of an orderbook update, sent by binance.
struct BinanceDepthUpdate {
    E: u64,
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

#[derive(Clone, PartialEq, Eq, Hash, Debug, Deserialize)]
#[allow(non_snake_case)]
/// A JSON representation of an order update, sent by binance.
struct BinanceExecutionReport {
    c: String,
    S: String,
    q: String,
    p: String,
    x: String,
    l: String,
    z: String,
    L: String,
    n: String,
    T: u64,
}

impl HandlerImpl {
    /// Utility function for converting a `BinanceLimitUpdate` into a `LimitUpdate` (with
    /// conversion in ticks and so on).
    fn convert_binance_update(&self, l: &BinanceLimitUpdate, side: Side, timestamp: u64)
        -> Result<LimitUpdate, ConversionError>
    {
        Ok(
            LimitUpdate {
                side,
                price: self.params.symbol.price_tick.convert_unticked(&l.price)?,
                size: self.params.symbol.size_tick.convert_unticked(&l.size)?,
                timestamp,
            }
        )
    }

    /// Parse a (should-be) JSON message sent by binance.
    fn parse_message(&mut self, json: &str) -> Result<Option<Notification>, Error> {
        let json: serde_json::Value = serde_json::from_str(json)?;
        let event = match json["e"].as_str() {
            Some(event) => event.to_string(),
            None => return Ok(None),
        };

        let notif = match event.as_ref() {
            "trade" => {
                let trade: BinanceTrade = serde_json::from_value(json)?;
                Some(
                    Notification::Trade(Trade {
                        size: self.params.symbol.size_tick.convert_unticked(&trade.q)?,
                        timestamp: trade.T,
                        price: self.params.symbol.price_tick.convert_unticked(&trade.p)?,
                        maker_side: if trade.m { Side::Bid } else { Side::Ask },
                    })
                )
            },

            "depthUpdate" => {
                let depth_update: BinanceDepthUpdate = serde_json::from_value(json)?;
                let time = depth_update.E;

                // The order is consistent if the previous `u + 1` is equal to current `U`.
                if let Some(previous_u) = self.previous_u {
                    if previous_u + 1 != depth_update.U {
                        // FIXME: Maybe we should just shutdown here?
                        bail!("previous `u + 1` and current `U` do not match");
                    }
                }
                self.previous_u = Some(depth_update.u);

                let bid = depth_update.b
                                      .iter()
                                      .map(|l| self.convert_binance_update(l, Side::Bid, time));
                let ask = depth_update.a
                                      .iter()
                                      .map(|l| self.convert_binance_update(l, Side::Ask, time));

                Some(
                    Notification::LimitUpdates(
                        bid.chain(ask).collect::<Result<Vec<_>, ConversionError>>()?
                    )
                )
            },

            "executionReport" => {
                let report: BinanceExecutionReport = serde_json::from_value(json)?;

                match report.x.as_ref() {
                    "TRADE" => Some(
                        Notification::OrderUpdate(OrderUpdate {
                            order_id: report.c,

                            consumed_size: self.params.symbol.size_tick
                                .convert_unticked(&report.l)?,

                            remaining_size:
                                self.params.symbol.size_tick
                                    .convert_unticked(&report.q)?
                                -
                                self.params.symbol.size_tick
                                    .convert_unticked(&report.z)?,

                            consumed_price: self.params.symbol.price_tick
                                .convert_unticked(&report.L)?,

                            commission: self.params.symbol.commission_tick
                                .convert_unticked(&report.n)?,

                            timestamp: report.T,
                        })
                    ),

                    "EXPIRED" => Some(
                        Notification::OrderExpired(report.c)
                    ),

                    // "NEW", "CANCELED" and "REJECTED" should already be handled by the
                    // REST API.
                    _ => None,
                }
            }

            _ => None,
        };
        Ok(notif)
    }

    fn process_book_snapshot(
        &self,
        snapshot: Result<BinanceBookSnapshot, Error>,
        buffered_events: Vec<LimitUpdates>
    ) -> Result<Notification, Error>
    {
        let snapshot = snapshot?;
        let time = timestamp_ms();

        let bid = snapshot.bids
                          .iter()
                          .map(|l| self.convert_binance_update(l, Side::Bid, time));

        let ask = snapshot.asks
                          .iter()
                          .map(|l| self.convert_binance_update(l, Side::Ask, time));

        let buffered = buffered_events
            .into_iter()
            .filter(|update| update.u > snapshot.lastUpdateId)
            .flat_map(|update| update.updates)
            .map(|update| Ok(update));

        let notif = Notification::LimitUpdates(
            bid.chain(ask).chain(buffered).collect::<Result<Vec<_>, ConversionError>>()?
        );

        Ok(notif)
    }

    fn maybe_recv_book(&mut self, state: BookWaitingState)
        -> Option<Notification>
    {
        match state.rcv.try_recv() {
            Ok(book) => {
                info!("Received LOB snapshot");
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

    fn request_book_snapshot(&mut self, updates: Vec<LimitUpdate>) {
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
            self.params.symbol.name.to_uppercase()
        ).parse().expect("invalid address");

        info!("Initiating LOB request at `{}`", address);

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

                Ok(serde_json::from_slice(&body)?)
            }).then(move |res| {
                let _ = snd.send(res);
                Ok(())
            });

            use tokio::runtime;
            let mut runtime = runtime::current_thread::Runtime::new().unwrap();
            runtime.spawn(fut);
            runtime.run().unwrap();
        });
    }
}

impl wss::HandlerImpl for HandlerImpl {
    fn on_open(&mut self, out: &ws::Sender) -> ws::Result<()> {
        out.ping(vec![])
    }

    fn on_message(&mut self, text: String) -> Result<Option<Notification>, Error> {
        match self.parse_message(&text) {
            // Depth update notif: behavior depends on the status of the order book snapshot.
            Ok(Some(Notification::LimitUpdates(updates))) => {
                match mem::replace(&mut self.book_snapshot_state, BookSnapshotState::Ok) {
                    // Very first limit update event received: time to ask for the book snapshot.
                    BookSnapshotState::None => {
                        self.request_book_snapshot(updates);
                        Ok(None)
                    },

                    // Still waiting: buffer incoming events.
                    BookSnapshotState::Waiting(mut state) => {
                        state.events.push(LimitUpdates {
                            u: self.previous_u.unwrap(),
                            updates,
                        });
                        Ok(self.maybe_recv_book(state))
                    },

                    // We already received the book snapshot and notified the final consumer,
                    // we can now notify further notifications to them.
                    BookSnapshotState::Ok => {
                        Ok(Some(Notification::LimitUpdates(updates)))
                    },
                }
            },

            // Other notif: just forward to the consumer.
            other => other,
        }
    }
}
