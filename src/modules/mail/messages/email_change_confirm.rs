use askama::Template;

use crate::modules::mail::{error::MailError, transport::OutgoingMail};

/// Sent to the NEW address to confirm the change. The email shows that address back to the
/// reader ("change your account email to ..."), and since this message is always addressed to
/// it, `render` takes it from the recipient rather than making every caller pass it twice.
pub struct EmailChangeConfirm<'a> {
    pub link: &'a str,
    pub expires_in: &'a str,
}

#[derive(Template)]
#[template(path = "email_change_confirm.html")]
struct Html<'a> {
    link: &'a str,
    expires_in: &'a str,
    new_email: &'a str,
}

#[derive(Template)]
#[template(path = "email_change_confirm.txt")]
struct Text<'a> {
    link: &'a str,
    expires_in: &'a str,
    new_email: &'a str,
}

impl EmailChangeConfirm<'_> {
    pub fn render(&self, to: &str) -> Result<OutgoingMail, MailError> {
        super::assemble(
            to,
            "Confirm your new email address",
            Text {
                link: self.link,
                expires_in: self.expires_in,
                new_email: to,
            },
            Html {
                link: self.link,
                expires_in: self.expires_in,
                new_email: to,
            },
        )
    }
}
