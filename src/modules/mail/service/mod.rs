use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use chrono::Duration as Ttl;
use sqlx::PgPool;
use tokio_util::task::TaskTracker;

use super::{
    error::MailError,
    repository::OutboxRepository,
    transport::{Outbox, OutgoingMail, Transport},
};
use crate::config::{FrontendConfig, SmtpConfig};

mod senders;

const RETRY_DELAYS: [Duration; 2] = [Duration::from_secs(1), Duration::from_secs(4)];

/// Sends application emails.
///
/// Every `send_*` method returns immediately: rendering happens inline, delivery runs in a
/// background task with retries, and failures are logged. A mail outage never fails a request.
/// With [`Self::with_durable_outbox`], every email is also persisted before the attempt, so a
/// crash between rendering and sending is recovered (resent) at the next startup instead of the
/// email being silently lost.
#[derive(Clone)]
pub struct MailService {
    transport: Arc<Transport>,
    frontend_url: Arc<str>,
    outbox: Option<Outbox>,
    /// `Some` only for the real, SMTP-backed service (see `with_durable_outbox`); the in-memory
    /// test transport and the plain `Log` dev transport have nothing worth persisting.
    durable: Option<OutboxRepository>,
    tasks: TaskTracker,
}

impl MailService {
    pub fn new(smtp: &SmtpConfig, frontend: &FrontendConfig) -> Result<Self, MailError> {
        let transport = if smtp.enabled {
            Transport::smtp(smtp)?
        } else {
            tracing::warn!("MAIL_ENABLED=false: emails are logged, not sent");
            Transport::Log
        };
        Ok(Self::with_transport(transport, &frontend.url, None))
    }

    /// Service that records messages in memory. Used by tests.
    pub fn in_memory(frontend_url: &str) -> Self {
        let outbox: Outbox = Arc::new(Mutex::new(Vec::new()));
        Self::with_transport(
            Transport::Memory(outbox.clone()),
            frontend_url,
            Some(outbox),
        )
    }

    fn with_transport(transport: Transport, frontend_url: &str, outbox: Option<Outbox>) -> Self {
        Self {
            transport: Arc::new(transport),
            frontend_url: frontend_url.trim_end_matches('/').into(),
            outbox,
            durable: None,
            tasks: TaskTracker::new(),
        }
    }

    /// Persists every outgoing email before attempting delivery and, right away, resends
    /// whatever was left `pending` by a previous crash. Called once by `AppState::new` for the
    /// real service; never for `in_memory`, which has no crash to recover from.
    pub fn with_durable_outbox(mut self, db: PgPool) -> Self {
        let repo = OutboxRepository::new(db);
        self.recover_pending(repo.clone());
        self.durable = Some(repo);
        self
    }

    fn recover_pending(&self, repo: OutboxRepository) {
        let transport = self.transport.clone();
        self.tasks.spawn(async move {
            let pending = match repo.pending().await {
                Ok(pending) => pending,
                Err(error) => {
                    tracing::error!(%error, "could not read pending outbound mail");
                    return;
                }
            };
            if !pending.is_empty() {
                tracing::info!(
                    count = pending.len(),
                    "resending mail left pending by a previous shutdown/crash"
                );
            }
            for (id, mail) in pending {
                finish(&repo, id, deliver(&transport, &mail).await).await;
            }
        });
    }

    /// Messages recorded by an in-memory service (empty for other transports).
    pub async fn sent(&self) -> Vec<OutgoingMail> {
        self.tasks.close();
        self.tasks.wait().await;
        self.tasks.reopen();
        self.outbox
            .as_ref()
            .map(|o| o.lock().unwrap_or_else(|e| e.into_inner()).clone())
            .unwrap_or_default()
    }

    /// Waits (bounded) for in-flight deliveries so shutdown does not drop queued mail.
    pub async fn shutdown(&self, timeout: Duration) {
        self.tasks.close();
        if tokio::time::timeout(timeout, self.tasks.wait())
            .await
            .is_err()
        {
            tracing::warn!("timed out waiting for pending emails to be delivered");
        }
    }

    fn dispatch(&self, rendered: Result<OutgoingMail, MailError>) {
        let mail = match rendered {
            Ok(mail) => mail,
            Err(error) => {
                tracing::error!(?error, "failed to render email");
                return;
            }
        };
        let transport = self.transport.clone();
        let durable = self.durable.clone();
        self.tasks.spawn(async move {
            let Some(repo) = durable else {
                deliver(&transport, &mail).await;
                return;
            };
            let id = match repo.insert(&mail).await {
                Ok(id) => id,
                Err(error) => {
                    // Best effort: still try to send it, just without crash recovery for this one.
                    tracing::error!(%error, "could not persist outgoing mail before sending");
                    deliver(&transport, &mail).await;
                    return;
                }
            };
            finish(&repo, id, deliver(&transport, &mail).await).await;
        });
    }
}

enum DeliveryOutcome {
    Sent,
    GaveUp,
}

async fn deliver(transport: &Transport, mail: &OutgoingMail) -> DeliveryOutcome {
    let domain = mail.to.rsplit('@').next().unwrap_or("?");
    let mut attempt = 0;
    loop {
        match transport.send(mail).await {
            Ok(()) => {
                metrics::counter!("mail_send_total", "outcome" => "success").increment(1);
                return DeliveryOutcome::Sent;
            }
            Err(error) if error.is_retryable() && attempt < RETRY_DELAYS.len() => {
                tracing::warn!(%error, attempt, recipient_domain = domain, "email delivery failed, retrying");
                tokio::time::sleep(RETRY_DELAYS[attempt]).await;
                attempt += 1;
            }
            Err(error) => {
                // Never log the body: it contains a credential link.
                tracing::error!(%error, recipient_domain = domain, "email delivery failed");
                metrics::counter!("mail_send_total", "outcome" => "failure").increment(1);
                return DeliveryOutcome::GaveUp;
            }
        }
    }
}

/// Clears the durability record for a finished delivery: gone if it was sent, kept (marked
/// `dead`, for investigation) if delivery was permanently given up on.
async fn finish(repo: &OutboxRepository, id: uuid::Uuid, outcome: DeliveryOutcome) {
    let result = match outcome {
        DeliveryOutcome::Sent => repo.remove(id).await,
        DeliveryOutcome::GaveUp => repo.mark_dead(id).await,
    };
    if let Err(error) = result {
        tracing::error!(%error, mail_id = %id, "could not update the outbound mail record");
    }
}

fn describe(ttl: Ttl) -> String {
    let minutes = ttl.num_minutes();
    if minutes >= 60 && minutes % 60 == 0 {
        let hours = minutes / 60;
        format!("{hours} hour{}", if hours == 1 { "" } else { "s" })
    } else {
        format!("{minutes} minute{}", if minutes == 1 { "" } else { "s" })
    }
}

#[cfg(test)]
mod tests;
