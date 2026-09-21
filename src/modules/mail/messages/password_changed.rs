use askama::Template;

use crate::modules::mail::{error::MailError, transport::OutgoingMail};

pub struct PasswordChanged;

#[derive(Template)]
#[template(path = "password_changed.html")]
struct Html;

#[derive(Template)]
#[template(path = "password_changed.txt")]
struct Text;

impl PasswordChanged {
    pub fn render(&self, to: &str) -> Result<OutgoingMail, MailError> {
        super::assemble(to, "Your password was changed", Text, Html)
    }
}
