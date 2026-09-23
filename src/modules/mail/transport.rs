use std::sync::{Arc, Mutex};

use lettre::{
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
    message::{Mailbox, MultiPart},
    transport::smtp::authentication::Credentials,
};
use serde::{Deserialize, Serialize};

use super::error::MailError;
use crate::config::{SmtpConfig, SmtpTls};

/// A fully rendered email, before it is encoded for the wire. `Serialize`/`Deserialize` so it can
/// be persisted in `outbound_mail` (see `modules::mail::repository`) until delivery succeeds.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutgoingMail {
    pub to: String,
    pub subject: String,
    pub text: String,
    pub html: String,
}

pub type Outbox = Arc<Mutex<Vec<OutgoingMail>>>;

pub enum Transport {
    /// Real delivery to the mail server.
    Smtp {
        transport: AsyncSmtpTransport<Tokio1Executor>,
        from: Mailbox,
    },
    /// `MAIL_ENABLED=false`: log instead of sending (local development).
    Log,
    /// Keeps messages in memory so tests can read them back.
    Memory(Outbox),
}

impl Transport {
    pub fn smtp(config: &SmtpConfig) -> Result<Self, MailError> {
        let from: Mailbox = config
            .from
            .parse()
            .map_err(|e| MailError::Config(format!("MAIL_FROM: {e}")))?;

        let mut builder = match config.tls {
            SmtpTls::None => AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&config.host),
            SmtpTls::StartTls => AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&config.host)
                .map_err(|e| MailError::Config(format!("SMTP_HOST: {e}")))?,
            SmtpTls::Tls => AsyncSmtpTransport::<Tokio1Executor>::relay(&config.host)
                .map_err(|e| MailError::Config(format!("SMTP_HOST: {e}")))?,
        }
        .port(config.port);

        if let (Some(user), Some(pass)) = (&config.username, &config.password) {
            builder = builder.credentials(Credentials::new(user.clone(), pass.clone()));
        }

        Ok(Self::Smtp {
            transport: builder.build(),
            from,
        })
    }

    pub async fn send(&self, mail: &OutgoingMail) -> Result<(), MailError> {
        match self {
            Self::Smtp { transport, from } => {
                let message = build_message(from, mail)?;
                transport
                    .send(message)
                    .await
                    .map(|_| ())
                    .map_err(|e| MailError::Delivery {
                        permanent: e.is_permanent(),
                        reason: e.to_string(),
                    })
            }
            Self::Log => {
                tracing::info!(to = %mail.to, subject = %mail.subject, "mail disabled, not sending");
                // Links are credentials; the body is only ever shown by this dev-only transport.
                tracing::debug!(body = %mail.text, "mail body (dev transport)");
                Ok(())
            }
            Self::Memory(outbox) => {
                outbox
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .push(mail.clone());
                Ok(())
            }
        }
    }
}

fn build_message(from: &Mailbox, mail: &OutgoingMail) -> Result<Message, MailError> {
    let to: Mailbox = mail
        .to
        .parse()
        .map_err(|e| MailError::Build(format!("recipient: {e}")))?;
    Message::builder()
        .from(from.clone())
        .to(to)
        .subject(mail.subject.clone())
        .multipart(MultiPart::alternative_plain_html(
            mail.text.clone(),
            mail.html.clone(),
        ))
        .map_err(|e| MailError::Build(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_multipart_message() {
        let from: Mailbox = "App <no-reply@example.com>".parse().unwrap();
        let mail = OutgoingMail {
            to: "user@example.com".into(),
            subject: "Hello".into(),
            text: "plain".into(),
            html: "<p>html</p>".into(),
        };
        let raw = String::from_utf8(build_message(&from, &mail).unwrap().formatted()).unwrap();
        assert!(raw.contains("multipart/alternative"));
        assert!(raw.contains("Subject: Hello"));
    }

    #[test]
    fn rejects_invalid_recipient() {
        let from: Mailbox = "no-reply@example.com".parse().unwrap();
        let mail = OutgoingMail {
            to: "not an address".into(),
            subject: "x".into(),
            text: String::new(),
            html: String::new(),
        };
        assert!(build_message(&from, &mail).is_err());
    }
}
