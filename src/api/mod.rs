pub mod binance;

use crate::*;
use notify::Notifier;

pub use crossbeam_channel::Sender;

pub enum ApiAction {
    RequestOrderBookSnapshot,
}

pub trait ApiClient<N: Notifier> {
    fn sender(&self) -> Sender<ApiAction>;
    fn stream(&mut self, notifier: N);
}
