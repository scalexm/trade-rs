use trade::prelude::*;
use failure::{format_err, bail};

/// Send one buy order at `(best bid) - margin` and one sell order at `(best ask) + margin`.
/// Do not wait for the orders to be filled, just wait that they are confirmed by the exchange.
/// 
/// `margin` is measured in tick units, i.e. the smallest possible price increment. For symbols
/// quoted in USD/USDT/TUSD, one tick unit is usually equal to one cent (0.01$).
/// 
/// This function may work with any client implementing the `ApliClient` trait.
fn send_orders<C: ApiClient>(client: &C, symbol: &str, margin: TickUnit)
    -> Result<(), failure::Error>
{
    let symbol = client.find_symbol(symbol)
        .ok_or_else(|| format_err!("cannot find requested symbol"))?;
    
    // `live_order_book` is a self-maintained copy of the exchange order book, it is
    // continuously updated in a background thread.
    let live_order_book = LiveOrderBook::new::<C>(
        client.stream_with_flags(symbol, NotificationFlags::ORDER_BOOK)
    );

    let (best_bid, best_ask) = match live_order_book.order_book() {
        BookState::Live(copy) => (copy.best_bid(), copy.best_ask()),
        BookState::Disconnected => bail!("stream has disconnected"),
    };

    // One can specify order prices and sizes either in tick units or with a string
    // numerical representation.
    let bid_order = trade::api::Order::new(best_bid - margin, "1.00000000", Side::Bid)
        .with_order_id::<C>("my_bid_order");
    let ask_order = trade::api::Order::new(best_ask + margin, "1.00000000", Side::Ask)
        .with_order_id::<C>("my_ask_order");

    // We need an event loop + scheduler in order to run our HTTP requests.
    let mut runtime = tokio::runtime::current_thread::Runtime::new().unwrap();

    // Try to send the buy order.
    runtime.block_on(client.order(bid_order.add_symbol(symbol)))?;

    // Now try to send the sell order.
    if let Err(_) = runtime.block_on(client.order(ask_order.add_symbol(symbol))) {
        println!("we were not able to execute the sell order, better to cancel the buy one");
        let cancel_order = trade::api::Cancel::new(
            // Do not use the name "my_bid_order" directly, as it was only given
            // as a hint and the actual order ID may be different.
            // However we did provide an order ID, so `unwrap` is ok here.
            bid_order.order_id().unwrap().to_owned()
        );
        runtime.block_on(client.cancel(cancel_order.add_symbol(symbol)))?;
    }

    Ok(())
}

fn main() -> Result<(), failure::Error> {
    let params = trade::api::Params {
        streaming_endpoint: "wss://ws-feed-public.sandbox.pro.coinbase.com".to_owned(),
        rest_endpoint: "https://api-public.sandbox.pro.coinbase.com".to_owned(),
    };

    let key_pair = trade::api::gdax::KeyPair::new(
        "my_api_key".to_owned(),
        "my_secret_key".to_owned(),
        "my_pass_phrase".to_owned(),
    );

    // Use a client to the Coinbase Pro sandbox as an example.
    let client = trade::api::gdax::Client::new(params, Some(key_pair))?;
    send_orders(&client, "BTCUSD", 10)?;

    Ok(())
}