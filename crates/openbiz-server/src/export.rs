//! `GET /api/export` — one graph, in one of the six serialisations `CLAUDE.md` §2 commits to.
//!
//! # Why this is a plain URL
//!
//! Exporting from PoolParty, TopBraid EDG, or VocBench means a modal, a wizard, or a job you come
//! back for. That is not a UI complaint: it means the export cannot be scripted, scheduled, diffed
//! in CI, or put in a runbook, which is most of what a governance team actually wants from one.
//! Here it is a `GET` at a stable URL with two query parameters, so `curl` and the interface are
//! the same path and neither is privileged.
//!
//! # What the response promises
//!
//! - **Exactly one graph, and nothing of ours.** See [`openbiz_store::Store::export_graph`] — it
//!   is the named-graph model, not a filter, that makes this true.
//! - **A missing graph is a 404, never an empty file.** An empty export for a vocabulary that does
//!   not exist is a valid, well-formed, entirely wrong document with nothing to warn the caller.
//! - **The syntax is the one you asked for, or a refusal.** `?format=` wins; otherwise `Accept` is
//!   negotiated; a name we do not know is a 400 and an `Accept` we cannot satisfy is a 406.
//!   Silently substituting the default would hand somebody who typed `?format=turtel` a file in a
//!   format they did not ask for.
//! - **The graph's identity is in the response even when it cannot be in the payload.** Turtle,
//!   N-Triples, and RDF/XML have nowhere to record a graph name, so `X-OpenBiz-Graph` carries it.

use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::{header, HeaderMap, HeaderName, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use openbiz_api::{ExportFormat, ExportFormats};
use openbiz_store::{RdfSyntax, Store, StoreError};
use serde::Deserialize;

use crate::accept;
use crate::graphs::{AppState, Failure};

/// Names the graph an export is of, for the syntaxes that cannot say so themselves.
const GRAPH_HEADER: HeaderName = HeaderName::from_static("x-openbiz-graph");

/// What a caller may ask for.
///
/// Both optional, and each absence means something different: no `graph` is a mistake, because
/// there is no sensible default vocabulary to export; no `format` is a preference expressed
/// through `Accept`, or none at all.
#[derive(Debug, Deserialize)]
pub(crate) struct ExportQuery {
    /// The IRI of the graph to export.
    graph: Option<String>,
    /// A syntax, named by token, file extension, or media type.
    format: Option<String>,
}

/// `GET /api/export/formats` — every serialisation this build can write.
///
/// Served rather than duplicated in the frontend so the interface cannot offer a format the
/// serialiser does not have, and cannot describe one wrongly: `recordsGraphNames` is read from the
/// same constant the writer branches on.
pub(crate) async fn formats() -> Json<ExportFormats> {
    Json(ExportFormats {
        formats: RdfSyntax::ALL
            .into_iter()
            .map(|syntax| ExportFormat {
                token: syntax.token().to_owned(),
                label: syntax.label().to_owned(),
                media_type: syntax.media_type().to_owned(),
                file_extension: syntax.file_extension().to_owned(),
                records_graph_names: syntax.records_graph_names(),
            })
            .collect(),
    })
}

/// `GET /api/export?graph=<iri>&format=<syntax>` — that graph, serialised.
pub(crate) async fn export(
    State(state): State<AppState>,
    Query(query): Query<ExportQuery>,
    headers: HeaderMap,
) -> Result<Response, Failure> {
    let Some(iri) = query.graph.filter(|iri| !iri.trim().is_empty()) else {
        return Err(Failure::new(
            StatusCode::BAD_REQUEST,
            "name the graph to export: /api/export?graph=<iri>. \
             GET /api/graphs lists the graphs this store holds",
        ));
    };

    let syntax = match &query.format {
        Some(requested) => RdfSyntax::parse(requested).ok_or_else(|| {
            Failure::new(
                StatusCode::BAD_REQUEST,
                format!(
                    "{requested:?} is not a serialisation OpenBiz can write; ask for one of {}",
                    offered()
                ),
            )
        })?,
        None => negotiate(&headers)?,
    };

    Ok(rendered(
        iri.clone(),
        syntax,
        serialise(state.store(), iri, syntax).await?,
    ))
}

/// Run the export off the async runtime.
///
/// Reading a whole graph is a RocksDB scan: it blocks, and for a large vocabulary it blocks for a
/// while. On an async worker thread that stalls every other request the runtime has, so one user's
/// download would degrade the interface for everyone. [`Store::export_graph`] itself streams and
/// takes no write lock, so an export never blocks an author either.
///
/// The result is *buffered* here even though the store streams, because the response body is built
/// in one piece. That bounds a single export by memory rather than by graph size, and it is
/// recorded in `docs/UNTESTED.md` rather than described as if it were streaming end to end.
async fn serialise(store: Arc<Store>, iri: String, syntax: RdfSyntax) -> Result<Vec<u8>, Failure> {
    let exported = tokio::task::spawn_blocking(move || {
        let mut bytes = Vec::new();
        store.export_graph(&iri, syntax, &mut bytes).map(|()| bytes)
    })
    .await;

    match exported {
        Ok(Ok(bytes)) => Ok(bytes),
        Ok(Err(StoreError::NoSuchGraph { iri })) => Err(Failure::new(
            StatusCode::NOT_FOUND,
            // The IRI came from the caller, so echoing it tells them nothing they did not send.
            // What it does tell them is that the store was asked and answered, rather than that
            // the *endpoint* is missing — the distinction a 404 otherwise loses.
            format!("no graph is registered at {iri}. GET /api/graphs lists the ones that are"),
        )),
        Ok(Err(error)) => {
            tracing::error!(%error, "a graph could not be exported");
            Err(Failure::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "the graph could not be exported; the server log records why",
            ))
        }
        Err(error) => {
            tracing::error!(%error, "the export task did not finish");
            Err(Failure::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "the graph could not be exported; the server log records why",
            ))
        }
    }
}

/// Wrap the serialised bytes in the headers that say what they are.
fn rendered(iri: String, syntax: RdfSyntax, body: Vec<u8>) -> Response {
    (
        [
            (
                header::CONTENT_TYPE,
                format!("{}; charset=utf-8", syntax.media_type()),
            ),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{}\"", download_name(&iri, syntax)),
            ),
            (GRAPH_HEADER, ascii_escaped(&iri)),
        ],
        body,
    )
        .into_response()
}

/// Choose a syntax from `Accept`.
///
/// Ranking lives in [`crate::accept`], shared with the SPARQL endpoint so the two cannot come to
/// different conclusions about the same header.
///
/// No `Accept` at all, or one that expresses no usable preference, is the default. An `Accept` that
/// *does* express a preference we cannot meet is a 406 rather than a quiet fallback: a client that
/// asked for JSON-LD and got Turtle will not notice until its parser does.
fn negotiate(headers: &HeaderMap) -> Result<RdfSyntax, Failure> {
    let Some(accept) = accept::header(headers) else {
        return Ok(RdfSyntax::DEFAULT);
    };

    let ranked = accept::preferences(accept);
    if ranked.is_empty() {
        return Ok(RdfSyntax::DEFAULT);
    }

    for range in ranked {
        if range == accept::ANYTHING {
            return Ok(RdfSyntax::DEFAULT);
        }
        if let Some(syntax) = RdfSyntax::parse(range) {
            return Ok(syntax);
        }
    }

    Err(Failure::new(
        StatusCode::NOT_ACCEPTABLE,
        format!("OpenBiz cannot write {accept:?}; it writes {}", offered()),
    ))
}

/// The syntaxes we can write, named the way a caller should ask for them.
fn offered() -> String {
    RdfSyntax::ALL
        .iter()
        .map(|syntax| format!("{} ({})", syntax.token(), syntax.media_type()))
        .collect::<Vec<_>>()
        .join(", ")
}

/// A filename a browser can save and a human can recognise afterwards.
///
/// Derived from the IRI's last meaningful segment, because `finance.ttl` in a downloads folder is
/// worth something and `export.ttl` is not. Everything outside a conservative ASCII set becomes a
/// hyphen: a filename is written to whatever filesystem the user has, and a graph IRI is allowed
/// characters that several of them are not.
fn download_name(iri: &str, syntax: RdfSyntax) -> String {
    let tail = iri
        .rsplit(['/', '#', ':'])
        .find(|segment| !segment.is_empty())
        .unwrap_or_default();

    let mut slug = String::with_capacity(tail.len());
    for character in tail.chars() {
        let safe = character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.');
        let replacement = if safe { character } else { '-' };
        if replacement == '-' && slug.ends_with('-') {
            continue;
        }
        slug.push(replacement);
    }

    let slug = slug.trim_matches(['-', '.']);
    let slug = if slug.is_empty() { "graph" } else { slug };

    format!("{slug}.{}", syntax.file_extension())
}

/// Percent-escape anything an HTTP header value may not carry.
///
/// An IRI may hold non-ASCII; a header value may not. Escaping rather than dropping the header
/// keeps the graph's identity in the response for every graph, which is the point of sending it at
/// all — and it is reversible, which "sanitised to hyphens" would not be. `%` is escaped too, so
/// the encoding round-trips.
fn ascii_escaped(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte == b'%' || !(0x21..=0x7e).contains(&byte) {
            escaped.push_str(&format!("%{byte:02X}"));
        } else {
            escaped.push(byte as char);
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    fn accepting(value: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::ACCEPT,
            HeaderValue::from_str(value).expect("a valid header value"),
        );
        headers
    }

    fn chosen(accept: &str) -> RdfSyntax {
        negotiate(&accepting(accept)).expect("a syntax")
    }

    #[test]
    fn no_accept_header_is_the_default_syntax() {
        assert_eq!(
            negotiate(&HeaderMap::new()).expect("a syntax"),
            RdfSyntax::DEFAULT
        );
    }

    #[test]
    fn every_syntax_can_be_asked_for_by_media_type() {
        for syntax in RdfSyntax::ALL {
            assert_eq!(chosen(syntax.media_type()), syntax);
        }
    }

    /// What a browser sends. It must not be read as a request for RDF/XML — `application/xml` is
    /// in that list at a high weight, and the engine's own media-type table maps it to RDF/XML.
    #[test]
    fn a_browsers_accept_header_gets_the_readable_default() {
        assert_eq!(
            chosen("text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8"),
            RdfSyntax::DEFAULT
        );
        assert_eq!(chosen("*/*"), RdfSyntax::DEFAULT);
    }

    #[test]
    fn weights_decide_and_order_breaks_a_tie() {
        assert_eq!(
            chosen("text/turtle;q=0.4, application/ld+json;q=0.9"),
            RdfSyntax::JsonLd
        );
        assert_eq!(
            chosen("application/n-quads, application/trig"),
            RdfSyntax::NQuads
        );
        assert_eq!(
            chosen("application/trig, application/n-quads"),
            RdfSyntax::TriG
        );
    }

    /// `q=0` is a client ruling something out. Treating it as a preference would give them the one
    /// format they explicitly said they could not read.
    #[test]
    fn a_zero_weight_is_a_refusal_not_a_preference() {
        assert_eq!(
            chosen("text/turtle;q=0, application/n-triples"),
            RdfSyntax::NTriples
        );
        assert_eq!(chosen("text/turtle;q=0, */*"), RdfSyntax::DEFAULT);
    }

    #[test]
    fn an_accept_we_cannot_satisfy_is_refused_rather_than_substituted() {
        let failure = negotiate(&accepting("text/html")).expect_err("406");
        assert_eq!(failure.status(), StatusCode::NOT_ACCEPTABLE);
        assert!(
            failure.message().contains("turtle"),
            "a refusal must say what it can do instead: {}",
            failure.message()
        );
    }

    #[test]
    fn the_filename_is_taken_from_the_iri_and_is_safe_to_write() {
        for (iri, expected) in [
            ("http://acme.example/v/finance", "finance.ttl"),
            ("http://acme.example/v/finance#", "finance.ttl"),
            ("http://acme.example/v/risk-2024.v2", "risk-2024.v2.ttl"),
            ("urn:openbiz:graph:system", "system.ttl"),
            ("http://acme.example/v/a b/c", "c.ttl"),
            ("http://acme.example/v/../..", "graph.ttl"),
            ("http://acme.example/v/金融", "graph.ttl"),
            ("http:///", "http.ttl"),
        ] {
            assert_eq!(download_name(iri, RdfSyntax::Turtle), expected, "for {iri}");
        }
    }

    /// A path separator or a leading dot in a downloaded filename is how a save turns into a write
    /// somewhere the user did not choose.
    #[test]
    fn a_filename_can_never_carry_a_separator_or_a_quote() {
        for iri in [
            "http://acme.example/v/..%2f..%2fetc%2fpasswd",
            "http://acme.example/v/.hidden",
            "http://acme.example/v/say\"hello\"",
            "http://acme.example/v/back\\slash",
        ] {
            let name = download_name(iri, RdfSyntax::NQuads);
            assert!(
                !name.contains(['/', '\\', '"']) && !name.starts_with('.'),
                "{iri} produced {name}"
            );
            assert!(name.ends_with(".nq"));
        }
    }

    #[test]
    fn a_non_ascii_iri_survives_into_a_header_value_escaped() {
        let iri = "http://acme.example/v/金融";
        let escaped = ascii_escaped(iri);

        assert!(escaped.is_ascii());
        assert!(HeaderValue::from_str(&escaped).is_ok());
        assert!(escaped.starts_with("http://acme.example/v/"));
        assert!(escaped.contains('%'), "the non-ASCII part must be escaped");
        assert_eq!(
            ascii_escaped("a%b"),
            "a%25b",
            "the escape itself is escaped"
        );
        assert_eq!(ascii_escaped("http://a/b#c"), "http://a/b#c");
    }

    /// The endpoint advertises what the store can write, so the two lists cannot drift.
    #[tokio::test]
    async fn the_advertised_formats_are_the_ones_the_store_has() {
        let Json(advertised) = formats().await;

        assert_eq!(advertised.formats.len(), RdfSyntax::ALL.len());
        for (offered, syntax) in advertised.formats.iter().zip(RdfSyntax::ALL) {
            assert_eq!(offered.token, syntax.token());
            assert_eq!(offered.label, syntax.label());
            assert_eq!(offered.media_type, syntax.media_type());
            assert_eq!(offered.file_extension, syntax.file_extension());
            assert_eq!(offered.records_graph_names, syntax.records_graph_names());
            assert_eq!(
                RdfSyntax::parse(&offered.token),
                Some(syntax),
                "the advertised token must be one `?format=` accepts"
            );
        }
        assert_eq!(
            advertised.formats[0].token,
            RdfSyntax::DEFAULT.token(),
            "the interface offers the first entry first, so it must be the default"
        );
    }
}
