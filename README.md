# trade-rs

Utilities for trading on crypto-currencies exchanges. Long term goal is to
provide a general enough, unified API for abstracting over various exchanges,
hence making it easier to develop cross exchange automated trading
strategies.

Uses [tokio](https://github.com/tokio-rs/tokio), [hyper](https://github.com/hyperium/hyper)
and [ws-rs](https://github.com/housleyjk/ws-rs) for asynchronous requests and
streaming.

Some (currently undocumented) sample code can be found in the `binance_prompt`
CLI app.

Exchanges currently implemented:
* binance

Next to come:
* GDAX
