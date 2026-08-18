//! Conformance of the line-based exports, checked against the specifications' own text.
//!
//! # Why this module exists
//!
//! Everything in `lib.rs`'s `export` tests proves **round-trip fidelity**: a graph is serialised
//! by Oxigraph and read back by Oxigraph, and the statements survive. That is a real property and
//! it kills real bugs, but it is self-consistency, not conformance. A writer and a reader that
//! share a misreading of a grammar agree with each other perfectly and hand a third party a file
//! it cannot read. `CLAUDE.md` §4.5 requires a standards claim to be backed by a test against the
//! spec's own examples or test suite, and until this module existed the only evidence that an
//! OpenBiz export was Turtle, or N-Triples, was that OpenBiz could read it back.
//!
//! So the reader in this module is **ours**, transcribed from the grammar productions published
//! in [N-Triples] §7 and [N-Quads] §4, and it shares no code with the writer under test. When it
//! and the serialiser disagree, one of them is wrong about the specification, and that is a
//! finding rather than a silent pass.
//!
//! # What this proves, and what it does not
//!
//! Two of the six serialisations are covered: **N-Triples and N-Quads**. They were chosen because
//! their grammars are small enough to transcribe faithfully in one sitting — the whole of both is
//! seven productions and eight terminals — so the checker can be *read against the spec* by a
//! reviewer rather than taken on trust. Turtle, TriG, RDF/XML, and JSON-LD are not covered here
//! and their round trip remains self-consistency only; `docs/UNTESTED.md` says so.
//!
//! It is not nothing that the two covered are the two that matter most for interchange: N-Triples
//! is the canonical form every RDF test suite is expressed in, and N-Quads is the only syntax in
//! which a whole OpenBiz store could be backed up without losing which vocabulary a statement
//! belongs to.
//!
//! # A checker nobody has broken proves nothing
//!
//! A recogniser that accepts everything passes every test it is given. So
//! `the_reader_refuses_what_the_grammar_refuses` and
//! `the_canonical_checker_refuses_what_canonical_n_triples_refuses` feed it documents that violate
//! one named production or one named §4 constraint each, and require it to reject them. Those
//! tests are the reason to believe the passing ones.
//!
//! [N-Triples]: https://www.w3.org/TR/n-triples/
//! [N-Quads]: https://www.w3.org/TR/n-quads/

use super::*;

/// Which of the two line-based grammars a document is being read against.
///
/// They differ in exactly one production — N-Quads' `statement` carries an optional `graphLabel`
/// where N-Triples' `triple` does not — so one reader with a flag is more faithful than two
/// readers that could drift apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Dialect {
    /// [N-Triples §7](https://www.w3.org/TR/n-triples/#sec-grammar).
    NTriples,
    /// [N-Quads §4](https://www.w3.org/TR/n-quads/#sec-grammar).
    NQuads,
}

/// An RDF term as the grammar defines it, deliberately *not* the engine's term type.
///
/// Recovering into our own type is what makes this an independent check. If the comparison ran
/// through `oxigraph::model::Term` then a term the engine mis-constructs on the way in and
/// mis-renders on the way out would compare equal to itself.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum SpecTerm {
    /// `IRIREF`, with any `UCHAR` already decoded.
    Iri(String),
    /// `BLANK_NODE_LABEL`, without its `_:` prefix.
    Blank(String),
    /// `literal` — the lexical form after escape processing, plus what follows it.
    Literal { lexical: String, tag: LiteralTag },
}

/// What follows a literal's lexical form.
///
/// [N-Triples §2.3](https://www.w3.org/TR/n-triples/#sec-literals): "If there is no datatype IRI
/// and no language tag it is a simple literal and the datatype is
/// `http://www.w3.org/2001/XMLSchema#string`." So `Simple` and `Datatype(xsd:string)` denote the
/// same term, and `same_term_as` below is where that equivalence is stated once.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum LiteralTag {
    Simple,
    Language(String),
    Datatype(String),
}

const XSD_STRING: &str = "http://www.w3.org/2001/XMLSchema#string";

impl SpecTerm {
    fn simple(lexical: &str) -> Self {
        Self::Literal {
            lexical: lexical.to_owned(),
            tag: LiteralTag::Simple,
        }
    }

    fn language(lexical: &str, tag: &str) -> Self {
        Self::Literal {
            lexical: lexical.to_owned(),
            tag: LiteralTag::Language(tag.to_owned()),
        }
    }

    fn typed(lexical: &str, datatype: &str) -> Self {
        Self::Literal {
            lexical: lexical.to_owned(),
            tag: LiteralTag::Datatype(datatype.to_owned()),
        }
    }

    fn iri(iri: &str) -> Self {
        Self::Iri(iri.to_owned())
    }

    /// Collapse the two spellings of a simple literal, so a comparison tests the *term* rather
    /// than which of two equivalent spellings the writer chose.
    fn normalised(self) -> Self {
        match self {
            Self::Literal {
                lexical,
                tag: LiteralTag::Datatype(datatype),
            } if datatype == XSD_STRING => Self::Literal {
                lexical,
                tag: LiteralTag::Simple,
            },
            // Blank node labels are scoped to the document ([N-Triples §2.4]), so a writer is free
            // to rename them. Comparing them would test the label generator rather than whether
            // the statement survived, so every label collapses to one placeholder. Every fixture
            // here has at most one blank node, so this cannot mask two being conflated.
            //
            // [N-Triples §2.4]: https://www.w3.org/TR/n-triples/#BNodes
            Self::Blank(_) => Self::Blank("anonymous".to_owned()),
            other => other,
        }
    }
}

/// One `statement` (N-Quads) or `triple` (N-Triples).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct SpecStatement {
    subject: SpecTerm,
    predicate: SpecTerm,
    object: SpecTerm,
    /// `None` for N-Triples, and for an N-Quads statement with no `graphLabel`.
    graph: Option<SpecTerm>,
}

impl SpecStatement {
    fn triple(subject: SpecTerm, predicate: SpecTerm, object: SpecTerm) -> Self {
        Self {
            subject,
            predicate,
            object,
            graph: None,
        }
    }

    fn in_graph(mut self, iri: &str) -> Self {
        self.graph = Some(SpecTerm::iri(iri));
        self
    }

    fn normalised(self) -> Self {
        Self {
            subject: self.subject.normalised(),
            predicate: self.predicate.normalised(),
            object: self.object.normalised(),
            graph: self.graph.map(SpecTerm::normalised),
        }
    }
}

/// A reader for N-Triples and N-Quads, transcribed from the published grammars.
///
/// It reports two independent things. **Grammar conformance** is the `Result`: an `Err` means the
/// document is not a document of the language. **Canonical-form conformance** is `canonical`,
/// which accumulates every departure from [Canonical N-Triples §4] without failing the parse —
/// because a non-canonical document is still a perfectly legal N-Triples document, and conflating
/// the two would let a layout complaint masquerade as a syntax error.
///
/// [Canonical N-Triples §4]: https://www.w3.org/TR/n-triples/#canonical-ntriples
struct SpecReader {
    chars: Vec<char>,
    pos: usize,
    dialect: Dialect,
    canonical: Vec<String>,
}

impl SpecReader {
    fn new(document: &str, dialect: Dialect) -> Self {
        Self {
            chars: document.chars().collect(),
            pos: 0,
            dialect,
            canonical: Vec::new(),
        }
    }

    /// Read a whole document. `nquadsDoc ::= statement? (EOL statement)* EOL?`
    fn document(&mut self) -> Result<Vec<SpecStatement>, String> {
        let mut statements = Vec::new();

        self.layout("at the start of a line");
        if !self.at_end() && !self.at_eol() {
            statements.push(self.statement()?);
        }

        loop {
            self.layout("after the '.' ending a statement");
            if self.at_end() {
                break;
            }
            if !self.at_eol() {
                return Err(self.fault("expected a line break or the end of the document"));
            }
            self.eol();
            self.layout("at the start of a line");
            if self.at_end() {
                break;
            }
            statements.push(self.statement()?);
        }

        Ok(statements)
    }

    /// `statement ::= subject predicate object graphLabel? '.'`
    fn statement(&mut self) -> Result<SpecStatement, String> {
        let subject = self.subject()?;
        self.separator("after the subject");
        let predicate = self.predicate()?;
        self.separator("after the predicate");
        let object = self.object()?;
        self.separator("after the object");

        let graph = if self.dialect == Dialect::NQuads && !matches!(self.peek(), Some('.')) {
            let label = self.graph_label()?;
            self.separator("after the graph label");
            Some(label)
        } else {
            None
        };

        if !self.eat('.') {
            return Err(self.fault("expected '.' to end the statement"));
        }

        Ok(SpecStatement {
            subject,
            predicate,
            object,
            graph,
        })
    }

    /// `subject ::= IRIREF | BLANK_NODE_LABEL`
    fn subject(&mut self) -> Result<SpecTerm, String> {
        match self.peek() {
            Some('<') => self.iriref().map(SpecTerm::Iri),
            Some('_') => self.blank_node_label(),
            _ => Err(self.fault("a subject must be an IRI or a blank node label")),
        }
    }

    /// `predicate ::= IRIREF`
    fn predicate(&mut self) -> Result<SpecTerm, String> {
        if !matches!(self.peek(), Some('<')) {
            return Err(self.fault("a predicate must be an IRI"));
        }
        self.iriref().map(SpecTerm::Iri)
    }

    /// `object ::= IRIREF | BLANK_NODE_LABEL | literal`
    fn object(&mut self) -> Result<SpecTerm, String> {
        match self.peek() {
            Some('<') => self.iriref().map(SpecTerm::Iri),
            Some('_') => self.blank_node_label(),
            Some('"') => self.literal(),
            _ => Err(self.fault("an object must be an IRI, a blank node label, or a literal")),
        }
    }

    /// `graphLabel ::= IRIREF | BLANK_NODE_LABEL`
    fn graph_label(&mut self) -> Result<SpecTerm, String> {
        match self.peek() {
            Some('<') => self.iriref().map(SpecTerm::Iri),
            Some('_') => self.blank_node_label(),
            _ => Err(self.fault("a graph label must be an IRI or a blank node label")),
        }
    }

    /// `literal ::= STRING_LITERAL_QUOTE ('^^' IRIREF | LANGTAG)?`
    fn literal(&mut self) -> Result<SpecTerm, String> {
        let lexical = self.string_literal_quote()?;

        let tag = if self.eat('@') {
            LiteralTag::Language(self.langtag()?)
        } else if matches!(self.peek(), Some('^')) {
            self.pos += 1;
            if !self.eat('^') {
                return Err(self.fault("a datatype is introduced by '^^'"));
            }
            if !matches!(self.peek(), Some('<')) {
                return Err(self.fault("a datatype must be an IRI"));
            }
            LiteralTag::Datatype(self.iriref()?)
        } else {
            LiteralTag::Simple
        };

        Ok(SpecTerm::Literal { lexical, tag })
    }

    /// `IRIREF ::= '<' ([^#x00-#x20<>"{}|^`\] | UCHAR)* '>'`
    ///
    /// Note what the character class excludes: an IRI may not contain a raw space, and may not
    /// contain a raw backslash. Both are common ways for a serialiser to produce a document that
    /// its own permissive reader accepts and a strict one does not.
    ///
    /// Absoluteness is checked here too. It is not in the EBNF — it is stated normatively in
    /// [N-Triples §2.2], "IRIs may be written only as absolute IRIs" — and it is the one an
    /// export is most likely to get wrong, because a relative IRI resolves against the document's
    /// own location and so means something different to every consumer that fetches it.
    ///
    /// [N-Triples §2.2]: https://www.w3.org/TR/n-triples/#sec-iri
    fn iriref(&mut self) -> Result<String, String> {
        if !self.eat('<') {
            return Err(self.fault("an IRI starts with '<'"));
        }

        let mut iri = String::new();
        loop {
            let Some(c) = self.bump() else {
                return Err(self.fault("the document ended inside an IRI"));
            };
            match c {
                '>' => break,
                '\\' => iri.push(self.uchar()?),
                c if c <= '\u{20}'
                    || matches!(c, '<' | '>' | '"' | '{' | '}' | '|' | '^' | '`') =>
                {
                    return Err(self.fault(&format!(
                        "U+{:04X} is excluded from IRIREF by the grammar",
                        c as u32
                    )))
                }
                c => iri.push(c),
            }
        }

        if !is_absolute(&iri) {
            return Err(self.fault(&format!(
                "<{iri}> is relative, and N-Triples §2.2 permits only absolute IRIs"
            )));
        }

        Ok(iri)
    }

    /// `STRING_LITERAL_QUOTE ::= '"' ([^#x22#x5C#xA#xD] | ECHAR | UCHAR)* '"'`,
    /// with `ECHAR ::= '\' [tbnrf"\]`.
    ///
    /// Returns the lexical form — the characters between the delimiters *after* processing escapes
    /// ([N-Triples §2.3]).
    ///
    /// [N-Triples §2.3]: https://www.w3.org/TR/n-triples/#sec-literals
    fn string_literal_quote(&mut self) -> Result<String, String> {
        if !self.eat('"') {
            return Err(self.fault("a literal starts with '\"'"));
        }

        let mut lexical = String::new();
        loop {
            let Some(c) = self.bump() else {
                return Err(self.fault("the document ended inside a literal"));
            };
            match c {
                '"' => break,
                '\n' | '\r' => return Err(self.fault(
                    "a raw line break is excluded from STRING_LITERAL_QUOTE; it must be escaped",
                )),
                '\\' => {
                    let Some(escape) = self.peek() else {
                        return Err(self.fault("the document ended inside an escape"));
                    };
                    match escape {
                        // Canonical N-Triples §4 permits ECHAR only for U+0022, U+005C, U+000A and
                        // U+000D — every other character it covers is legal directly, so escaping
                        // it is a departure from the canonical form even though it is valid
                        // N-Triples.
                        't' | 'b' | 'f' => {
                            self.violate(&format!(
                                "\\{escape} escapes a character that STRING_LITERAL_QUOTE allows \
                                 directly, which Canonical N-Triples §4 forbids"
                            ));
                            self.pos += 1;
                            lexical.push(match escape {
                                't' => '\t',
                                'b' => '\u{8}',
                                _ => '\u{c}',
                            });
                        }
                        'n' | 'r' | '"' | '\\' => {
                            self.pos += 1;
                            lexical.push(match escape {
                                'n' => '\n',
                                'r' => '\r',
                                '"' => '"',
                                _ => '\\',
                            });
                        }
                        'u' | 'U' => lexical.push(self.uchar()?),
                        other => {
                            let echar = "ECHAR's escapes (t b n r f \" \\) or a UCHAR";
                            return Err(self.fault(&format!("\\{other} is not one of {echar}")));
                        }
                    }
                }
                c => lexical.push(c),
            }
        }

        Ok(lexical)
    }

    /// `UCHAR ::= '\u' HEX HEX HEX HEX | '\U' HEX HEX HEX HEX HEX HEX HEX HEX`, called with the
    /// leading `\` already consumed.
    fn uchar(&mut self) -> Result<char, String> {
        let width = match self.bump() {
            Some('u') => 4,
            Some('U') => 8,
            _ => return Err(self.fault("a '\\' inside an IRI must introduce a UCHAR")),
        };

        // Canonical N-Triples §4: "Characters MUST NOT be represented by UCHAR." The whole point
        // of that rule is that a document is UTF-8, so an escape is never needed and two documents
        // denoting the same graph should not differ in whether they used one.
        self.violate("a UCHAR escape, which Canonical N-Triples §4 forbids");

        let mut digits = String::new();
        for _ in 0..width {
            let Some(digit) = self.bump().filter(char::is_ascii_hexdigit) else {
                return Err(self.fault("a UCHAR needs its full complement of hex digits"));
            };
            if digit.is_ascii_lowercase() {
                self.violate("a lower-case hex digit, where Canonical N-Triples §4 requires [A-F]");
            }
            digits.push(digit);
        }

        u32::from_str_radix(&digits, 16)
            .ok()
            .and_then(char::from_u32)
            .ok_or_else(|| self.fault(&format!("\\u{digits} is not a Unicode code point")))
    }

    /// `LANGTAG ::= '@' [a-zA-Z]+ ('-' [a-zA-Z0-9]+)*`, called with the `@` already consumed.
    fn langtag(&mut self) -> Result<String, String> {
        let mut tag = String::new();
        while matches!(self.peek(), Some(c) if c.is_ascii_alphabetic()) {
            tag.push(self.chars[self.pos]);
            self.pos += 1;
        }
        if tag.is_empty() {
            return Err(self.fault("a language tag needs at least one letter after '@'"));
        }

        while matches!(self.peek(), Some('-')) {
            self.pos += 1;
            tag.push('-');
            let mut subtag = 0;
            while matches!(self.peek(), Some(c) if c.is_ascii_alphanumeric()) {
                tag.push(self.chars[self.pos]);
                self.pos += 1;
                subtag += 1;
            }
            if subtag == 0 {
                return Err(self.fault("a language subtag needs at least one character after '-'"));
            }
        }

        Ok(tag)
    }

    /// `BLANK_NODE_LABEL ::= '_:' (PN_CHARS_U | [0-9]) ((PN_CHARS | '.')* PN_CHARS)?`
    ///
    /// The trailing-`PN_CHARS` clause is the subtle one: a label may contain a `.` but may not
    /// *end* with one, because that dot would be indistinguishable from the statement terminator.
    fn blank_node_label(&mut self) -> Result<SpecTerm, String> {
        if !self.eat('_') || !self.eat(':') {
            return Err(self.fault("a blank node label starts with '_:'"));
        }

        let mut label = String::new();
        match self.peek() {
            Some(c) if is_pn_chars_u(c) || c.is_ascii_digit() => {
                label.push(c);
                self.pos += 1;
            }
            _ => {
                return Err(self
                    .fault("a blank node label's first character must be PN_CHARS_U or a digit"))
            }
        }

        // A '.' is only part of the label if a PN_CHARS follows it, so look ahead rather than
        // consuming greedily and backing up.
        loop {
            match self.peek() {
                Some(c) if is_pn_chars(c) => {
                    label.push(c);
                    self.pos += 1;
                }
                Some('.') if matches!(self.chars.get(self.pos + 1), Some(&c) if is_pn_chars(c) || c == '.') =>
                {
                    label.push('.');
                    self.pos += 1;
                }
                _ => break,
            }
        }

        if label.ends_with('.') {
            return Err(self.fault("a blank node label may not end with '.'"));
        }

        Ok(SpecTerm::Blank(label))
    }

    /// White space and comments, at a position where the grammar allows them.
    ///
    /// `position` names the thing this white space *follows*, so a canonical violation reads as a
    /// place in the document rather than as an offset.
    fn separator(&mut self, position: &str) {
        let seen = self.inline_layout();
        if seen != " " {
            self.violate(&format!(
                "{seen:?} {position}, where Canonical N-Triples §4 requires a single space"
            ));
        }
    }

    /// White space at a position where the canonical form requires there to be none.
    fn layout(&mut self, position: &str) {
        let seen = self.inline_layout();
        if !seen.is_empty() {
            self.violate(&format!(
                "{seen:?} {position}, where Canonical N-Triples §4 requires no white space"
            ));
        }
    }

    /// Consume spaces, tabs, and comments, returning the white space seen.
    ///
    /// N-Quads §4: a comment runs from `#` to the end of the line and is treated as white space.
    /// It is *legal*, so this does not fail; it is not canonical, so it is recorded.
    fn inline_layout(&mut self) -> String {
        let mut seen = String::new();
        loop {
            match self.peek() {
                Some(c @ (' ' | '\t')) => {
                    seen.push(c);
                    self.pos += 1;
                }
                Some('#') => {
                    self.violate("a comment, which Canonical N-Triples §4 forbids");
                    while !matches!(self.peek(), None | Some('\n') | Some('\r')) {
                        self.pos += 1;
                    }
                }
                _ => return seen,
            }
        }
    }

    /// `EOL ::= [#xD#xA]+` — note the `+`: a run of blank lines is one EOL, not several.
    fn eol(&mut self) {
        while matches!(self.peek(), Some('\n' | '\r')) {
            self.pos += 1;
        }
    }

    fn at_eol(&self) -> bool {
        matches!(self.peek(), Some('\n' | '\r'))
    }

    fn at_end(&self) -> bool {
        self.pos >= self.chars.len()
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek();
        if c.is_some() {
            self.pos += 1;
        }
        c
    }

    fn eat(&mut self, expected: char) -> bool {
        if self.peek() == Some(expected) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn violate(&mut self, what: &str) {
        let at = self.pos;
        self.canonical.push(format!("at character {at}: {what}"));
    }

    fn fault(&self, why: &str) -> String {
        let context: String = self
            .chars
            .iter()
            .skip(self.pos.saturating_sub(20))
            .take(40)
            .collect();
        format!("at character {}: {why} (near {context:?})", self.pos)
    }
}

/// Whether an IRI is absolute — it begins with a scheme, per RFC 3986's `scheme` production.
///
/// This is not a full IRI validator and does not pretend to be; it is the one distinction
/// [N-Triples §2.2] draws, which is the one an export can plausibly get wrong.
///
/// [N-Triples §2.2]: https://www.w3.org/TR/n-triples/#sec-iri
fn is_absolute(iri: &str) -> bool {
    let Some((scheme, _)) = iri.split_once(':') else {
        return false;
    };
    let mut characters = scheme.chars();
    matches!(characters.next(), Some(c) if c.is_ascii_alphabetic())
        && characters.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
}

/// `PN_CHARS_BASE ::= [A-Z] | [a-z] | [#x00C0-#x00D6] | …`, transcribed from N-Quads §4.
fn is_pn_chars_base(c: char) -> bool {
    matches!(c,
        'A'..='Z' | 'a'..='z'
        | '\u{00C0}'..='\u{00D6}' | '\u{00D8}'..='\u{00F6}' | '\u{00F8}'..='\u{02FF}'
        | '\u{0370}'..='\u{037D}' | '\u{037F}'..='\u{1FFF}' | '\u{200C}'..='\u{200D}'
        | '\u{2070}'..='\u{218F}' | '\u{2C00}'..='\u{2FEF}' | '\u{3001}'..='\u{D7FF}'
        | '\u{F900}'..='\u{FDCF}' | '\u{FDF0}'..='\u{FFFD}' | '\u{10000}'..='\u{EFFFF}')
}

/// `PN_CHARS_U ::= PN_CHARS_BASE | '_' | ':'`
fn is_pn_chars_u(c: char) -> bool {
    is_pn_chars_base(c) || c == '_' || c == ':'
}

/// `PN_CHARS ::= PN_CHARS_U | '-' | [0-9] | #x00B7 | [#x0300-#x036F] | [#x203F-#x2040]`
fn is_pn_chars(c: char) -> bool {
    is_pn_chars_u(c)
        || c == '-'
        || c.is_ascii_digit()
        || c == '\u{00B7}'
        || matches!(c, '\u{0300}'..='\u{036F}' | '\u{203F}'..='\u{2040}')
}

/// Read `document` against `dialect`, requiring it to be a document of that language.
fn read(document: &str, dialect: Dialect) -> Vec<SpecStatement> {
    let mut reader = SpecReader::new(document, dialect);
    let statements = reader
        .document()
        .unwrap_or_else(|error| panic!("our own output is not a {dialect:?} document: {error}"));
    statements
        .into_iter()
        .map(SpecStatement::normalised)
        .collect()
}

/// Every departure of `document` from Canonical N-Triples §4.
fn canonical_violations(document: &str) -> Vec<String> {
    let mut reader = SpecReader::new(document, Dialect::NTriples);
    reader.document().unwrap_or_else(|error| {
        panic!("canonical form is only defined for a valid document: {error}")
    });
    reader.canonical
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxigraph::model::vocab::xsd;
    use oxigraph::model::BlankNode;
    use std::collections::BTreeSet;

    fn temp_dir() -> tempfile::TempDir {
        tempfile::tempdir().expect("a temporary directory")
    }

    fn vocabulary(iri: &str) -> GraphId {
        GraphId::vocabulary(iri).expect("a valid absolute IRI outside the reserved namespace")
    }

    fn iri(iri: &str) -> NamedNode {
        NamedNode::new_unchecked(iri)
    }

    // ---------------------------------------------------------------------------------------
    // The fixtures
    // ---------------------------------------------------------------------------------------

    /// The graph holding [N-Triples Example 3] verbatim.
    ///
    /// [N-Triples Example 3]: https://www.w3.org/TR/n-triples/#sec-literals
    const SPEC_GRAPH: &str = "http://example.org/spec/n-triples-2-3";

    /// A graph exercising the two productions Example 3 does not: `BLANK_NODE_LABEL`, and an IRI
    /// carrying a percent-encoded space.
    const EXTRA_GRAPH: &str = "http://acme.example/v/finance";

    const SHOW: &str = "http://example.org/show/218";
    const RDFS_LABEL: &str = "http://www.w3.org/2000/01/rdf-schema#label";
    const LOCAL_NAME: &str = "http://example.org/show/localName";
    const SPIDERMAN: &str = "http://example.org/#spiderman";
    const TEXT: &str = "http://example.org/text";
    const HELIUM: &str = "http://en.wikipedia.org/wiki/Helium";
    const ATOMIC_NUMBER: &str = "http://example.org/elements/atomicNumber";
    const SPECIFIC_GRAVITY: &str = "http://example.org/elements/specificGravity";
    const XSD_INTEGER: &str = "http://www.w3.org/2001/XMLSchema#integer";
    const XSD_DOUBLE: &str = "http://www.w3.org/2001/XMLSchema#double";

    /// The lexical form of Example 3's multi-line literal, written as Rust rather than as
    /// N-Triples so that the escaping under test is not also the escaping of the expectation.
    ///
    /// Its value in the specification is
    /// `"This is a multi-line\nliteral with many quotes (\"\"\"\"\")\nand two apostrophes ('')."`,
    /// which covers three things at once: the two characters `ECHAR` must encode in canonical form
    /// (`"` and the line feed), and one it must *not* (the apostrophe, which
    /// `STRING_LITERAL_QUOTE` allows directly).
    const MULTI_LINE: &str =
        "This is a multi-line\nliteral with many quotes (\"\"\"\"\")\nand two apostrophes ('').";

    /// The specification's own example, as terms.
    ///
    /// Seven rows, six triples: the first two rows of Example 3 are annotated "same as above" in
    /// the specification, because [RDF 1.1] gives a simple literal the datatype `xsd:string`. That
    /// they collapse is asserted by `the_specifications_two_spellings_of_a_simple_literal_are_one_triple`
    /// rather than assumed here.
    ///
    /// [RDF 1.1]: https://www.w3.org/TR/n-triples/#sec-literals
    fn spec_example_3() -> Vec<(NamedNode, NamedNode, Term)> {
        vec![
            (
                iri(SHOW),
                iri(RDFS_LABEL),
                Literal::new_typed_literal("That Seventies Show", xsd::STRING).into(),
            ),
            (
                iri(SHOW),
                iri(RDFS_LABEL),
                Literal::new_simple_literal("That Seventies Show").into(),
            ),
            (
                iri(SHOW),
                iri(LOCAL_NAME),
                Literal::new_language_tagged_literal_unchecked("That Seventies Show", "en").into(),
            ),
            (
                iri(SHOW),
                iri(LOCAL_NAME),
                Literal::new_language_tagged_literal_unchecked(
                    "Cette Série des Années Septante",
                    "fr-be",
                )
                .into(),
            ),
            (
                iri(SPIDERMAN),
                iri(TEXT),
                Literal::new_simple_literal(MULTI_LINE).into(),
            ),
            (
                iri(HELIUM),
                iri(ATOMIC_NUMBER),
                Literal::new_typed_literal("2", xsd::INTEGER).into(),
            ),
            (
                iri(HELIUM),
                iri(SPECIFIC_GRAVITY),
                Literal::new_typed_literal("1.663E-4", xsd::DOUBLE).into(),
            ),
        ]
    }

    /// The lexical form the specification writes for the `xsd:double`, and the one the store
    /// hands back instead.
    ///
    /// These are **different RDF terms** — [RDF 1.1] defines a literal as the pair of a lexical
    /// form and a datatype IRI, so `"1.663E-4"` and `"0.0001663"` are no more the same term than
    /// two different strings are. The rewrite is the store's, not the serialiser's, and it is a
    /// defect rather than a freedom the specification grants; it is named here so that every test
    /// that has to accommodate it says why, and so that fixing it turns those tests red.
    /// `the_store_rewrites_the_lexical_form_of_the_datatypes_it_models_natively` is where it is
    /// measured, and `docs/UNTESTED.md` is where it is owned.
    ///
    /// [RDF 1.1]: https://www.w3.org/TR/rdf11-concepts/#section-Graph-Literal
    const SPEC_DOUBLE: &str = "1.663E-4";
    const STORED_DOUBLE: &str = "0.0001663";

    /// The same six triples, expressed against the grammar's term model, **as the store returns
    /// them** — which for one of the six is not what the specification wrote. See `SPEC_DOUBLE`.
    fn expected_spec_example_3() -> BTreeSet<SpecStatement> {
        [
            SpecStatement::triple(
                SpecTerm::iri(SHOW),
                SpecTerm::iri(RDFS_LABEL),
                SpecTerm::simple("That Seventies Show"),
            ),
            SpecStatement::triple(
                SpecTerm::iri(SHOW),
                SpecTerm::iri(LOCAL_NAME),
                SpecTerm::language("That Seventies Show", "en"),
            ),
            SpecStatement::triple(
                SpecTerm::iri(SHOW),
                SpecTerm::iri(LOCAL_NAME),
                SpecTerm::language("Cette Série des Années Septante", "fr-be"),
            ),
            SpecStatement::triple(
                SpecTerm::iri(SPIDERMAN),
                SpecTerm::iri(TEXT),
                SpecTerm::simple(MULTI_LINE),
            ),
            SpecStatement::triple(
                SpecTerm::iri(HELIUM),
                SpecTerm::iri(ATOMIC_NUMBER),
                SpecTerm::typed("2", XSD_INTEGER),
            ),
            SpecStatement::triple(
                SpecTerm::iri(HELIUM),
                SpecTerm::iri(SPECIFIC_GRAVITY),
                SpecTerm::typed(STORED_DOUBLE, XSD_DOUBLE),
            ),
        ]
        .into_iter()
        .collect()
    }

    const CONCEPT: &str = "http://acme.example/v/finance#Derivative";
    const RELATED: &str = "http://www.w3.org/2004/02/skos/core#related";
    const EXACT_MATCH: &str = "http://www.w3.org/2004/02/skos/core#exactMatch";
    const PREF_LABEL: &str = "http://www.w3.org/2004/02/skos/core#prefLabel";
    const PERCENT_ENCODED: &str = "http://other.example/scheme/a%20b";
    const SCOPE_NOTE: &str = "http://www.w3.org/2004/02/skos/core#scopeNote";

    /// The pair of characters Canonical N-Triples §4 treats in *opposite* ways, which is what
    /// makes them the sharpest test of the canonical claim. A carriage return is one of the four
    /// characters §4 permits `ECHAR` to encode; a tab is not, and `STRING_LITERAL_QUOTE` admits it
    /// directly, so writing `\t` for it is a violation. A serialiser that escapes "anything
    /// unprintable" gets the tab wrong, and a serialiser that escapes nothing gets the carriage
    /// return wrong — the grammar excludes a raw #xD from a literal outright.
    const WHITESPACE_NOTE: &str = "a tab:\there, a carriage return:\rhere, a line feed:\nhere";

    fn extra_statements() -> Vec<(NamedNode, NamedNode, Term)> {
        vec![
            (
                iri(CONCEPT),
                iri(RELATED),
                BlankNode::new_unchecked("unnamedConcept").into(),
            ),
            (iri(CONCEPT), iri(EXACT_MATCH), iri(PERCENT_ENCODED).into()),
            (
                iri(CONCEPT),
                iri(PREF_LABEL),
                Literal::new_language_tagged_literal_unchecked("金融派生商品 🧾", "ja").into(),
            ),
            (
                iri(CONCEPT),
                iri(SCOPE_NOTE),
                Literal::new_simple_literal(WHITESPACE_NOTE).into(),
            ),
        ]
    }

    fn expected_extra() -> BTreeSet<SpecStatement> {
        [
            SpecStatement::triple(
                SpecTerm::iri(CONCEPT),
                SpecTerm::iri(RELATED),
                SpecTerm::Blank("anonymous".to_owned()),
            ),
            SpecStatement::triple(
                SpecTerm::iri(CONCEPT),
                SpecTerm::iri(EXACT_MATCH),
                SpecTerm::iri(PERCENT_ENCODED),
            ),
            SpecStatement::triple(
                SpecTerm::iri(CONCEPT),
                SpecTerm::iri(PREF_LABEL),
                SpecTerm::language("金融派生商品 🧾", "ja"),
            ),
            SpecStatement::triple(
                SpecTerm::iri(CONCEPT),
                SpecTerm::iri(SCOPE_NOTE),
                SpecTerm::simple(WHITESPACE_NOTE),
            ),
        ]
        .into_iter()
        .collect()
    }

    fn populated_store(dir: &tempfile::TempDir) -> Store {
        let store = Store::open(dir.path()).expect("a fresh store opens");
        for (graph_iri, statements) in [
            (SPEC_GRAPH, spec_example_3()),
            (EXTRA_GRAPH, extra_statements()),
        ] {
            let graph = vocabulary(graph_iri);
            store
                .transaction(|txn| {
                    txn.create_vocabulary_graph(&graph)?;
                    txn.insert(&graph, statements)
                })
                .expect("a fresh vocabulary takes its statements");
        }
        store
    }

    fn export(store: &Store, iri: &str, syntax: RdfSyntax) -> String {
        let mut bytes = Vec::new();
        store
            .export_graph(iri, syntax, &mut bytes)
            .unwrap_or_else(|error| panic!("exporting {iri} as {syntax}: {error}"));
        String::from_utf8(bytes).expect("an RDF document is UTF-8")
    }

    // ---------------------------------------------------------------------------------------
    // What the exports must satisfy
    // ---------------------------------------------------------------------------------------

    /// The claim `CLAUDE.md` §2 makes about N-Triples, tested against the grammar rather than
    /// against our own parser. A writer that emitted a raw newline inside a literal, a bare space
    /// inside an IRI, or a language tag with an empty subtag would round-trip through Oxigraph
    /// perfectly and fail here.
    #[test]
    fn our_n_triples_export_is_a_document_of_the_published_grammar() {
        let dir = temp_dir();
        let store = populated_store(&dir);

        for (graph, expected) in [
            (SPEC_GRAPH, expected_spec_example_3()),
            (EXTRA_GRAPH, expected_extra()),
        ] {
            let document = export(&store, graph, RdfSyntax::NTriples);
            let recovered: BTreeSet<_> = read(&document, Dialect::NTriples).into_iter().collect();

            assert_eq!(
                recovered, expected,
                "reading our own N-Triples for {graph} against the grammar"
            );
        }
    }

    /// N-Quads is the same grammar with `graphLabel`, and it is the only one of the six in which a
    /// whole store could be backed up without losing which vocabulary a statement belongs to — so
    /// the label actually being there, and being the vocabulary's IRI, is the property that makes
    /// that true.
    #[test]
    fn our_n_quads_export_is_a_document_of_the_published_grammar_and_names_the_graph() {
        let dir = temp_dir();
        let store = populated_store(&dir);

        for (graph, triples) in [
            (SPEC_GRAPH, expected_spec_example_3()),
            (EXTRA_GRAPH, expected_extra()),
        ] {
            let document = export(&store, graph, RdfSyntax::NQuads);
            let recovered: BTreeSet<_> = read(&document, Dialect::NQuads).into_iter().collect();
            let expected: BTreeSet<_> = triples
                .into_iter()
                .map(|statement| statement.in_graph(graph))
                .collect();

            assert_eq!(
                recovered, expected,
                "reading our own N-Quads for {graph} against the grammar"
            );
        }
    }

    /// Canonical N-Triples §4 is a *layout* conformance level, and it is the one that decides
    /// whether two tools serialising the same graph produce the same bytes — which is what makes a
    /// vocabulary diffable in git (`CLAUDE.md` §1, "vocabularies are code"). A writer that emits
    /// `\u00E9` for an accented letter, or pads with two spaces, is still valid N-Triples and
    /// still ruins a diff.
    ///
    /// **We are one constraint short of canonical, and this test says which.** §4 requires that
    /// `ECHAR` be used only for U+0022, U+005C, U+000A and U+000D — "ECHAR MUST NOT be used for
    /// characters that are allowed directly in `STRING_LITERAL_QUOTE`" — and a tab is allowed
    /// directly. Our writer escapes it as `\t` anyway. Nothing is *lost*: the document is valid
    /// N-Triples and any reader recovers the same term, which is why the round-trip tests never
    /// saw it. What is lost is the ability to claim canonical form, and with it the guarantee
    /// that two tools serialising one graph agree byte-for-byte.
    ///
    /// Pinned rather than papered over: the fixture keeps the tab, and this test requires exactly
    /// the one known violation and no other. Owned in `docs/UNTESTED.md`; the work is in
    /// `docs/PROPOSED.md`.
    #[test]
    fn our_n_triples_export_is_canonical_n_triples_but_for_one_known_violation() {
        let dir = temp_dir();
        let store = populated_store(&dir);

        let clean = export(&store, SPEC_GRAPH, RdfSyntax::NTriples);
        assert_eq!(
            canonical_violations(&clean),
            Vec::<String>::new(),
            "our N-Triples for {SPEC_GRAPH} departs from Canonical N-Triples §4:\n{clean}"
        );

        // The other fixture is the one carrying a tab.
        let escaped = export(&store, EXTRA_GRAPH, RdfSyntax::NTriples);
        let violations = canonical_violations(&escaped);

        assert_eq!(
            violations.len(),
            1,
            "exactly one departure from §4 is known; anything else is new:\n{escaped}"
        );
        assert!(
            violations[0].contains("\\t escapes a character that STRING_LITERAL_QUOTE allows"),
            "the known departure is the escaped tab, and this is something else: {violations:?}"
        );
        assert!(
            escaped.contains(r"a tab:\there"),
            "if the tab is now written raw the defect is fixed — delete this exception and strike \
             the entry in docs/UNTESTED.md:\n{escaped}"
        );
    }

    /// The specification annotates the first two rows of Example 3 "same as above", because
    /// [RDF 1.1] says a simple literal's datatype *is* `xsd:string`. Two rows in, one triple out.
    /// If the store ever held both, an export would contain a duplicate that no consumer could
    /// tell from a modelling mistake.
    ///
    /// [RDF 1.1]: https://www.w3.org/TR/n-triples/#sec-literals
    #[test]
    fn the_specifications_two_spellings_of_a_simple_literal_are_one_triple() {
        let dir = temp_dir();
        let store = populated_store(&dir);

        let document = export(&store, SPEC_GRAPH, RdfSyntax::NTriples);
        let statements = read(&document, Dialect::NTriples);

        assert_eq!(
            spec_example_3().len(),
            7,
            "the fixture is the specification's seven rows"
        );
        assert_eq!(
            statements.len(),
            6,
            "the two spellings of \"That Seventies Show\" denote one triple"
        );
    }

    /// The strongest form the check can take: our bytes, against the specification's bytes.
    ///
    /// Example 3 is published as N-Triples, so its lines are exactly what a conforming writer
    /// given those terms may produce — modulo the one freedom the specification itself names, the
    /// two spellings of a simple literal. Everything else is pinned: which characters are escaped,
    /// which are not, where the spaces go.
    #[test]
    fn the_specifications_own_example_is_written_the_way_the_specification_writes_it() {
        let dir = temp_dir();
        let store = populated_store(&dir);

        let document = export(&store, SPEC_GRAPH, RdfSyntax::NTriples);
        let written: BTreeSet<&str> = document.lines().collect();

        // Transcribed from https://www.w3.org/TR/n-triples/#sec-literals, with the trailing
        // comments removed because Canonical N-Triples §4 forbids them.
        let published = [
            concat!(
                r#"<http://example.org/show/218> <http://www.w3.org/2000/01/rdf-schema#label> "#,
                r#""That Seventies Show" ."#
            ),
            concat!(
                r#"<http://example.org/show/218> <http://example.org/show/localName> "#,
                r#""That Seventies Show"@en ."#
            ),
            concat!(
                r#"<http://example.org/show/218> <http://example.org/show/localName> "#,
                r#""Cette Série des Années Septante"@fr-be ."#
            ),
            concat!(
                r#"<http://example.org/#spiderman> <http://example.org/text> "#,
                r#""This is a multi-line\nliteral with many quotes (\"\"\"\"\")\n"#,
                r#"and two apostrophes ('')." ."#
            ),
            concat!(
                r#"<http://en.wikipedia.org/wiki/Helium> "#,
                r#"<http://example.org/elements/atomicNumber> "#,
                r#""2"^^<http://www.w3.org/2001/XMLSchema#integer> ."#
            ),
            concat!(
                r#"<http://en.wikipedia.org/wiki/Helium> "#,
                r#"<http://example.org/elements/specificGravity> "#,
                r#""1.663E-4"^^<http://www.w3.org/2001/XMLSchema#double> ."#
            ),
        ];

        // The one documented freedom: the specification prints the `rdfs:label` triple twice, in
        // the two spellings it calls equivalent, so either is conformant here.
        let long_form = concat!(
            r#"<http://example.org/show/218> <http://www.w3.org/2000/01/rdf-schema#label> "#,
            r#""That Seventies Show"^^<http://www.w3.org/2001/XMLSchema#string> ."#
        );

        // The one place we do *not* write what the specification writes, and it is a defect, not a
        // freedom — the store rewrites the lexical form (see `SPEC_DOUBLE`). Recorded here as an
        // exception with a name rather than by quietly editing the expectation, so the assertion
        // below still says what the specification says.
        let mut divergences = 0;

        for line in published {
            if line.contains(SPEC_DOUBLE) {
                assert!(
                    !written.contains(line),
                    "the store no longer rewrites {SPEC_DOUBLE:?} — the defect is fixed. Delete \
                     this exception, drop SPEC_DOUBLE/STORED_DOUBLE, and strike the entry in \
                     docs/UNTESTED.md."
                );
                assert!(
                    written.contains(line.replace(SPEC_DOUBLE, STORED_DOUBLE).as_str()),
                    "we wrote neither the specification's line nor the known rewrite of it:\n{}",
                    document.trim_end()
                );
                divergences += 1;
                continue;
            }

            let ok = written.contains(line)
                || (line.contains("rdf-schema#label") && written.contains(long_form));
            assert!(
                ok,
                "the specification publishes\n  {line}\nand we wrote\n  {}",
                document.trim_end()
            );
        }

        assert_eq!(divergences, 1, "exactly one line is expected to diverge");
        assert_eq!(
            written.len(),
            published.len(),
            "we wrote lines the specification's example does not contain:\n{document}"
        );
    }

    /// **A defect, recorded as a test rather than as prose.**
    ///
    /// The store does not preserve the lexical form of a literal whose datatype it models
    /// natively. It decodes the lexical form into a value on the way in and re-renders it on the
    /// way out, so what comes back is a *different RDF term* from the one that was written —
    /// [RDF 1.1] defines a literal as the pair (lexical form, datatype IRI), and `"007"` and
    /// `"7"` are as different a pair as two different strings are.
    ///
    /// Three things make this worse than "the store normalises numbers".
    ///
    /// 1. **It is silent.** Nothing in the API, the export, or the interface says it happened.
    ///    That is the exact failure mode `RdfSyntax::records_graph_names` exists to prevent for a
    ///    different kind of loss, and the charter's wedge is built on not committing it.
    /// 2. **What survives is the ill-typed value.** `"abc"^^xsd:nonNegativeInteger` — which is not
    ///    a `nonNegativeInteger` at all — comes back byte-for-byte, and `"007"^^xsd:integer`,
    ///    which is perfectly well-typed, does not. The store is faithful to the data it cannot
    ///    interpret and lossy with the data it can, which is precisely backwards from what a
    ///    governance team would assume, and is not a rule we control or publish.
    /// 3. **Zero-padded codes are the normal case in this market.** A `skos:notation`
    ///    `"007"^^xsd:integer` is exactly what an enterprise classification scheme carries, and
    ///    it comes back as `"7"`.
    ///
    /// This test pins the behaviour so it cannot change unnoticed in either direction. It is
    /// owned in `docs/UNTESTED.md` and the work to fix it is in `docs/PROPOSED.md`.
    ///
    /// [RDF 1.1]: https://www.w3.org/TR/rdf11-concepts/#section-Graph-Literal
    #[test]
    fn the_store_rewrites_the_lexical_form_of_the_datatypes_it_models_natively() {
        const XSD: &str = "http://www.w3.org/2001/XMLSchema#";
        let xsd = |name: &str| format!("{XSD}{name}");

        // (datatype, what was written, what comes back)
        let rewritten = [
            (xsd("double"), "1.663E-4", "0.0001663"),
            (xsd("float"), "1.0E1", "10"),
            (xsd("integer"), "007", "7"),
            (xsd("integer"), "+7", "7"),
            (xsd("nonNegativeInteger"), "007", "7"),
            (xsd("decimal"), "4.00", "4"),
            (xsd("boolean"), "1", "true"),
            (
                xsd("dateTime"),
                "2026-08-19T00:00:00+00:00",
                "2026-08-19T00:00:00Z",
            ),
        ];

        // The control, without which this test would be consistent with a store that mangles
        // every literal — a different and larger claim. Two of these are the interesting ones: a
        // datatype the engine has never heard of survives, and so does a value that is *invalid*
        // for a datatype the engine does know. Fidelity is what is left over when interpretation
        // fails.
        let preserved = [
            (xsd("string"), "0007"),
            (xsd("integer"), "7"),
            (xsd("decimal"), "4.002602"),
            (xsd("nonNegativeInteger"), "abc"),
            ("http://acme.example/datatype/ProductCode".to_owned(), "007"),
        ];

        let dir = temp_dir();
        let store = Store::open(dir.path()).expect("a fresh store opens");
        let graph = vocabulary("http://acme.example/v/lexical");

        let subject = |index: usize| iri(&format!("http://acme.example/v/lexical#s{index}"));
        let predicate = iri("http://example.org/p");
        let typed = |lexical: &str, datatype: &str| -> Term {
            Literal::new_typed_literal(lexical, NamedNode::new_unchecked(datatype)).into()
        };

        let mut triples = Vec::new();
        for (index, (datatype, written, _)) in rewritten.iter().enumerate() {
            triples.push((subject(index), predicate.clone(), typed(written, datatype)));
        }
        for (offset, (datatype, written)) in preserved.iter().enumerate() {
            let index = rewritten.len() + offset;
            triples.push((subject(index), predicate.clone(), typed(written, datatype)));
        }

        store
            .transaction(|txn| {
                txn.create_vocabulary_graph(&graph)?;
                txn.insert(&graph, triples)
            })
            .expect("the vocabulary takes its statements");

        let document = export(&store, graph.iri(), RdfSyntax::NTriples);
        let statements = read(&document, Dialect::NTriples);
        let lexical_at = |index: usize| -> String {
            let wanted = SpecTerm::iri(&format!("http://acme.example/v/lexical#s{index}"));
            statements
                .iter()
                .find(|statement| statement.subject == wanted)
                .and_then(|statement| match &statement.object {
                    SpecTerm::Literal { lexical, .. } => Some(lexical.clone()),
                    _ => None,
                })
                .unwrap_or_else(|| panic!("no literal came back for s{index}:\n{document}"))
        };

        for (index, (datatype, written, expected)) in rewritten.iter().enumerate() {
            assert_eq!(
                &lexical_at(index),
                expected,
                "xsd:{datatype} {written:?} — if this now comes back unchanged the defect is \
                 fixed, and docs/UNTESTED.md and docs/PROPOSED.md both need striking"
            );
            assert_ne!(
                written, expected,
                "a row that rewrites to itself proves nothing"
            );
        }

        for (offset, (datatype, written)) in preserved.iter().enumerate() {
            assert_eq!(
                &lexical_at(rewritten.len() + offset),
                written,
                "xsd:{datatype} {written:?} was expected to survive untouched"
            );
        }
    }

    /// Whether the loss above is the *export's* or the *store's*, answered rather than assumed.
    ///
    /// It matters which. If only the export rewrote lexical forms then the store would still hold
    /// the term that was written, and a fix would be a serialiser change with no data migration. It
    /// does not: a `CONSTRUCT` over the same graph, which never touches `export_graph`, returns the
    /// rewritten form too. So the loss is in the term encoding, everything that reads the store
    /// inherits it, and any fix has to touch stored data.
    #[test]
    fn the_rewrite_is_the_stores_and_not_the_exports() {
        let dir = temp_dir();
        let store = Store::open(dir.path()).expect("a fresh store opens");
        let graph = vocabulary("http://acme.example/v/via-sparql");
        let subject = iri("http://acme.example/v/via-sparql#Concept");
        let notation = iri("http://www.w3.org/2004/02/skos/core#notation");

        store
            .transaction(|txn| {
                txn.create_vocabulary_graph(&graph)?;
                txn.insert(
                    &graph,
                    vec![(
                        subject.clone(),
                        notation.clone(),
                        Literal::new_typed_literal(
                            "007",
                            NamedNode::new_unchecked("http://www.w3.org/2001/XMLSchema#integer"),
                        )
                        .into(),
                    )],
                )
            })
            .expect("the vocabulary takes its statement");

        let mut answer = Vec::new();
        store
            .query(
                "CONSTRUCT { ?s ?p ?o } WHERE { ?s ?p ?o }",
                QueryFormats::new(ResultsSyntax::DEFAULT, RdfSyntax::NTriples),
                QueryLimits::default(),
                &mut answer,
            )
            .expect("a CONSTRUCT over the vocabulary graphs");
        let answer = String::from_utf8(answer).expect("an RDF document is UTF-8");

        let statements = read(&answer, Dialect::NTriples);
        assert_eq!(statements.len(), 1, "one statement was written:\n{answer}");
        assert_eq!(
            statements[0].object,
            SpecTerm::typed("7", "http://www.w3.org/2001/XMLSchema#integer"),
            "if SPARQL returns \"007\" then the rewrite is the export's alone, which is a much \
             smaller defect than docs/UNTESTED.md currently claims — go and correct it:\n{answer}"
        );
    }

    /// The sharpest form of the harm above: it does not merely change how a term is *written*, it
    /// **loses a statement**. Two triples that differ only in the lexical form of their object are
    /// two distinct triples in RDF; here they collapse to one, and the graph a user gets back is
    /// smaller than the one they put in with nothing said about it.
    #[test]
    fn two_triples_that_differ_only_in_lexical_form_collapse_into_one() {
        let dir = temp_dir();
        let store = Store::open(dir.path()).expect("a fresh store opens");
        let graph = vocabulary("http://acme.example/v/notation");
        let subject = iri("http://acme.example/v/notation#Concept");
        let notation = iri("http://www.w3.org/2004/02/skos/core#notation");
        let integer = NamedNode::new_unchecked("http://www.w3.org/2001/XMLSchema#integer");

        store
            .transaction(|txn| {
                txn.create_vocabulary_graph(&graph)?;
                txn.insert(
                    &graph,
                    vec![
                        (
                            subject.clone(),
                            notation.clone(),
                            Literal::new_typed_literal("007", integer.clone()).into(),
                        ),
                        (
                            subject.clone(),
                            notation.clone(),
                            Literal::new_typed_literal("7", integer.clone()).into(),
                        ),
                    ],
                )
            })
            .expect("the vocabulary takes both statements");

        let document = export(&store, graph.iri(), RdfSyntax::NTriples);
        let statements = read(&document, Dialect::NTriples);

        assert_eq!(
            statements.len(),
            1,
            "two distinct RDF triples went in; this asserts the loss rather than excusing it. If \
             two come back the defect is fixed — see docs/UNTESTED.md.\n{document}"
        );
    }

    /// An empty graph must produce an *empty document*, not a malformed one.
    /// `nquadsDoc ::= statement? (EOL statement)* EOL?` admits zero statements, so this is a
    /// conformance question and not a cosmetic one — and it is what every newly created
    /// vocabulary exports until somebody authors into it.
    #[test]
    fn an_empty_graph_is_an_empty_but_well_formed_document() {
        let dir = temp_dir();
        let store = Store::open(dir.path()).expect("a fresh store opens");
        let graph = vocabulary("http://acme.example/v/empty");
        store
            .transaction(|txn| txn.create_vocabulary_graph(&graph))
            .expect("an empty vocabulary is creatable");

        for (syntax, dialect) in [
            (RdfSyntax::NTriples, Dialect::NTriples),
            (RdfSyntax::NQuads, Dialect::NQuads),
        ] {
            let document = export(&store, graph.iri(), syntax);
            assert!(
                read(&document, dialect).is_empty(),
                "{syntax} invented a statement for an empty graph"
            );
        }
        assert!(canonical_violations(&export(&store, graph.iri(), RdfSyntax::NTriples)).is_empty());
    }

    // ---------------------------------------------------------------------------------------
    // Proving the checker discriminates
    // ---------------------------------------------------------------------------------------

    /// A recogniser that accepts everything makes every test above vacuous. Each of these
    /// documents violates exactly one named production, so a rule quietly dropped from the reader
    /// shows up as one of these being accepted.
    #[test]
    fn the_reader_refuses_what_the_grammar_refuses() {
        let refused = [
            (
                "a relative IRI — N-Triples §2.2: \"IRIs may be written only as absolute IRIs\", \
                 and the grammar has no production for a bare name",
                "<a> <http://example.org/p> <http://example.org/o> .\n",
                "permits only absolute IRIs",
            ),
            (
                "a raw space inside an IRIREF, excluded by [^#x00-#x20…]",
                "<http://example.org/a b> <http://example.org/p> \"x\" .\n",
                "excluded from IRIREF",
            ),
            (
                "a raw line feed inside STRING_LITERAL_QUOTE",
                "<http://example.org/s> <http://example.org/p> \"two\nlines\" .\n",
                "a raw line break is excluded",
            ),
            (
                "an escape ECHAR does not define",
                r#"<http://example.org/s> <http://example.org/p> "a \q b" ."#,
                "is not one of ECHAR's escapes",
            ),
            (
                "an unterminated literal at the end of the document",
                "<http://example.org/s> <http://example.org/p> \"open .",
                "the document ended inside a literal",
            ),
            (
                "a literal as a predicate",
                "<http://example.org/s> \"p\" \"o\" .\n",
                "a predicate must be an IRI",
            ),
            (
                "a language tag with an empty subtag",
                "<http://example.org/s> <http://example.org/p> \"x\"@en- .\n",
                "language subtag needs at least one character",
            ),
            (
                "a blank node label ending in '.' — the trailing PN_CHARS clause stops the \
                 label before the dot, and a '.' is not a predicate",
                "_:a. <http://example.org/p> \"x\" .\n",
                "a predicate must be an IRI",
            ),
            (
                "a blank node label starting with '-', which is PN_CHARS but not PN_CHARS_U",
                "_:-a <http://example.org/p> \"x\" .\n",
                "first character must be PN_CHARS_U or a digit",
            ),
            (
                "a UCHAR with too few hex digits",
                r#"<http://example.org/s> <http://example.org/p> "\u00E" ."#,
                "needs its full complement of hex digits",
            ),
            (
                "a missing terminator",
                "<http://example.org/s> <http://example.org/p> \"x\"\n",
                "expected '.' to end the statement",
            ),
            (
                "two statements on one line, where the grammar requires an EOL between them",
                concat!(
                    "<http://example.org/s> <http://example.org/p> \"x\" . ",
                    "<http://example.org/s> <http://example.org/p> \"y\" .\n"
                ),
                "expected a line break",
            ),
            (
                "a graph label, which N-Triples has no production for",
                concat!(
                    "<http://example.org/s> <http://example.org/p> \"x\" ",
                    "<http://example.org/g> .\n"
                ),
                "expected '.' to end the statement",
            ),
        ];

        for (why, document, expected) in refused {
            let error = SpecReader::new(document, Dialect::NTriples)
                .document()
                .expect_err(&format!("the reader accepted {why}: {document:?}"));
            assert!(
                error.contains(expected),
                "{why}: expected an error mentioning {expected:?}, got {error:?}"
            );
        }
    }

    /// The same discipline for the canonical layer. Each document here is *valid* N-Triples — it
    /// must parse — and departs from exactly one of §4's constraints.
    #[test]
    fn the_canonical_checker_refuses_what_canonical_n_triples_refuses() {
        let refused = [
            (
                "two spaces after the subject",
                "<http://example.org/s>  <http://example.org/p> \"x\" .\n",
                "requires a single space",
            ),
            (
                "a tab as the separator",
                "<http://example.org/s>\t<http://example.org/p> \"x\" .\n",
                "requires a single space",
            ),
            (
                "an indented line",
                "  <http://example.org/s> <http://example.org/p> \"x\" .\n",
                "requires no white space",
            ),
            (
                "trailing space after the '.'",
                "<http://example.org/s> <http://example.org/p> \"x\" . \n",
                "requires no white space",
            ),
            (
                "a comment",
                "<http://example.org/s> <http://example.org/p> \"x\" . # why\n",
                "a comment",
            ),
            (
                "\\t, where the tab is allowed directly",
                r#"<http://example.org/s> <http://example.org/p> "a\tb" ."#,
                "allows directly",
            ),
            (
                "a UCHAR for a character that needs no escape",
                r#"<http://example.org/s> <http://example.org/p> "caf\u00E9" ."#,
                "a UCHAR escape",
            ),
            (
                "lower-case hex in a UCHAR",
                r#"<http://example.org/s> <http://example.org/p> "caf\u00e9" ."#,
                "lower-case hex digit",
            ),
        ];

        for (why, document, expected) in refused {
            let violations = canonical_violations(document);
            assert!(
                violations.iter().any(|found| found.contains(expected)),
                "{why}: expected a violation mentioning {expected:?}, got {violations:?}"
            );
        }
    }

    /// The counterweight to the test above: the canonical checker must not complain about a
    /// document that is canonical. Without this, a checker that reported a violation
    /// unconditionally would satisfy every negative case and still be useless.
    #[test]
    fn the_canonical_checker_accepts_a_canonical_document() {
        for document in [
            "",
            "\n",
            "<http://example.org/s> <http://example.org/p> \"x\" .\n",
            concat!(
                "<http://example.org/s> <http://example.org/p> \"a\\\"b\\\\c\\nd\" .\n",
                "_:b0 <http://example.org/p> <http://example.org/o> .\n",
                "<http://example.org/s> <http://example.org/p> \"café 🧾\"@fr .\n"
            ),
        ] {
            assert_eq!(
                canonical_violations(document),
                Vec::<String>::new(),
                "a canonical document was reported non-canonical: {document:?}"
            );
        }
    }
}
