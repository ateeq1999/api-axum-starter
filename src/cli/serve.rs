use std::{net::SocketAddr, time::Duration};

use tokio::{net::TcpListener, signal};

use crate::{app::build_router, config::Config, infra, state::AppState};

pub async fn run() -> anyhow::Result<()> {
    let config = Config::from_env()?;
    let pool =
        infra::database::connect(&config.database_url, config.database_max_connections).await?;
    let bind_addr = config.bind_addr;
    let bootstrap_admin = config.bootstrap_admin.clone();

    let jobs = infra::jobs::spawn(pool.clone());
    let metrics_config = config.metrics.clone();
    let state = AppState::new(pool, config).await?;
    if let Some(admin) = bootstrap_admin {
        state
            .users
            .ensure_bootstrap_admin(&admin.email, admin.password)
            .await?;
    }
    let mail = state.mail.clone();

    infra::metrics::spawn_gauge_sampler(state.db.clone(), state.rate_limiter.clone());
    let (_layer, metrics_handle) = infra::metrics::layer_and_handle();
    tokio::spawn(infra::metrics::serve(
        metrics_config.bind_addr,
        metrics_handle,
        metrics_config.token,
    ));

    let app = build_router(state);

    let listener = TcpListener::bind(bind_addr).await?;
    tracing::info!(%bind_addr, "listening");
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;

    mail.shutdown(Duration::from_secs(5)).await;
    jobs.shutdown(Duration::from_secs(5)).await;
    tracing::info!("shutdown complete");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("shutdown signal received");
}
