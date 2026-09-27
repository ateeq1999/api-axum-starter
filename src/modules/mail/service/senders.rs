//! What the app sends: one method per email. Delivery (queueing, retries, the outbox) lives in mod.rs.

use super::{MailService, describe};
use crate::modules::mail::messages::{
    email_change_confirm::EmailChangeConfirm, email_change_notice::EmailChangeNotice,
    invitation::Invitation, password_changed::PasswordChanged, password_reset::PasswordReset,
    verify_email::VerifyEmail,
};
use chrono::Duration as Ttl;

impl MailService {
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
}
