//! The RDF serialisations OpenBiz reads and writes.
//!
//! [`RdfSyntax`] is **our** enumeration, not the engine's. Per `CLAUDE.md` §3 no third-party type
//! crosses this crate's boundary, and there is a second reason here that is worth stating: the
//! engine's own format list is *wider* than what we support. It carries N3, which is not a W3C
//! Recommendation and is not on the charter's standards surface (§2). A re-export would have
//! quietly published a seventh format we have never tested, documented, or committed to.
//!
//! # What we commit to, and what we do not
//!
//! Six serialisations, each named in `CLAUDE.md` §2: Turtle, N-Triples, N-Quads, TriG, RDF/XML,
//! and JSON-LD. For each one this type owns the media type, the file extension, and the short
//! token an API caller may ask for — so those are a contract of ours rather than an accident of
//! whichever parser we happen to link against. [`RdfSyntax::records_graph_names`] is the one that
//! matters most to a user and is the one every incumbent leaves implicit: **three of the six
//! cannot record which graph a statement came from.** Exporting a vocabulary as Turtle loses its
//! graph IRI. That is a property of Turtle, not a defect of ours, but a governance tool that does
//! not *say so* is asking its user to rediscover it from a broken re-import.

use oxigraph::io::{JsonLdProfileSet, RdfFormat};

/// An RDF serialisation OpenBiz can write, and will be able to read.
///
/// `#[non_exhaustive]` because §2 lists further pragmatic formats as targets — ISO 25964 XML,
/// MADS/RDF, SKOS-shaped CSV — and a caller matching exhaustively today should not be broken by
/// one arriving.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum RdfSyntax {
    /// [Turtle](https://www.w3.org/TR/turtle/) — the readable default.
    Turtle,
    /// [N-Triples](https://www.w3.org/TR/n-triples/) — line-based, diff-friendly.
    NTriples,
    /// [N-Quads](https://www.w3.org/TR/n-quads/) — N-Triples plus the graph name.
    NQuads,
    /// [TriG](https://www.w3.org/TR/trig/) — Turtle plus the graph name.
    TriG,
    /// [RDF/XML](https://www.w3.org/TR/rdf-syntax-grammar/) — the legacy interchange format.
    RdfXml,
    /// [JSON-LD 1.1](https://www.w3.org/TR/json-ld11/) — for consumers that speak JSON.
    JsonLd,
}

impl RdfSyntax {
    /// Every syntax, in the order the interface offers them: readable first, lossless first.
    ///
    /// Ordered rather than arbitrary because it is what a format chooser renders, and the default
    /// — the first entry — is the one most users will accept without reading further.
    pub const ALL: [Self; 6] = [
        Self::Turtle,
        Self::NTriples,
        Self::NQuads,
        Self::TriG,
        Self::RdfXml,
        Self::JsonLd,
    ];

    /// The syntax a caller gets when they express no preference.
    pub const DEFAULT: Self = Self::Turtle;

    /// The stable short token an API caller uses, as in `?format=turtle`.
    ///
    /// Lower-case and punctuation-free so it survives a URL, a filename, and a shell without
    /// quoting. This is a published contract: renaming one is a breaking API change.
    pub const fn token(self) -> &'static str {
        match self {
            Self::Turtle => "turtle",
            Self::NTriples => "ntriples",
            Self::NQuads => "nquads",
            Self::TriG => "trig",
            Self::RdfXml => "rdfxml",
            Self::JsonLd => "jsonld",
        }
    }

    /// The name a human recognises, for a menu or an error message.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Turtle => "Turtle",
            Self::NTriples => "N-Triples",
            Self::NQuads => "N-Quads",
            Self::TriG => "TriG",
            Self::RdfXml => "RDF/XML",
            Self::JsonLd => "JSON-LD",
        }
    }

    /// The IANA media type, as sent in `Content-Type` and accepted in `Accept`.
    ///
    /// Stated here rather than delegated to the engine, and
    /// `our_media_types_are_the_ones_the_parser_recognises` asserts the two agree — so if a
    /// backend swap ever disagreed with our published contract, that is a failing test rather
    /// than a client that stops being able to re-read what we wrote.
    pub const fn media_type(self) -> &'static str {
        match self {
            Self::Turtle => "text/turtle",
            Self::NTriples => "application/n-triples",
            Self::NQuads => "application/n-quads",
            Self::TriG => "application/trig",
            Self::RdfXml => "application/rdf+xml",
            Self::JsonLd => "application/ld+json",
        }
    }

    /// The conventional file extension, without a dot.
    pub const fn file_extension(self) -> &'static str {
        match self {
            Self::Turtle => "ttl",
            Self::NTriples => "nt",
            Self::NQuads => "nq",
            Self::TriG => "trig",
            Self::RdfXml => "rdf",
            Self::JsonLd => "jsonld",
        }
    }

    /// Whether this syntax can record *which graph* a statement belongs to.
    ///
    /// The three that cannot — Turtle, N-Triples, RDF/XML — are not lesser formats; they are
    /// *triple* formats, and a triple has no graph. But it means an export in one of them cannot
    /// be re-imported into the right vocabulary without being told which one, and a user who
    /// discovers that after the fact reasonably concludes the export was lossy by accident. So it
    /// is a property callers can read and the interface states before the download, not a footnote.
    pub const fn records_graph_names(self) -> bool {
        match self {
            Self::NQuads | Self::TriG | Self::JsonLd => true,
            Self::Turtle | Self::NTriples | Self::RdfXml => false,
        }
    }

    /// Resolve a caller's request for a syntax.
    ///
    /// Deliberately generous about *how* it is named — the token, the file extension, or the media
    /// type, in any case, with any media-type parameters ignored — and deliberately strict about
    /// *which*: an unrecognised name is `None`, never a silent fall back to the default. Guessing
    /// here would hand somebody who typed `?format=turtel` a file in a format they did not ask for
    /// and no indication of it.
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

    /// The engine's equivalent. Crate-private: §3 keeps `oxigraph::io` out of our API.
    pub(crate) fn backend(self) -> RdfFormat {
        match self {
            Self::Turtle => RdfFormat::Turtle,
            Self::NTriples => RdfFormat::NTriples,
            Self::NQuads => RdfFormat::NQuads,
            Self::TriG => RdfFormat::TriG,
            Self::RdfXml => RdfFormat::RdfXml,
            Self::JsonLd => RdfFormat::JsonLd {
                profile: JsonLdProfileSet::empty(),
            },
        }
    }
}

impl std::fmt::Display for RdfSyntax {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn every_syntax_is_in_all() {
        // `ALL` is hand-written, so it can silently fall behind a new variant. Nothing in the
        // language checks it, so this does: every other test iterates `ALL`, and a syntax missing
        // from it would be a syntax nothing here tests.
        let distinct: HashSet<_> = RdfSyntax::ALL.iter().map(|s| s.token()).collect();
        assert_eq!(
            distinct.len(),
            RdfSyntax::ALL.len(),
            "duplicate token in ALL"
        );
        assert_eq!(
            RdfSyntax::ALL.len(),
            6,
            "CLAUDE.md §2 names six serialisations; adding a seventh means updating that list too"
        );
    }

    #[test]
    fn tokens_extensions_and_media_types_are_all_distinct() {
        for accessor in [
            RdfSyntax::token as fn(RdfSyntax) -> &'static str,
            RdfSyntax::file_extension,
            RdfSyntax::media_type,
        ] {
            let distinct: HashSet<_> = RdfSyntax::ALL.into_iter().map(accessor).collect();
            assert_eq!(
                distinct.len(),
                RdfSyntax::ALL.len(),
                "two syntaxes share a name, so `parse` would be ambiguous"
            );
        }
    }

    #[test]
    fn a_syntax_can_be_named_by_token_extension_or_media_type() {
        for syntax in RdfSyntax::ALL {
            for name in [syntax.token(), syntax.file_extension(), syntax.media_type()] {
                assert_eq!(RdfSyntax::parse(name), Some(syntax), "naming it {name:?}");
                assert_eq!(
                    RdfSyntax::parse(&name.to_ascii_uppercase()),
                    Some(syntax),
                    "naming it {name:?} in upper case"
                );
            }
            assert_eq!(
                RdfSyntax::parse(&format!("  {}; charset=utf-8 ", syntax.media_type())),
                Some(syntax),
                "a media type with parameters and whitespace"
            );
        }
    }

    /// The failure this guards is silent: a caller asking for something we do not have and being
    /// handed the default without being told.
    #[test]
    fn an_unrecognised_name_is_refused_rather_than_defaulted() {
        for name in ["", "turtel", "n3", "text/n3", "xml", "*/*", "text/html"] {
            assert_eq!(RdfSyntax::parse(name), None, "{name:?} must not resolve");
        }
    }

    /// N3 is in the engine's format list and is not in ours. This asserts the gap is deliberate:
    /// `CLAUDE.md` §2 does not list it, so publishing it because the parser happens to have it
    /// would be a standards claim nobody made.
    #[test]
    fn the_engines_extra_formats_are_not_ours() {
        assert!(
            RdfFormat::from_media_type("text/n3").is_some(),
            "the engine still has N3"
        );
        assert_eq!(RdfSyntax::parse("text/n3"), None);
    }

    #[test]
    fn our_media_types_are_the_ones_the_parser_recognises() {
        for syntax in RdfSyntax::ALL {
            assert_eq!(
                RdfFormat::from_media_type(syntax.media_type()),
                Some(syntax.backend()),
                "{syntax} publishes a media type its own parser does not map back to it"
            );
        }
    }

    #[test]
    fn our_extensions_are_the_ones_the_parser_recognises() {
        for syntax in RdfSyntax::ALL {
            assert_eq!(
                RdfFormat::from_extension(syntax.file_extension()),
                Some(syntax.backend()),
                "{syntax} publishes an extension its own parser does not map back to it"
            );
        }
    }

    /// The claim `records_graph_names` makes is a claim about the *serialiser*, so it is checked
    /// against the serialiser rather than restated. A backend that changed its mind about JSON-LD
    /// datasets would otherwise leave our interface telling users the opposite of what it does.
    #[test]
    fn the_graph_name_claim_matches_what_the_engine_does() {
        for syntax in RdfSyntax::ALL {
            assert_eq!(
                syntax.records_graph_names(),
                syntax.backend().supports_datasets(),
                "{syntax} claims one thing about graph names and the engine does another"
            );
        }
        assert!(!RdfSyntax::Turtle.records_graph_names());
        assert!(RdfSyntax::NQuads.records_graph_names());
    }

    #[test]
    fn the_default_is_the_first_offered() {
        assert_eq!(RdfSyntax::DEFAULT, RdfSyntax::ALL[0]);
    }
}
