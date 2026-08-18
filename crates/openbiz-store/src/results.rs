//! The SPARQL query-results serialisations OpenBiz writes.
//!
//! [`ResultsSyntax`] is the sibling of [`crate::RdfSyntax`], and it exists for the same two
//! reasons. `CLAUDE.md` §3 keeps third-party types out of our API, and — the reason that actually
//! bites — the engine's own list is not the list we have committed to. Here the shapes differ
//! rather than the membership: `sparesults` reports `text/csv; charset=utf-8` as the media type
//! for CSV, which is a media type *with a parameter* and therefore not a thing you can compare a
//! caller's `Accept` entry against without stripping it first. Publishing our own bare media types
//! and asserting the engine maps them back is cheaper than discovering the mismatch in a client.
//!
//! # What a SPARQL query answers with
//!
//! Two of the three answer shapes land here. A `SELECT` produces *solutions* — a table of variable
//! bindings — and an `ASK` produces a boolean; both are written in one of the four formats
//! `CLAUDE.md` §2 commits to via SPARQL 1.1. A `CONSTRUCT` or `DESCRIBE` produces RDF, which is
//! [`crate::RdfSyntax`]'s job, not this type's. Keeping the two enumerations apart is what lets an
//! HTTP layer negotiate both from one `Accept` header and then report honestly which family it
//! could not satisfy.

use oxigraph::sparql::results::QueryResultsFormat;

/// A SPARQL query-results serialisation OpenBiz can write.
///
/// `#[non_exhaustive]` for the same reason [`crate::RdfSyntax`] is: the set is fixed by the
/// specification today, and a caller matching exhaustively should not be broken if that changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum ResultsSyntax {
    /// [SPARQL 1.1 Query Results JSON](https://www.w3.org/TR/sparql11-results-json/).
    Json,
    /// [SPARQL Query Results XML](https://www.w3.org/TR/rdf-sparql-XMLres/).
    Xml,
    /// [SPARQL 1.1 Query Results CSV](https://www.w3.org/TR/sparql11-results-csv-tsv/).
    Csv,
    /// [SPARQL 1.1 Query Results TSV](https://www.w3.org/TR/sparql11-results-csv-tsv/).
    Tsv,
}

impl ResultsSyntax {
    /// Every results syntax, in the order an interface should offer them.
    ///
    /// JSON first because it is what a client library expects and what a console renders; CSV and
    /// TSV last because they are *lossy* — see [`ResultsSyntax::preserves_term_detail`].
    pub const ALL: [Self; 4] = [Self::Json, Self::Xml, Self::Csv, Self::Tsv];

    /// The syntax a caller gets when they express no preference.
    pub const DEFAULT: Self = Self::Json;

    /// The stable short token an API caller uses, as in `?format=json`.
    ///
    /// A published contract, exactly as [`crate::RdfSyntax::token`] is: renaming one is a breaking
    /// API change. Chosen not to collide with any RDF syntax token, so one `?format=` parameter
    /// can name either family without ambiguity.
    pub const fn token(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Xml => "xml",
            Self::Csv => "csv",
            Self::Tsv => "tsv",
        }
    }

    /// The name a human recognises, for a menu or an error message.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Json => "SPARQL Results JSON",
            Self::Xml => "SPARQL Results XML",
            Self::Csv => "CSV",
            Self::Tsv => "TSV",
        }
    }

    /// The IANA media type, without parameters.
    ///
    /// Bare on purpose. The engine appends `; charset=utf-8` to two of these, which makes its
    /// strings unusable for the one thing a media type is needed for here — comparing against an
    /// entry in a caller's `Accept` header. The charset belongs on the response, added once by
    /// whoever writes the header, not baked into the identity of the format.
    pub const fn media_type(self) -> &'static str {
        match self {
            Self::Json => "application/sparql-results+json",
            Self::Xml => "application/sparql-results+xml",
            Self::Csv => "text/csv",
            Self::Tsv => "text/tab-separated-values",
        }
    }

    /// The conventional file extension, without a dot.
    pub const fn file_extension(self) -> &'static str {
        match self {
            Self::Json => "srj",
            Self::Xml => "srx",
            Self::Csv => "csv",
            Self::Tsv => "tsv",
        }
    }

    /// Whether this syntax records what *kind* of RDF term each binding is.
    ///
    /// Three of the four do. JSON and XML say outright whether a value is an IRI, a blank node, or
    /// a literal, and carry a literal's datatype and language tag in named fields. TSV writes each
    /// term in SPARQL's own syntax — `<iri>`, `"text"@en-GB`, `"1"^^xsd:integer` — so the same
    /// information survives, though numeric and boolean literals are written in their abbreviated
    /// form, which is lossless but does not *look* typed.
    ///
    /// **CSV does not.** Every value is written as bare text: `"1"` the string and `1` the integer
    /// come out identical, an IRI is indistinguishable from a literal that happens to look like
    /// one, and a language tag is simply gone. For a multilingual thesaurus that last one is not a
    /// technicality — it is the difference between a label and *which language the label is in*.
    ///
    /// This is stated rather than left to be discovered, because the shape of the mistake is a
    /// governance team exporting a review spreadsheet as CSV, editing it, and re-importing a
    /// vocabulary whose language tags have all quietly become the default. The specification says
    /// so; no tool in this market says so at the point of choosing.
    pub const fn preserves_term_detail(self) -> bool {
        match self {
            Self::Json | Self::Xml | Self::Tsv => true,
            Self::Csv => false,
        }
    }

    /// Resolve a caller's request for a results syntax.
    ///
    /// Generous about *how* it is named — token, file extension, or media type, in any case, with
    /// media-type parameters ignored — and strict about *which*: an unrecognised name is `None`,
    /// never a silent fall back to the default.
    ///
    /// Deliberately narrower than the engine's own table, which also answers to `application/xml`,
    /// `application/json`, and `text/plain`. Those are ambiguous here: `application/xml` is what a
    /// browser sends and what RDF/XML has historically been served as, so reading it as a request
    /// for SPARQL Results XML would hand a browser a results document it never asked for.
    pub fn parse(requested: &str) -> Option<Self> {
        let requested = requested.trim();
        let name = requested
            .split(';')
            .next()
            .unwrap_or(requested)
            .trim()
            .to_ascii_lowercase();

        Self::ALL.into_iter().find(|syntax| {
            name == syntax.token() || name == syntax.file_extension() || name == syntax.media_type()
        })
    }

    /// The engine's equivalent. Crate-private: §3 keeps `sparesults` out of our API.
    pub(crate) fn backend(self) -> QueryResultsFormat {
        match self {
            Self::Json => QueryResultsFormat::Json,
            Self::Xml => QueryResultsFormat::Xml,
            Self::Csv => QueryResultsFormat::Csv,
            Self::Tsv => QueryResultsFormat::Tsv,
        }
    }
}

impl std::fmt::Display for ResultsSyntax {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RdfSyntax;
    use std::collections::HashSet;

    #[test]
    fn every_syntax_is_in_all() {
        let distinct: HashSet<_> = ResultsSyntax::ALL.iter().map(|s| s.token()).collect();
        assert_eq!(
            distinct.len(),
            ResultsSyntax::ALL.len(),
            "duplicate token in ALL"
        );
        assert_eq!(
            ResultsSyntax::ALL.len(),
            4,
            "SPARQL 1.1 defines four results formats; a fifth means a standards claim to check"
        );
    }

    #[test]
    fn tokens_extensions_and_media_types_are_all_distinct() {
        for accessor in [
            ResultsSyntax::token as fn(ResultsSyntax) -> &'static str,
            ResultsSyntax::file_extension,
            ResultsSyntax::media_type,
        ] {
            let distinct: HashSet<_> = ResultsSyntax::ALL.into_iter().map(accessor).collect();
            assert_eq!(
                distinct.len(),
                ResultsSyntax::ALL.len(),
                "two syntaxes share a name, so `parse` would be ambiguous"
            );
        }
    }

    /// The two families are negotiated from one `Accept` header and named by one `?format=`
    /// parameter. A name that resolved in both would make that ambiguous, and the ambiguity would
    /// surface as a caller getting the wrong document rather than as an error.
    #[test]
    fn no_name_resolves_in_both_syntax_families() {
        for results in ResultsSyntax::ALL {
            for name in [
                results.token(),
                results.file_extension(),
                results.media_type(),
            ] {
                assert_eq!(
                    RdfSyntax::parse(name),
                    None,
                    "{name:?} names a results syntax and an RDF syntax"
                );
            }
        }
        for rdf in RdfSyntax::ALL {
            for name in [rdf.token(), rdf.file_extension(), rdf.media_type()] {
                assert_eq!(
                    ResultsSyntax::parse(name),
                    None,
                    "{name:?} names an RDF syntax and a results syntax"
                );
            }
        }
    }

    #[test]
    fn a_syntax_can_be_named_by_token_extension_or_media_type() {
        for syntax in ResultsSyntax::ALL {
            for name in [syntax.token(), syntax.file_extension(), syntax.media_type()] {
                assert_eq!(
                    ResultsSyntax::parse(name),
                    Some(syntax),
                    "naming it {name:?}"
                );
                assert_eq!(
                    ResultsSyntax::parse(&name.to_ascii_uppercase()),
                    Some(syntax),
                    "naming it {name:?} in upper case"
                );
            }
            assert_eq!(
                ResultsSyntax::parse(&format!("  {}; charset=utf-8 ", syntax.media_type())),
                Some(syntax),
                "a media type with parameters and whitespace"
            );
        }
    }

    /// The engine answers to these; we do not. `application/xml` in particular arrives in every
    /// browser's `Accept` header, and reading it as "SPARQL Results XML" would mean a person
    /// typing the endpoint into an address bar gets a results document instead of a refusal.
    #[test]
    fn the_engines_ambiguous_aliases_are_not_ours() {
        for alias in ["application/xml", "application/json", "text/plain", "txt"] {
            assert!(
                QueryResultsFormat::from_media_type(alias).is_some()
                    || QueryResultsFormat::from_extension(alias).is_some(),
                "{alias} is supposed to be an engine alias; if it no longer is, this test is stale"
            );
            assert_eq!(
                ResultsSyntax::parse(alias),
                None,
                "{alias:?} is ambiguous and must not resolve"
            );
        }
    }

    #[test]
    fn an_unrecognised_name_is_refused_rather_than_defaulted() {
        for name in ["", "jsno", "sparql", "*/*", "text/html", "turtle"] {
            assert_eq!(
                ResultsSyntax::parse(name),
                None,
                "{name:?} must not resolve"
            );
        }
    }

    /// Our media types are bare and the engine's carry a charset, so this compares through the
    /// engine's own parser rather than by string equality — which is the comparison that matters,
    /// since it is what decides whether a document we label can be read back.
    #[test]
    fn our_media_types_are_the_ones_the_engine_recognises() {
        for syntax in ResultsSyntax::ALL {
            assert_eq!(
                QueryResultsFormat::from_media_type(syntax.media_type()),
                Some(syntax.backend()),
                "{syntax} publishes a media type the engine does not map back to it"
            );
        }
    }

    /// The reason `media_type` is hand-written rather than delegated. If the engine ever stops
    /// appending a charset, this fails and the duplication can go — until then it is load-bearing.
    #[test]
    fn the_engines_media_types_carry_parameters_and_ours_do_not() {
        assert!(
            QueryResultsFormat::Csv.media_type().contains(';'),
            "the engine used to append a charset; if it no longer does, `media_type` can delegate"
        );
        for syntax in ResultsSyntax::ALL {
            assert!(
                !syntax.media_type().contains(';'),
                "{syntax} publishes a media type with a parameter, which cannot be compared to an \
                 Accept entry"
            );
        }
    }

    #[test]
    fn our_extensions_are_the_ones_the_engine_recognises() {
        for syntax in ResultsSyntax::ALL {
            assert_eq!(
                QueryResultsFormat::from_extension(syntax.file_extension()),
                Some(syntax.backend()),
                "{syntax} publishes an extension the engine does not map back to it"
            );
        }
    }

    /// CSV's lossiness is a property of the specification, not an opinion, so it is asserted
    /// against what the serialiser actually produces rather than restated.
    ///
    /// The probe is a **language tag**, chosen over a datatype because a datatype is ambiguous
    /// evidence: TSV writes `"1"^^xsd:integer` as `1`, which is abbreviation rather than loss and
    /// would make this test claim a loss that has not happened. A language tag has no abbreviated
    /// form — it is either in the document or it is gone — and for a multilingual thesaurus it is
    /// the piece of term detail that matters most.
    #[test]
    fn the_term_detail_claim_matches_what_the_serialiser_does() {
        use oxigraph::model::Literal;
        use oxigraph::sparql::results::QueryResultsSerializer;
        use oxigraph::sparql::{QuerySolution, Variable};
        use std::sync::Arc;

        let variables: Arc<[Variable]> = vec![Variable::new("v").expect("a variable")].into();
        let tagged =
            Literal::new_language_tagged_literal("colour", "en-GB").expect("a valid language tag");

        for syntax in ResultsSyntax::ALL {
            let serializer = QueryResultsSerializer::from_format(syntax.backend());
            let mut writer = serializer
                .serialize_solutions_to_writer(Vec::new(), variables.to_vec())
                .expect("a solutions writer");
            let solution: QuerySolution =
                (Arc::clone(&variables), vec![Some(tagged.clone().into())]).into();
            writer.serialize(&solution).expect("a solution serialises");
            let written = String::from_utf8(writer.finish().expect("a finished document"))
                .expect("UTF-8 output");

            assert!(
                written.contains("colour"),
                "{syntax} lost the value itself, so this test proves nothing about its tag: \
                 {written:?}"
            );
            // Lower-cased before comparing: the engine normalises language tags to lower case,
            // which BCP 47 makes case-insensitive and which is therefore a normalisation rather
            // than a loss — but comparing against `en-GB` verbatim would report it as one.
            assert_eq!(
                syntax.preserves_term_detail(),
                written.to_ascii_lowercase().contains("en-gb"),
                "{syntax} claims preserves_term_detail = {} but wrote {written:?}",
                syntax.preserves_term_detail()
            );
        }
    }

    #[test]
    fn the_default_is_the_first_offered() {
        assert_eq!(ResultsSyntax::DEFAULT, ResultsSyntax::ALL[0]);
    }
}
