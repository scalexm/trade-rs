use crate::Tick;

#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
/// A type carrying information about the traded symbol.
pub struct SymbolInfo {
    /// Symbol name.
    pub name: String,

    /// Tick unit for prices.
    pub price_tick: Tick,

    /// Tick unit for sizes.
    pub size_tick: Tick,

    /// Tick unit for commissions.
    pub commission_tick: Tick,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
/// A binance key pair: api key + secret key.
pub struct KeyPair {
    crate api_key: String,
    crate secret_key: String,
}

impl KeyPair {
    pub fn new(api_key: String, secret_key: String) -> Self {
        KeyPair {
            api_key,
            secret_key,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
/// Params needed for a binance API client.
pub struct Params {
    /// Symbol information.
    pub symbol: SymbolInfo,

    /// WebSocket API address.
    pub ws_address: String,

    /// HTTP REST API address.
    pub http_address: String,
}
