use crate::{Msg, ServerError};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::broadcast::{Sender, Receiver},
    time::{sleep, Duration},
};
use tokio_tungstenite::{
    accept_async,
    WebSocketStream,
    tungstenite::{self, Message},
};
use futures_util::{
    StreamExt,
    SinkExt,
    stream::{SplitStream, SplitSink},
};
use log::{debug, info, warn, error};

pub async fn ws_loop(tx: Sender<Msg>, websocket_url: String) -> Result<(), ServerError> {
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

async fn accept_connection(listener: &TcpListener) -> Result<TcpStream, ServerError> {
    let (client, addr) = listener.accept().await?;
    info!("Accepted client {}", addr);
    Ok(client)
}
 
async fn handle_connection(client: TcpStream, rx: Receiver<Msg>) -> Result<(), ServerError> {
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
 
 
async fn listen_message(mut read: SplitStream<WebSocketStream<TcpStream>>, mut write: SplitSink<WebSocketStream<TcpStream>, Message>, mut rx: Receiver<Msg>, username: &str) -> Result<(), ServerError> {
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

async fn check_message(write: &mut SplitSink<WebSocketStream<TcpStream>, Message>, rx: &mut Receiver<Msg>, username: &str) -> Result<(), ServerError> {
    if let Ok(msg) = rx.recv().await {
        if msg.username == username { // checks if username in message is same as client's username
            let msg = &serde_json::to_string(&msg)?;
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