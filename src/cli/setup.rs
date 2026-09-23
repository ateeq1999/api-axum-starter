use std::path::Path;

use super::{USAGE, generate_secrets};
use crate::{config::Config, infra};

const ENV_PATH: &str = ".env";

/// `cargo run -- setup [--seed]`: gets a fresh checkout ready to serve. Creates `.env` from
/// `.env.example` if missing, fills in secrets (see `infra::secrets`), runs migrations, and
/// with `--seed` also loads the development seed data. Idempotent: safe to run again on an
/// already-set-up checkout.
///
/// Deliberately out of scope: creating the bootstrap admin row (still happens on first `cargo
/// run`, same as today) and `--fresh`-style wiping (stays exclusive to `seed --fresh`).
pub async fn run(flags: Vec<String>) -> anyhow::Result<()> {
    let seed = match flags.as_slice() {
        [] => false,
        [flag] if flag == "--seed" => true,
        _ => anyhow::bail!("unknown option `{}`\n\n{USAGE}", flags.join(" ")),
    };

    // A just-created `.env` holds .env.example's literal placeholders (e.g. `JWT_SECRET=change-
    // me-...`), which are committed to git, not real secrets — so this is the one case where
    // generation must overwrite rather than respect what's "already set".
    let just_created = !Path::new(ENV_PATH).exists();
    if just_created {
        std::fs::copy(".env.example", ENV_PATH)
            .map_err(|e| anyhow::anyhow!("could not create {ENV_PATH} from .env.example: {e}"))?;
        println!("Created {ENV_PATH} from .env.example.");
    }
    // `.env` may not have existed (or may have just changed) when the process started, so load
    // it now; `_override` makes sure freshly generated values below are what `Config::from_env`
    // sees, not whatever was already in the process environment.
    dotenvy::from_path_override(ENV_PATH).ok();

    let result = infra::secrets::generate(
        Path::new(ENV_PATH),
        Path::new("monitoring/prometheus.yml"),
        just_created,
    )?;
    generate_secrets::report(&result);
    dotenvy::from_path_override(ENV_PATH).ok();

    let config = Config::from_env()?;
    let pool = infra::database::connect(&config.database_url, 1).await?;
    println!("Database ready (migrations applied).");

    if seed {
        let users = infra::seed::run(&pool).await?;
        println!("Seed data loaded ({users} users in the database).");
    }

    if config.bootstrap_admin.is_none() {
        println!(
            "No ADMIN_EMAIL/ADMIN_PASSWORD set: no administrator will be created on first `cargo run`. Set both in {ENV_PATH} if you want one."
        );
    }

    println!("Ready. Run `cargo run` to start the server.");
    Ok(())
}
