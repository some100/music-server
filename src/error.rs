use crate::Msg;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ServerError {
    #[error("WebSocket Error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),
    #[error("Send Error: {0}")]
    Send(#[from] tokio::sync::broadcast::error::SendError<Msg>),
    #[error("IO Error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Address Parse Error: {0}")]
    AddrParse(#[from] std::net::AddrParseError),
    #[error("HTTP Error: {0}")]
    Http(#[from] warp::Error),
    #[error("JSON Error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Join Error: {0}")]
    Join(#[from] tokio::task::JoinError),
    #[error("Connection with {0} closed")]
    Closed(String),
    #[error("No tasks spawned")]
    NoTasks(),
    #[error("Failed to receive inputs")]
    RecvClosed(),
}
