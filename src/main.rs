use std::{net::SocketAddr, time::Duration};

use api_starter_axum::{
    app::build_router, common::security::secret_token, config::Config, infra, state::AppState,
};
use tokio::{net::TcpListener, signal};

const USAGE: &str = "usage: api-starter-axum [seed [--fresh [--yes]] | generate-secrets [--force]]

  (no command)              run the API server
  seed                      add the development seed data (seeds/seed.sql), then exit
  seed --fresh              DELETE ALL DATA first (seeds/reset.sql), then seed; asks to confirm
  seed --fresh --yes        same, without asking
  generate-secrets          fill in JWT_SECRET, METRICS_TOKEN, ADMIN_PASSWORD in .env if unset
  generate-secrets --force  also overwrite ones that are already set";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        None => {
            infra::telemetry::init();
            serve().await
        }
        Some("seed" | "--seed") => {
            infra::telemetry::init();
            seed(args.collect()).await
        }
        Some("generate-secrets") => generate_secrets(args.collect()),
        Some(other) => anyhow::bail!("unknown command `{other}`\n\n{USAGE}"),
    }
}

/// `cargo run -- generate-secrets [--force]`: fills in random values for `JWT_SECRET`,
/// `METRICS_TOKEN` and `ADMIN_PASSWORD` in `.env`. Leaves anything already set untouched unless
/// `--force` is given, so it is safe to re-run.
fn generate_secrets(flags: Vec<String>) -> anyhow::Result<()> {
    let force = match flags.as_slice() {
        [] => false,
        [flag] if flag == "--force" => true,
        _ => anyhow::bail!("unknown option `{}`\n\n{USAGE}", flags.join(" ")),
    };

    const PATH: &str = ".env";
    let contents = std::fs::read_to_string(PATH)
        .map_err(|_| anyhow::anyhow!("no {PATH} found; run `cp .env.example .env` first"))?;
    let mut lines: Vec<String> = contents.lines().map(str::to_string).collect();

    let mut changed = Vec::new();
    if set_env_var(
        &mut lines,
        "JWT_SECRET",
        &secret_token::generate().raw,
        force,
    ) {
        changed.push("JWT_SECRET");
    }
    let metrics_token = secret_token::generate().raw;
    if set_env_var(&mut lines, "METRICS_TOKEN", &metrics_token, force) {
        changed.push("METRICS_TOKEN");
        sync_prometheus_token(&metrics_token)?;
    }
    // ADMIN_EMAIL and ADMIN_PASSWORD must be set together (see Config::from_env), so a password
    // is never generated for an email that is not there to pair with it.
    if has_non_empty_value(&lines, "ADMIN_EMAIL") {
        if set_env_var(
            &mut lines,
            "ADMIN_PASSWORD",
            &secret_token::generate().raw,
            force,
        ) {
            changed.push("ADMIN_PASSWORD");
        }
    } else {
        println!(
            "ADMIN_EMAIL is not set, so ADMIN_PASSWORD was left alone (they must be set together)."
        );
    }

    if changed.is_empty() {
        println!("Nothing to do: every secret already has a value (pass --force to regenerate).");
    } else {
        std::fs::write(PATH, lines.join("\n") + "\n")?;
        println!("Generated new value(s) for: {}", changed.join(", "));
    }
    Ok(())
}

/// Sets `name=value` in `lines`, replacing an existing line for `name` (commented out or not)
/// unless it already holds a non-empty value and `force` is false. Appends a new line if `name`
/// is not present at all. Returns whether a line was changed.
fn set_env_var(lines: &mut Vec<String>, name: &str, value: &str, force: bool) -> bool {
    let prefix = format!("{name}=");
    let commented_prefix = format!("# {name}=");
    for line in lines.iter_mut() {
        let trimmed = line.trim_start();
        if let Some(existing) = trimmed.strip_prefix(&prefix) {
            if !existing.is_empty() && !force {
                return false;
            }
            *line = format!("{name}={value}");
            return true;
        }
        if trimmed.starts_with(&commented_prefix) {
            *line = format!("{name}={value}");
            return true;
        }
    }
    lines.push(format!("{name}={value}"));
    true
}

/// Keeps `monitoring/prometheus.yml`'s scrape `credentials:` matching whatever `METRICS_TOKEN`
/// was just generated, so a rotated token does not leave local Prometheus scraping stuck on a
/// 401 (see the README's Monitoring section).
fn sync_prometheus_token(token: &str) -> anyhow::Result<()> {
    const PATH: &str = "monitoring/prometheus.yml";
    let Ok(contents) = std::fs::read_to_string(PATH) else {
        return Ok(()); // no local monitoring stack checked out; nothing to sync
    };

    let mut found = false;
    let lines: Vec<String> = contents
        .lines()
        .map(|line| match line.find("credentials:") {
            Some(indent_end) => {
                found = true;
                format!("{}credentials: \"{token}\"", &line[..indent_end])
            }
            None => line.to_string(),
        })
        .collect();

    if found {
        std::fs::write(PATH, lines.join("\n") + "\n")?;
        println!("Updated the bearer token in {PATH} to match.");
    } else {
        println!(
            "Note: {PATH} has no `credentials:` line to update; add one under `authorization:` if /metrics scraping needs it."
        );
    }
    Ok(())
}

fn has_non_empty_value(lines: &[String], name: &str) -> bool {
    let prefix = format!("{name}=");
    lines.iter().any(|line| {
        line.trim_start()
            .strip_prefix(prefix.as_str())
            .is_some_and(|v| !v.is_empty())
    })
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
