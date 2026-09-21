use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use chrono::Duration as Ttl;
use tokio_util::task::TaskTracker;

use super::{
    error::MailError,
    messages::{
        email_change_confirm::EmailChangeConfirm, email_change_notice::EmailChangeNotice,
        invitation::Invitation, password_changed::PasswordChanged, password_reset::PasswordReset,
        verify_email::VerifyEmail,
    },
    transport::{Outbox, OutgoingMail, Transport},
};
use crate::config::{FrontendConfig, SmtpConfig};

const RETRY_DELAYS: [Duration; 2] = [Duration::from_secs(1), Duration::from_secs(4)];

/// Sends application emails.
///
/// Every `send_*` method returns immediately: rendering happens inline, delivery runs in a
/// background task with retries, and failures are logged. A mail outage never fails a request.
#[derive(Clone)]
pub struct MailService {
    transport: Arc<Transport>,
    frontend_url: Arc<str>,
    outbox: Option<Outbox>,
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
            tasks: TaskTracker::new(),
        }
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

    pub fn send_verification(&self, to: &str, raw_token: &str, ttl: Ttl) {
        let link = self.link("/verify-email", raw_token);
        let expires_in = describe(ttl);
        self.dispatch(
            VerifyEmail {
                link: &link,
                expires_in: &expires_in,
            }
            .render(to),
        );
    }

    pub fn send_password_reset(&self, to: &str, raw_token: &str, ttl: Ttl) {
        let link = self.link("/reset-password", raw_token);
        let expires_in = describe(ttl);
        self.dispatch(
            PasswordReset {
                link: &link,
                expires_in: &expires_in,
            }
            .render(to),
        );
    }

    pub fn send_invitation(&self, to: &str, raw_token: &str, ttl: Ttl) {
        let link = self.link("/reset-password", raw_token);
        let expires_in = describe(ttl);
        self.dispatch(
            Invitation {
                link: &link,
                expires_in: &expires_in,
            }
            .render(to),
        );
    }

    pub fn send_password_changed(&self, to: &str) {
        self.dispatch(PasswordChanged.render(to));
    }

    pub fn send_email_change_confirm(&self, new_email: &str, raw_token: &str, ttl: Ttl) {
        let link = self.link("/confirm-email-change", raw_token);
        let expires_in = describe(ttl);
        self.dispatch(
            EmailChangeConfirm {
                link: &link,
                expires_in: &expires_in,
            }
            .render(new_email),
        );
    }

    pub fn send_email_change_notice(&self, old_email: &str, new_email: &str) {
        self.dispatch(EmailChangeNotice { new_email }.render(old_email));
    }

    fn link(&self, path: &str, raw_token: &str) -> String {
        format!("{}{path}?token={raw_token}", self.frontend_url)
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
        self.tasks.spawn(async move {
            deliver(&transport, &mail).await;
        });
    }
}

async fn deliver(transport: &Transport, mail: &OutgoingMail) {
    let domain = mail.to.rsplit('@').next().unwrap_or("?");
    let mut attempt = 0;
    loop {
        match transport.send(mail).await {
            Ok(()) => return,
            Err(error) if error.is_retryable() && attempt < RETRY_DELAYS.len() => {
                tracing::warn!(%error, attempt, recipient_domain = domain, "email delivery failed, retrying");
                tokio::time::sleep(RETRY_DELAYS[attempt]).await;
                attempt += 1;
            }
            Err(error) => {
                // Never log the body: it contains a credential link.
                tracing::error!(%error, recipient_domain = domain, "email delivery failed");
                return;
            }
        }
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
mod tests {
    use super::*;

    #[test]
    fn describes_durations() {
        assert_eq!(describe(Ttl::minutes(30)), "30 minutes");
        assert_eq!(describe(Ttl::minutes(60)), "1 hour");
        assert_eq!(describe(Ttl::hours(24)), "24 hours");
        assert_eq!(describe(Ttl::minutes(1)), "1 minute");
    }

    #[tokio::test]
    async fn renders_link_and_records_mail() {
        let mail = MailService::in_memory("https://app.example.com/");
        mail.send_password_reset("a@b.com", "tok123", Ttl::minutes(30));
        let sent = mail.sent().await;
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].to, "a@b.com");
        assert!(
            sent[0]
                .text
                .contains("https://app.example.com/reset-password?token=tok123")
        );
        assert!(sent[0].html.contains("30 minutes"));
    }

    #[tokio::test]
    async fn renders_every_message() {
        let mail = MailService::in_memory("https://x.test");
        mail.send_verification("a@b.com", "t", Ttl::hours(24));
        mail.send_invitation("a@b.com", "t", Ttl::hours(24));
        mail.send_password_changed("a@b.com");
        mail.send_email_change_confirm("new@b.com", "t", Ttl::minutes(60));
        mail.send_email_change_notice("a@b.com", "new@b.com");
        assert_eq!(mail.sent().await.len(), 5);
    }
}
