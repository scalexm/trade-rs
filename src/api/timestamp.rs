use std::ops::Deref;

pub type Timestamp = u64;

/// Return UTC timestamp in milliseconds.
pub fn timestamp_ms() -> Timestamp {
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time went backward");
    timestamp.as_secs() * 1000 + u64::from(timestamp.subsec_millis())
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Deserialize, Serialize)]
/// Wrapper around a type carrying an additionnal timestamp. Deref to `T`.
pub struct Timestamped<T> {
    timestamp: Timestamp,
    #[serde(flatten)]
    inner: T,
}

impl<T> Timestamped<T> {
    /// Registered timestamp.
    pub fn timestamp(&self) -> Timestamp {
        self.timestamp
    }

    /// Return the wrapped value.
    pub fn into_inner(self) -> T {
        self.inner
    }
}

impl<T> Deref for Timestamped<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

pub trait IntoTimestamped: Sized {
    fn timestamped(self) -> Timestamped<Self> {
        Timestamped {
            timestamp: timestamp_ms(),
            inner: self,
        }
    }

    fn with_timestamp(self, timestamp: Timestamp) -> Timestamped<Self> {
        Timestamped {
            timestamp,
            inner: self,
        }
    }
}

impl<T: Sized> IntoTimestamped for T { }
