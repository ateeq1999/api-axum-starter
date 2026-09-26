mod common;

use axum::http::{Method, StatusCode};
use common::spawn;
use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
use serde_json::json;

fn png(width: u32, height: u32) -> Vec<u8> {
    let image = DynamicImage::ImageRgba8(RgbaImage::from_pixel(
        width,
        height,
        Rgba([20, 120, 220, 255]),
    ));
    let mut bytes = Vec::new();
    image
        .write_to(&mut std::io::Cursor::new(&mut bytes), ImageFormat::Png)
        .unwrap();
    bytes
}

async fn put_avatar(app: &common::TestApp, token: &str, body: Vec<u8>) -> common::RawResponse {
    let auth = format!("Bearer {token}");
    app.raw(
        Method::PUT,
        "/api/v1/users/me/avatar",
        &[
            ("authorization", &auth),
            ("content-type", "application/octet-stream"),
        ],
        body,
    )
    .await
}

#[tokio::test]
async fn upload_serve_replace_and_remove() {
    let app = spawn().await;
    let (_, token) = app.user("photo@example.com").await;

    let (_, me) = app.get("/api/v1/users/me", Some(&token)).await;
    assert!(me["avatar_url"].is_null());

    // upload
    let response = put_avatar(&app, &token, png(640, 320)).await;
    assert_eq!(response.status, StatusCode::OK, "{}", response.json());
    let first_url = response.json()["avatar_url"].as_str().unwrap().to_string();
    assert!(first_url.starts_with("/api/v1/avatars/") && first_url.ends_with(".jpg"));

    // served publicly (no Authorization), as a 256x256 JPEG, cacheable forever
    let served = app.raw(Method::GET, &first_url, &[], vec![]).await;
    assert_eq!(served.status, StatusCode::OK);
    assert_eq!(served.headers["content-type"], "image/jpeg");
    assert!(
        served.headers["cache-control"]
            .to_str()
            .unwrap()
            .contains("immutable")
    );
    assert_eq!(served.headers["x-content-type-options"], "nosniff");
    let decoded = image::load_from_memory_with_format(&served.body, ImageFormat::Jpeg).unwrap();
    assert_eq!((decoded.width(), decoded.height()), (256, 256));

    // visible wherever the user is shown
    let (_, me) = app.get("/api/v1/users/me", Some(&token)).await;
    assert_eq!(me["avatar_url"], first_url.as_str());

    // replacing gives a new URL and removes the old file
    let second = put_avatar(&app, &token, png(100, 100)).await.json();
    let second_url = second["avatar_url"].as_str().unwrap().to_string();
    assert_ne!(first_url, second_url);
    assert_eq!(
        app.raw(Method::GET, &first_url, &[], vec![]).await.status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        app.raw(Method::GET, &second_url, &[], vec![]).await.status,
        StatusCode::OK
    );

    // removing
    let (status, body) = app.delete("/api/v1/users/me/avatar", Some(&token)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body["avatar_url"].is_null());
    assert_eq!(
        app.raw(Method::GET, &second_url, &[], vec![]).await.status,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn rejects_bad_uploads() {
    let app = spawn().await;
    let (_, token) = app.user("bad@example.com").await;

    let not_an_image = put_avatar(&app, &token, b"<svg onload=alert(1)>".to_vec()).await;
    assert_eq!(not_an_image.status, StatusCode::BAD_REQUEST);
    assert_eq!(not_an_image.json()["error"]["code"], "bad_request");
    assert_eq!(
        put_avatar(&app, &token, vec![]).await.status,
        StatusCode::BAD_REQUEST
    );

    // a valid PNG header cut short
    let truncated = png(64, 64)[..40].to_vec();
    assert_eq!(
        put_avatar(&app, &token, truncated).await.status,
        StatusCode::BAD_REQUEST
    );

    // over the 2 MiB limit, whatever the content
    let too_big = put_avatar(&app, &token, vec![0u8; 2 * 1024 * 1024 + 1]).await;
    assert_eq!(too_big.status, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(too_big.json()["error"]["code"], "payload_too_large");

    // nothing was stored
    let (_, me) = app.get("/api/v1/users/me", Some(&token)).await;
    assert!(me["avatar_url"].is_null());
}

#[tokio::test]
async fn avatar_endpoints_are_protected_and_names_are_strict() {
    let app = spawn().await;
    let (_, token) = app.user("guard@example.com").await;

    let anonymous = app
        .raw(Method::PUT, "/api/v1/users/me/avatar", &[], png(10, 10))
        .await;
    assert_eq!(anonymous.status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        app.delete("/api/v1/users/me/avatar", None).await.0,
        StatusCode::UNAUTHORIZED
    );

    // read-only API keys cannot change the photo, write keys can
    let read = app
        .post(
            "/api/v1/api-keys",
            Some(&token),
            json!({"name":"r","scope":"read"}),
        )
        .await
        .1;
    let write = app
        .post(
            "/api/v1/api-keys",
            Some(&token),
            json!({"name":"w","scope":"write"}),
        )
        .await
        .1;
    assert_eq!(
        put_avatar(&app, read["key"].as_str().unwrap(), png(10, 10))
            .await
            .status,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        put_avatar(&app, write["key"].as_str().unwrap(), png(10, 10))
            .await
            .status,
        StatusCode::OK
    );

    // only generated file names resolve: no path tricks, no other extensions
    for bad in [
        "/api/v1/avatars/..%2F..%2FCargo.toml",
        "/api/v1/avatars/secret.jpg",
        "/api/v1/avatars/0123456789abcdef0123456789abcdef.png",
        "/api/v1/avatars/0123456789abcdef0123456789abcdef.jpg",
    ] {
        assert_eq!(
            app.raw(Method::GET, bad, &[], vec![]).await.status,
            StatusCode::NOT_FOUND,
            "{bad}"
        );
    }
}

/// Runs the same upload/serve/replace/remove flow with S3 as the backing store. Needs a
/// reachable S3 (or emulator such as floci) and is skipped unless `TEST_S3_BUCKET` is set:
///
/// ```text
/// AWS_ENDPOINT_URL=http://localhost:4566 AWS_DEFAULT_REGION=us-east-1 \
/// AWS_ACCESS_KEY_ID=test AWS_SECRET_ACCESS_KEY=test TEST_S3_BUCKET=my-bucket cargo test s3
/// ```
#[tokio::test]
async fn upload_serve_replace_and_remove_with_s3_storage() {
    let Ok(bucket) = std::env::var("TEST_S3_BUCKET") else {
        eprintln!("skipped: set TEST_S3_BUCKET (and the AWS_* variables) to run the S3 test");
        return;
    };
    let mut config = common::test_config();
    config.storage.s3 = Some(api_starter_axum::config::S3StorageConfig {
        bucket,
        force_path_style: std::env::var("AWS_ENDPOINT_URL").is_ok(),
        endpoint_url: std::env::var("AWS_ENDPOINT_URL").ok(),
    });
    let app = common::spawn_with(config).await;
    let (_, token) = app.user("s3photo@example.com").await;

    let first = put_avatar(&app, &token, png(640, 320)).await;
    assert_eq!(first.status, StatusCode::OK, "{}", first.json());
    let first_url = first.json()["avatar_url"].as_str().unwrap().to_string();

    let served = app.raw(Method::GET, &first_url, &[], vec![]).await;
    assert_eq!(served.status, StatusCode::OK);
    assert_eq!(served.headers["content-type"], "image/jpeg");
    let decoded = image::load_from_memory_with_format(&served.body, ImageFormat::Jpeg).unwrap();
    assert_eq!((decoded.width(), decoded.height()), (256, 256));

    let second = put_avatar(&app, &token, png(100, 100)).await.json();
    let second_url = second["avatar_url"].as_str().unwrap().to_string();
    assert_ne!(first_url, second_url);
    assert_eq!(
        app.raw(Method::GET, &first_url, &[], vec![]).await.status,
        StatusCode::NOT_FOUND,
        "the replaced object must be deleted from S3"
    );
    assert_eq!(
        app.raw(Method::GET, &second_url, &[], vec![]).await.status,
        StatusCode::OK
    );

    let (status, _) = app.delete("/api/v1/users/me/avatar", Some(&token)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        app.raw(Method::GET, &second_url, &[], vec![]).await.status,
        StatusCode::NOT_FOUND
    );
}
