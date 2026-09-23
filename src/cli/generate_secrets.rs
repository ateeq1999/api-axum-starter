use std::path::Path;

use super::USAGE;
use crate::infra::secrets::{self, PrometheusSync};

/// `cargo run -- gen --secrets [--force]`: fills in random values for `JWT_SECRET`,
/// `METRICS_TOKEN` and `ADMIN_PASSWORD` in `.env`. Leaves anything already set untouched unless
/// `--force` is given, so it is safe to re-run.
pub fn run(flags: Vec<String>) -> anyhow::Result<()> {
    let (mut secrets_flag, mut force) = (false, false);
    for flag in &flags {
        match flag.as_str() {
            "--secrets" => secrets_flag = true,
            "--force" => force = true,
            other => anyhow::bail!("unknown option `{other}`\n\n{USAGE}"),
        }
    }
    if !secrets_flag {
        anyhow::bail!("usage: api-starter-axum gen --secrets [--force]\n\n{USAGE}");
    }

    let result = secrets::generate(
        Path::new(".env"),
        Path::new("monitoring/prometheus.yml"),
        force,
    )?;
    report(&result);
    Ok(())
}

pub(super) fn report(result: &secrets::GeneratedSecrets) {
    if result.admin_email_missing {
        println!(
            "ADMIN_EMAIL is not set, so ADMIN_PASSWORD was left alone (they must be set together)."
        );
    }
    match result.prometheus_sync {
        Some(PrometheusSync::Updated) => {
            println!("Updated the bearer token in monitoring/prometheus.yml to match.");
        }
        Some(PrometheusSync::NoCredentialsLine) => println!(
            "Note: monitoring/prometheus.yml has no `credentials:` line to update; add one under `authorization:` if /metrics scraping needs it."
        ),
        Some(PrometheusSync::NotFound) | None => {}
    }
    if result.changed.is_empty() {
        println!("Nothing to do: every secret already has a value (pass --force to regenerate).");
    } else {
        println!("Generated new value(s) for: {}", result.changed.join(", "));
    }
}
