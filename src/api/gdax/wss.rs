// `Timeout`, `Token`
#![allow(deprecated)]

use crate::*;
use api::*;
use serde_json;
use ws::{self, util::{Timeout, Token}};
use futures::sync::mpsc::*;
use super::{Client, Params};
use std::thread;
use chrono::{Utc, TimeZone};

impl Client {
    crate fn new_stream(&self) -> UnboundedReceiver<Notification> {
        let params = self.params.clone();
        let (snd, rcv) = unbounded();
        thread::spawn(move || {
            info!("Initiating WebSocket connection at {}", params.ws_address);
            
            if let Err(err) = ws::connect(params.ws_address.as_ref(), |out| Handler {
                out,
                snd: snd.clone(),
                params: params.clone(),
                timeout: None,
                state: SubscriptionState::NotSubscribed,
            })
            {
                error!("WebSocket connection terminated with error: `{}`", err);
            }   
        });
        
        rcv
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
enum SubscriptionState {
    NotSubscribed,
    Subscribed,
}

struct Handler {
    out: ws::Sender,
    snd: UnboundedSender<Notification>,
    params: Params,

    timeout: Option<Timeout>,

    state: SubscriptionState,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize)]
struct Subscription {
    #[serde(rename = "type")]
    type_: String,
    product_ids: Vec<String>,
    channels: Vec<String>,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Deserialize)]
struct BookSnapshot {
    bids: Vec<(String, String)>,
    asks: Vec<(String, String)>,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Deserialize)]
struct GdaxLimitUpdate {
    changes: Vec<(String, String, String)>,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Deserialize)]
struct GdaxMatch {
    time: String,
    size: String,
    price: String,
    side: String,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Deserialize)]
struct GdaxError {
    message: String,
    reason: String,
}

impl Handler {
    fn send(&mut self, notif: Notification) -> ws::Result<()> {
        if let Err(..) = self.snd.unbounded_send(notif) {
            // The corresponding receiver was dropped, this connection does not make sense
            // anymore.
            self.out.shutdown()?;
        }
        Ok(())
    }

    fn process_message(&mut self, json: &str) -> Result<Option<Notification>, Error> {
        let json: serde_json::Value = serde_json::from_str(json)?;
        let event = match json["type"].as_str() {
            Some(event) => event.to_string(),
            None => return Ok(None),
        };

        let notif = match event.as_ref() {
            "subscribe" => {
                if self.state != SubscriptionState::NotSubscribed {
                    error!("received `subscribe` event while already subscribed");
                }
                self.state = SubscriptionState::Subscribed;
                None
            }
            "snapshot" => {
                let snapshot: BookSnapshot = serde_json::from_value(json)?;
                let timestamp = timestamp_ms();

                let bid = snapshot.bids
                    .into_iter()
                    .map(|(price, size)| Ok(LimitUpdate {
                        side: Side::Bid,
                        price: self.params.symbol.price_tick.convert_unticked(&price)?,
                        size: self.params.symbol.size_tick.convert_unticked(&size)?,
                        timestamp,
                    }));
                let ask = snapshot.asks
                    .into_iter()
                    .map(|(price, size)| Ok(LimitUpdate {
                        side: Side::Ask,
                        price: self.params.symbol.price_tick.convert_unticked(&price)?,
                        size: self.params.symbol.size_tick.convert_unticked(&size)?,
                        timestamp,
                    }));
                
                Some(
                    Notification::LimitUpdates(
                        bid.chain(ask).collect::<Result<Vec<_>, ConversionError>>()?
                    )
                )
            },
            "l2update" => {
                let update: GdaxLimitUpdate = serde_json::from_value(json)?;
                let timestamp = timestamp_ms();

                let updates = update.changes
                    .into_iter()
                    .map(|(side, price, size)| Ok(LimitUpdate {
                        side: match side.as_ref() {
                            "buy" => Side::Bid,
                            "sell" => Side::Ask,
                            other => bail!("wrong side: `{}`", other),
                        },
                        price: self.params.symbol.price_tick.convert_unticked(&price)?,
                        size: self.params.symbol.size_tick.convert_unticked(&size)?,
                        timestamp,
                    }));
                Some(
                    Notification::LimitUpdates(
                        updates.collect::<Result<Vec<_>, Error>>()?
                    )
                )
            },
            "match" => {
                let trade: GdaxMatch = serde_json::from_value(json)?;
                let time = Utc.datetime_from_str(
                    &trade.time,
                    "%FT%T.%fZ"
                )?;
                let timestamp = (time.timestamp() as u64) * 1000
                    + u64::from(time.timestamp_subsec_millis());

                Some(
                    Notification::Trade(Trade {
                        size: self.params.symbol.size_tick.convert_unticked(&trade.size)?,
                        price: self.params.symbol.price_tick.convert_unticked(&trade.price)?,
                        maker_side: match trade.side.as_ref() {
                            "buy" => Side::Bid,
                            "sell" => Side::Ask,
                            other => bail!("wrong side: `{}`", other),
                        },
                        timestamp,
                    })
                )
            }
            "error" => {
                let error: GdaxError = serde_json::from_value(json)?;
                error!("{}: {}", error.message, error.reason);
                None
            },
            _ => None,
        };
        Ok(notif)
    }
}

const PING: Token = Token(1);
const EXPIRE: Token = Token(2);

const PING_TIMEOUT: u64 = 10_000;
const EXPIRE_TIMEOUT: u64 = 30_000;

impl ws::Handler for Handler {
    fn on_open(&mut self, _: ws::Handshake) -> ws::Result<()> {
        let subscription = Subscription {
            type_: "subscribe".to_string(),
            product_ids: vec![self.params.symbol.name.clone()],
            channels: vec![
                "level2".to_string(),
                "matches".to_string(),
            ],
        };
        
        match serde_json::to_string(&subscription) {
            Ok(value) => self.out.send(value)?,
            Err(err) => {
                panic!("failed to serialize `Subscription`: `{}`", err);
            }
        }

        self.out.timeout(PING_TIMEOUT, PING)?;
        self.out.timeout(EXPIRE_TIMEOUT, EXPIRE)
    }

    fn on_shutdown(&mut self) {
        info!("Client shutting down");
    }

    fn on_timeout(&mut self, event: Token) -> ws::Result<()> {
        match event {
            PING => {
                self.out.ping(vec![])?;
                self.out.timeout(PING_TIMEOUT, PING)
            }
            EXPIRE => self.out.close(ws::CloseCode::Away),
            _ => Err(ws::Error::new(ws::ErrorKind::Internal, "invalid timeout token encountered")),
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

    fn on_message(&mut self, msg: ws::Message) -> ws::Result<()> {
        if let ws::Message::Text(json) = msg {
            match self.process_message(&json) {
                Ok(Some(notif)) => {
                    self.send(notif)?;
                },

                Ok(None) => (),

                Err(err) => {
                    error!("Message parsing encountered error: `{}`", err)
                }
            }
        }
        Ok(())
    }
}
