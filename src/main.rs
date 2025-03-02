use std::net::SocketAddr;
use tokio::{
    net::{TcpStream, TcpListener},
    sync::broadcast,
    task::{self, JoinSet},
    time::{sleep, Duration},
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
 
/// Music server that listens through HTTP POST
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
enum ServerError {
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
    #[error("Connection with {0} closed")]
    Closed(String),
    #[error("No tasks spawned")]
    NoTasks(),
    #[error("Failed to receive inputs")]
    RecvClosed(),
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

    tokio::select! {
        _ = wait_for_tasks(&mut tasks) => (),
        _ = tokio::signal::ctrl_c() => {
            warn!("Received interrupt, exiting");
            tasks.shutdown().await;
        }
    }
    Ok(())
}
 
async fn accept_connection(listener: &TcpListener) -> Result<TcpStream, ServerError> {
    let (client, addr) = listener.accept().await?;
    info!("Accepted client {}", addr);
    Ok(client)
}
 
 
async fn ws_loop(tx: broadcast::Sender<Msg>, websocket_url: String) -> Result<(), ServerError> {
    let ws_listener = TcpListener::bind(&websocket_url).await?;
    info!("WebSockets listening on {}", websocket_url);
    loop {
        match accept_connection(&ws_listener).await {
            Ok(client) => {
                tokio::spawn(handle_connection(client, tx.subscribe()));
            },
            Err(e) => {
                error!("{}", e);
                sleep(Duration::from_millis(500)).await;
            },
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
                    if let Err(e) = transmit_json(&tx, message) {
                        error!("{}", e);
                        return Err(warp::reject::reject());
                    }
                    Ok::<_, warp::Rejection>(warp::reply())
                }
        });
    let server = warp::serve(post);
    let socket: SocketAddr = http_url.parse()?;
    let (socket, server) = server.try_bind_ephemeral(socket)?;
    info!("HTTP listening on {}", socket);
    server.await;
    Ok(())
}
 
fn transmit_json(tx: &broadcast::Sender<Msg>, msg: Msg) -> Result<(), ServerError> {
    if let Err(e) = tx.send(msg) {
        error!("{} (this is probably because a message was POSTed but no one is connected)", e);
        return Err(ServerError::Send(e));
    }
    Ok(())
}
 
 
async fn handle_connection(client: TcpStream, rx: broadcast::Receiver<Msg>) -> Result<(), ServerError> {
    let client = accept_async(client).await?;
    let (write, mut read) = client.split();

    let username = read_username(&mut read).await?;
    info!("{} connected", &username);
   
    listen_message(read, write, rx, &username).await?;
 
    Ok(())
}
 
 
async fn read_username(read: &mut SplitStream<WebSocketStream<TcpStream>>) -> Result<String, ServerError> {
    let username = read.next().await
        .ok_or(ServerError::WebSocket(tungstenite::Error::ConnectionClosed))??
        .to_string();
    debug!("Got username {}", username);
    Ok(username)
}
 
 
async fn listen_message(mut read: SplitStream<WebSocketStream<TcpStream>>, mut write: SplitSink<WebSocketStream<TcpStream>, Message>, mut rx: broadcast::Receiver<Msg>, username: &str) -> Result<(), ServerError> {
    loop {
        tokio::select! {
            result = check_message(&mut write, &mut rx, username) => {
                if let Err(e) = result {
                    error!("{}", e);
                    break;
                }
            },
            result = check_disconnect(&mut read, username) => {
                if let Err(e) = result { 
                    warn!("{}", e);
                    break;
                };
            },
        }
    }
    write.close().await?;
    Ok(())
}

async fn check_message(write: &mut SplitSink<WebSocketStream<TcpStream>, Message>, rx: &mut broadcast::Receiver<Msg>, username: &str) -> Result<(), ServerError> {
    if let Ok(msg) = rx.recv().await {
        if msg.username == username { // checks if username in message is same as client's username
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
    } else {
        warn!("No available senders (likely that HTTP listener exited)");
        return Err(ServerError::RecvClosed());
    }
    Ok(())
}

async fn check_disconnect(read: &mut SplitStream<WebSocketStream<TcpStream>>, username: &str) -> Result<(), ServerError> {
    if (read.next().await).is_none() {
        return Err(ServerError::Closed(username.to_owned()));
    }
    Ok(())
}

async fn wait_for_tasks(tasks: &mut JoinSet<Result<(), ServerError>>) -> Result<(), ServerError> {
    match tasks.join_next().await {
        Some(Err(e)) => {
            error!("{}", e);
            return Err(ServerError::Join(e));
        },
        None => {
            error!("Somehow, no tasks were spawned");
            return Err(ServerError::NoTasks());
        },
        _ => warn!("At least one task exited, ending program"),
    }
    Ok(())
}