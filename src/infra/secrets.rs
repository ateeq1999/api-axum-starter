//! File-based generation of local secrets (`.env`) and keeping dependent config in sync.
//! Pure logic, no printing: callers (see `cli::generate_secrets`, `cli::setup`) report the
//! result however fits their command.

use std::path::Path;

use crate::common::security::secret_token;

/// What happened when syncing `monitoring/prometheus.yml`'s scrape token, if `METRICS_TOKEN`
/// was regenerated.
pub enum PrometheusSync {
    /// No local monitoring stack checked out; nothing to do.
    NotFound,
    /// The file exists but has no `credentials:` line to update.
    NoCredentialsLine,
    Updated,
}

/// What [`generate`] did, for the caller to report.
pub struct GeneratedSecrets {
    pub changed: Vec<&'static str>,
    /// `ADMIN_PASSWORD` is only ever generated for an `ADMIN_EMAIL` that is already there to
    /// pair with it (see `Config::from_env`, which requires both together).
    pub admin_email_missing: bool,
    pub prometheus_sync: Option<PrometheusSync>,
}

/// Fills in random values for `JWT_SECRET`, `METRICS_TOKEN` and (if `ADMIN_EMAIL` is already
/// set) `ADMIN_PASSWORD` in the `.env` file at `env_path`. Leaves anything already set untouched
/// unless `force` is given, so it is safe to re-run.
pub fn generate(
    env_path: &Path,
    prometheus_path: &Path,
    force: bool,
) -> anyhow::Result<GeneratedSecrets> {
    let contents = std::fs::read_to_string(env_path).map_err(|_| {
        anyhow::anyhow!(
            "no {} found; run `cp .env.example .env` first",
            env_path.display()
        )
    })?;
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
    let mut prometheus_sync = None;
    if set_env_var(&mut lines, "METRICS_TOKEN", &metrics_token, force) {
        changed.push("METRICS_TOKEN");
        prometheus_sync = Some(sync_prometheus_token(prometheus_path, &metrics_token)?);
    }

    let admin_email_missing = !has_non_empty_value(&lines, "ADMIN_EMAIL");
    if !admin_email_missing
        && set_env_var(
            &mut lines,
            "ADMIN_PASSWORD",
            &secret_token::generate().raw,
            force,
        )
    {
        changed.push("ADMIN_PASSWORD");
    }

    if !changed.is_empty() {
        std::fs::write(env_path, lines.join("\n") + "\n")?;
    }
    Ok(GeneratedSecrets {
        changed,
        admin_email_missing,
        prometheus_sync,
    })
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

fn has_non_empty_value(lines: &[String], name: &str) -> bool {
    let prefix = format!("{name}=");
    lines.iter().any(|line| {
        line.trim_start()
            .strip_prefix(prefix.as_str())
            .is_some_and(|v| !v.is_empty())
    })
}

/// Keeps `monitoring/prometheus.yml`'s scrape `credentials:` matching whatever `METRICS_TOKEN`
/// was just generated, so a rotated token does not leave local Prometheus scraping stuck on a
/// 401 (see the README's Monitoring section).
fn sync_prometheus_token(path: &Path, token: &str) -> anyhow::Result<PrometheusSync> {
    let Ok(contents) = std::fs::read_to_string(path) else {
        return Ok(PrometheusSync::NotFound);
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

    if !found {
        return Ok(PrometheusSync::NoCredentialsLine);
    }
    std::fs::write(path, lines.join("\n") + "\n")?;
    Ok(PrometheusSync::Updated)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(suffix: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "api-starter-axum-secrets-test-{}-{suffix}",
            uuid::Uuid::new_v4()
        ))
    }

    #[test]
    fn fills_in_unset_secrets_but_leaves_set_ones_alone() {
        let env = temp_path("env");
        std::fs::write(
            &env,
            "JWT_SECRET=already-set\n# ADMIN_EMAIL=admin@example.com\nMETRICS_TOKEN=existing\n",
        )
        .unwrap();
        let prometheus = temp_path("yml"); // does not exist

        let result = generate(&env, &prometheus, false).unwrap();

        assert!(result.changed.is_empty());
        assert!(result.admin_email_missing);
        assert_eq!(
            std::fs::read_to_string(&env).unwrap(),
            "JWT_SECRET=already-set\n# ADMIN_EMAIL=admin@example.com\nMETRICS_TOKEN=existing\n"
        );
        std::fs::remove_file(&env).unwrap();
    }

    #[test]
    fn generates_admin_password_only_when_email_is_already_set() {
        let env = temp_path("env");
        std::fs::write(&env, "ADMIN_EMAIL=admin@example.com\n").unwrap();
        let prometheus = temp_path("yml");

        let result = generate(&env, &prometheus, false).unwrap();

        assert!(result.changed.contains(&"ADMIN_PASSWORD"));
        assert!(!result.admin_email_missing);
        assert!(
            std::fs::read_to_string(&env)
                .unwrap()
                .contains("ADMIN_PASSWORD=")
        );
        std::fs::remove_file(&env).unwrap();
    }

    #[test]
    fn force_overwrites_a_value_that_is_already_set() {
        let env = temp_path("env");
        std::fs::write(&env, "JWT_SECRET=old-value\n").unwrap();
        let prometheus = temp_path("yml");

        let result = generate(&env, &prometheus, true).unwrap();

        assert!(result.changed.contains(&"JWT_SECRET"));
        assert!(!std::fs::read_to_string(&env).unwrap().contains("old-value"));
        std::fs::remove_file(&env).unwrap();
    }

    #[test]
    fn syncs_the_prometheus_credentials_line_when_the_token_rotates() {
        let env = temp_path("env");
        std::fs::write(&env, "METRICS_TOKEN=old\n").unwrap();
        let prometheus = temp_path("yml");
        std::fs::write(&prometheus, "      credentials: \"old\"\n").unwrap();

        let result = generate(&env, &prometheus, true).unwrap();

        assert!(matches!(
            result.prometheus_sync,
            Some(PrometheusSync::Updated)
        ));
        assert!(
            !std::fs::read_to_string(&prometheus)
                .unwrap()
                .contains("\"old\"")
        );
        std::fs::remove_file(&env).unwrap();
        std::fs::remove_file(&prometheus).unwrap();
    }

    #[test]
    fn missing_env_file_is_a_clear_error() {
        let env = temp_path("env"); // never created
        let prometheus = temp_path("yml");
        let error = match generate(&env, &prometheus, false) {
            Ok(_) => panic!("expected an error"),
            Err(error) => error,
        };
        assert!(error.to_string().contains(".env"));
    }
}
