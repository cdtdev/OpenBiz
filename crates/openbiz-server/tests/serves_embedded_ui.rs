//! End-to-end: a real listener, a real TCP client, no test harness shortcuts.
//!
//! The unit tests in `src/ui.rs` drive the router directly via `tower::ServiceExt::oneshot`, which
//! proves the routing but not that `axum::serve` actually hands the embedded bytes to a socket.
//! `CLAUDE.md` §1 promises a downloadable binary that serves the interface, so that last hop is
//! worth asserting for real.
//!
//! The client is hand-rolled against `tokio::net::TcpStream` rather than pulling in an HTTP client
//! crate: `Connection: close` makes read-to-end a complete response, and every dependency is a
//! liability (`CLAUDE.md` §1.5).

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;

use openbiz_server::{app, AppState};
use openbiz_store::{GraphId, Store};
use tempfile::TempDir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// Bind an ephemeral port, serve the real router over a real store, and return the address.
///
/// The `TempDir` is returned rather than dropped: dropping it would delete the store out from
/// under the running server, and the resulting failures would look like server bugs.
async fn start() -> (SocketAddr, TempDir) {
    let dir = TempDir::new().expect("a temporary data directory");
    let store = Arc::new(Store::open(dir.path()).expect("a fresh store opens"));
    store
        .create_vocabulary_graph(
            &GraphId::vocabulary("http://example.org/v/animals").expect("a valid IRI"),
        )
        .expect("a fresh IRI is registrable");

    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("bind an ephemeral loopback port");
    let addr = listener.local_addr().expect("listener has a local address");

    tokio::spawn(async move {
        axum::serve(listener, app(AppState::new(store)))
            .await
            .expect("serve");
    });

    (addr, dir)
}

/// Issue `GET <path>` and return the whole response as text.
async fn get(addr: SocketAddr, path: &str) -> String {
    let mut stream = TcpStream::connect(addr)
        .await
        .expect("connect to the server");

    let request =
        format!("GET {path} HTTP/1.1\r\nHost: {addr}\r\nAccept: */*\r\nConnection: close\r\n\r\n");
    stream
        .write_all(request.as_bytes())
        .await
        .expect("write the request");

    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .await
        .expect("read the response");

    String::from_utf8_lossy(&response).into_owned()
}

#[tokio::test]
async fn a_running_server_serves_the_embedded_ui_and_the_health_probe() {
    let (addr, _dir) = start().await;

    let index = get(addr, "/").await;
    assert!(
        index.starts_with("HTTP/1.1 200 OK"),
        "expected 200 for the UI shell, got:\n{index}"
    );
    assert!(
        index.contains("text/html"),
        "expected an HTML content type, got:\n{index}"
    );
    assert!(
        index.contains(r#"id="root""#),
        "expected the SPA mount point in the body, got:\n{index}"
    );

    // The UI's first act is `fetch("/healthz")`. Prove the same origin answers it, because in
    // development only the Vite proxy makes that work and the proxy is not what ships.
    let health = get(addr, "/healthz").await;
    assert!(
        health.starts_with("HTTP/1.1 200 OK"),
        "expected 200 from /healthz, got:\n{health}"
    );
    assert!(
        health.contains(r#""status":"ok""#),
        "expected an ok health report, got:\n{health}"
    );
}

/// The UI's second act is `fetch("/api/graphs")`. Prove the registry reaches a socket, from a real
/// store, in the JSON the frontend parses — the unit tests drive the router directly and would
/// stay green if `axum::serve` never carried the state.
#[tokio::test]
async fn a_running_server_serves_the_graph_registry_from_its_store() {
    let (addr, _dir) = start().await;

    let response = get(addr, "/api/graphs").await;
    assert!(
        response.starts_with("HTTP/1.1 200 OK"),
        "expected 200 from /api/graphs, got:\n{response}"
    );
    assert!(
        response.contains("application/json"),
        "expected a JSON content type, got:\n{response}"
    );
    assert!(
        response.contains(r#"{"iri":"http://example.org/v/animals","kind":"vocabulary"}"#),
        "expected the vocabulary registered before start, got:\n{response}"
    );
    assert!(
        response.contains(r#"{"iri":"urn:openbiz:graph:system","kind":"system"}"#),
        "the registry must not hide OpenBiz's own graph; the UI filters, the API does not, got:\n{response}"
    );
}
