pub mod binance;

use crate::*;
use notify::Notifier;

pub trait ApiClient<N: Notifier> {
    type Params;
    type Notifier;

    fn new(params: Self::Params, notifier: N) -> Self;

    fn start_streaming(&mut self);
}