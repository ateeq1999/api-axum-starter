//! One struct per email. Each renders a text part and an HTML part from `../templates`.

use askama::Template;

use super::{error::MailError, transport::OutgoingMail};

pub mod email_change_confirm;
pub mod email_change_notice;
pub mod invitation;
pub mod password_changed;
pub mod password_reset;
pub mod verify_email;

fn assemble(
    to: &str,
    subject: &str,
    text: impl Template,
    html: impl Template,
) -> Result<OutgoingMail, MailError> {
    Ok(OutgoingMail {
        to: to.to_string(),
        subject: subject.to_string(),
        text: text.render()?,
        html: html.render()?,
    })
}

/// Declares an email whose only variables are a link and how long it stays valid.
macro_rules! link_message {
    ($name:ident, $subject:literal, $html:literal, $txt:literal) => {
        pub struct $name<'a> {
            pub link: &'a str,
            pub expires_in: &'a str,
        }

        #[derive(askama::Template)]
        #[template(path = $html)]
        struct Html<'a> {
            link: &'a str,
            expires_in: &'a str,
        }

        #[derive(askama::Template)]
        #[template(path = $txt)]
        struct Text<'a> {
            link: &'a str,
            expires_in: &'a str,
        }

        impl $name<'_> {
            pub fn render(
                &self,
                to: &str,
            ) -> Result<
                crate::modules::mail::transport::OutgoingMail,
                crate::modules::mail::error::MailError,
            > {
                super::assemble(
                    to,
                    $subject,
                    Text {
                        link: self.link,
                        expires_in: self.expires_in,
                    },
                    Html {
                        link: self.link,
                        expires_in: self.expires_in,
                    },
                )
            }
        }
    };
}
pub(crate) use link_message;
