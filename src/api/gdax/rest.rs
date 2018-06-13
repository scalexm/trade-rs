use super::*;
use openssl::{sign::Signer, hash::MessageDigest};
use hyper::{Method, Request, Body};
use chrono::{Utc, TimeZone};
use base64;

#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize)]
struct GdaxOrder<'a> {
    size: &'a str,
    price: &'a str,
    side: &'a str,
    product_id: &'a str,
    #[serde(borrow)]
    client_oid: Option<&'a str>,
    time_in_force: &'a str,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Deserialize)]
struct GdaxOrderAck<'a> {
    id: &'a str,
    created_at: &'a str,
}

impl Client {
    fn request(&self, endpoint: &str, method: Method, body: String)
        -> Box<Future<Item = hyper::Chunk, Error = Error> + Send + 'static>
    {
        let keys = self.keys.as_ref().expect(
            "cannot perform an HTTP request without a GDAX key pair"
        );

        let address = format!(
            "{}/{}",
            self.params.http_address,
            endpoint,
        );

        let timestamp = timestamp_ms() / 1000;

        let mut signer = Signer::new(MessageDigest::sha256(), &keys.secret_key).unwrap();
        let what = format!("{}{}/{}{}", timestamp, method, endpoint, body);
        signer.update(what.as_bytes()).unwrap();
        let signature = base64::encode(&signer.sign_to_vec().unwrap());

        let request = Request::builder()
            .method(method)
            .uri(&address)
            .header("CB-ACCESS-KEY", keys.api_key.as_bytes())
            .header("CB-ACCESS-SIGN", signature.as_bytes())
            .header("CB-ACCESS-TIMESTAMP", format!("{}", timestamp).as_bytes())
            .header("CB-ACCESS-PASSPHRASE", keys.pass_phrase.as_bytes())
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
                let gdax_error = serde_json::from_slice(&body);
                Err(RestError::from_gdax_error(status, gdax_error.ok()))?;
            }
            Ok(body)
        });
        Box::new(fut)
    }

    crate fn order_impl(&self, order: &Order)
        -> Box<Future<Item = OrderAck, Error = Error> + Send + 'static>
    {
        let order = GdaxOrder {
            size: &self.params.symbol.size_tick.convert_ticked(order.size)
                .expect("bad size tick"),
            price: &self.params.symbol.price_tick.convert_ticked(order.price)
                .expect("bad price tick"),
            side: &order.side.as_str().to_lowercase(),
            product_id: &self.params.symbol.name,
            client_oid: order.order_id.as_ref().map(|oid| oid.as_ref()),
            time_in_force: order.time_in_force.as_str(),
        };

        let body = serde_json::to_string(&order).expect("invalid json");

        let fut = self.request("orders", Method::POST, body).and_then(|body| {
            let ack: GdaxOrderAck = serde_json::from_slice(&body)?;

            let time = Utc.datetime_from_str(
                    ack.created_at,
                    "%FT%T.%fZ"
                )?;
            let timestamp = (time.timestamp() as u64) * 1000
                + u64::from(time.timestamp_subsec_millis());
            Ok(OrderAck {
                // FIXME: keep a hash map client id <-> gdax id
                order_id: ack.id.to_owned(),
                timestamp,
            })
        });
        Box::new(fut)
    }
}