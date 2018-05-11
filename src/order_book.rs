use std::collections::BTreeMap;
use crate::*;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct OrderBook {
    pub ask: BTreeMap<Price, usize>,
    pub bid: BTreeMap<Price, usize>,
}
