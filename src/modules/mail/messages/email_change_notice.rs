use askama::Template;

use crate::modules::mail::{error::MailError, transport::OutgoingMail};

/// Sent to the OLD address so a real owner can react to a hijacked session.
pub struct EmailChangeNotice<'a> {
    pub new_email: &'a str,
}

#[derive(Template)]
#[template(path = "email_change_notice.html")]
struct Html<'a> {
    new_email: &'a str,
}

#[derive(Template)]
#[template(path = "email_change_notice.txt")]
struct Text<'a> {
    new_email: &'a str,
}

impl EmailChangeNotice<'_> {
    pub fn render(&self, to: &str) -> Result<OutgoingMail, MailError> {
        super::assemble(
            to,
            "A change of your email address was requested",
            Text {
                new_email: self.new_email,
            },
            Html {
                new_email: self.new_email,
            },
        )
    }
}
