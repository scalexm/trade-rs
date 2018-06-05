use super::*;
use openssl::{sign::Signer, hash::MessageDigest, pkey::{PKey, Private}};
use hex;
use hyper::{Method, Request, Body};
use std::fmt;

struct QueryString {
    query: String,
}

impl QueryString {
    fn new() -> Self {
        QueryString {
            query: String::new(),
        }
    }

    fn push<P: fmt::Display>(&mut self, name: &str, arg: P) {
        if !self.query.is_empty() {
            self.query += "&";
        }
        self.query += &format!("{}={}", name, arg);
    }

    fn into_string(self) -> String {
        self.query
    }

    fn into_string_with_signature(mut self, key: &PKey<Private>) -> String {
        let mut signer = Signer::new(MessageDigest::sha256(), key).unwrap();
        signer.update(self.query.as_bytes()).unwrap();
        let signature = hex::encode(&signer.sign_to_vec().unwrap());
        self.push("signature", &signature);
        self.query
    }
}

trait AsStr {
    fn as_str(&self) -> &'static str;
}

impl AsStr for Side {
    fn as_str(&self) -> &'static str {
        match self {
            Side::Ask => "SELL",
            Side::Bid => "BUY",
        }
    }
}

impl AsStr for TimeInForce {
    fn as_str(&self) -> &'static str {
        match self {
            TimeInForce::GoodTilCanceled => "GTC",
            TimeInForce::FillOrKilll => "FOK",
            TimeInForce::ImmediateOrCancel => "IOC",
        }
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Deserialize)]
#[allow(non_snake_case)]
struct BinanceOrderAck {
    symbol: String,
    orderId: u64,
    clientOrderId: String,
    transactTime: u64,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Deserialize)]
#[allow(non_snake_case)]
struct BinanceCancelAck {
    symbol: String,
    origClientOrderId: String,
    orderId: u64,
    clientOrderId: String,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Deserialize)]
#[allow(non_snake_case)]
struct BinanceAccountInformation {
    makerCommission: u64,
    takerCommission: u64,
    buyerCommission: u64,
    sellerCommission: u64,
    canTrade: bool,
    canWithdraw: bool,
    canDeposit: bool,
    updateTime: u64,
    balances: Vec<Balance>,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
enum Signature {
    No,
    Yes,
}

impl Client {
    fn request(&self, endpoint: &str, method: Method, query: QueryString, sig: Signature)
        -> Box<Future<Item = hyper::Chunk, Error = Error> + Send + 'static>
    {
        let keys = self.keys.as_ref().expect(
            "cannot perform an HTTP request without a binance key pair"
        );

        let query = match sig {
            Signature::No => query.into_string(),
            Signature::Yes => query.into_string_with_signature(&keys.secret_key),
        };

        let address = format!(
            "{}/{}{}{}",
            self.params.http_address,
            endpoint,
            if query.is_empty() { "" } else { "?" },
            query
        );

        let request = Request::builder()
            .method(method)
            .uri(&address)
            .header("X-MBX-APIKEY", keys.api_key.as_bytes())
            .body(Body::empty());
        
        let request = match request {
            Ok(request) => request,
            Err(err) => return Box::new(
                Err(err).map_err(From::from).into_future()
            )
        };
        
        let https = match hyper_tls::HttpsConnector::new(2) {
            Ok(https) => https,
            Err(err) => return Box::new(
                Err(err).map_err(From::from).into_future()
            ),
        };

        let client = hyper::Client::builder().build::<_, hyper::Body>(https);
        let fut = client.request(request).and_then(|res| {
            let status = res.status();
            res.into_body().concat2().and_then(move |body| {
                Ok((status, body))
            })
        }).map_err(From::from).and_then(|(status, body)| {
            if status != hyper::StatusCode::OK {
                let binance_error = serde_json::from_slice(&body);
                Err(RestError::from_binance_error(status, binance_error.ok()))?;
            }
            Ok(body)
        });
        Box::new(fut)
    }

    crate fn order_impl(&self, order: Order)
        -> Box<Future<Item = OrderAck, Error = Error> + Send + 'static>
    {
        let mut query = QueryString::new();
        query.push("symbol", self.params.symbol.name.to_uppercase());
        query.push("side", order.side.as_str());
        query.push("type", "LIMIT");
        query.push("timeInForce", order.time_in_force.as_str());
        query.push("quantity", &order.size);
        query.push("price", &order.price);
        if let Some(order_id) = &order.order_id {
            query.push("newClientOrderId", order_id);
        }
        query.push("recvWindow", order.time_window);
        query.push("timestamp", timestamp_ms());

        let fut = self.request("api/v3/order", Method::POST, query, Signature::Yes)
            .and_then(|body|
        {
            let ack: BinanceOrderAck = serde_json::from_slice(&body)?;
            Ok(OrderAck {
                order_id: ack.clientOrderId,
                timestamp: ack.transactTime,
            })
        });
        Box::new(fut)
    }

    crate fn cancel_impl(&self, cancel: Cancel)
        -> Box<Future<Item = CancelAck, Error = Error> + Send + 'static>
    {
        let mut query = QueryString::new();
        query.push("symbol", self.params.symbol.name.to_uppercase());
        query.push("origClientOrderId", cancel.order_id);
        if let Some(cancel_id) = cancel.cancel_id {
            query.push("newClientOrderId", cancel_id);
        }
        query.push("recvWindow", cancel.time_window);
        query.push("timestamp", timestamp_ms());

        let fut = self.request("api/v3/order", Method::DELETE, query, Signature::Yes)
            .and_then(|body|
        {
            let ack: BinanceCancelAck = serde_json::from_slice(&body)?;
            Ok(CancelAck {
                cancel_id: ack.clientOrderId,
            })
        });
        Box::new(fut)
    }

    crate fn get_listen_key(&self)
        -> Box<Future<Item = String, Error = Error> + Send + 'static>
    {
        let query = QueryString::new();
        let fut = self.request("api/v1/userDataStream", Method::POST, query, Signature::No)
            .and_then(|body|
        {
            let body: serde_json::Value = serde_json::from_slice(&body)?;
            match body["listenKey"].as_str() {
                Some(key) => Ok(key.to_string()),
                None => bail!("status code 200 but no listen key was found"),
            }
        });
        Box::new(fut)
    }

    crate fn ping_impl(&self)
        -> Box<Future<Item = (), Error = Error> + Send + 'static>
    {
        let mut query = QueryString::new();
        query.push(
            "listenKey",
            &self.keys.as_ref().expect(
                "cannot perform an HTTP request without a binance key pair"
            ).listen_key
        );

        let fut = self.request("api/v1/userDataStream", Method::PUT, query, Signature::No)
            .and_then(|_| Ok(()));
        Box::new(fut)
    }

    /// Retrieve account information for this client.
    pub fn account_information(&self, time_window: u64)
        -> Box<Future<Item = AccountInformation, Error = Error> + Send + 'static>
    {
        let mut query = QueryString::new();
        query.push("recvWindow", time_window);
        query.push("timestamp", timestamp_ms());

        let fut = self.request("api/v3/account", Method::GET, query, Signature::Yes)
            .and_then(|body|
        {
            let info: BinanceAccountInformation = serde_json::from_slice(&body)?;
            Ok(AccountInformation {
                maker_commission: info.makerCommission,
                taker_commission: info.takerCommission,
                buyer_commission: info.buyerCommission,
                seller_commission: info.sellerCommission,
                can_trade: info.canTrade,
                can_withdraw: info.canWithdraw,
                can_deposit: info.canDeposit,
                update_timestamp: info.updateTime,
                balances: info.balances,
            })
        });
        Box::new(fut)
    }
}
