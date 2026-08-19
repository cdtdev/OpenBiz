//! When something was recorded, and on whose clock.
//!
//! Every timestamp OpenBiz writes into the audit trail — a candidate raised, a candidate decided,
//! an IRI policy recorded, a migration applied — goes through [`RecordedAt`]. It exists because a
//! provenance record whose reader cannot tell which clock it is on is not a provenance record;
//! it is a number that happens to look like a time.
//!
//! # Two rules, and they are deliberately not the same rule
//!
//! **What this build writes is UTC, always.** [`RecordedAt::now`] stamps `…Z`. Not local time:
//! the loop that produced this file runs in `Pacific/Auckland`, where for half of every day the
//! local calendar date is one day ahead of UTC's, so two entries a reader would order by their
//! dates can be thirty-six hours apart. A server that stamps in its own zone hands that problem
//! to whoever reads the trail later, in a different zone, with no way to recover which zone the
//! writer meant.
//!
//! **What this build reads back must carry an explicit offset — any offset.** A bare
//! `2026-08-19T14:17:03` is a *valid* `xsd:dateTime` and an unusable audit record: XSD leaves it
//! to the reader's implicit timezone, so two such stamps written by servers in different zones
//! cannot be ordered against each other at all, and neither can be ordered against a stamp that
//! does carry one. Reading is looser than writing because a store may hold records this build did
//! not write — restored from a backup, imported, or produced by a later version — and `+12:00` is
//! a perfectly orderable answer even though we would not have written it. What is refused is the
//! absence of an answer.
//!
//! # Why not just a `String`
//!
//! Because the store already re-validates every other field it reads back and did not re-validate
//! these. The record on disk is data: hand-editable, restorable from a doctored backup, writable
//! by a build with a bug. A candidate carrying `proposed_at "yesterday"` used to be read, kept,
//! and printed to a reviewer. Now it is [`crate::StoreError::Corrupt`], named as such, at the
//! boundary — which is where every other field of that record is already checked.
//!
//! # Ordering is the point, and it is delegated to the datatype
//!
//! `RecordedAt` deliberately exposes no comparison of its own. A trail is ordered by asking the
//! store, in SPARQL, over `xsd:dateTime`-typed literals — which is why the writers here pair with
//! typed literals rather than plain strings, and why the untyped IRI-policy stamp that shipped
//! before format version 5 was a real defect rather than an inconsistency. Ordering by *value*
//! belongs to the datatype's own semantics, and reimplementing it in Rust would be a second
//! answer to a question SPARQL already answers correctly.

use std::fmt;
use std::str::FromStr;

use oxsdatatypes::DateTime;
use thiserror::Error;

/// A point in time fit to be written into, or read out of, the audit trail.
///
/// Constructed only by [`RecordedAt::now`] (always UTC) or [`RecordedAt::parse`] (any explicit
/// offset). There is no constructor that accepts an unqualified instant, because there is no
/// honest thing to do with one.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RecordedAt {
    /// The lexical form exactly as it will be written, or exactly as it was read.
    ///
    /// Kept verbatim rather than re-rendered from the parsed value: a stamp read as `+12:00` is
    /// written back as `+12:00`. Normalising it to UTC on the way through would be silently
    /// rewriting somebody else's provenance record to say something it did not say.
    lexical: String,
}

impl RecordedAt {
    /// Now, on the UTC clock, as an `xsd:dateTime` ending in `Z`.
    pub fn now() -> Self {
        // `oxsdatatypes::DateTime::now` is built on the Unix epoch with `TimezoneOffset::UTC`, so
        // the offset is present by construction rather than by our arrangement. `clock.rs`'s
        // tests pin that, because it is a promise of a dependency and not of this module.
        Self {
            lexical: DateTime::now().to_string(),
        }
    }

    /// Read a stamp back out of a record, refusing one that names no clock.
    pub fn parse(text: &str) -> Result<Self, ClockError> {
        let value = DateTime::from_str(text).map_err(|_| ClockError::NotADateTime {
            found: text.to_owned(),
        })?;
        if value.timezone_offset().is_none() {
            return Err(ClockError::NoTimezone {
                found: text.to_owned(),
            });
        }
        Ok(Self {
            lexical: text.to_owned(),
        })
    }

    /// The lexical form, for writing and for display.
    pub fn as_str(&self) -> &str {
        &self.lexical
    }
}

impl fmt::Display for RecordedAt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.lexical)
    }
}

/// Why a stamp read out of a record is not one this build will act on.
///
/// Both variants say what was found, because the operator's next move is to look at the record and
/// the message has to be enough to find it.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ClockError {
    /// It is not an `xsd:dateTime` at all.
    #[error(
        "{found:?} is not a date and time, and a provenance record whose timestamp cannot be read \
         is not one this build will present as evidence"
    )]
    NotADateTime {
        /// The lexical form as found in the record.
        found: String,
    },
    /// It is a well-formed `xsd:dateTime` that names no timezone.
    #[error(
        "{found:?} names no timezone, so it cannot be ordered against any other record — an audit \
         trail this build writes stamps UTC, and one it reads must at least say which clock it \
         means"
    )]
    NoTimezone {
        /// The lexical form as found in the record.
        found: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn now_is_utc_and_says_so() {
        let stamp = RecordedAt::now();
        assert!(
            stamp.as_str().ends_with('Z'),
            "a stamp this build writes must be UTC and say so: {stamp}"
        );
        let parsed =
            DateTime::from_str(stamp.as_str()).expect("now() must be a valid xsd:dateTime");
        assert_eq!(
            parsed.timezone_offset(),
            Some(oxsdatatypes::TimezoneOffset::UTC),
            "now() must carry the UTC offset, not merely look like it"
        );
    }

    #[test]
    fn now_round_trips_through_parse() {
        let stamp = RecordedAt::now();
        assert_eq!(
            RecordedAt::parse(stamp.as_str()).as_ref(),
            Ok(&stamp),
            "what this build writes must be what it will accept back"
        );
    }

    #[test]
    fn a_stamp_with_no_timezone_is_refused() {
        let error = RecordedAt::parse("2026-08-19T14:17:03").expect_err("no timezone, so refused");
        assert_eq!(
            error,
            ClockError::NoTimezone {
                found: "2026-08-19T14:17:03".to_owned()
            }
        );
        assert!(
            error.to_string().contains("names no timezone"),
            "the message must name the reason: {error}"
        );
    }

    #[test]
    fn a_bare_date_is_refused_as_unreadable() {
        // The failure the product owner named: a date with no time and no offset, which two
        // readers twelve hours apart will place on different days.
        let error = RecordedAt::parse("2026-08-19").expect_err("a bare date is not a dateTime");
        assert_eq!(
            error,
            ClockError::NotADateTime {
                found: "2026-08-19".to_owned()
            }
        );
    }

    #[test]
    fn prose_is_refused() {
        assert!(matches!(
            RecordedAt::parse("yesterday"),
            Err(ClockError::NotADateTime { .. })
        ));
        assert!(matches!(
            RecordedAt::parse(""),
            Err(ClockError::NotADateTime { .. })
        ));
    }

    #[test]
    fn an_offset_this_build_would_not_write_is_still_read() {
        // Reading is looser than writing on purpose: `+12:00` orders perfectly well against `Z`,
        // and a store may hold records this build did not write.
        for lexical in [
            "2026-08-20T02:17:03+12:00",
            "2026-08-19T02:17:03-05:00",
            "2026-08-19T14:17:03Z",
        ] {
            let stamp = RecordedAt::parse(lexical).unwrap_or_else(|error| {
                panic!("{lexical} carries an explicit offset and should be read: {error}")
            });
            assert_eq!(
                stamp.as_str(),
                lexical,
                "the lexical form must survive verbatim rather than being normalised to UTC"
            );
        }
    }

    /// The two lexical forms the query engine keeps verbatim and will not compare — a leap second
    /// and a timezone past ±14:00, both outside what XSD admits (`docs/adr/0014`) — are refused
    /// here, before they can be stored.
    ///
    /// That is the property worth having and it is stronger than it looks: everything this seam
    /// accepts is something the engine can order. A validator that let one of these through would
    /// pass every assertion about the record and still leave a stamp that silently drops out of
    /// every `ORDER BY` and `FILTER` over the trail — present, readable, and uncomparable, which
    /// is the failure mode this whole module is built against.
    #[test]
    fn the_forms_the_engine_will_not_compare_are_refused_before_they_can_be_stored() {
        for lexical in ["2016-12-31T23:59:60Z", "2026-08-19T00:00:00+15:00"] {
            assert!(
                matches!(
                    RecordedAt::parse(lexical),
                    Err(ClockError::NotADateTime { .. })
                ),
                "{lexical} is not XSD-valid and the engine treats it as inert, so it must not \
                 reach a record"
            );
        }

        // And the one XSD *does* admit that looks equally odd is accepted, because it is a real
        // instant the engine normalises: `24:00:00` is the next day's midnight.
        assert!(RecordedAt::parse("2026-08-19T24:00:00Z").is_ok());
    }

    #[test]
    fn a_time_that_is_not_a_time_is_refused_even_with_an_offset() {
        for lexical in [
            "2026-13-01T00:00:00Z",
            "2026-02-30T00:00:00Z",
            "not-a-timeZ",
        ] {
            assert!(
                matches!(
                    RecordedAt::parse(lexical),
                    Err(ClockError::NotADateTime { .. })
                ),
                "{lexical} should be refused"
            );
        }
    }
}
