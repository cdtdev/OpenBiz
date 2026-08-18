//! Reading an `Accept` header.
//!
//! Shared by every endpoint that negotiates content, because the two that do — the RDF export and
//! the SPARQL endpoint — must not disagree about what `q=0` means or which of two equally-weighted
//! entries a client meant first. One implementation, one set of tests, one behaviour.
//!
//! What is handled is exactly what is worth handling: comma-separated media ranges, `q=` weights,
//! and `*/*`. Subtype wildcards (`text/*`) are not matched. They are vanishingly rare from real
//! clients, and guessing which of two `text/…` syntaxes was meant is the silent substitution these
//! endpoints refuse everywhere else.

use axum::http::{header, HeaderMap};

/// The media range every syntax satisfies.
pub(crate) const ANYTHING: &str = "*/*";

/// The `Accept` header's value, if the caller sent one that is readable as text.
pub(crate) fn header(headers: &HeaderMap) -> Option<&str> {
    headers.get(header::ACCEPT).and_then(|it| it.to_str().ok())
}

/// The media ranges in `accept`, most preferred first.
///
/// Sorted descending by weight, and by the order written where weights tie — which is what a
/// client listing `text/turtle, application/ld+json` means by writing Turtle first. Entries with
/// `q=0` are dropped: that is the client saying "not this", so honouring it is the whole point of
/// the weight, and an endpoint that treated it as a preference would hand back the one format the
/// caller explicitly said it could not read.
///
/// An empty result means the header expressed no usable preference, which is not the same as an
/// unsatisfiable one — the caller gets a default rather than a refusal.
pub(crate) fn preferences(accept: &str) -> Vec<&str> {
    let mut ranked: Vec<(usize, f32, &str)> = accept
        .split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .enumerate()
        .map(|(order, entry)| {
            let mut parts = entry.split(';').map(str::trim);
            let range = parts.next().unwrap_or(entry);
            let weight = parts
                .find_map(|parameter| parameter.strip_prefix("q="))
                .and_then(|value| value.trim().parse::<f32>().ok())
                .unwrap_or(1.0);
            (order, weight, range)
        })
        .filter(|(_, weight, _)| *weight > 0.0)
        .collect();

    ranked.sort_by(|left, right| {
        right
            .1
            .partial_cmp(&left.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(left.0.cmp(&right.0))
    });

    ranked.into_iter().map(|(_, _, range)| range).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_absent_or_unreadable_header_is_no_preference() {
        assert_eq!(header(&HeaderMap::new()), None);
    }

    #[test]
    fn ranges_come_back_in_preference_order() {
        assert_eq!(
            preferences("text/turtle;q=0.4, application/ld+json;q=0.9"),
            vec!["application/ld+json", "text/turtle"]
        );
    }

    #[test]
    fn equal_weights_keep_the_order_they_were_written_in() {
        assert_eq!(
            preferences("application/trig, application/n-quads"),
            vec!["application/trig", "application/n-quads"]
        );
    }

    #[test]
    fn a_zero_weight_is_a_refusal_and_is_dropped() {
        assert_eq!(
            preferences("text/turtle;q=0, application/n-triples"),
            vec!["application/n-triples"]
        );
    }

    #[test]
    fn a_header_expressing_nothing_ranks_nothing() {
        assert!(preferences("").is_empty());
        assert!(preferences(" , ,").is_empty());
        assert!(preferences("text/html;q=0").is_empty());
    }

    /// What a browser sends. The wildcard is present but ranked last, which is what lets an
    /// endpoint prefer a real match over the default without ignoring the wildcard entirely.
    #[test]
    fn a_browsers_header_ranks_the_wildcard_last() {
        assert_eq!(
            preferences("text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8"),
            vec![
                "text/html",
                "application/xhtml+xml",
                "application/xml",
                ANYTHING
            ]
        );
    }

    /// A malformed weight must not be read as zero — dropping the entry would refuse a client that
    /// only fumbled a parameter, and RFC 9110 says a missing or unparseable `q` is 1.
    #[test]
    fn an_unparseable_weight_is_full_weight() {
        assert_eq!(
            preferences("text/turtle;q=banana, application/n-triples;q=0.5"),
            vec!["text/turtle", "application/n-triples"]
        );
    }
}
