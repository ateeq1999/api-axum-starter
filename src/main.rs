use std::{net::SocketAddr, time::Duration};

use api_starter_axum::{app::build_router, config::Config, infra, state::AppState};
use tokio::{net::TcpListener, signal};

const USAGE: &str = "usage: api-starter-axum [seed [--fresh [--yes]]]

  (no command)          run the API server
  seed                  add the development seed data (seeds/seed.sql), then exit
  seed --fresh          DELETE ALL DATA first (seeds/reset.sql), then seed; asks to confirm
  seed --fresh --yes    same, without asking";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    infra::telemetry::init();

    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        None => serve().await,
        Some("seed" | "--seed") => seed(args.collect()).await,
        Some(other) => anyhow::bail!("unknown command `{other}`\n\n{USAGE}"),
    }
}

/// `cargo run -- seed [--fresh [--yes]]`: runs the migrations, loads the seed data and exits.
async fn seed(flags: Vec<String>) -> anyhow::Result<()> {
    let (mut fresh, mut yes) = (false, false);
    for flag in &flags {
        match flag.as_str() {
            "--fresh" => fresh = true,
            "--yes" | "-y" => yes = true,
            other => anyhow::bail!("unknown option `{other}`\n\n{USAGE}"),
        }
    }
    if yes && !fresh {
        anyhow::bail!("--yes only applies to --fresh\n\n{USAGE}");
    }

    let config = Config::from_env()?;
    if fresh && !yes {
        confirm_wipe(&config.database_url)?;
    }
    let pool = infra::database::connect(&config.database_url, 1).await?;
    let users = if fresh {
        infra::seed::fresh(&pool).await?
    } else {
        infra::seed::run(&pool).await?
    };

    if fresh {
        println!("Database wiped and re-seeded ({users} users).");
    } else {
        println!("Seed data loaded ({users} users in the database).");
    }
    println!(
        "Log in as admin@example.com, alice@example.com, bob@example.com ... with password: Password123!"
    );
    Ok(())
}

/// Destructive, so ask first. Without a terminal to ask on, require `--yes` explicitly.
fn confirm_wipe(database_url: &str) -> anyhow::Result<()> {
    use std::io::{IsTerminal, Write};

    if !std::io::stdin().is_terminal() {
        anyhow::bail!("`seed --fresh` deletes all data in {database_url}; pass --yes to confirm");
    }
    print!("This deletes ALL users and tokens in {database_url}. Type `yes` to continue: ");
    std::io::stdout().flush()?;
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer)?;
    if answer.trim() != "yes" {
        anyhow::bail!("aborted, nothing was changed");
    }
    Ok(())
}

async fn serve() -> anyhow::Result<()> {
    let config = Config::from_env()?;
    let pool = infra::database::connect(&config.database_url, 5).await?;
    let bind_addr = config.bind_addr;
    let bootstrap_admin = config.bootstrap_admin.clone();

    let jobs = infra::jobs::spawn(pool.clone());
    let state = AppState::new(pool, config).await?;
    if let Some(admin) = bootstrap_admin {
        state
            .users
            .ensure_bootstrap_admin(&admin.email, admin.password)
            .await?;
    }
    let mail = state.mail.clone();
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
