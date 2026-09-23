//! Drives the real lettre SMTP transport against a tiny in-process SMTP server.

mod common;

use std::time::Duration;

use api_starter_axum::{
    config::{FrontendConfig, SmtpConfig, SmtpTls},
    modules::mail::{MailService, OutgoingMail, repository::OutboxRepository},
};
use chrono::Duration as Ttl;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpListener,
};

/// Accepts one SMTP session and returns everything the client sent in DATA.
async fn fake_smtp_server(listener: TcpListener) -> String {
    let (stream, _) = listener.accept().await.unwrap();
    let (read, mut write) = stream.into_split();
    let mut lines = BufReader::new(read).lines();
    let mut data = String::new();
    let mut in_data = false;

    write.write_all(b"220 fake ESMTP\r\n").await.unwrap();
    while let Some(line) = lines.next_line().await.unwrap() {
        if in_data {
            if line == "." {
                write.write_all(b"250 queued\r\n").await.unwrap();
                // Pooled clients keep the connection open, so stop after the first message.
                break;
            } else {
                data.push_str(&line);
                data.push('\n');
            }
            continue;
        }
        let upper = line.to_ascii_uppercase();
        let reply: &[u8] = if upper.starts_with("EHLO") || upper.starts_with("HELO") {
            b"250 fake\r\n"
        } else if upper.starts_with("DATA") {
            in_data = true;
            b"354 go ahead\r\n"
        } else if upper.starts_with("QUIT") {
            write.write_all(b"221 bye\r\n").await.unwrap();
            break;
        } else {
            b"250 ok\r\n"
        };
        write.write_all(reply).await.unwrap();
    }
    data
}

#[tokio::test]
async fn delivers_over_smtp() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = tokio::spawn(fake_smtp_server(listener));

    let mail = MailService::new(
        &SmtpConfig {
            enabled: true,
            host: "127.0.0.1".into(),
            port,
            tls: SmtpTls::None,
            username: None,
            password: None,
            from: "App <no-reply@example.com>".into(),
        },
        &FrontendConfig {
            url: "https://app.test".into(),
        },
    )
    .unwrap();

    mail.send_password_reset("user@example.com", "tok123", Ttl::minutes(30));
    mail.shutdown(Duration::from_secs(5)).await;

    let data = tokio::time::timeout(Duration::from_secs(5), server)
        .await
        .expect("server finished")
        .unwrap();
    assert!(data.contains("Subject: Reset your password"), "{data}");
    assert!(data.contains("To: user@example.com"), "{data}");
    assert!(data.contains("multipart/alternative"), "{data}");
}

#[tokio::test]
async fn unreachable_server_does_not_panic_or_block_the_caller() {
    // Nothing listens on this port.
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    let mail = MailService::new(
        &SmtpConfig {
            enabled: true,
            host: "127.0.0.1".into(),
            port,
            tls: SmtpTls::None,
            username: None,
            password: None,
            from: "no-reply@example.com".into(),
        },
        &FrontendConfig {
            url: "https://app.test".into(),
        },
    )
    .unwrap();

    let started = std::time::Instant::now();
    mail.send_password_changed("user@example.com");
    assert!(
        started.elapsed() < Duration::from_millis(500),
        "send_* must return immediately"
    );
    // Don't wait for the retries; just make sure shutdown honors its timeout.
    mail.shutdown(Duration::from_millis(200)).await;
}

/// Simulates a crash between rendering an email and sending it: the row is inserted directly
/// (bypassing `dispatch`, exactly as if the process died right after `insert`), and a fresh
/// `MailService` — standing in for the next startup — must resend it without being asked to.
#[tokio::test]
async fn a_crash_before_delivery_is_recovered_on_next_startup() {
    let pool = common::fresh_pool().await;
    let outbox = OutboxRepository::new(pool.clone());
    outbox
        .insert(&OutgoingMail {
            to: "user@example.com".into(),
            subject: "Reset your password".into(),
            text: "reset link".into(),
            html: "<p>reset link</p>".into(),
        })
        .await
        .unwrap();

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = tokio::spawn(fake_smtp_server(listener));

    let mail = MailService::new(
        &SmtpConfig {
            enabled: true,
            host: "127.0.0.1".into(),
            port,
            tls: SmtpTls::None,
            username: None,
            password: None,
            from: "App <no-reply@example.com>".into(),
        },
        &FrontendConfig {
            url: "https://app.test".into(),
        },
    )
    .unwrap()
    .with_durable_outbox(pool.clone());

    let data = tokio::time::timeout(Duration::from_secs(5), server)
        .await
        .expect("server finished")
        .unwrap();
    assert!(data.contains("Subject: Reset your password"), "{data}");

    mail.shutdown(Duration::from_secs(5)).await;

    let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM outbound_mail")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        remaining, 0,
        "the recovered email should be removed once delivered"
    );
}
