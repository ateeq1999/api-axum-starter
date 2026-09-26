mod common;

use axum::http::{Method, StatusCode};
use common::{RawResponse, TestApp, spawn};
use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};

fn png() -> Vec<u8> {
    let image = DynamicImage::ImageRgba8(RgbaImage::from_pixel(32, 16, Rgba([200, 30, 30, 255])));
    let mut bytes = Vec::new();
    image
        .write_to(&mut std::io::Cursor::new(&mut bytes), ImageFormat::Png)
        .unwrap();
    bytes
}

fn pdf() -> Vec<u8> {
    b"%PDF-1.4\n1 0 obj\n<< /Type /Catalog >>\nendobj\ntrailer\n<< /Root 1 0 R >>\n%%EOF\n".to_vec()
}

async fn upload(app: &TestApp, token: &str, filename: &str, body: Vec<u8>) -> RawResponse {
    let auth = format!("Bearer {token}");
    app.raw(
        Method::POST,
        &format!("/api/v1/media?filename={filename}"),
        &[
            ("authorization", &auth),
            // Deliberately a lie: the server must trust the bytes, not this header.
            ("content-type", "image/png"),
        ],
        body,
    )
    .await
}

async fn fetch(app: &TestApp, token: &str, uri: &str) -> RawResponse {
    let auth = format!("Bearer {token}");
    app.raw(Method::GET, uri, &[("authorization", &auth)], vec![])
        .await
}

#[tokio::test]
async fn upload_list_fetch_and_delete() {
    let app = spawn().await;
    let (_, token) = app.user("media@example.com").await;
    let original = png();

    let created = upload(&app, &token, "holiday%20photo.png", original.clone()).await;
    assert_eq!(created.status, StatusCode::CREATED, "{}", created.json());
    let created = created.json();
    let id = created["id"].as_str().unwrap().to_string();
    assert_eq!(created["content_type"], "image/png");
    assert_eq!(created["size_bytes"], original.len());
    assert_eq!(created["original_filename"], "holiday photo.png");
    assert_eq!(created["url"], format!("/api/v1/media/{id}/content"));

    let (status, listed) = app.get("/api/v1/media", Some(&token)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(listed["total"], 1);
    assert_eq!(listed["items"][0]["id"], id.as_str());

    let (status, metadata) = app.get(&format!("/api/v1/media/{id}"), Some(&token)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(metadata["content_type"], "image/png");

    let content = fetch(&app, &token, &format!("/api/v1/media/{id}/content")).await;
    assert_eq!(content.status, StatusCode::OK);
    assert_eq!(&content.body[..], &original[..]);
    assert_eq!(content.headers["content-type"], "image/png");
    assert_eq!(
        content.headers["content-disposition"],
        "inline; filename=\"holiday_photo.png\""
    );
    assert_eq!(content.headers["x-content-type-options"], "nosniff");
    assert!(
        content.headers["content-security-policy"]
            .to_str()
            .unwrap()
            .contains("sandbox")
    );

    let (status, _) = app
        .delete(&format!("/api/v1/media/{id}"), Some(&token))
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(
        fetch(&app, &token, &format!("/api/v1/media/{id}/content"))
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    let (_, listed) = app.get("/api/v1/media", Some(&token)).await;
    assert_eq!(listed["total"], 0);
}

#[tokio::test]
async fn documents_download_instead_of_rendering() {
    let app = spawn().await;
    let (_, token) = app.user("docs@example.com").await;

    let created = upload(&app, &token, "report.pdf", pdf()).await;
    assert_eq!(created.status, StatusCode::CREATED, "{}", created.json());
    let id = created.json()["id"].as_str().unwrap().to_string();
    assert_eq!(created.json()["content_type"], "application/pdf");

    let content = fetch(&app, &token, &format!("/api/v1/media/{id}/content")).await;
    assert_eq!(content.headers["content-type"], "application/pdf");
    assert_eq!(
        content.headers["content-disposition"],
        "attachment; filename=\"report.pdf\""
    );
}

#[tokio::test]
async fn rejects_bad_uploads() {
    let app = spawn().await;
    let (_, token) = app.user("badmedia@example.com").await;

    // HTML/SVG claiming to be a PNG: the bytes decide, and they match nothing allowed.
    for hostile in [
        b"<html><script>alert(1)</script></html>".to_vec(),
        b"<svg xmlns='http://www.w3.org/2000/svg' onload='alert(1)'/>".to_vec(),
        b"just some text".to_vec(),
    ] {
        let response = upload(&app, &token, "x.png", hostile).await;
        assert_eq!(response.status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
        assert_eq!(response.json()["error"]["code"], "unsupported_media_type");
    }

    // A real GIF is recognized, but the test configuration only allows jpeg/png/pdf.
    let mut gif = b"GIF89a".to_vec();
    gif.extend_from_slice(&[0u8; 32]);
    let response = upload(&app, &token, "x.gif", gif).await;
    assert_eq!(response.status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    assert!(
        response.json()["error"]["message"]
            .as_str()
            .unwrap()
            .contains("image/gif")
    );

    assert_eq!(
        upload(&app, &token, "x.png", vec![]).await.status,
        StatusCode::BAD_REQUEST
    );

    let mut too_big = png();
    too_big.resize(8 * 1024 * 1024 + 1, 0);
    let response = upload(&app, &token, "big.png", too_big).await;
    assert_eq!(response.status, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(response.json()["error"]["code"], "payload_too_large");

    let (_, listed) = app.get("/api/v1/media", Some(&token)).await;
    assert_eq!(listed["total"], 0, "nothing was stored");
}

#[tokio::test]
async fn files_are_private_to_their_owner_but_admins_can_reach_them() {
    let app = spawn().await;
    let (_, owner) = app.user("owner@example.com").await;
    let (_, stranger) = app.user("stranger@example.com").await;
    let (_, admin) = app.admin("mediaadmin@example.com").await;

    let id = upload(&app, &owner, "mine.png", png()).await.json()["id"]
        .as_str()
        .unwrap()
        .to_string();
    let content_uri = format!("/api/v1/media/{id}/content");

    // unauthenticated
    let anonymous = app.raw(Method::GET, &content_uri, &[], vec![]).await;
    assert_eq!(anonymous.status, StatusCode::UNAUTHORIZED);

    // another user learns nothing, not even that the file exists
    assert_eq!(
        fetch(&app, &stranger, &content_uri).await.status,
        StatusCode::NOT_FOUND
    );
    let (status, _) = app
        .get(&format!("/api/v1/media/{id}"), Some(&stranger))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = app
        .delete(&format!("/api/v1/media/{id}"), Some(&stranger))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (_, strangers_list) = app.get("/api/v1/media", Some(&stranger)).await;
    assert_eq!(strangers_list["total"], 0);

    // the stranger's attempts did not delete it
    assert_eq!(
        fetch(&app, &owner, &content_uri).await.status,
        StatusCode::OK
    );
    // an admin can read it
    assert_eq!(
        fetch(&app, &admin, &content_uri).await.status,
        StatusCode::OK
    );
}

#[tokio::test]
async fn deleting_a_user_removes_their_media() {
    let app = spawn().await;
    let (owner_id, owner) = app.user("leaving@example.com").await;
    let (_, admin) = app.admin("cleanup-admin@example.com").await;

    let id = upload(&app, &owner, "mine.png", png()).await.json()["id"]
        .as_str()
        .unwrap()
        .to_string();

    let (status, _) = app
        .delete(&format!("/api/v1/users/{owner_id}"), Some(&admin))
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    // The row is gone, so even an admin can no longer fetch the file.
    assert_eq!(
        fetch(&app, &admin, &format!("/api/v1/media/{id}/content"))
            .await
            .status,
        StatusCode::NOT_FOUND
    );
}

/// The same upload/fetch/delete flow with S3 as the backing store. Needs a reachable S3 (or an
/// emulator such as floci) and is skipped unless `TEST_S3_BUCKET` is set; see the README.
#[tokio::test]
async fn media_round_trips_through_s3() {
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
    let (_, token) = app.user("s3media@example.com").await;
    let original = pdf();

    let created = upload(&app, &token, "s3.pdf", original.clone()).await;
    assert_eq!(created.status, StatusCode::CREATED, "{}", created.json());
    let id = created.json()["id"].as_str().unwrap().to_string();

    let content = fetch(&app, &token, &format!("/api/v1/media/{id}/content")).await;
    assert_eq!(content.status, StatusCode::OK);
    assert_eq!(&content.body[..], &original[..]);

    let (status, _) = app
        .delete(&format!("/api/v1/media/{id}"), Some(&token))
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(
        fetch(&app, &token, &format!("/api/v1/media/{id}/content"))
            .await
            .status,
        StatusCode::NOT_FOUND
    );
}
