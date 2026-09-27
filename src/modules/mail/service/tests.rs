//! Rendering and recording checks for every email.

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

/// Every email must render to a complete, self-contained document: no unresolved template
/// syntax, a title, a hidden preview line, the shared layout, and a non-empty text part.
#[tokio::test]
async fn every_email_renders_a_complete_layout_and_text_part() {
    let mail = MailService::in_memory("https://x.test");
    mail.send_verification("a@b.com", "t1", Ttl::hours(24));
    mail.send_password_reset("a@b.com", "t2", Ttl::minutes(30));
    mail.send_invitation("a@b.com", "t3", Ttl::hours(24));
    mail.send_password_changed("a@b.com");
    mail.send_email_change_confirm("new@b.com", "t4", Ttl::minutes(60));
    mail.send_email_change_notice("a@b.com", "new@b.com");

    let sent = mail.sent().await;
    assert_eq!(sent.len(), 6);
    for message in &sent {
        let context = &message.subject;
        for part in [&message.html, &message.text] {
            assert!(!part.contains("{{"), "unresolved expression in `{context}`");
            assert!(!part.contains("{%"), "unresolved tag in `{context}`");
        }
        assert!(
            message.html.contains("<title>"),
            "no <title> in `{context}`"
        );
        assert!(
            message.html.contains("mso-hide: all"),
            "no hidden preview line in `{context}`"
        );
        assert!(
            message.html.contains("support@starter.example.com"),
            "layout footer missing from `{context}`"
        );
        assert!(!message.text.trim().is_empty(), "empty text in `{context}`");
    }

    // Link emails put the same link in the button, the paste-in fallback and the text part.
    for message in sent.iter().filter(|m| m.text.contains("token=")) {
        let link_start = message.text.find("https://").unwrap();
        let link: String = message.text[link_start..]
            .chars()
            .take_while(|c| !c.is_whitespace())
            .collect();
        assert!(
            message.html.matches(link.as_str()).count() >= 3,
            "`{}` should carry its link in the button href, the visible fallback and its href",
            message.subject
        );
    }

    // The confirm email shows the address being confirmed, escaped, in both parts.
    let confirm = sent
        .iter()
        .find(|m| m.subject == "Confirm your new email address")
        .unwrap();
    assert!(confirm.html.contains("<strong>new@b.com</strong>"));
    assert!(
        confirm
            .text
            .contains("change your account email to new@b.com")
    );
}

#[tokio::test]
async fn recipient_supplied_values_are_html_escaped() {
    let mail = MailService::in_memory("https://x.test");
    mail.send_email_change_notice("a@b.com", "<script>alert(1)</script>@evil.test");
    let sent = mail.sent().await;
    assert!(!sent[0].html.contains("<script>"));
    assert!(sent[0].html.contains("&#60;script&#62;"));
}
