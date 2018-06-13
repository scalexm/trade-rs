use crate::*;
use api::*;
use super::{Client, Params};
use serde_json;
use ws;
use futures::sync::mpsc::{unbounded, UnboundedReceiver};
use std::thread;
use chrono::{Utc, TimeZone};

impl Client {
    crate fn new_stream(&self) -> UnboundedReceiver<Notification> {
        let params = self.params.clone();
        let (snd, rcv) = unbounded();
        thread::spawn(move || {
            info!("Initiating WebSocket connection at {}", params.ws_address);
            
            if let Err(err) = ws::connect(params.ws_address.as_ref(), |out| {
                wss::Handler::new(out, snd.clone(), HandlerImpl {
                    params: params.clone(),
                    state: SubscriptionState::NotSubscribed,
                })
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

struct HandlerImpl {
    params: Params,
    state: SubscriptionState,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize)]
struct Subscription<'a> {
    #[serde(rename = "type")]
    type_: &'a str,
    product_ids: &'a [&'a str],
    channels: Vec<&'a str>,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Deserialize)]
struct BookSnapshot<'a> {
    #[serde(borrow)]
    bids: Vec<(&'a str, &'a str)>,
    #[serde(borrow)]
    asks: Vec<(&'a str, &'a str)>,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Deserialize)]
struct GdaxLimitUpdate<'a> {
    #[serde(borrow)]
    changes: Vec<(&'a str, &'a str, &'a str)>,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Deserialize)]
struct GdaxMatch<'a> {
    time: &'a str,
    size: &'a str,
    price: &'a str,
    side: &'a str,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Deserialize)]
struct GdaxError<'a> {
    message: &'a str,
    reason: &'a str,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Deserialize)]
struct EventType<'a> {
    #[serde(rename = "type")]
    type_: &'a str,
}

impl HandlerImpl {
    fn convert_gdax_update(&self, l: (&str, &str), side: Side, timestamp: u64)
        -> Result<LimitUpdate, ConversionError>
    {
        Ok(
            LimitUpdate {
                side,
                price: self.params.symbol.price_tick.convert_unticked(l.0)?,
                size: self.params.symbol.size_tick.convert_unticked(l.1)?,
                timestamp,
            }
        )
    }

    fn convert_gdax_side(&self, side: &str) -> Result<Side, Error> {
        let side = match side {
            "buy" => Side::Bid,
            "sell" => Side::Ask,
            other => bail!("wrong side: `{}`", other),
        };
        Ok(side)
    }

    /// Parse a (should-be) JSON message sent by gdax.
    fn parse_message(&mut self, json: &str) -> Result<Option<Notification>, Error> {
        let event_type: EventType = serde_json::from_str(json)?;

        let notif = match event_type.type_.as_ref() {
            "subscribe" => {
                if self.state != SubscriptionState::NotSubscribed {
                    error!("received `subscribe` event while already subscribed");
                }
                self.state = SubscriptionState::Subscribed;
                None
            },

            "snapshot" => {
                let snapshot: BookSnapshot = serde_json::from_str(json)?;
                let timestamp = timestamp_ms();

                let bid = snapshot.bids
                    .into_iter()
                    .map(|(price, size)| {
                        self.convert_gdax_update((price, size), Side::Bid, timestamp)
                    });
                let ask = snapshot.asks
                    .into_iter()
                    .map(|(price, size)| {
                        self.convert_gdax_update((price, size), Side::Ask, timestamp)
                    });
                
                Some(
                    Notification::LimitUpdates(
                        bid.chain(ask).collect::<Result<Vec<_>, ConversionError>>()?
                    )
                )
            },

            "l2update" => {
                let update: GdaxLimitUpdate = serde_json::from_str(json)?;
                let timestamp = timestamp_ms();

                let updates = update.changes
                    .into_iter()
                    .map(|(side, price, size)| {
                        let side = self.convert_gdax_side(side)?;
                        Ok(self.convert_gdax_update((price, size), side, timestamp)?)
                    });
                Some(
                    Notification::LimitUpdates(
                        updates.collect::<Result<Vec<_>, Error>>()?
                    )
                )
            },

            "match" => {
                let trade: GdaxMatch = serde_json::from_str(json)?;
                let time = Utc.datetime_from_str(
                    trade.time,
                    "%FT%T.%fZ"
                )?;
                let timestamp = (time.timestamp() as u64) * 1000
                    + u64::from(time.timestamp_subsec_millis());

                Some(
                    Notification::Trade(Trade {
                        size: self.params.symbol.size_tick.convert_unticked(trade.size)?,
                        price: self.params.symbol.price_tick.convert_unticked(trade.price)?,
                        maker_side: self.convert_gdax_side(trade.side)?,
                        timestamp,
                    })
                )
            },

            "error" => {
                let error: GdaxError = serde_json::from_str(json)?;
                bail!("{}: {}", error.message, error.reason);
            },

            _ => None,
        };
        Ok(notif)
    }
}

impl wss::HandlerImpl for HandlerImpl {
    fn on_open(&mut self, out: &ws::Sender) -> ws::Result<()> {
        let subscription = Subscription {
            type_: "subscribe",
            product_ids: &[&self.params.symbol.name],
            channels: vec![
                "level2",
                "matches",
            ],
        };
        
        match serde_json::to_string(&subscription) {
            Ok(value) => out.send(value),
            Err(err) => {
                panic!("failed to serialize `Subscription`: `{}`", err);
            }
        }
    }

    fn on_message(&mut self, text: String) -> Result<Option<Notification>, Error> {
        self.parse_message(&text)
    }
}
