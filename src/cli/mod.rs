//! The `api-starter-axum` binary's commands. `main.rs` just calls [`run`]; each subcommand's
//! logic lives in its own file here, with the underlying work (migrations, seeding, secrets)
//! delegated to `infra`, so it stays testable outside of a spawned process.

mod generate_secrets;
mod seed;
mod serve;
mod setup;

use crate::infra;

pub(crate) const USAGE: &str =
    "usage: api-starter-axum [seed [--fresh [--yes]] | setup [--seed] | gen --secrets [--force]]

  (no command)          run the API server
  seed                  add the development seed data (seeds/seed.sql), then exit
  seed --fresh          DELETE ALL DATA first (seeds/reset.sql), then seed; asks to confirm
  seed --fresh --yes    same, without asking
  setup                 create .env if missing, generate secrets, run migrations
  setup --seed          same, then also load development seed data
  gen --secrets         fill in JWT_SECRET, METRICS_TOKEN, ADMIN_PASSWORD in .env if unset
  gen --secrets --force also overwrite ones that are already set";

pub async fn run() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        None => {
            infra::telemetry::init();
            serve::run().await
        }
        Some("seed" | "--seed") => {
            infra::telemetry::init();
            seed::run(args.collect()).await
        }
        Some("setup") => setup::run(args.collect()).await,
        Some("gen") => generate_secrets::run(args.collect()),
        Some(other) => anyhow::bail!("unknown command `{other}`\n\n{USAGE}"),
    }
}
