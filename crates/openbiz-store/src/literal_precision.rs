//! Where a typed literal stops being a value, and what we say about it.
//!
//! `docs/BUILD-PLAN.md` Phase 1 asks for this: *"characterise Oxigraph's numeric/calendar/duration
//! literal precision limits and decide our documented behaviour at the boundary"*. Iteration 10
//! had already found that the store rewrites the lexical form of a literal it can interpret —
//! `"007"^^xsd:integer` comes back `"7"` — and pinned it in [`crate::spec_conformance`]. This
//! module answers the larger question that finding opened: **what happens at the edge of what it
//! can interpret at all?**
//!
//! # The rule, in one paragraph
//!
//! The backend's term encoding is a **value-space** encoding for the datatypes it models natively
//! and a **lexical-space** encoding for everything else. Where it interprets a literal it stores
//! the *value* and re-renders a canonical lexical form on the way out. Where it fails to interpret
//! one — because the value is outside the range of the Rust type behind the datatype, or because
//! the literal is ill-typed — it keeps the bytes exactly and stores no value at all. The literal
//! still round-trips byte-for-byte, and it is no longer a number, a date, or a duration to the
//! query engine: `isNumeric` is false, an inequality does not match it, and arithmetic on it is
//! unbound.
//!
//! # Why that is the finding rather than "big numbers get rounded"
//!
//! A precision limit that *rounds* is visible in the data. This one is invisible in the data and
//! visible only in the answers. `"170141183460469231731.687303715884105727"^^xsd:decimal` is a
//! number to this store; `…728`, one digit different, is not. Both export identically, both carry
//! `xsd:decimal`, and nothing anywhere says which is which. A governance team filtering
//! `FILTER(?value > 1000)` over a column that crosses the boundary silently loses the rows that
//! crossed it — and losing rows from a filter reads exactly like "there were no such rows".
//!
//! Three boundaries, measured in [`docs/adr/0014-literal-precision-boundaries.md`]:
//!
//! | family | interpreted while | beyond it |
//! |---|---|---|
//! | `xsd:integer` and its derived types | fits `i64` | bytes kept, value gone |
//! | `xsd:decimal` | fits a 128-bit fixed point with 18 fraction digits | bytes kept, value gone |
//! | `xsd:float` / `xsd:double` | always — IEEE 754 saturates | `INF` / `0`, per XSD |
//! | `xsd:dateTime` and friends | the lexical form is XSD-valid | bytes kept, value gone |
//!
//! Note the third row is *different in kind* from the others and is not a defect: XSD 1.1 defines
//! `float` and `double` over IEEE 754, so overflow to `INF` and underflow to `0` is the specified
//! answer rather than a limitation of this store. It is pinned here anyway, because "we round to
//! double" is a thing a customer needs told before they put a measurement in a vocabulary.
//!
//! # The one that is a defect, not a boundary
//!
//! Along the way this module found something the item did not ask about and which is worse than
//! anything it did: **the datatype IRI of a derived integer type is not preserved**. `"5"^^xsd:int`
//! is stored, and returned, as `"5"^^xsd:integer`. Under [RDF 1.1] a literal is the pair of a
//! lexical form and a datatype IRI, so those are different terms — and because they are stored as
//! the same term, writing both writes **one statement**. That is silent triple loss on a perfectly
//! ordinary input, and it is pinned by
//! [`tests::terms_differing_only_in_their_datatype_collapse_into_one_statement`].
//!
//! It also lands directly on two later phases. A SHACL `sh:datatype xsd:int` constraint (Phase 4)
//! can never be satisfied by anything in this store, and an OWL 2 datatype range over a derived
//! type (Phase 5) is untestable, because the datatype the shape names is not the datatype the
//! store returns. Both are recorded in `docs/UNTESTED.md` against those phases rather than left
//! to be discovered there.
//!
//! # What we document
//!
//! `docs/adr/0014-literal-precision-boundaries.md` records the decision. The short form is that we
//! **state the boundary rather than move it** — moving it means replacing the term encoding of the
//! store the whole product rests on — and that the disclosure this implies is a user-facing
//! feature a human authorises, so it is in `docs/PROPOSED.md` and not built here.
//!
//! [RDF 1.1]: https://www.w3.org/TR/rdf11-concepts/#section-Graph-Literal

#[cfg(test)]
mod tests {
    use oxigraph::model::{Literal, NamedNode, Term};

    use crate::{GraphId, QueryFormats, QueryLimits, RdfSyntax, Store};

    fn temp_dir() -> tempfile::TempDir {
        tempfile::tempdir().expect("a temporary directory")
    }

    fn xsd(name: &str) -> NamedNode {
        NamedNode::new_unchecked(format!("http://www.w3.org/2001/XMLSchema#{name}"))
    }

    /// A store holding one statement per row, subject `…#<label>`, object the typed literal.
    ///
    /// Returns the store and the graph so a caller can both export it and query it — the two
    /// readers whose disagreement, or agreement, is half of what these tests are about.
    fn stored(rows: &[(&str, &str, &str)]) -> (tempfile::TempDir, Store, GraphId) {
        let dir = temp_dir();
        let store = Store::open(dir.path()).expect("a fresh store opens");
        let graph = GraphId::vocabulary("http://acme.example/v/boundary")
            .expect("a valid absolute IRI outside the reserved namespace");
        let predicate = NamedNode::new_unchecked("http://example.org/p");

        let triples = rows
            .iter()
            .map(|(label, datatype, lexical)| {
                (
                    NamedNode::new_unchecked(format!("http://acme.example/v/boundary#{label}")),
                    predicate.clone(),
                    Term::from(Literal::new_typed_literal(*lexical, xsd(datatype))),
                )
            })
            .collect();

        store
            .transaction(|txn| {
                txn.create_vocabulary_graph(&graph)?;
                txn.insert(&graph, triples)
            })
            .expect("the vocabulary takes its statements");

        (dir, store, graph)
    }

    fn export(store: &Store, graph: &GraphId) -> String {
        let mut bytes = Vec::new();
        store
            .export_graph(graph.iri(), RdfSyntax::NTriples, &mut bytes)
            .expect("a registered graph serialises");
        String::from_utf8(bytes).expect("N-Triples is UTF-8")
    }

    fn answer(store: &Store, query: &str) -> String {
        let mut bytes = Vec::new();
        store
            .query(
                query,
                QueryFormats::default(),
                QueryLimits::default(),
                &mut bytes,
            )
            .expect("a well-formed query answers");
        String::from_utf8(bytes).expect("the results syntax is UTF-8")
    }

    /// The N-Triples line for `…#<label>`, or a panic naming the document that lacked it.
    fn line_for(document: &str, label: &str) -> String {
        let marker = format!("#{label}>");
        document
            .lines()
            .find(|line| line.starts_with(&format!("<http://acme.example/v/boundary{marker}")))
            .unwrap_or_else(|| panic!("no statement came back for {label}:\n{document}"))
            .to_owned()
    }

    /// Whether the query engine treats `…#<label>`'s object as a number.
    fn is_numeric(store: &Store, label: &str) -> bool {
        answer(store, "SELECT ?s WHERE { ?s ?p ?o FILTER(isNumeric(?o)) }")
            .contains(&format!("boundary#{label}\""))
    }

    /// **The boundary is a cliff, and one digit is the whole of it.**
    ///
    /// For each family, two literals that differ by the smallest possible amount: one inside the
    /// range the backend models, one outside. Both survive the round trip byte-for-byte and carry
    /// the same datatype. Only one of them is a number.
    ///
    /// This is the finding the plan item asked for. It is not that large values lose precision —
    /// they do not, the lexical form is exact — it is that they lose their *value*, silently, and
    /// the data gives the reader no way to tell which side of the line a literal fell.
    #[test]
    fn one_digit_decides_whether_a_literal_is_still_a_number() {
        // (label, datatype, lexical, is it interpreted?)
        let rows = [
            // `xsd:integer` is modelled as `i64`, so the line is at 2^63.
            ("int_max", "integer", "9223372036854775807"),
            ("int_over", "integer", "9223372036854775808"),
            // `xsd:decimal` is a 128-bit fixed point with 18 fraction digits: two lines, one at
            // the magnitude and one at the scale.
            (
                "dec_max",
                "decimal",
                "170141183460469231731.687303715884105727",
            ),
            (
                "dec_over",
                "decimal",
                "170141183460469231731.687303715884105728",
            ),
            ("dec_scale_ok", "decimal", "0.000000000000000001"),
            ("dec_scale_over", "decimal", "0.0000000000000000001"),
        ];
        let interpreted = ["int_max", "dec_max", "dec_scale_ok"];
        let inert = ["int_over", "dec_over", "dec_scale_over"];

        let (_dir, store, graph) = stored(&rows);
        let document = export(&store, &graph);

        // Both sides round-trip exactly. That is what makes the difference invisible in the data:
        // an export, a diff, and a git history all show these as equally well-formed.
        for (label, datatype, lexical) in rows {
            let line = line_for(&document, label);
            assert!(
                line.contains(&format!(
                    "\"{lexical}\"^^<http://www.w3.org/2001/XMLSchema#{datatype}>"
                )),
                "{label} did not come back as written: {line}"
            );
        }

        for label in interpreted {
            assert!(
                is_numeric(&store, label),
                "{label} is inside the range the backend models and should be a number"
            );
        }
        for label in inert {
            assert!(
                !is_numeric(&store, label),
                "{label} is now a number — the backend has widened its numeric range, so \
                 docs/adr/0014 and docs/UNTESTED.md both need revisiting"
            );
        }

        // And the consequence a user actually meets: a filter over the column silently omits the
        // rows that fell outside the range, rather than failing or warning.
        let over_one = answer(&store, "SELECT ?s WHERE { ?s ?p ?o FILTER(?o > 1) }");
        assert!(
            over_one.contains("boundary#int_max\""),
            "the in-range integer should match ?o > 1"
        );
        assert!(
            !over_one.contains("boundary#int_over\""),
            "the out-of-range integer is not compared, it is skipped — if this now matches, the \
             silent-omission finding in docs/adr/0014 is fixed"
        );

        // Arithmetic does not error either. It simply fails to bind, which a caller reading the
        // answer sees as a missing cell rather than as a problem — the row is still there.
        //
        // `?o - 1` rather than `?o + 1` because these rows sit at the top of their ranges, and
        // adding to them overflows the arithmetic itself. That is a *second* way to get an unbound
        // cell and worth keeping distinct: `int_max + 1` is unbound because the sum has nowhere to
        // go, while `int_over - 1` is unbound because the operand was never a number. Both look
        // identical in the answer, which is the point.
        let computed = answer(
            &store,
            "SELECT ?s WHERE { ?s ?p ?o BIND(?o - 1 AS ?n) FILTER(BOUND(?n)) }",
        );
        assert!(
            computed.contains("boundary#int_max\""),
            "the in-range integer should compute: {computed}"
        );
        assert!(
            !computed.contains("boundary#int_over\""),
            "the out-of-range integer should leave ?o + 1 unbound: {computed}"
        );
    }

    /// **A defect, recorded as a test rather than as prose: derived integer datatypes are erased.**
    ///
    /// `xsd:int`, `xsd:short`, `xsd:byte`, `xsd:long`, `xsd:unsignedLong`,
    /// `xsd:nonNegativeInteger`, and `xsd:positiveInteger` all come back as `xsd:integer` when
    /// their value fits `i64`. The lexical form survives; the datatype IRI the author wrote does
    /// not.
    ///
    /// This is worse than the lexical rewriting pinned in [`crate::spec_conformance`], because a
    /// datatype IRI is what every downstream constraint language names. A SHACL shape asserting
    /// `sh:datatype xsd:int` (Phase 4) can never be satisfied by data in this store, and an OWL 2
    /// datatype range over a derived type (Phase 5) is untestable for the same reason.
    ///
    /// Note the asymmetry, which is the same one iteration 10 found: the store is faithful to
    /// what it cannot interpret. `"9223372036854775808"^^xsd:long` is out of range for `long`, so
    /// it is not interpreted, so its datatype **is** preserved. The well-typed value loses its
    /// type and the ill-typed value keeps it.
    #[test]
    fn a_derived_integer_datatype_is_replaced_by_xsd_integer() {
        let erased = [
            "int",
            "short",
            "byte",
            "long",
            "unsignedLong",
            "nonNegativeInteger",
            "positiveInteger",
        ];
        let rows: Vec<(&str, &str, &str)> = erased
            .iter()
            .map(|datatype| (*datatype, *datatype, "5"))
            .chain([
                // The control: out of range for `xsd:long`, therefore uninterpreted, therefore
                // its datatype survives. Without this row the test is consistent with a store
                // that simply ignores datatype IRIs.
                ("long_over", "long", "9223372036854775808"),
            ])
            .collect();

        let (_dir, store, graph) = stored(&rows);
        let document = export(&store, &graph);

        for datatype in erased {
            let line = line_for(&document, datatype);
            assert!(
                line.contains("\"5\"^^<http://www.w3.org/2001/XMLSchema#integer>"),
                "xsd:{datatype} was expected to come back as xsd:integer, and did not — if the \
                 datatype is now preserved the defect is fixed and docs/adr/0014, \
                 docs/UNTESTED.md and docs/PROPOSED.md all need striking. Line: {line}"
            );
        }

        let control = line_for(&document, "long_over");
        assert!(
            control.contains("^^<http://www.w3.org/2001/XMLSchema#long>"),
            "a value out of range for xsd:long is not interpreted, so its datatype should \
             survive: {control}"
        );

        // The store is the source of the substitution, not the serialiser: `DATATYPE()` is
        // evaluated by the query engine over the stored term and never touches `export_graph`.
        let datatypes = answer(
            &store,
            "SELECT (DATATYPE(?o) AS ?d) WHERE { ?s ?p ?o FILTER(STR(?o) = \"5\") }",
        );
        assert!(
            !datatypes.contains("XMLSchema#int\""),
            "the substitution is in the term encoding, so a query sees it too: {datatypes}"
        );
    }

    /// **The consequence of the substitution above: statements are silently lost.**
    ///
    /// Four objects that are four distinct RDF terms under [RDF 1.1] — `"5"^^xsd:int`,
    /// `"5"^^xsd:integer`, `"5"^^xsd:byte`, and `"5"^^xsd:string` — written against one subject
    /// and one predicate. Two statements come back. Three of the four were folded into one, and
    /// nothing in the API, the export, or the transaction's result says so.
    ///
    /// This is the shape of loss `CLAUDE.md`'s wedge is built on not committing: a vocabulary that
    /// went in with four assertions comes out with two, and the diff a reviewer approves is
    /// against the two.
    ///
    /// [RDF 1.1]: https://www.w3.org/TR/rdf11-concepts/#section-Graph-Literal
    #[test]
    fn terms_differing_only_in_their_datatype_collapse_into_one_statement() {
        let dir = temp_dir();
        let store = Store::open(dir.path()).expect("a fresh store opens");
        let graph = GraphId::vocabulary("http://acme.example/v/collapse")
            .expect("a valid absolute IRI outside the reserved namespace");
        let subject = NamedNode::new_unchecked("http://acme.example/v/collapse#Concept");
        let notation = NamedNode::new_unchecked("http://www.w3.org/2004/02/skos/core#notation");

        let written = ["int", "integer", "byte", "string"];
        let triples = written
            .iter()
            .map(|datatype| {
                (
                    subject.clone(),
                    notation.clone(),
                    Term::from(Literal::new_typed_literal("5", xsd(datatype))),
                )
            })
            .collect();

        store
            .transaction(|txn| {
                txn.create_vocabulary_graph(&graph)?;
                txn.insert(&graph, triples)
            })
            .expect("the vocabulary takes its statements");

        let document = export(&store, &graph);
        let statements = document.lines().filter(|line| !line.is_empty()).count();
        assert_eq!(
            statements,
            2,
            "{} distinct RDF terms were written and {statements} statements came back — if this \
             is now {} the collapse is fixed and docs/adr/0014 needs striking:\n{document}",
            written.len(),
            written.len()
        );

        // Which two survived, named rather than merely counted: the string (a different value
        // space entirely) and the integer the three numeric ones collapsed onto.
        assert!(
            document.contains("\"5\" .") || document.contains("XMLSchema#string>"),
            "the xsd:string literal should survive as its own statement:\n{document}"
        );
        assert!(
            document.contains("\"5\"^^<http://www.w3.org/2001/XMLSchema#integer>"),
            "the three numeric literals should collapse onto xsd:integer:\n{document}"
        );
    }

    /// **`xsd:float` and `xsd:double` saturate, and that is correct rather than a limitation.**
    ///
    /// XSD 1.1 defines both over IEEE 754, so a value too large for the format is `INF` and one
    /// too small is zero. That is the specified answer, not this store's shortcut, and it is
    /// pinned here so a future reader does not mistake it for one — and so the *other* half is on
    /// the record: a decimal number written into a vocabulary as `xsd:double` is a binary
    /// approximation from the moment it lands, and 35 significant digits come back as 16.
    ///
    /// The last row is the one worth knowing about in an export: the store renders a large double
    /// in positional notation rather than the scientific form XSD's canonical mapping specifies,
    /// so `1.0E308` becomes 309 characters of digits. It is a valid `xsd:double` lexical form and
    /// it is not the canonical one.
    #[test]
    fn float_and_double_saturate_the_way_ieee_754_says() {
        let rows = [
            ("double_over", "double", "1.0E400"),
            ("double_under", "double", "1.0E-400"),
            (
                "double_precision",
                "double",
                "3.14159265358979323846264338327950288",
            ),
            ("float_over", "float", "1.0E40"),
            ("float_under", "float", "1.0E-50"),
            ("double_large", "double", "1.0E308"),
        ];
        let expected = [
            ("double_over", "INF"),
            ("double_under", "0"),
            ("double_precision", "3.141592653589793"),
            ("float_over", "INF"),
            ("float_under", "0"),
        ];

        let (_dir, store, graph) = stored(&rows);
        let document = export(&store, &graph);

        for (label, lexical) in expected {
            let line = line_for(&document, label);
            assert!(
                line.contains(&format!("\"{lexical}\"^^")),
                "{label} was expected to come back as {lexical:?}: {line}"
            );
        }

        // Unlike the integer and decimal boundaries, a saturated double is still a number: it was
        // interpreted, so the engine can compare it. That is the difference between a *value*
        // limit and an *interpretation* limit, and it is why only one of the two loses rows from
        // a filter.
        assert!(
            is_numeric(&store, "double_over"),
            "a saturated double is still a number to the engine"
        );

        // The non-canonical rendering, stated as a fact rather than implied.
        let large = line_for(&document, "double_large");
        assert!(
            large.contains(&format!("\"1{}\"^^", "0".repeat(308))),
            "a large double is written in positional notation, not the canonical scientific \
             form — if this is now \"1.0E308\" the rendering has been fixed: {large}"
        );
    }

    /// **Calendar and duration literals follow the same rule, and the canonical forms are XSD's.**
    ///
    /// The normalisations here are all specified: `24:00:00` is the next day's midnight, `PT25H`
    /// is `P1DT1H`, `P13M` as a `yearMonthDuration` is `P1Y1M`. What matters for this module is
    /// the *other* column — a lexical form XSD does not admit, such as a leap second or a
    /// timezone beyond ±14:00, is kept verbatim and is not a date to the engine, exactly as an
    /// oversized integer is kept verbatim and is not a number.
    #[test]
    fn calendar_and_duration_literals_normalise_or_go_inert() {
        let rows = [
            ("end_of_day", "dateTime", "2026-08-19T24:00:00Z"),
            ("hours", "dayTimeDuration", "PT25H"),
            ("months", "yearMonthDuration", "P13M"),
            ("leap_second", "dateTime", "2016-12-31T23:59:60Z"),
            ("far_tz", "dateTime", "2026-08-19T00:00:00+15:00"),
            ("ordinary", "dateTime", "2026-08-19T00:00:00Z"),
        ];
        let normalised = [
            ("end_of_day", "2026-08-20T00:00:00Z"),
            ("hours", "P1DT1H"),
            ("months", "P1Y1M"),
        ];
        // XSD admits neither a 60th second nor a timezone past ±14:00, so neither is interpreted.
        let inert = [
            ("leap_second", "2016-12-31T23:59:60Z"),
            ("far_tz", "2026-08-19T00:00:00+15:00"),
        ];

        let (_dir, store, graph) = stored(&rows);
        let document = export(&store, &graph);

        for (label, lexical) in normalised {
            let line = line_for(&document, label);
            assert!(
                line.contains(&format!("\"{lexical}\"^^")),
                "{label} should normalise to the XSD canonical form {lexical:?}: {line}"
            );
        }
        for (label, lexical) in inert {
            let line = line_for(&document, label);
            assert!(
                line.contains(&format!("\"{lexical}\"^^")),
                "{label} is not XSD-valid, so it should be kept verbatim: {line}"
            );
        }

        // And the inert ones are not dates: an ordering over the column cannot place them, which
        // is the calendar-shaped form of the same silent omission the numeric test measures.
        let ordered = answer(
            &store,
            "SELECT ?s WHERE { ?s ?p ?o FILTER(?o > \"2000-01-01T00:00:00Z\"^^\
             <http://www.w3.org/2001/XMLSchema#dateTime>) }",
        );
        assert!(
            ordered.contains("boundary#ordinary\""),
            "the well-formed dateTime should compare: {ordered}"
        );
        for (label, _) in inert {
            assert!(
                !ordered.contains(&format!("boundary#{label}\"")),
                "{label} is not a dateTime to the engine, so it should not compare: {ordered}"
            );
        }
    }
}
