# binance-prompt

Small CLI app for testing `trade-rs` implementation of the binance API.
Require two config files in the same directory as the executable:
* `params.json`: JSON representation of a `trade::api::binance::Params` object
* `keys.json`: JSON representation of a `trade::api::binance::KeyPair` object

Example config files can be found as `params.example.json` and `keys.example.json`.