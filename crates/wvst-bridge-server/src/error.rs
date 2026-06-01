use std::error::Error;
use std::fmt::{Display, Formatter};

pub type BridgeResult<T> = Result<T, BridgeError>;

#[derive(Debug)]
pub enum BridgeError {
    InvalidBindAddress(String),
    Io(std::io::Error),
    WebSocket(tokio_tungstenite::tungstenite::Error),
}

impl Display for BridgeError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidBindAddress(value) => write!(formatter, "invalid bind address: {value}"),
            Self::Io(error) => write!(formatter, "io error: {error}"),
            Self::WebSocket(error) => write!(formatter, "websocket error: {error}"),
        }
    }
}

impl Error for BridgeError {}

impl From<std::io::Error> for BridgeError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<tokio_tungstenite::tungstenite::Error> for BridgeError {
    fn from(value: tokio_tungstenite::tungstenite::Error) -> Self {
        Self::WebSocket(value)
    }
}
