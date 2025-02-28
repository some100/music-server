use std::net::SocketAddr;
use tokio::{
    net::{TcpStream, TcpListener},
    sync::broadcast,
    task::{self, JoinSet},
};
use tokio_tungstenite::{
    accept_async,
    WebSocketStream,
    tungstenite::{self, Message}
};
use futures_util::{
    StreamExt,
    SinkExt,
    stream::{SplitStream, SplitSink}
};
use clap::Parser;
use warp::Filter;
use serde::{Deserialize, Serialize};
use serde_json as json;
use thiserror::Error;
use log::{debug, info, warn, error};
 
#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Args {
    /// URL for WebSocket server to listen on
    #[arg(long, default_value = "0.0.0.0:8081")]
    websocket_url: String,
    /// URL for HTTP POST listener to listen on
    #[arg(long, default_value = "0.0.0.0:8082")]
    http_url: String,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
struct Msg {
    #[serde(skip_serializing)] // username is useless for passing onto client
    username: String,
    #[serde(alias = "msgType")] // fe2io compat
    msg_type: String,
    #[serde(alias = "audioUrl")]
    audio_url: Option<String>,
    #[serde(alias = "statusType")]
    status_type: Option<String>,
}
 
 
#[derive(Error, Debug)]
pub (crate) enum ServerError {
    #[error("WebSocket Error: {0}")]
    WebSocket(#[from] tungstenite::Error),
    #[error("Send Error: {0}")]
    Send(#[from] broadcast::error::SendError<Msg>),
    #[error("IO Error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Address Parse Error: {0}")]
    AddrParse(#[from] std::net::AddrParseError),
    #[error("HTTP Error: {0}")]
    Http(#[from] warp::Error),
    #[error("JSON Error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Join Error: {0}")]
    Join(#[from] task::JoinError),
    #[error("Generic Error: {0}")]
    Generic(String),
}

#[tokio::main]
async fn main() -> Result<(), ServerError> {
    let mut tasks = JoinSet::new();

    let env = env_logger::Env::default().filter_or("LOG_LEVEL", "info");
    env_logger::init_from_env(env);

    let args = Args::parse();

    let (tx, _) = broadcast::channel(64);
    let tx_clone = tx.clone();
    tasks.spawn(ws_loop(tx_clone, args.websocket_url));
    tasks.spawn(http_listener(tx, args.http_url));
    wait_for_tasks(tasks).await?;
    Ok(())
}
 
async fn accept_connection(listener: &TcpListener) -> Result<TcpStream, ServerError> {
    let (client, addr) = listener.accept().await?;
    info!("Accepted client {}", addr);
    Ok(client)
}
 
 
async fn ws_loop(tx: broadcast::Sender<Msg>, websocket_url: String) -> Result<(), ServerError> {
    let ws_listener = match TcpListener::bind(&websocket_url).await {
        Ok(ws_listener) => ws_listener,
        Err(e) => {
            error!("{}", e);
            return Err(ServerError::Io(e));
        },
    };
    info!("WebSockets listening on {}", websocket_url);
    loop {
        match accept_connection(&ws_listener).await {
            Ok(client) => {
                tokio::spawn(handle_connection(client, tx.subscribe()));
            },
            Err(e) => error!("{}", e),
        }
    }
}
 
 
async fn http_listener(tx: broadcast::Sender<Msg>, http_url: String) -> Result<(), ServerError> {
    let post = warp::post()
        .and(warp::body::json())
        .and_then(
            move |message: Msg| {
                let tx = tx.clone();
                async move {
                    let _ = transmit_json(&tx, message).await;
                    Ok::<_, warp::Rejection>(warp::reply())
                }
        });
    let server = warp::serve(post);
    let socket: SocketAddr = http_url.parse()?;
    match server.try_bind_ephemeral(socket) {
        Ok((_, server)) => {
            info!("HTTP listening on {}", socket);
            server.await;
        },
        Err(e) => {
            error!("{}", e);
            return Err(ServerError::Http(e));
        },
    }
    Ok(())
}
 
async fn transmit_json(tx: &broadcast::Sender<Msg>, msg: Msg) -> Result<(), ServerError> {
    match tx.send(msg) {
        Err(e) => {
            error!("{} (this is probably because a message was POSTed but no one is connected)", e);
            return Err(ServerError::Send(e));
        },
        _ => (),
    }
    Ok(())
}
 
 
async fn handle_connection(client: TcpStream, rx: broadcast::Receiver<Msg>) -> Result<(), ServerError> {
    let client = accept_async(client).await?;
    let (mut write, mut read) = client.split();

    let username = read_username(&mut read).await?;
    info!("{} connected", &username);
   
    tokio::spawn(async move {
        let _ = listen_message(&mut write, rx, &username).await;
    });
 
    Ok(())
}
 
 
async fn read_username(read: &mut SplitStream<WebSocketStream<TcpStream>>) -> Result<String, ServerError> {
    let username = match read.next().await {
        Some(response) => response?.to_string(),
        None => return Err(ServerError::WebSocket(tungstenite::Error::ConnectionClosed)),
    };
    debug!("Got username {}", username);
    Ok(username)
}
 
 
async fn listen_message(write: &mut SplitSink<WebSocketStream<TcpStream>, Message>, mut rx: broadcast::Receiver<Msg>, username: &str) -> Result<(), ServerError> {
    loop {
        match rx.recv().await {
            Ok(msg) => {
                if msg.username.to_lowercase() == username.to_lowercase() {
                    let msg = &json::to_string(&msg)?;
                    match write.send(Message::Text(msg.into())).await {
                        Err(e) => {
                            error!("{}", e);
                            return Err(ServerError::WebSocket(e));
                        },
                        _ => {
                            debug!("Got message {} for {}", msg, username);
                        },
                    }
                }
            },
            Err(_) => {
                warn!("Sender disconnected");
                break;
            },
        }
    }
    Ok(())
}

async fn wait_for_tasks(mut tasks: JoinSet<Result<(), ServerError>>) -> Result<(), ServerError> {
    match tasks.join_next().await {
        Some(Err(e)) => {
            error!("{}", e);
            return Err(ServerError::Join(e));
        },
        None => {
            error!("Somehow, no tasks were spawned");
            return Err(ServerError::Generic("No tasks spawned".to_owned()));
        },
        _ => warn!("At least one task exited, ending program"),
    }
    Ok(())
}