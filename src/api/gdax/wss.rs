use order_book::LimitUpdate;
use tick::ConversionError;
use api::*;
use super::{Keys, Client, Params};
use serde_json;
use ws;
use futures::sync::mpsc::{unbounded, UnboundedReceiver};
use std::thread;
use chrono::{Utc, TimeZone};
use std::collections::HashMap;
use chashmap::CHashMap;
use std::sync::Arc;

impl Client {
    crate fn new_stream(&self) -> UnboundedReceiver<Notification> {
        let params = self.params.clone();
        let keys = self.keys.clone();
        let order_ids = self.order_ids.clone();
        let (snd, rcv) = unbounded();
        thread::spawn(move || {
            info!("Initiating WebSocket connection at {}", params.ws_address);
            
            if let Err(err) = ws::connect(params.ws_address.as_ref(), |out| {
                wss::Handler::new(out, snd.clone(), false, HandlerImpl {
                    params: params.clone(),
                    state: SubscriptionState::NotSubscribed,
                    keys: keys.clone(),
                    orders: HashMap::new(),
                    order_ids: order_ids.clone(),
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
    keys: Option<Keys>,

    // server order id => client order
    orders: HashMap<String, OrderConfirmation>,

    // client order id => server order id (shared with `Client`)
    order_ids: Arc<CHashMap<String, String>>,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize)]
#[serde(untagged)]
enum GdaxChannel<'a> {
    Channel(&'a str),
    WithProducts {
        name: &'a str,
        product_ids: &'a [&'a str],
    },
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize)]
struct GdaxAuth<'a> {
    key: &'a str,
    signature: String,
    timestamp: u64,
    passphrase: &'a str,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize)]
/// Subscription parameters to be sent to GDAX.
struct GdaxSubscription<'a> {
    #[serde(rename = "type")]
    type_: &'a str,
    product_ids: &'a [&'a str],
    channels: Vec<GdaxChannel<'a>>,

    #[serde(flatten)]
    auth: Option<GdaxAuth<'a>>,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Deserialize)]
/// A JSON representation of an order book snapshot, sent by GDAX.
struct GdaxBookSnapshot<'a> {
    #[serde(borrow)]
    bids: Vec<(&'a str, &'a str)>,
    #[serde(borrow)]
    asks: Vec<(&'a str, &'a str)>,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Deserialize)]
/// A JSON representation of an order book update, sent by GDAX.
struct GdaxLimitUpdate<'a> {
    #[serde(borrow)]
    changes: Vec<(&'a str, &'a str, &'a str)>,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Deserialize)]
/// A JSON representation of a trade, sent by GDAX.
struct GdaxMatch<'a> {
    time: &'a str,
    size: &'a str,
    price: &'a str,
    side: &'a str,
    maker_order_id: &'a str,
    taker_order_id: &'a str,
    profile_id: Option<&'a str>,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Deserialize)]
struct GdaxReceived<'a> {
    time: &'a str,
    client_oid: Option<&'a str>,
    order_id: &'a str,
    size: &'a str,
    price: &'a str,
    side: &'a str,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Deserialize)]
struct GdaxDone<'a> {
    reason: &'a str,
    order_id: &'a str,
    time: &'a str,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Deserialize)]
struct GdaxError<'a> {
    message: &'a str,
    reason: Option<&'a str>,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Deserialize)]
struct EventType<'a> {
    #[serde(rename = "type")]
    type_: &'a str,
}

impl HandlerImpl {
    fn convert_gdax_update(&self, l: (&str, &str), side: Side)
        -> Result<LimitUpdate, ConversionError>
    {
        Ok(
            LimitUpdate {
                side,
                price: self.params.symbol.price_tick.convert_unticked(l.0)?,
                size: self.params.symbol.size_tick.convert_unticked(l.1)?,
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
    fn parse_message(&mut self, json: &str) -> Result<Vec<Notification>, Error> {
        let event_type: EventType = serde_json::from_str(json)?;

        let notifs = match event_type.type_ {
            "subscribe" => {
                // FIXME: close the stream if we get an error when trying to subscribe
                if self.state != SubscriptionState::NotSubscribed {
                    error!("received `subscribe` event while already subscribed");
                }
                self.state = SubscriptionState::Subscribed;
                vec![]
            },

            "snapshot" => {
                let snapshot: GdaxBookSnapshot = serde_json::from_str(json)?;

                let bid = snapshot.bids
                    .into_iter()
                    .map(|(price, size)| self.convert_gdax_update((price, size), Side::Bid))
                    .map(|l| Ok(l?.timestamped()));

                let ask = snapshot.asks
                    .into_iter()
                    .map(|(price, size)| self.convert_gdax_update((price, size), Side::Ask))
                    .map(|l| Ok(l?.timestamped()));
                
                vec![
                    Notification::LimitUpdates(
                        bid.chain(ask).collect::<Result<Vec<_>, ConversionError>>()?
                    )
                ]
            },

            "l2update" => {
                let update: GdaxLimitUpdate = serde_json::from_str(json)?;

                let updates = update.changes
                    .into_iter()
                    .map(|(side, price, size)| {
                        let side = self.convert_gdax_side(side)?;
                        Ok(self.convert_gdax_update((price, size), side)?)
                    })
                    .map(|l: Result<_, Error>| Ok(l?.timestamped()));
                vec![
                    Notification::LimitUpdates(
                        updates.collect::<Result<Vec<_>, Error>>()?
                    )
                ]
            },

            "match" => {
                let trade: GdaxMatch = serde_json::from_str(json)?;
                let time = Utc.datetime_from_str(
                    trade.time,
                    "%FT%T.%fZ"
                )?;
                let timestamp = (time.timestamp() as u64) * 1000
                    + u64::from(time.timestamp_subsec_millis());
                
                let size = self.params.symbol.size_tick.convert_unticked(trade.size)?;
                let price = self.params.symbol.price_tick.convert_unticked(trade.price)?;
                
                let mut notifs = Vec::new();

                // An order which is about us
                if trade.profile_id.is_some() {
                    let mut update_order = |order: &mut OrderConfirmation| {
                        order.size -= size;

                        notifs.push(
                            Notification::OrderUpdate(OrderUpdate {
                                order_id: order.order_id.clone(),
                                consumed_size: size,
                                consumed_price: price,
                                remaining_size: order.size,
                                commission: 0,
                            }.with_timestamp(timestamp))
                        );
                    };

                    // These two conditions are exclusive.
                    if let Some(order) = self.orders.get_mut(trade.taker_order_id) {
                        update_order(order);
                    }
                    if let Some(order) = self.orders.get_mut(trade.maker_order_id) {
                        update_order(order);
                    }
                }

                notifs.push(
                    Notification::Trade(Trade {
                        size,
                        price,
                        maker_side: self.convert_gdax_side(trade.side)?,
                    }.with_timestamp(timestamp))
                );

                notifs
            },

            "received" => {
                let received: GdaxReceived = serde_json::from_str(json)?;
                let time = Utc.datetime_from_str(
                    received.time,
                    "%FT%T.%fZ"
                )?;
                let timestamp = (time.timestamp() as u64) * 1000
                    + u64::from(time.timestamp_subsec_millis());

                let size = self.params.symbol.size_tick.convert_unticked(received.size)?;
                let price = self.params.symbol.price_tick.convert_unticked(received.price)?;
                let side = self.convert_gdax_side(received.side)?;

                // The order id specified by the user, which defaults to the server order id
                // in case it was left unspecified.
                let order_id = received.client_oid.map(|oid| oid.to_owned())
                    .unwrap_or_else(|| received.order_id.to_owned());
                
                // Don't forget to update the concurrent map `server order id => client order id`
                // in case the WebSocket notif arrives before the HTTP response
                self.order_ids.insert(order_id.clone(), received.order_id.to_owned());
                debug!("insert order id {} (from WSS)", order_id);
                
                let order = OrderConfirmation {
                    size,
                    price,
                    side,
                    order_id,
                };

                self.orders.insert(received.order_id.to_owned(), order.clone());

                vec![Notification::OrderConfirmation(order.with_timestamp(timestamp))]
            }

            "done" => {
                let done: GdaxDone = serde_json::from_str(json)?;
                let time = Utc.datetime_from_str(
                    done.time,
                    "%FT%T.%fZ"
                )?;
                let timestamp = (time.timestamp() as u64) * 1000
                    + u64::from(time.timestamp_subsec_millis());

                if done.reason != "canceled" {
                    return Ok(vec![]);
                }

                let order_id = match self.orders.get(done.order_id) {
                    Some(order) => order.order_id.to_owned(),
                    None => return Ok(vec![]),
                };

                vec![Notification::OrderExpiration(OrderExpiration {
                    order_id,
                }.with_timestamp(timestamp))]
            }

            "error" => {
                let error: GdaxError = serde_json::from_str(json)?;
                bail!("{}: {:?}", error.message, error.reason);
            },

            _ => vec![],
        };
        Ok(notifs)
    }
}

impl wss::HandlerImpl for HandlerImpl {
    fn on_open(&mut self, out: &ws::Sender) -> ws::Result<()> {
        let product_ids = [self.params.symbol.name.as_ref()];
        let mut channels = vec![
            GdaxChannel::Channel("level2"),
            GdaxChannel::Channel("matches"),
            GdaxChannel::WithProducts {
                name: "heartbeat",
                product_ids: &product_ids,
            },
        ];
        let mut auth = None;

        if let Some(keys) = self.keys.as_ref() {
            use openssl::{sign::Signer, hash::MessageDigest};

            let timestamp = timestamp_ms() / 1000;
            let mut signer = Signer::new(MessageDigest::sha256(), &keys.secret_key).unwrap();
            let what = format!("{}GET/users/self/verify", timestamp);
            signer.update(what.as_bytes()).unwrap();
            let signature = base64::encode(&signer.sign_to_vec().unwrap());

            auth = Some(GdaxAuth {
                key: &keys.api_key,
                signature,
                timestamp,
                passphrase: &keys.pass_phrase,
            });

            channels.push(GdaxChannel::Channel("user"));
        }

        let subscription = GdaxSubscription {
            type_: "subscribe",
            product_ids: &product_ids,
            channels,
            auth,
        };
        
        match serde_json::to_string(&subscription) {
            Ok(value) => out.send(value),
            Err(err) => {
                panic!("failed to serialize `Subscription`: `{}`", err);
            }
        }
    }

    fn on_message(&mut self, text: String) -> Result<Vec<Notification>, Error> {
        self.parse_message(&text)
    }
}
