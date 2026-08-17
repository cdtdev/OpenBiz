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

use openbiz_server::app;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// Bind an ephemeral port, serve the real router on it, and return the address.
async fn start() -> SocketAddr {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("bind an ephemeral loopback port");
    let addr = listener.local_addr().expect("listener has a local address");

    tokio::spawn(async move {
        axum::serve(listener, app()).await.expect("serve");
    });

    addr
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
    let addr = start().await;

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
