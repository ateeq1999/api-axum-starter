use super::USAGE;
use crate::{config::Config, infra};

/// `cargo run -- seed [--fresh [--yes]]`: runs the migrations, loads the seed data and exits.
pub async fn run(flags: Vec<String>) -> anyhow::Result<()> {
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
