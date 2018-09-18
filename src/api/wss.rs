// `Timeout`, `Token`
#![allow(deprecated)]

use ws::util::{Timeout, Token};
use futures::sync::mpsc::UnboundedSender;
use log::error;
use crate::api::Notification;

pub type NotifSender = UnboundedSender<Notification>;

/// An object handling a WebSocket API connection.
/// Inside handler functions, panicking can be used to terminate
/// the connection easily (the connection always happen in a
/// separate, free thread).
crate struct Handler<T> {
    out: ws::Sender,
    snd: NotifSender,
    keep_alive: bool,

    /// We keep a reference to the `EXPIRE` timeout so that we can cancel it when we receive
    /// something from the server.
    timeout: Option<Timeout>,

    inner: T,
}

crate trait HandlerImpl {
    fn on_open(&mut self, out: &ws::Sender) -> ws::Result<()>;
    fn on_message(&mut self, text: &str, out: &NotifSender) -> Result<(), failure::Error>;
}

const PING: Token = Token(1);
const EXPIRE: Token = Token(2);

const PING_TIMEOUT: u64 = 10_000;
const EXPIRE_TIMEOUT: u64 = 30_000;

impl<T> Handler<T> {
    crate fn new(
        out: ws::Sender,
        snd: UnboundedSender<Notification>,
        keep_alive: bool,
        inner: T
    ) -> Self
    {
        Handler {
            out,
            snd,
            keep_alive,
            timeout: None,
            inner,
        }
    }
}

impl<T: HandlerImpl> ws::Handler for Handler<T> {
    fn on_open(&mut self, _: ws::Handshake) -> ws::Result<()> {
        self.inner.on_open(&self.out)?;

        if self.keep_alive {
            self.out.timeout(PING_TIMEOUT, PING)?;
        }
        self.out.timeout(EXPIRE_TIMEOUT, EXPIRE)
    }

    fn on_timeout(&mut self, event: Token) -> ws::Result<()> {
        match event {
            PING => {
                self.out.ping(vec![])?;
                self.out.timeout(PING_TIMEOUT, PING)
            }
            EXPIRE => self.out.close(ws::CloseCode::Away),
            _ => Err(ws::Error::new(ws::ErrorKind::Internal, "invalid timeout token encountered")),
        }
    }

    fn on_new_timeout(&mut self, event: Token, timeout: Timeout) -> ws::Result<()> {
        if event == EXPIRE {
            if let Some(t) = self.timeout.take() {
                self.out.cancel(t)?;
            }
            self.timeout = Some(timeout)
        }
        Ok(())
    }

    fn on_frame(&mut self, frame: ws::Frame) -> ws::Result<Option<ws::Frame>> {
        self.out.timeout(EXPIRE_TIMEOUT, EXPIRE)?;
        Ok(Some(frame))
    }

    fn on_message(&mut self, msg: ws::Message) -> ws::Result<()> {
        if let ws::Message::Text(text) = msg {
            if let Err(err) = self.inner.on_message(&text, &self.snd) {
                error!("message handling encountered error: `{}`", err)
            }
        }
        Ok(())
    }
}
