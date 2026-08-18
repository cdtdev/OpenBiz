//! Shared HTTP/JSON types for the OpenBiz API.
//!
//! These types are the contract between the Rust backend and the TypeScript frontend. Keep them
//! free of storage and reasoning concerns so the contract stays stable while engines behind it
//! change.

use serde::{Deserialize, Serialize};

/// Health report returned by `GET /healthz`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Health {
    /// `"ok"` when the server is able to serve requests.
    pub status: String,
    /// The server's crate version.
    pub version: String,
}

impl Health {
    /// A healthy report stamped with the compiled-in crate version.
    pub fn ok() -> Self {
        Self {
            status: "ok".to_owned(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
        }
    }
}

/// How a named graph is used, as it appears on the wire.
///
/// Deliberately a *separate* type from `openbiz_store::GraphKind` rather than a re-export. The
/// wire format is a published contract that a customer's script and our own TypeScript both parse;
/// the store's enum is an internal model that may gain a variant or rename a token for reasons
/// that have nothing to do with HTTP. Keeping them apart means the conversion in the server is an
/// exhaustive `match` — so adding a kind to the store fails the build here until someone decides
/// what it is called on the wire, instead of silently changing the API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GraphKind {
    /// A user-authored vocabulary.
    Vocabulary,
    /// OpenBiz's own metadata. Not a vocabulary, and never presented as one.
    System,
    /// Materialised inferences, derived rather than asserted.
    Inferred,
    /// The staged statements of a proposed change, not yet part of any vocabulary.
    Candidate,
}

/// One entry in the graph registry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphSummary {
    /// The graph's IRI.
    pub iri: String,
    /// What the graph holds.
    pub kind: GraphKind,
}

/// The graph registry, as returned by `GET /api/graphs`.
///
/// Every registered graph is listed, including OpenBiz's own. Filtering the store's bookkeeping
/// out of the *registry* would make this endpoint lie about what the store contains, and an
/// operator asking "what is in my store" is entitled to the whole answer. Keeping our graphs out
/// of the places a subject-matter expert works is the **UI's** job, and it can only do it because
/// `kind` is on the wire.
///
/// An object rather than a bare array so the response can gain fields — paging, a total, a
/// registry revision — without becoming a breaking change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphList {
    /// Every registered graph, ordered by IRI.
    pub graphs: Vec<GraphSummary>,
}

/// One serialisation `GET /api/export` can produce, as advertised by `GET /api/export/formats`.
///
/// The list is served rather than hard-coded in the frontend on purpose. The UI and the server
/// ship in one binary (`CLAUDE.md` §1.2), so a divergence between what the interface offers and
/// what the server can produce would never be caught by a type check or a deployment — it would be
/// caught by a user picking a format and getting a 400. Serving the list makes the interface
/// unable to offer a format the serialiser does not have.
///
/// `records_graph_names` is the field that earns the endpoint. Turtle, N-Triples, and RDF/XML are
/// *triple* syntaxes: there is nowhere in the file to record which graph a statement came from, so
/// an export in one of them cannot say which vocabulary it is. Every tool in this market has that
/// property and none of them mention it, which is why users discover it from a re-import that
/// lands in the wrong place. Here it comes from the same constant the serialiser branches on, so
/// the interface cannot say one thing while the writer does another.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportFormat {
    /// The value to pass as `?format=`.
    pub token: String,
    /// The name to show a human.
    pub label: String,
    /// The IANA media type, also accepted in `Accept`.
    pub media_type: String,
    /// The conventional file extension, without a dot.
    pub file_extension: String,
    /// Whether the syntax can record which graph the statements belong to.
    pub records_graph_names: bool,
}

/// Every serialisation this build can export, in the order the interface should offer them.
///
/// The first entry is the default a caller gets when they express no preference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportFormats {
    /// The formats, most readable first.
    pub formats: Vec<ExportFormat>,
}

/// One SPARQL query-results serialisation the SPARQL endpoint can write.
///
/// The sibling of [`ExportFormat`], and served for the same reason: the interface must not be able
/// to offer a format the server cannot write, and must not describe one wrongly.
///
/// `preserves_term_detail` is the field that earns *this* endpoint, and it is a sharper warning
/// than its export counterpart. Three of the four results formats record what kind of RDF term
/// each binding is — JSON and XML name it outright, TSV writes it in SPARQL's own syntax. **CSV
/// does not.** Every value is bare text, so `"1"` the string and `1` the integer are identical, an
/// IRI is indistinguishable from a literal that looks like one, and a **language tag is simply
/// gone**. For a multilingual thesaurus that is not a technicality — it is the difference between
/// a label and which language the label is in, and the shape of the mistake is a governance team
/// exporting a review spreadsheet as CSV, editing it, and re-importing a vocabulary whose language
/// tags have all quietly become the default. The specification says so; no tool in this market
/// says so at the point of choosing, which is the only point where saying it helps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultsFormat {
    /// The value to pass as `?format=`.
    pub token: String,
    /// The name to show a human.
    pub label: String,
    /// The IANA media type, also accepted in `Accept`.
    pub media_type: String,
    /// The conventional file extension, without a dot.
    pub file_extension: String,
    /// Whether the syntax records what *kind* of RDF term each binding is, including its language
    /// tag and datatype. False for CSV, which loses both silently.
    pub preserves_term_detail: bool,
}

/// Every query-results serialisation this build can write, in the order to offer them.
///
/// The first entry is the default a caller gets when they express no preference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResultsFormats {
    /// The formats, least lossy first.
    pub formats: Vec<ResultsFormat>,
}

/// The body of any failed API response.
///
/// One shape for every error so a client has exactly one thing to parse. `message` is written for
/// the person reading it, not for a machine to branch on — the status code is the machine-readable
/// part.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiError {
    /// What went wrong, in words a human can act on.
    pub message: String,
}

impl ApiError {
    /// An error carrying `message`.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The tokens are the contract. A rename here is a breaking API change, not a refactor, so
    /// they are asserted against literal strings rather than against `serde`'s behaviour.
    #[test]
    fn graph_kinds_serialise_to_their_documented_tokens() {
        for (kind, token) in [
            (GraphKind::Vocabulary, "\"vocabulary\""),
            (GraphKind::System, "\"system\""),
            (GraphKind::Inferred, "\"inferred\""),
            (GraphKind::Candidate, "\"candidate\""),
        ] {
            assert_eq!(serde_json::to_string(&kind).expect("serialisable"), token);
            assert_eq!(
                serde_json::from_str::<GraphKind>(token).expect("deserialisable"),
                kind
            );
        }
    }

    /// A token this build does not know must fail loudly. A client — or a future OpenBiz — reading
    /// a fourth kind as one of these three would misreport a graph's nature, and "is this asserted
    /// or inferred?" is the question the whole named-graph model exists to answer.
    #[test]
    fn an_unknown_graph_kind_is_refused_rather_than_guessed_at() {
        assert!(serde_json::from_str::<GraphKind>("\"ontology\"").is_err());
        assert!(serde_json::from_str::<GraphKind>("\"Vocabulary\"").is_err());
    }

    #[test]
    fn a_graph_list_round_trips_through_json() {
        let list = GraphList {
            graphs: vec![
                GraphSummary {
                    iri: "http://example.org/v/1".to_owned(),
                    kind: GraphKind::Vocabulary,
                },
                GraphSummary {
                    iri: "urn:openbiz:graph:system".to_owned(),
                    kind: GraphKind::System,
                },
            ],
        };

        let json = serde_json::to_string(&list).expect("serialisable");
        assert_eq!(
            json,
            r#"{"graphs":[{"iri":"http://example.org/v/1","kind":"vocabulary"},{"iri":"urn:openbiz:graph:system","kind":"system"}]}"#
        );
        assert_eq!(
            serde_json::from_str::<GraphList>(&json).expect("deserialisable"),
            list
        );
    }

    /// An empty registry is `[]`, never `null` and never an absent key — a client that has to
    /// handle three encodings of "nothing" will eventually handle one of them wrong.
    #[test]
    fn an_empty_graph_list_is_an_empty_array() {
        let json = serde_json::to_string(&GraphList { graphs: Vec::new() }).expect("serialisable");
        assert_eq!(json, r#"{"graphs":[]}"#);
    }

    #[test]
    fn an_api_error_carries_its_message_under_a_stable_key() {
        let json = serde_json::to_string(&ApiError::new("the roof is on fire")).expect("ok");
        assert_eq!(json, r#"{"message":"the roof is on fire"}"#);
    }

    /// The wire keys are the contract the TypeScript reads. Asserted literally, because renaming
    /// a field is a breaking change that `serde` would otherwise perform silently.
    #[test]
    fn an_export_format_publishes_camel_case_keys() {
        let json = serde_json::to_string(&ExportFormats {
            formats: vec![ExportFormat {
                token: "turtle".to_owned(),
                label: "Turtle".to_owned(),
                media_type: "text/turtle".to_owned(),
                file_extension: "ttl".to_owned(),
                records_graph_names: false,
            }],
        })
        .expect("serialisable");

        assert_eq!(
            json,
            r#"{"formats":[{"token":"turtle","label":"Turtle","mediaType":"text/turtle","fileExtension":"ttl","recordsGraphNames":false}]}"#
        );
    }

    #[test]
    fn ok_health_reports_status_and_version() {
        let health = Health::ok();
        assert_eq!(health.status, "ok");
        assert!(
            !health.version.is_empty(),
            "version must be stamped from the crate metadata"
        );
    }
}
