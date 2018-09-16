use std::ops::Deref;
use arrayvec::ArrayString;
use crate::Tick;

/// A small string type used for symbol names.
pub type SymbolName = ArrayString<[u8; 10]>;

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
/// A type carrying information about the traded symbol.
pub struct Symbol {
    name: SymbolName,
    price_tick: Tick,
    size_tick: Tick,
    commission_tick: Tick,
}

impl Symbol {
    pub fn name(&self) -> &str {
        &self.name
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct WithSymbol<T> {
    symbol: Symbol,
    inner: T,
}

impl<T> WithSymbol<T> {
    pub fn symbol(&self) -> Symbol {
        self.symbol
    }

    pub fn into_inner(self) -> T {
        self.inner
    }
}

impl<T> Deref for WithSymbol<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

pub trait IntoWithSymbol: Sized {
    fn with_symbol(self, symbol: Symbol) -> WithSymbol<Self> {
        WithSymbol {
            symbol,
            inner: self,
        }
    }
}

impl<T: Sized> IntoWithSymbol for T { }
