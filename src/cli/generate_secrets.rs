use std::path::Path;

use super::USAGE;
use crate::infra::secrets::{self, PrometheusSync};

/// `cargo run -- generate-secrets [--force]`: fills in random values for `JWT_SECRET`,
/// `METRICS_TOKEN` and `ADMIN_PASSWORD` in `.env`. Leaves anything already set untouched unless
/// `--force` is given, so it is safe to re-run.
pub fn run(flags: Vec<String>) -> anyhow::Result<()> {
    let force = match flags.as_slice() {
        [] => false,
        [flag] if flag == "--force" => true,
        _ => anyhow::bail!("unknown option `{}`\n\n{USAGE}", flags.join(" ")),
    };

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
