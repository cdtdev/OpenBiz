//! Serving the embedded React interface.
//!
//! `CLAUDE.md` §1 makes "one binary" a non-negotiable: the server, the UI assets, and (later) the
//! store ship as a single executable. This module is the half of that promise the Rust side owns —
//! `ui/dist` is compiled into the binary by `rust-embed` and served from the router's fallback, so
//! there is no static-file directory to deploy and no web server to put in front.
//!
//! The fallback is deliberately not a catch-all "return index.html for everything":
//!
//! * `/api/…` that matches no route returns **404**. An API client must get a 404, not a page of
//!   HTML that parses as neither JSON nor an error.
//! * A missing file under `/assets/…` returns **404**. Returning `index.html` for a mistyped
//!   bundle name makes the browser fail with an opaque MIME-type error instead of a clear 404.
//! * Anything else returns `index.html` so client-side routes deep-link correctly.

use axum::body::{Body, Bytes};
use axum::extract::Request;
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use rust_embed::{Embed, EmbeddedFile};

/// The built frontend, compiled into the binary.
///
/// `debug-embed` is on so debug and release builds behave identically — without it `rust-embed`
/// reads from the filesystem in debug builds, and the tests below would prove the disk works
/// rather than proving the binary is self-contained.
#[derive(Embed)]
#[folder = "../../ui/dist"]
#[exclude = ".openbiz-placeholder"]
struct Assets;

/// Path prefix reserved for the JSON API. Unmatched paths under it are 404, never the UI shell.
const API_PREFIX: &str = "/api";

/// Vite fingerprints everything it emits under this prefix, so the content can never change under
/// a given URL.
const ASSET_PREFIX: &str = "/assets/";

/// The SPA shell.
const INDEX: &str = "index.html";

/// Serve an embedded asset, or the SPA shell for a client-side route.
pub(crate) async fn serve(request: Request) -> Response {
    let path = request.uri().path();

    if path.starts_with(API_PREFIX) {
        return StatusCode::NOT_FOUND.into_response();
    }

    if let Some(file) = Assets::get(path.trim_start_matches('/')) {
        return respond(path, file, request.headers());
    }

    if path.starts_with(ASSET_PREFIX) {
        return StatusCode::NOT_FOUND.into_response();
    }

    match Assets::get(INDEX) {
        Some(index) => respond(&format!("/{INDEX}"), index, request.headers()),
        // Unreachable in a real build: `build.rs` fails the compile when `ui/dist/index.html` is
        // absent. Handled rather than unwrapped because `CLAUDE.md` §6 forbids panicking here.
        None => (StatusCode::INTERNAL_SERVER_ERROR, "UI assets are missing").into_response(),
    }
}

/// Build the response for one embedded file, honouring `If-None-Match`.
fn respond(path: &str, file: EmbeddedFile, request_headers: &HeaderMap) -> Response {
    let etag = etag(&file);

    let mut response = if request_headers
        .get(header::IF_NONE_MATCH)
        .is_some_and(|sent| sent.as_bytes() == etag.as_bytes())
    {
        StatusCode::NOT_MODIFIED.into_response()
    } else {
        let body = match file.data {
            std::borrow::Cow::Borrowed(data) => Body::from(Bytes::from_static(data)),
            std::borrow::Cow::Owned(data) => Body::from(Bytes::from(data)),
        };
        body.into_response()
    };

    let headers = response.headers_mut();

    let mime = mime_guess::from_path(path).first_or_octet_stream();
    if let Ok(value) = HeaderValue::from_str(mime.as_ref()) {
        headers.insert(header::CONTENT_TYPE, value);
    }

    headers.insert(
        header::CACHE_CONTROL,
        // Fingerprinted assets are immutable; the shell must be revalidated or a deploy would
        // leave browsers pinned to a stale bundle graph.
        HeaderValue::from_static(if path.starts_with(ASSET_PREFIX) {
            "public, max-age=31536000, immutable"
        } else {
            "no-cache"
        }),
    );

    if let Ok(value) = HeaderValue::from_str(&etag) {
        headers.insert(header::ETAG, value);
    }

    response
}

/// A strong validator derived from the content hash `rust-embed` already computes.
fn etag(file: &EmbeddedFile) -> String {
    let mut etag = String::with_capacity(2 + 64 + 1);
    etag.push('"');
    for byte in file.metadata.sha256_hash() {
        use std::fmt::Write as _;
        // Writing to a String cannot fail; the result is discarded rather than unwrapped.
        let _ = write!(etag, "{byte:02x}");
    }
    etag.push('"');
    etag
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app;
    use axum::http::Request as HttpRequest;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    async fn get(uri: &str) -> Response {
        get_with(uri, HeaderMap::new()).await
    }

    async fn get_with(uri: &str, headers: HeaderMap) -> Response {
        let mut request = HttpRequest::builder().uri(uri);
        for (name, value) in &headers {
            request = request.header(name, value);
        }
        app()
            .oneshot(request.body(Body::empty()).expect("valid test request"))
            .await
            .expect("router is infallible")
    }

    async fn body_string(response: Response) -> String {
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("collect body")
            .to_bytes();
        String::from_utf8(bytes.to_vec()).expect("body is utf-8")
    }

    fn header(response: &Response, name: header::HeaderName) -> String {
        response
            .headers()
            .get(name)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_owned()
    }

    #[test]
    fn the_index_is_embedded() {
        assert!(
            Assets::get(INDEX).is_some(),
            "the binary must contain the UI shell"
        );
    }

    #[test]
    fn the_placeholder_sentinel_is_never_embedded() {
        assert!(Assets::get(".openbiz-placeholder").is_none());
    }

    /// Only meaningful when the real Vite output was compiled in — `build.rs` sets this cfg when
    /// it had to synthesise a placeholder instead, and the placeholder has no fingerprinted
    /// bundles to assert on. CI always builds the UI, so this always runs there.
    #[cfg(not(openbiz_placeholder_ui))]
    #[test]
    fn the_real_vite_build_is_embedded() {
        assert!(
            Assets::iter().any(|path| path.starts_with("assets/")),
            "expected fingerprinted bundles under assets/; \
             was ui/dist produced by something other than Vite?"
        );
    }

    #[tokio::test]
    async fn serves_the_ui_shell_at_the_root() {
        let response = get("/").await;

        assert_eq!(response.status(), StatusCode::OK);
        assert!(header(&response, header::CONTENT_TYPE).starts_with("text/html"));
        assert_eq!(header(&response, header::CACHE_CONTROL), "no-cache");
        assert!(body_string(response).await.contains(r#"id="root""#));
    }

    #[tokio::test]
    async fn serves_the_ui_shell_for_client_side_routes() {
        // A deep link into a client-side route must load the app, not 404.
        let response = get("/vocabularies/acme/concepts/widget").await;

        assert_eq!(response.status(), StatusCode::OK);
        assert!(body_string(response).await.contains(r#"id="root""#));
    }

    /// Every embedded file must be reachable at its own path with a plausible content type. This
    /// covers the fingerprinted bundles without hard-coding a hash that changes on every UI edit.
    #[tokio::test]
    async fn serves_every_embedded_asset() {
        for path in Assets::iter() {
            let uri = format!("/{path}");
            let response = get(&uri).await;

            assert_eq!(response.status(), StatusCode::OK, "{uri}");

            let expected = mime_guess::from_path(path.as_ref())
                .first_or_octet_stream()
                .to_string();
            assert_eq!(header(&response, header::CONTENT_TYPE), expected, "{uri}");

            assert!(!body_string(response).await.is_empty(), "{uri} was empty");
        }
    }

    #[tokio::test]
    async fn fingerprinted_assets_are_cached_immutably() {
        let Some(asset) = Assets::iter().find(|path| path.starts_with("assets/")) else {
            return; // placeholder build; covered by `the_real_vite_build_is_embedded` in CI
        };

        let response = get(&format!("/{asset}")).await;

        assert_eq!(
            header(&response, header::CACHE_CONTROL),
            "public, max-age=31536000, immutable"
        );
    }

    #[tokio::test]
    async fn a_matching_etag_is_answered_with_not_modified() {
        let first = get("/").await;
        let etag = header(&first, header::ETAG);
        assert!(
            etag.starts_with('"'),
            "expected a strong validator: {etag:?}"
        );

        let mut headers = HeaderMap::new();
        headers.insert(
            header::IF_NONE_MATCH,
            HeaderValue::from_str(&etag).expect("etag is a valid header value"),
        );
        let second = get_with("/", headers).await;

        assert_eq!(second.status(), StatusCode::NOT_MODIFIED);
        assert_eq!(header(&second, header::ETAG), etag);
        assert!(body_string(second).await.is_empty());
    }

    #[tokio::test]
    async fn a_stale_etag_is_answered_with_the_body() {
        let mut headers = HeaderMap::new();
        headers.insert(header::IF_NONE_MATCH, HeaderValue::from_static("\"stale\""));

        let response = get_with("/", headers).await;

        assert_eq!(response.status(), StatusCode::OK);
        assert!(body_string(response).await.contains(r#"id="root""#));
    }

    #[tokio::test]
    async fn a_missing_asset_is_not_found_rather_than_the_shell() {
        // Returning HTML here would surface in the browser as an unreadable MIME-type error.
        let response = get("/assets/does-not-exist.js").await;

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn unmatched_api_routes_are_not_found_rather_than_the_shell() {
        let response = get("/api/nope").await;

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn the_shell_is_not_served_for_writes() {
        let request = HttpRequest::builder()
            .method("POST")
            .uri("/somewhere")
            .body(Body::empty())
            .expect("valid test request");

        let response = app().oneshot(request).await.expect("router is infallible");

        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    }
}
