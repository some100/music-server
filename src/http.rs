use crate::{Msg, ServerError};
use axum::{Json, Router, extract::State, http::StatusCode, response::IntoResponse, routing::post};
use tokio::net::TcpListener;
use channelmap::{ChannelMap, Error as ChannelError};
use tracing::{error, info};

#[derive(Clone)]
struct AppState {
    channels: ChannelMap<Msg>,
}

pub async fn http_listener(channels: ChannelMap<Msg>, http_url: String) -> Result<(), ServerError> {
    let state = AppState { channels };
    let app = Router::new()
        .route("/", post(post_handler))
        .with_state(state);
    let listener = TcpListener::bind(&http_url).await?;
    info!("HTTP listening on {http_url}");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn post_handler(State(state): State<AppState>, Json(msg): Json<Msg>) -> impl IntoResponse {
    match state.channels.send(&msg.username, msg.clone()).await {
        Err(ChannelError::Send(e)) => {
            error!("{e}");
            return StatusCode::INTERNAL_SERVER_ERROR
        },
        Err(ChannelError::Nonexistent) => (),
        _ => (),
    }
    StatusCode::ACCEPTED
}
