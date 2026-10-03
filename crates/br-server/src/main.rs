//! Drop-in replacement for the Bun server.
//!
//! Env (see `br_core::config`): `PORT` (default 3000, `0` = OS-picked), `BR_API_KEY`, `LOG_LEVEL`,
//! `SEVEN_ZIP_PATH`, `BR_DATA_DIR`.
//! Prints `BR_SERVER_LISTENING <port>` on stdout once bound.

use br_core::config::Config;
use br_server::state::AppState;
use br_server::wiki::LiveWiki;
use std::net::SocketAddr;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_env()?;
    tracing_subscriber::fmt()
        .with_env_filter(config.log_level.as_str())
        .with_writer(std::io::stderr)
        .init();

    let (port, data_dir, auth) = (config.port, config.data_dir.clone(), config.api_key.is_some());
    let state = AppState::open_with_wiki(config, std::sync::Arc::new(LiveWiki::new(br_wiki::WikiService::default())))?;
    // Like the Bun `LibraryModel` constructor: scan in the background. Library requests wait for
    // it (`await libModel.ready`); the scan is registered before the port opens.
    state.spawn_rescan();

    let listener = tokio::net::TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], port))).await?;
    let bound = listener.local_addr()?.port();
    println!("BR_SERVER_LISTENING {bound}");
    tracing::info!(data_dir = %data_dir.display(), auth, "br-server up");

    axum::serve(listener, br_server::app(state))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
