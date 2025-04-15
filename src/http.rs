use crate::{Msg, ServerError};
use axum::{Json, Router, extract::State, http::StatusCode, response::IntoResponse, routing::post};
use log::{error, info};
use tokio::{net::TcpListener, sync::broadcast::Sender};

#[derive(Clone)]
struct AppState {
    tx: Sender<Msg>,
}

pub async fn http_listener(tx: Sender<Msg>, http_url: String) -> Result<(), ServerError> {
    let state = AppState { tx };
    let app = Router::new()
        .route("/", post(post_handler))
        .with_state(state);
    let listener = TcpListener::bind(&http_url).await?;
    info!("HTTP listening on {http_url}");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn post_handler(State(state): State<AppState>, Json(msg): Json<Msg>) -> impl IntoResponse {
    if let Err(e) = state.tx.send(msg) {
        error!("{e}");
        return StatusCode::INTERNAL_SERVER_ERROR;
    };
    StatusCode::ACCEPTED
}
