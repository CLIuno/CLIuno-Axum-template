//! `POST /api/v1/uploads/image` — multipart field `file` in, absolute URL out.
//!
//! The bytes are written to `UPLOAD_DIR` as `<uuid>.<ext>` (extension from the sniffed
//! image type, never the client's filename) and served publicly, unversioned, at
//! `GET /uploads/<filename>` (see `app.rs`). Uploading mutates nothing: the client
//! attaches the URL itself (`PATCH /users/current {avatar_url}`, `POST /posts {image_url}`).

use axum::Router;
use axum::extract::multipart::MultipartRejection;
use axum::extract::{DefaultBodyLimit, Multipart, State};
use axum::http::HeaderMap;
use axum::routing::post;
use serde_json::json;

use crate::auth::AuthUser;
use crate::config::Config;
use crate::error::{ApiError, ApiResult};
use crate::response::created;
use crate::state::AppState;

/// 5 MB, like every other CLIuno backend.
pub const MAX_IMAGE_BYTES: usize = 5 * 1024 * 1024;

pub fn routes() -> Router<AppState> {
    // Room for the multipart framing on top of the file; anything bigger is a 413.
    Router::new().route(
        "/uploads/image",
        post(image).layer(DefaultBodyLimit::max(MAX_IMAGE_BYTES + 64 * 1024)),
    )
}

/// `POST /uploads/image` (multipart `file`) → 201 `data.url`, `data.filename`
async fn image(
    State(state): State<AppState>,
    _auth: AuthUser,
    headers: HeaderMap,
    multipart: Result<Multipart, MultipartRejection>,
) -> ApiResult {
    let mut multipart =
        multipart.map_err(|err| ApiError::from_status(err.status(), err.body_text()))?;
    let mut file = None;
    while let Some(field) = multipart.next_field().await? {
        if field.name() == Some("file") {
            file = Some(field.bytes().await?);
            break;
        }
    }
    let bytes = file
        .filter(|bytes| !bytes.is_empty())
        .ok_or_else(|| ApiError::bad_request("No image file provided (multipart field `file`)"))?;
    if bytes.len() > MAX_IMAGE_BYTES {
        return Err(ApiError::PayloadTooLarge(
            "Image exceeds the 5 MB limit".to_owned(),
        ));
    }
    let extension = sniff_image(&bytes)
        .ok_or_else(|| ApiError::bad_request("Only PNG, JPEG, WebP and GIF images are allowed"))?;

    let filename = format!("{}.{extension}", uuid::Uuid::new_v4());
    let dir = &state.config.upload_dir;
    tokio::fs::create_dir_all(dir)
        .await
        .map_err(|err| ApiError::internal(format!("cannot create upload dir: {err}")))?;
    tokio::fs::write(dir.join(&filename), &bytes)
        .await
        .map_err(|err| ApiError::internal(format!("cannot store upload: {err}")))?;

    let url = format!(
        "{}/uploads/{filename}",
        public_base_url(&state.config, &headers)
    );
    Ok(created(
        "Image uploaded successfully",
        json!({ "url": url, "filename": filename }),
    ))
}

/// The image type from the file's magic bytes, as the stored extension.
pub fn sniff_image(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("png")
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("jpg")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("gif")
    } else if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        Some("webp")
    } else {
        None
    }
}

/// `PUBLIC_BASE_URL` when set, else the origin the request came in on — absolute,
/// so a frontend on another origin can use the URL directly as an image source.
fn public_base_url(config: &Config, headers: &HeaderMap) -> String {
    if let Some(base) = &config.public_base_url {
        return base.trim_end_matches('/').to_owned();
    }
    let header = |name: &str| headers.get(name).and_then(|value| value.to_str().ok());
    let scheme = header("x-forwarded-proto").unwrap_or("http");
    let host = header("x-forwarded-host")
        .or_else(|| header("host"))
        .map_or_else(|| format!("localhost:{}", config.port), str::to_owned);
    format!("{scheme}://{host}")
}

#[cfg(test)]
mod tests {
    use super::sniff_image;

    #[test]
    fn sniffs_allowed_image_types() {
        assert_eq!(sniff_image(b"\x89PNG\r\n\x1a\nrest"), Some("png"));
        assert_eq!(sniff_image(&[0xFF, 0xD8, 0xFF, 0xE0]), Some("jpg"));
        assert_eq!(sniff_image(b"GIF89a..."), Some("gif"));
        assert_eq!(sniff_image(b"RIFF\0\0\0\0WEBPVP8 "), Some("webp"));
    }

    #[test]
    fn rejects_everything_else() {
        assert_eq!(
            sniff_image(b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>"),
            None
        );
        assert_eq!(sniff_image(b"%PDF-1.7"), None);
        assert_eq!(sniff_image(b""), None);
    }
}
