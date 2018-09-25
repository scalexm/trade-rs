use futures::sync::mpsc::{unbounded, UnboundedReceiver};
use failure::{bail, format_err};
use std::thread;
use serde_derive::{Deserialize, Serialize};
use log::{debug, error};
use crate::Side;
use crate::order_book::LimitUpdate;
use crate::tick;
use crate::api::{
    Notification,
    NotificationFlags,
    Trade,
    OrderConfirmation,
    OrderExpiration,
    OrderUpdate,
};
use crate::api::wss;
use crate::api::symbol::Symbol;
use crate::api::timestamp::{convert_str_timestamp, IntoTimestamped};
use crate::api::hitbtc::{KeyPair, Client};

impl Client {
    crate fn new_stream(&self, symbol: Symbol, flags: NotificationFlags)
        -> UnboundedReceiver<Notification>
    {
        let ws_address = self.params.ws_address.clone();
        let keys = self.keys.clone();
        let (snd, rcv) = unbounded();
        thread::spawn(move || {
            debug!("initiating WebSocket connection at {}", ws_address);
            
            if let Err(err) = ws::connect(ws_address.as_ref(), |out| {
                wss::Handler::new(out, snd.clone(), wss::KeepAlive::False, HandlerImpl {
                    symbol,
                    flags,
                    state: SubscriptionState::new(),
                    keys: keys.clone(),
                    last_sequence: None,
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
struct SubscriptionState {
    order_book: bool,
    trades: bool,
    report: bool,
}

impl SubscriptionState {
    fn new() -> Self {
        SubscriptionState {
            order_book: false,
            trades: false,
            report: false,
        }
    }
}

type SequenceNumber = u64;

struct HandlerImpl {
    symbol: Symbol,
    flags: NotificationFlags,
    keys: Option<KeyPair>,
    state: SubscriptionState,
    last_sequence: Option<SequenceNumber>,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Serialize)]
struct HitBtcSymbol<'a> {
    symbol: &'a str,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Serialize)]
struct HitBtcSubscription<'a> {
    method: &'a str,
    #[serde(borrow)]
    params: HitBtcSymbol<'a>,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Serialize)]
struct HitBtcReportSubscription<'a> {
    method: &'a str,
    params: (),
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Serialize)]
#[allow(non_snake_case)]
struct HitBtcAuthParams<'a> {
    algo: &'a str,
    pKey: &'a str,
    sKey: &'a str,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Serialize)]
struct HitBtcAuthentication<'a> {
    method: &'a str,
    params: HitBtcAuthParams<'a>,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Deserialize)]
struct HitBtcLimitUpdate<'a> {
    price: &'a str,
    size: &'a str,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Deserialize)]
struct HitBtcLimitUpdates<'a> {
    #[serde(borrow)]
    ask: Vec<HitBtcLimitUpdate<'a>>,
    #[serde(borrow)]
    bid: Vec<HitBtcLimitUpdate<'a>>,
    sequence: SequenceNumber,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Deserialize)]
struct HitBtcBookUpdate<'a> {
    #[serde(borrow)]
    params: HitBtcLimitUpdates<'a>,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Deserialize)]
struct HitBtcTradeData<'a> {
    price: &'a str,
    quantity: &'a str,
    side: &'a str,
    timestamp: &'a str,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Deserialize)]
struct HitBtcTradeParams<'a> {
    #[serde(borrow)]
    data: Vec<HitBtcTradeData<'a>>,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Deserialize)]
struct HitBtcTrades<'a> {
    #[serde(borrow)]
    params: HitBtcTradeParams<'a>
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Deserialize)]
#[allow(non_snake_case)]
struct HitBtcReportParams<'a> {
    clientOrderId: &'a str,
    side: &'a str,
    status: &'a str,
    quantity: &'a str,
    price: &'a str,
    cumQuantity: &'a str,
    #[serde(borrow)]
    tradeQuantity: Option<&'a str>,
    #[serde(borrow)]
    tradePrice: Option<&'a str>,
    updatedAt: &'a str,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Deserialize)]
struct HitBtcReport<'a> {
    #[serde(borrow)]
    params: HitBtcReportParams<'a>,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Deserialize)]
struct MethodType<'a> {
    #[serde(borrow)]
    method: Option<&'a str>,
}

impl HandlerImpl {
    fn convert_hit_btc_update(&self, l: HitBtcLimitUpdate<'_>, side: Side)
        -> Result<LimitUpdate, tick::ConversionError>
    {
        Ok(
            LimitUpdate {
                side,
                price: self.symbol.price_tick().ticked(l.price)?,
                size: self.symbol.size_tick().ticked(l.size)?,
            }
        )
    }

    fn convert_hit_btc_side(&self, side: &str) -> Result<Side, failure::Error> {
        let side = match side {
            "buy" => Side::Bid,
            "sell" => Side::Ask,
            other => bail!("wrong side: `{}`", other),
        };
        Ok(side)
    }

    fn parse_message(&mut self, json: &str, out: &wss::NotifSender) -> Result<(), failure::Error> {
        let method_type: MethodType<'_> = serde_json::from_str(json)?;

        let method = match method_type.method {
            Some(method) => method,
            None => return Ok(()),
        };

        match method {
            "snapshotOrderbook" | "updateOrderbook"
                if self.flags.contains(NotificationFlags::ORDER_BOOK) =>
            {
                let snapshot: HitBtcBookUpdate<'_> = serde_json::from_str(json)?;

                if !self.last_sequence.map(|s| s + 1 == snapshot.params.sequence).unwrap_or(true) {
                    panic!("desynchronized order book");
                }

                self.state.order_book = true;
                self.last_sequence = Some(snapshot.params.sequence);

                let bid = snapshot.params.bid
                    .into_iter()
                    .map(|l| self.convert_hit_btc_update(l, Side::Bid))
                    .map(|l| Ok(l?.timestamped()));

                let ask = snapshot.params.ask
                    .into_iter()
                    .map(|l| self.convert_hit_btc_update(l, Side::Ask))
                    .map(|l| Ok(l?.timestamped()));
                
                let updates = bid.chain(ask).collect::<Result<Vec<_>, tick::ConversionError>>()?;
                if !updates.is_empty() {
                    let notif = Notification::LimitUpdates(updates);
                    out.unbounded_send(notif).unwrap();
                }
            }

            "snapshotTrades" if self.flags.contains(NotificationFlags::TRADES) => {
                self.state.trades = true
            }

            "updateTrades" if self.flags.contains(NotificationFlags::TRADES) => {
                let trades: HitBtcTrades<'_> = serde_json::from_str(json)?;

                for trade in trades.params.data {
                    let timestamp = convert_str_timestamp(trade.timestamp)?;

                    let trade = Notification::Trade(Trade {
                        size: self.symbol.size_tick().ticked(trade.quantity)?,
                        price: self.symbol.price_tick().ticked(trade.price)?,
                        maker_side: self.convert_hit_btc_side(trade.side)?,
                    }.with_timestamp(timestamp));

                    out.unbounded_send(trade).unwrap();
                }
            }

            "activeOrders" if self.flags.contains(NotificationFlags::ORDERS) => {
                self.state.report = true
            }

            "report" if self.flags.contains(NotificationFlags::ORDERS) => {
                let report: HitBtcReport<'_> = serde_json::from_str(json)?;
                let timestamp = convert_str_timestamp(report.params.updatedAt)?;

                match report.params.status {
                    "new" => {
                        let order = OrderConfirmation {
                            size: self.symbol.size_tick().ticked(report.params.quantity)?,
                            price: self.symbol.price_tick().ticked(report.params.price)?,
                            side: self.convert_hit_btc_side(report.params.side)?,
                            order_id: report.params.clientOrderId.to_owned(),
                        }.with_timestamp(timestamp);
                        out.unbounded_send(Notification::OrderConfirmation(order)).unwrap();
                    }

                    "partiallyFilled" | "filled" => {
                        let update = OrderUpdate {
                            order_id: report.params.clientOrderId.to_owned(),
                            consumed_size: self.symbol.size_tick().ticked(
                                report.params.tradeQuantity
                                    .ok_or_else(|| format_err!("missing trade quantity"))?
                            )?,
                            consumed_price: self.symbol.price_tick().ticked(
                                report.params.tradePrice
                                    .ok_or_else(|| format_err!("missing trade price"))?
                            )?,
                            remaining_size: self.symbol.size_tick().ticked(report.params.quantity)?
                                - self.symbol.size_tick().ticked(report.params.cumQuantity)?,
                            commission: 0,
                        }.with_timestamp(timestamp);
                        out.unbounded_send(Notification::OrderUpdate(update)).unwrap();
                    }

                    "canceled" | "expired" | "suspended" => {
                        let expiration = OrderExpiration {
                            order_id: report.params.clientOrderId.to_owned(),
                        }.with_timestamp(timestamp);
                        out.unbounded_send(Notification::OrderExpiration(expiration)).unwrap();
                    }

                    _ => (),
                }
            }

            _ => (),
        }
        Ok(())
    }
}

impl wss::HandlerImpl for HandlerImpl {
    fn on_open(&mut self, out: &ws::Sender) -> ws::Result<()> {
        let params = HitBtcSymbol {
            symbol: self.symbol.name()
        };

        let subscription = HitBtcSubscription {
            method: "subscribeOrderbook",
            params,
        };
        
        match serde_json::to_string(&subscription) {
            Ok(value) => out.send(value)?,
            Err(err) => {
                panic!("failed to serialize `HitBtcSubscription`: `{}`", err);
            }
        }

        let subscription = HitBtcSubscription {
            method: "subscribeTrades",
            params,
        };
        
        match serde_json::to_string(&subscription) {
            Ok(value) => out.send(value)?,
            Err(err) => {
                panic!("failed to serialize `HitBtcSubscription`: `{}`", err);
            }
        }

        if let Some(keys) = self.keys.as_ref() {
            let auth = HitBtcAuthentication {
                method: "login",
                params: HitBtcAuthParams {
                    algo: "BASIC",
                    pKey: &keys.public_key,
                    sKey: &keys.secret_key,
                },
            };

            match serde_json::to_string(&auth) {
                Ok(value) => out.send(value)?,
                Err(err) => {
                    panic!("failed to serialize `HitBtcAuthentication`: `{}`", err);
                }
            }

            let report = HitBtcReportSubscription {
                method: "subscribeReports",
                params: (),
            };

            match serde_json::to_string(&report) {
                Ok(value) => out.send(value)?,
                Err(err) => {
                    panic!("failed to serialize `HitBtcReportSubscription`: `{}`", err);
                }
            }
        }

        Ok(())
    }

    fn on_message(&mut self, text: &str, out: &wss::NotifSender) -> Result<(), failure::Error> {
        self.parse_message(text, out)
    }
}
