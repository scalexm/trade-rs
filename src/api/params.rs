use serde_derive::{Serialize, Deserialize};
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
