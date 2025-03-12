mod http;
mod websocket;
mod error;

use crate::error::ServerError;
use tokio::{
    sync::broadcast,
    task::JoinSet,
};
use clap::Parser;
use serde::{Deserialize, Serialize};
use log::{warn, error};
 
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
    #[serde(rename = "msgType", alias = "msg_type")] // fe2io compat
    type_: String,
    #[serde(rename = "audioUrl", alias = "audio_url")]
    audio_url: Option<String>,
    #[serde(rename = "statusType", alias = "status_type")]
    status_type: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), ServerError> {
    let mut tasks = JoinSet::new();

    let env = env_logger::Env::default().filter_or("LOG_LEVEL", "info");
    env_logger::init_from_env(env);

    let args = Args::parse();

    let (tx, _) = broadcast::channel(64);
    let tx_clone = tx.clone();
    tasks.spawn(websocket::ws_loop(tx_clone, args.websocket_url));
    tasks.spawn(http::http_listener(tx, args.http_url));

    tokio::select! {
        _ = wait_for_tasks(&mut tasks) => (),
        _ = tokio::signal::ctrl_c() => {
            warn!("Received interrupt, exiting");
            tasks.shutdown().await;
        }
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