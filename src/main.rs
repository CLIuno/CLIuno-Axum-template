use std::net::SocketAddr;
use std::sync::Arc;

use cliuno_axum_template::{app, config::Config, db, state::AppState};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // A missing .env is fine: every setting has a development default.
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("cliuno_axum_template=info,tower_http=info")),
        )
        .with_ansi(std::io::IsTerminal::is_terminal(&std::io::stdout()))
        .init();

    let config = Config::from_env();
    let pool = db::connect(&config.database_url).await?;
    // Schema comes from ./migrations (embedded at compile time), so an empty
    // database file is all a fresh clone needs.
    db::migrate(&pool).await?;

    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    let state = AppState::new(pool, Arc::new(config));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("CLIuno Axum API listening on http://{addr}/api/v1");

    axum::serve(listener, app::router(state))
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

/// Resolves on Ctrl+C or SIGTERM so in-flight requests finish before exit.
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c().await.ok();
    };
    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut signal) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            signal.recv().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
}
