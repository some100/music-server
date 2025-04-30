use crate::{Msg, ServerError};
use channelmap::{ChannelMap, flume::Receiver};
use futures_util::{
    SinkExt as _, StreamExt as _,
    stream::{SplitSink, SplitStream},
};
use tokio::{
    net::{TcpListener, TcpStream},
    time::{Duration, sleep},
};
use tokio_tungstenite::{
    WebSocketStream, accept_async,
    tungstenite::{self, Message},
};
use tracing::{debug, error, info};

pub async fn ws_loop(channels: ChannelMap<Msg>, websocket_url: String) -> Result<(), ServerError> {
    let ws_listener = TcpListener::bind(&websocket_url).await?;
    info!("WebSockets listening on {websocket_url}");
    loop {
        let client = match accept_connection(&ws_listener).await {
            Ok(client) => client,
            Err(e) => {
                error!("{e}");
                sleep(Duration::from_millis(500)).await;
                continue;
            }
        };
        let channels_clone = channels.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_connection(client, channels_clone).await {
                error!("{e}");
            }
        });
    }
}

async fn accept_connection(listener: &TcpListener) -> Result<TcpStream, ServerError> {
    let (client, addr) = listener.accept().await?;
    info!("Accepted client {addr}");
    Ok(client)
}

async fn handle_connection(
    client: TcpStream,
    channels: ChannelMap<Msg>,
) -> Result<(), ServerError> {
    let client = accept_async(client).await?;
    let (write, mut read) = client.split();

    let username = read_username(&mut read).await?;
    info!("{} connected", &username);
    let rx = channels.add_with_buffer(&username, 10);

    listen_message(read, write, rx, &username).await?;

    Ok(())
}

async fn read_username(
    read: &mut SplitStream<WebSocketStream<TcpStream>>,
) -> Result<String, ServerError> {
    let username = read
        .next()
        .await
        .ok_or(ServerError::WebSocket(tungstenite::Error::ConnectionClosed))??
        .to_text()?
        .to_owned();
    debug!("Got username {username}");
    Ok(username)
}

async fn listen_message(
    mut read: SplitStream<WebSocketStream<TcpStream>>,
    mut write: SplitSink<WebSocketStream<TcpStream>, Message>,
    mut rx: Receiver<Msg>,
    username: &str,
) -> Result<(), ServerError> {
    loop {
        let result = tokio::select! {
            result = check_message(&mut write, &mut rx, username) => result,
            result = async {
                match read.next().await {
                    Some(Err(e)) => Err(ServerError::WebSocket(e)),
                    None => Err(ServerError::Closed(username.to_owned())),
                    _ => Ok(())
                }
            } => result,
        };
        if let Err(e) = result {
            error!("{e}");
            break;
        }
    }
    write.close().await?;
    Ok(())
}

async fn check_message(
    write: &mut SplitSink<WebSocketStream<TcpStream>, Message>,
    rx: &mut Receiver<Msg>,
    username: &str,
) -> Result<(), ServerError> {
    let msg = &serde_json::to_string(&rx.recv_async().await?)?;
    write.send(Message::Text(msg.into())).await?;
    debug!("Got message {msg} for {username}");
    Ok(())
}
