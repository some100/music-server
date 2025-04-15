use crate::{Msg, ServerError};
use log::{error, info};
use std::net::SocketAddr;
use tokio::sync::broadcast::Sender;
use warp::Filter;

pub async fn http_listener(tx: Sender<Msg>, http_url: String) -> Result<(), ServerError> {
    let post = warp::post()
        .and(warp::body::json())
        .and_then(move |msg: Msg| {
            let tx = tx.clone();
            async move {
                if let Err(e) = tx.send(msg) {
                    error!("{e}");
                    return Err(warp::reject::reject());
                }
                Ok::<_, warp::Rejection>(warp::reply())
            }
        });
    let server = warp::serve(post);
    let socket: SocketAddr = http_url.parse()?;
    let (socket, server) = server.try_bind_ephemeral(socket)?;
    info!("HTTP listening on {socket}");
    server.await;
    Ok(())
}
