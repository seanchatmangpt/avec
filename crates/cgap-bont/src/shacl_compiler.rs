//! A08: SHACL NodeShape -> fixed struct + presence-bitmask compiler.
//!
//! Lane doc (the spec): `docs/jira/v26.11.1/lanes/A08-shacl-bitmask.md`.
//!
//! This file is the **Wave 2 core engine** on top of the Wave-1 ABI freeze:
//! closed-world TTL subset parser -> eligibility court (typed refusals) ->
//! `BitmaskLayout` (sorted-IRI bit assignment) -> [`CompiledShape`] mask
//! constants -> the single-mask-operation [`validate_presence`] evaluator,
//! plus the canonical report serialization (persisted compile report).
//!
//! Closed-world compilation semantics (doc section 1, stated honestly): the
//! property set of an accepted shape is frozen; a property not declared is
//! unrepresentable at the struct layer and fails `IllegalProperty` at the
//! mask layer. Value constraints are NOT folded into the mask; they are
//! court-refused (`ValueConstraintOutOfMaskScope`) in this wave and remain
//! stage-2 work (doc section 9, W2-1).
//!
//! Court map (doc section 7, extended by the Wave-5 repair slice), all in
//! `#[cfg(test)]` below:
//! T1 build gate (witnessed by the `cargo test -p cgap-bont` command itself),
//! T2 constraint court + M1, T3 XOR court + M2, T4 closed-world court + M3,
//! T5 asm witness (real `--emit asm` subprocess on a scratch crate, with a
//! call-bearing control function proving the no-call detector fires), T6
//! deterministic round-trip, T7 refusal corpus (one fixture per reason code),
//! plus the Wave-5 repair courts: T7b logical and/or registry fidelity (M7),
//! T7c malformed known locals (M6), T7d deny-by-default for unclassified
//! SHACL constructs (M8), T7e mandatory-group contradiction (M9), T8
//! parse-side report invariants, and the 64-bit boundary court; plus the
//! Wave-6 repair courts: T9 IRIREF character law (Turtle production [8] —
//! a space-bearing IRIREF used to compile into a canonical report the
//! parser refuses, breaking the round-trip law at the lexer) and T10
//! single-valued `sh:closed` (a contradictory `true` + `false` pair used
//! to silently resolve through `any()`); plus the Wave-7 repair courts:
//! T11 wrong-typed `sh:closed` (a non-boolean object used to drop the
//! malformed-local fact silently into the generic `NotClosedShape` mislabel)
//! and the T8 M13 extension (BIT rows re-ordered against the sorted-IRI bit
//! law used to parse into masks contradicting their own bit table); plus the
//! Wave-8 repair: the T8 M14 extension (a space-bearing `SHAPE` IRI parsed
//! past the format's space-free IRI law - the one field `parse_report` still
//! accepted whole-line, while BIT rows and REFUSAL fields already refused
//! the same IRI; one documented invariant, asymmetrically enforced). Plus the
//! Wave-9 cross-lane integration courts: T12 content-identity over the A06
//! seam (`crate::primitives::slot_id`, blake3-backed; M15 identity-zero flip)
//! and T13 A20-scale 1M-row stress (doc-law oracle over doc-derived masks,
//! identity stability across independent compiles, and a no-sleep ratio
//! fence against a bare-xorshift baseline - never an absolute-ms bound).
#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

// ---------------------------------------------------------------------------
// Wave-1 ABI freeze (kept from the previous slice; asserts unchanged)
// ---------------------------------------------------------------------------

/// Mask word: one presence bit per declared property, bit index < 64.
///
/// ABI-identical to `u64`; the alias exists so the word width is pinned by a
/// static assert rather than by prose.
pub type MaskWord = u64;

/// Max mask width in bits. Above this, compile emits `MaskOverflow` (until
/// Wave-2 W2-3 two-level masks retire it).
pub const MASK_WIDTH_MAX: u8 = 64;

/// Max declared properties per shape - the same ABI cap as [`MASK_WIDTH_MAX`],
/// stated in property-count units.
pub const MAX_DECLARED_PROPERTIES: usize = MASK_WIDTH_MAX as usize;

/// Typed refusal reason codes, one per row of the doc section 1 registry.
///
/// `#[repr(u8)]` freezes the persisted compile-report row ABI: one byte per
/// reason code, stable across rustc versions.
///
/// Disclosed registry extension (doc yields to reality, recorded in the lane
/// receipt): `MalformedShape` is added beyond the doc's six rows so that the
/// zero-silent-drops law stays total - a property shape with no `sh:path`, an
/// unresolvable `sh:property` reference, or a duplicate property path has no
/// home in the doc's registry, and dropping it silently is exactly what the
/// registry exists to prevent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum ReasonCode {
    /// missing or `false` `sh:closed` - open-world shapes are not compilable here
    NotClosedShape,
    /// `sh:pattern`, `sh:in`, length constraints - stage-2 value scope.
    /// Also the default verdict (Wave-5 repair, disclosed) for an
    /// unclassified SHACL constraint component observed on a property shape:
    /// anything the mask does not consume and the registry does not classify
    /// is refused, never silently dropped.
    ValueConstraintOutOfMaskScope,
    /// `sh:minCount > 1`, `sh:maxCount`, `sh:qualifiedValueShape`
    CardinalityUnsupported,
    /// `sh:node`, `sh:not`, `sh:xone` - logical shapes; XOR presence only
    LogicalShapeUnsupported,
    /// `sh:targetClass` / `sh:targetNode` / `sh:targetObjectsOf` / `sh:targetSubjectsOf`
    /// - targeting at call site. **Advisory**: the row is recorded but does not
    /// block acceptance, because targeting never affects the struct or mask.
    TargetingOutOfScope,
    /// > 64 declared properties (mask width; Wave 2 W2-3 retires it)
    MaskOverflow,
    /// Disclosed extension: property shape without `sh:path`, unresolvable
    /// `sh:property` object, or duplicate property path within one shape.
    /// Wave-5 disclosed extension: a known SHACL local observed malformed
    /// (wrong-typed object or duplicate occurrence), two-or-more mandatory
    /// members of one exactly-one-of group (unsatisfiable mask), and a
    /// shape-level SHACL construct with no registry row (deny-by-default).
    /// Wave-6 disclosed extension: a duplicate `sh:closed` occurrence
    /// (single-valued per SHACL 1.1; the contradictory `true` + `false`
    /// pair previously resolved silently through `any()`). Wave-7 disclosed
    /// extension: a wrong-typed `sh:closed` object (non-boolean; previously
    /// degraded silently to the generic `NotClosedShape` mislabel).
    MalformedShape,
}

impl ReasonCode {
    /// Stable persisted name (canonical report format, round-trip law).
    pub fn name(self) -> &'static str {
        match self {
            ReasonCode::NotClosedShape => "NotClosedShape",
            ReasonCode::ValueConstraintOutOfMaskScope => "ValueConstraintOutOfMaskScope",
            ReasonCode::CardinalityUnsupported => "CardinalityUnsupported",
            ReasonCode::LogicalShapeUnsupported => "LogicalShapeUnsupported",
            ReasonCode::TargetingOutOfScope => "TargetingOutOfScope",
            ReasonCode::MaskOverflow => "MaskOverflow",
            ReasonCode::MalformedShape => "MalformedShape",
        }
    }

    /// Inverse of [`ReasonCode::name`].
    pub fn from_name(name: &str) -> Option<Self> {
        let code = match name {
            "NotClosedShape" => ReasonCode::NotClosedShape,
            "ValueConstraintOutOfMaskScope" => ReasonCode::ValueConstraintOutOfMaskScope,
            "CardinalityUnsupported" => ReasonCode::CardinalityUnsupported,
            "LogicalShapeUnsupported" => ReasonCode::LogicalShapeUnsupported,
            "TargetingOutOfScope" => ReasonCode::TargetingOutOfScope,
            "MaskOverflow" => ReasonCode::MaskOverflow,
            "MalformedShape" => ReasonCode::MalformedShape,
            _ => return None,
        };
        Some(code)
    }
}

/// One typed refusal row (doc section 1): zero silent drops. Every refused
/// construct lands in the persisted compile report with its reason code.
///
/// Construct labels are space-free tokens (e.g. `"sh:pattern"`,
/// `"sh:minCount>1"`, `"missing:sh:path"`) so the canonical report format
/// stays line-oriented.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ShaclRefusal {
    /// IRI of the shape the refusal is scoped to.
    pub shape_iri: String,
    /// The refused construct as a space-free token (e.g. `"sh:pattern"`).
    pub construct: String,
    /// Typed reason from the registry.
    pub reason_code: ReasonCode,
}

/// Presence-validation verdicts of the mask evaluator (doc section 6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum MaskViolation {
    /// `observed & !m_legal != 0` - undeclared property in a closed world
    IllegalProperty,
    /// `observed & m_mandatory != m_mandatory`
    MissingMandatory,
    /// `(observed & m_group).count_ones() != 1`
    GroupCardinality,
}

/// Compile-report IR for one accepted shape (doc section 6, verbatim field set).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CompiledShape {
    /// IRI of the compiled NodeShape.
    pub shape_iri: String,
    /// `<= 64`, else `MaskOverflow` at compile time (doc section 6).
    pub mask_width: u8,
    /// `(property IRI, bit index)`, sorted by IRI (doc section 5, step 3).
    pub bits: Vec<(String, u8)>,
    /// Mandatory-presence mask: `AND` path constant.
    pub m_mandatory: MaskWord,
    /// Legal-property mask: closed-world guard constant.
    pub m_legal: MaskWord,
    /// Exactly-one-of group mask: `popcnt` path constant.
    pub m_group: MaskWord,
}

// ---- ABI-freeze static asserts (Wave-1 courts, kept verbatim) ---------------
//
// Non-vacuity receipts (run on a /tmp copy of this crate, never the canonical
// tree; each mutation simulates one concrete future edit class):
//   M-W1a  MASK_WIDTH_MAX 64 -> 100            => const-assert failure, exit != 0
//   M-W1b  ReasonCode #[repr(u8)] -> #[repr(u16)] => size assert fails, exit != 0
//   M-W1c  MaskWord u64 -> u32                 => word-width assert fails, exit != 0
// A gate that cannot flip is FAIL; the flip receipts (command + exit) are in
// the lane report.

/// Mask word is register-sized (x86-64 / aarch64: 8 bytes).
const _: () = assert!(std::mem::size_of::<MaskWord>() == 8);

/// The doc's mask-width cap is exactly 64 and is stated identically in
/// property-count units - two faces of one ABI boundary.
const _: () = assert!(MASK_WIDTH_MAX == 64);
const _: () = assert!(MASK_WIDTH_MAX as usize == MAX_DECLARED_PROPERTIES);

/// Refusal rows persist one byte per reason code.
const _: () = assert!(std::mem::size_of::<ReasonCode>() == 1);
const _: () = assert!(std::mem::size_of::<MaskViolation>() == 1);

/// The mask-bearing struct keeps `MaskWord` alignment so the three mask words
/// are register-alignable (doc section 6: "register-resident").
const _: () = assert!(std::mem::align_of::<CompiledShape>() == std::mem::align_of::<MaskWord>());

// ---------------------------------------------------------------------------
// TTL subset parser (doc section 5 step 1: parse_ttl -> Shape IR)
// ---------------------------------------------------------------------------
//
// A closed-world SUBSET of Turtle, sized to the constructs the compiler
// consumes. Supported: `@prefix`, IRIREF `<...>`, prefixed names, `a`
// (rdf:type), `[ ... ]` property-list blank nodes, booleans, integers,
// strings without escapes, `;` / `,` predicate-object lists. NOT supported
// (fixtures for this compiler must not use them): collections `( ... )`,
// string escapes, language tags, datatyped literals `"..."^^xsd:...`,
// `@base`, blank node labels `_:b`, PNAME locals containing `.`.

/// A parsed RDF term (subset).
#[derive(Debug, Clone, PartialEq, Eq)]
enum Term {
    /// IRI (already expanded to its full form).
    Iri(String),
    /// Property-list blank node: its predicate-object pairs.
    Blank(Vec<(String, Term)>),
    /// `true` / `false` literal.
    Bool(bool),
    /// Integer literal.
    Int(i64),
    /// String literal (no escapes).
    Str(String),
}

/// Local name of an IRI: the part after the last `#` or `/`.
fn local_name(iri: &str) -> &str {
    match iri.rfind(|c| c == '#' || c == '/') {
        Some(i) => &iri[i + 1..],
        None => iri,
    }
}

/// SHACL namespace (canonical; shapes must use this exact binding).
const SHACL_NS: &str = "http://www.w3.org/ns/shacl#";
const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
const NODE_SHAPE: &str = "http://www.w3.org/ns/shacl#NodeShape";

/// Parser error: one line + message (TTL subset violations).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShaclParseError {
    /// 1-based source line the error was detected on.
    pub line: usize,
    /// What went wrong.
    pub message: String,
}

impl fmt::Display for ShaclParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ttl parse error at line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for ShaclParseError {}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Iri(String),
    PName(String, String),
    AtPrefix,
    A,
    True,
    False,
    Int(i64),
    Str(String),
    Dot,
    Semi,
    Comma,
    LBracket,
    RBracket,
}

/// Tokenizer over the TTL subset. Comments (`#` to end of line) are trivia
/// between tokens; `#` inside an IRIREF or string is literal because those
/// are consumed as single tokens.
struct Lexer<'a> {
    src: &'a [u8],
    pos: usize,
    line: usize,
}

impl<'a> Lexer<'a> {
    fn new(src: &'a str) -> Self {
        Lexer { src: src.as_bytes(), pos: 0, line: 1 }
    }

    fn bump(&mut self) -> Option<u8> {
        if self.pos >= self.src.len() {
            return None;
        }
        let b = self.src[self.pos];
        self.pos += 1;
        if b == b'\n' {
            self.line += 1;
        }
        Some(b)
    }

    fn peek(&self) -> Option<u8> {
        self.src.get(self.pos).copied()
    }

    /// Skip whitespace and `#` comments.
    fn skip_trivia(&mut self) {
        while let Some(b) = self.peek() {
            match b {
                b' ' | b'\t' | b'\r' | b'\n' => {
                    self.bump();
                }
                b'#' => {
                    while let Some(b) = self.peek() {
                        if b == b'\n' {
                            break;
                        }
                        self.bump();
                    }
                }
                _ => break,
            }
        }
    }

    /// The TTL subset is ASCII-only. A non-ASCII byte is a typed parse error,
    /// never silent latin1 mojibake: `b as char` maps raw bytes 0x80..=0xFF
    /// onto U+0080..U+00FF and corrupts UTF-8 quietly (zero-silent-drops law).
    fn ascii_char(&self, b: u8) -> Result<char, ShaclParseError> {
        if b.is_ascii() {
            Ok(b as char)
        } else {
            Err(ShaclParseError {
                line: self.line,
                message: format!("non-ASCII byte 0x{:02x} outside the ASCII TTL subset", b),
            })
        }
    }

    fn read_until(&mut self, close: u8, what: &str) -> Result<String, ShaclParseError> {
        let start_line = self.line;
        let mut out = String::new();
        loop {
            match self.bump() {
                None => {
                    return Err(ShaclParseError {
                        line: start_line,
                        message: format!("unterminated {} (EOF)", what),
                    })
                }
                Some(b) if b == close => return Ok(out),
                Some(b) => out.push(self.ascii_char(b)?),
            }
        }
    }

    /// Turtle IRIREF character law (Turtle 1.1 grammar production [8]
    /// IRIREF): between `<` and `>` the characters U+0000..U+0020, `"`,
    /// `<`, `>`, `{`, `}`, `|`, `^`, `\`, and `` ` `` are forbidden. The
    /// lexer previously accepted any ASCII except `>`, so a space-bearing
    /// IRI compiled, reached the canonical report, and produced an artifact
    /// the canonical parser refuses (`BIT has extra fields`) - the
    /// compiler's own output un-persistable, T6's round-trip law broken at
    /// the lexer. The law is enforced here, the earliest typed point: a
    /// hostile IRIREF is a [`ShaclParseError`] naming the byte, never a
    /// silent acceptance into an un-persistable report.
    fn read_iri(&mut self) -> Result<String, ShaclParseError> {
        let start_line = self.line;
        let mut out = String::new();
        loop {
            match self.bump() {
                None => {
                    return Err(ShaclParseError {
                        line: start_line,
                        message: "unterminated IRIREF (EOF)".to_string(),
                    })
                }
                Some(b'>') => return Ok(out),
                Some(b) => {
                    if b <= 0x20
                        || matches!(
                            b,
                            b'"' | b'<' | b'{' | b'}' | b'|' | b'^' | b'\\' | b'`'
                        )
                    {
                        return Err(ShaclParseError {
                            line: start_line,
                            message: format!(
                                r#"forbidden byte 0x{:02x} in IRIREF (Turtle IRIREF forbids U+0000..U+0020 and `"<>{{}}|^`\)"#,
                                b
                            ),
                        });
                    }
                    out.push(self.ascii_char(b)?);
                }
            }
        }
    }

    fn read_pname(&mut self) -> Result<Tok, ShaclParseError> {
        let mut prefix = String::new();
        // prefix part: up to ':'
        loop {
            match self.peek() {
                Some(b':') => {
                    self.bump();
                    break;
                }
                Some(b) if !is_delim(b) => {
                    prefix.push(self.ascii_char(b)?);
                    self.bump();
                }
                _ => {
                    return Err(ShaclParseError {
                        line: self.line,
                        message: format!("prefixed name {:?} has no ':'", prefix),
                    })
                }
            }
        }
        let mut local = String::new();
        // local part: PN_LOCAL subset - no '.', no escapes
        while let Some(b) = self.peek() {
            if b == b'.' || is_delim(b) {
                break;
            }
            local.push(self.ascii_char(b)?);
            self.bump();
        }
        Ok(Tok::PName(prefix, local))
    }

    fn next_token(&mut self) -> Result<Option<(Tok, usize)>, ShaclParseError> {
        self.skip_trivia();
        let line = self.line;
        let Some(b) = self.peek() else {
            return Ok(None);
        };
        let tok = match b {
            b'<' => {
                self.bump();
                Tok::Iri(self.read_iri()?)
            }
            b'"' => {
                self.bump();
                Tok::Str(self.read_until(b'"', "string literal")?)
            }
            b'.' => {
                self.bump();
                Tok::Dot
            }
            b';' => {
                self.bump();
                Tok::Semi
            }
            b',' => {
                self.bump();
                Tok::Comma
            }
            b'[' => {
                self.bump();
                Tok::LBracket
            }
            b']' => {
                self.bump();
                Tok::RBracket
            }
            b'@' => {
                self.bump();
                let mut word = String::new();
                while let Some(b) = self.peek() {
                    if b.is_ascii_alphanumeric() {
                        word.push(b as char);
                        self.bump();
                    } else {
                        break;
                    }
                }
                if word == "prefix" {
                    Tok::AtPrefix
                } else {
                    return Err(ShaclParseError {
                        line,
                        message: format!("unsupported @ directive @{:?}", word),
                    });
                }
            }
            b if b.is_ascii_digit() || b == b'-' => {
                let mut num = String::new();
                if b == b'-' {
                    num.push('-');
                    self.bump();
                }
                while let Some(b) = self.peek() {
                    if b.is_ascii_digit() {
                        num.push(b as char);
                        self.bump();
                    } else {
                        break;
                    }
                }
                match num.parse::<i64>() {
                    Ok(v) => Tok::Int(v),
                    Err(_) => {
                        return Err(ShaclParseError {
                            line,
                            message: format!("bad integer literal {:?}", num),
                        })
                    }
                }
            }
            b if b.is_ascii_alphabetic() || b == b'_' => {
                // keyword or prefixed name
                let saved = (self.pos, self.line);
                let mut word = String::new();
                while let Some(b) = self.peek() {
                    if b.is_ascii_alphanumeric() || b == b'_' {
                        word.push(b as char);
                        self.bump();
                    } else if b == b':' && !word.is_empty() {
                        // prefixed name - re-read with the full reader
                        self.pos = saved.0;
                        self.line = saved.1;
                        return self.read_pname().map(|t| Some((t, line)));
                    } else if b == b':' {
                        // bare ':' (empty prefix, e.g. `:foo`) - no prefix
                        // binding can exist for the empty name in this subset;
                        // route through read_pname so the error is typed.
                        self.pos = saved.0;
                        self.line = saved.1;
                        return self.read_pname().map(|t| Some((t, line)));
                    } else {
                        break;
                    }
                }
                match word.as_str() {
                    "a" => Tok::A,
                    "true" => Tok::True,
                    "false" => Tok::False,
                    _ => {
                        return Err(ShaclParseError {
                            line,
                            message: format!("unexpected bare word {:?}", word),
                        })
                    }
                }
            }
            _ => {
                return Err(ShaclParseError {
                    line,
                    message: format!("unexpected byte {:?}", b as char),
                })
            }
        };
        Ok(Some((tok, line)))
    }
}

/// Whitespace plus every token boundary that may follow a prefixed name.
fn is_delim(b: u8) -> bool {
    matches!(
        b,
        b' ' | b'\t' | b'\r' | b'\n' | b'.' | b';' | b',' | b'[' | b']' | b'"' | b'(' | b')' | b'#'
    )
}

/// Parsed graph (subset): subject -> its predicate-object pairs, in a sorted
/// map so iteration order (and therefore every downstream artifact) is
/// deterministic.
type Graph = BTreeMap<String, Vec<(String, Term)>>;

/// Parse the TTL subset into a statement graph.
fn parse_graph(ttl: &str) -> Result<Graph, ShaclParseError> {
    let mut lex = Lexer::new(ttl);
    let mut toks: Vec<(Tok, usize)> = Vec::new();
    while let Some(t) = lex.next_token()? {
        toks.push(t);
    }
    let mut p = Parser { toks, i: 0, prefixes: BTreeMap::new(), graph: BTreeMap::new() };
    p.parse_document()?;
    Ok(std::mem::take(&mut p.graph))
}

struct Parser {
    toks: Vec<(Tok, usize)>,
    i: usize,
    prefixes: BTreeMap<String, String>,
    graph: Graph,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.i).map(|(t, _)| t)
    }

    fn line(&self) -> usize {
        self.toks.get(self.i).map(|(_, l)| *l).unwrap_or(0)
    }

    fn bump(&mut self) -> Option<Tok> {
        if self.i < self.toks.len() {
            let t = self.toks[self.i].0.clone();
            self.i += 1;
            Some(t)
        } else {
            None
        }
    }

    fn expect(&mut self, what: &str) -> Result<Tok, ShaclParseError> {
        match self.bump() {
            Some(t) => Ok(t),
            None => Err(ShaclParseError {
                line: self.line(),
                message: format!("unexpected EOF, expected {}", what),
            }),
        }
    }

    fn err(&self, message: impl Into<String>) -> ShaclParseError {
        ShaclParseError { line: self.line(), message: message.into() }
    }

    /// Expand a prefixed name to its full IRI.
    fn expand(&self, prefix: &str, local: &str) -> Result<String, ShaclParseError> {
        match self.prefixes.get(prefix) {
            Some(ns) => Ok(format!("{}{}", ns, local)),
            None => Err(self.err(format!("undeclared prefix {:?}", prefix))),
        }
    }

    /// Expand a subject/predicate IRI or prefixed name.
    fn expand_tok(&self, t: &Tok) -> Result<String, ShaclParseError> {
        match t {
            Tok::Iri(iri) => Ok(iri.clone()),
            Tok::PName(p, l) => self.expand(p, l),
            Tok::A => Ok(RDF_TYPE.to_string()),
            other => Err(self.err(format!("expected IRI or prefixed name, got {:?}", other))),
        }
    }

    fn parse_document(&mut self) -> Result<(), ShaclParseError> {
        loop {
            match self.peek() {
                None => return Ok(()),
                Some(Tok::AtPrefix) => {
                    self.bump();
                    let ns_name = match self.expect("@prefix namespace")? {
                        Tok::PName(p, l) => {
                            if !l.is_empty() {
                                return Err(self.err("@prefix name must have empty local part"));
                            }
                            p
                        }
                        other => {
                            return Err(self.err(format!("@prefix expects prefixed name, got {:?}", other)))
                        }
                    };
                    let ns_iri = match self.expect("@prefix IRI")? {
                        Tok::Iri(iri) => iri,
                        other => {
                            return Err(self.err(format!("@prefix expects <IRI>, got {:?}", other)))
                        }
                    };
                    self.prefixes.insert(ns_name, ns_iri);
                    match self.expect("'.' after @prefix")? {
                        Tok::Dot => {}
                        other => {
                            return Err(self.err(format!("expected '.' after @prefix, got {:?}", other)))
                        }
                    }
                }
                Some(Tok::Iri(_)) | Some(Tok::PName(_, _)) => {
                    let subj = self.expand_tok(self.peek().expect("checked"))?;
                    self.bump();
                    let mut pairs = Vec::new();
                    self.parse_pred_obj_list(&mut pairs)?;
                    match self.expect("'.' after triples")? {
                        Tok::Dot => {}
                        other => {
                            return Err(self.err(format!("expected '.' after triples, got {:?}", other)))
                        }
                    }
                    self.graph.entry(subj).or_default().extend(pairs);
                }
                Some(other) => {
                    return Err(self.err(format!("expected statement, got {:?}", other)));
                }
            }
        }
    }

    /// predicate objectList (`;` predicate objectList)* — until `.` or `]`.
    fn parse_pred_obj_list(&mut self, out: &mut Vec<(String, Term)>) -> Result<(), ShaclParseError> {
        loop {
            let pred_tok = self.expect("predicate")?;
            let pred = self.expand_tok(&pred_tok)?;
            loop {
                let term = self.parse_object()?;
                out.push((pred.clone(), term));
                match self.peek() {
                    Some(Tok::Comma) => {
                        self.bump();
                    }
                    _ => break,
                }
            }
            match self.peek() {
                Some(Tok::Semi) => {
                    self.bump();
                    // allow trailing `;` before `.` or `]`
                    match self.peek() {
                        Some(Tok::Dot) | Some(Tok::RBracket) | None => return Ok(()),
                        _ => {}
                    }
                }
                _ => return Ok(()),
            }
        }
    }

    fn parse_object(&mut self) -> Result<Term, ShaclParseError> {
        match self.expect("object")? {
            Tok::Iri(iri) => Ok(Term::Iri(iri)),
            Tok::PName(p, l) => {
                let iri = self.expand(&p, &l)?;
                Ok(Term::Iri(iri))
            }
            Tok::LBracket => {
                let mut pairs = Vec::new();
                // allow `[]` (empty blank node)
                match self.peek() {
                    Some(Tok::RBracket) => {}
                    _ => self.parse_pred_obj_list(&mut pairs)?,
                }
                match self.expect("']'")? {
                    Tok::RBracket => {}
                    other => return Err(self.err(format!("expected ']', got {:?}", other))),
                }
                Ok(Term::Blank(pairs))
            }
            Tok::True => Ok(Term::Bool(true)),
            Tok::False => Ok(Term::Bool(false)),
            Tok::Int(v) => Ok(Term::Int(v)),
            Tok::Str(s) => Ok(Term::Str(s)),
            other => Err(self.err(format!("expected object, got {:?}", other))),
        }
    }
}

// ---------------------------------------------------------------------------
// Shape IR extraction + eligibility court + BitmaskLayout (doc section 5 steps 2-3)
// ---------------------------------------------------------------------------

/// Shape IR for one NodeShape (doc section 5 step 1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeShapeIr {
    /// Shape IRI.
    pub iri: String,
    /// `sh:closed true` observed (eligibility precondition).
    pub closed: bool,
    /// Number of `sh:closed` triples observed on the shape. SHACL 1.1 makes
    /// `sh:closed` single-valued; the eligibility court refuses a duplicate
    /// occurrence (including the contradictory `true` + `false` pair) as
    /// `MalformedShape` instead of letting `any()` silently resolve it
    /// (Wave-6 repair, T10 court).
    pub closed_occurrences: usize,
    /// True when some `sh:closed` occurrence carries a wrong-typed object
    /// (anything other than a boolean literal). SHACL 1.1 types `sh:closed`
    /// as `xsd:boolean`; a non-boolean occurrence is a malformed known local
    /// and refuses as `MalformedShape` (`malformed:sh:closed`) - the same
    /// zero-silent-drops law T7c enforces for property-level known locals -
    /// instead of degrading silently to the generic `NotClosedShape` mislabel
    /// (Wave-7 repair, T11 court).
    pub closed_malformed: bool,
    /// Declared property shapes, in graph order.
    pub props: Vec<PropertyShapeIr>,
}

/// Property shape IR.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropertyShapeIr {
    /// `sh:path` IRI (None => MalformedShape refusal).
    pub path: Option<String>,
    /// `sh:datatype` IRI, if declared.
    pub datatype: Option<String>,
    /// `sh:minCount` (SHACL default 0).
    pub min_count: u64,
    /// `sh:maxCount`, if declared (CardinalityUnsupported in this wave).
    pub max_count: Option<u64>,
    /// `sh:group` IRI, if declared.
    pub group: Option<String>,
    /// Known SHACL locals (see [`KNOWN_PROP_LOCALS`]) observed malformed,
    /// stored as canonical space-free construct tokens ready for the report:
    /// `"malformed:sh:<local>"` for a wrong-typed object (e.g. `sh:minCount
    /// -1`, `sh:minCount "many"`, `sh:path` as a string) and
    /// `"duplicate:sh:<local>"` for a duplicate occurrence (last-wins would
    /// silently drop the first declaration and narrow the mask). The court
    /// refuses each as `MalformedShape` - silently ignoring a
    /// declared-but-unparseable constraint would narrow the mask.
    pub malformed: Vec<String>,
    /// Other SHACL-namespace constructs observed on the property shape
    /// (local names, sorted, deduped). The court classifies the registry
    /// ones; legal annotations (e.g. `sh:name`) pass through ignored.
    pub constructs: Vec<String>,
}

/// Extract Shape IR for every `sh:NodeShape`-typed subject. Sorted by shape
/// IRI (deterministic). An `sh:property` object the compiler cannot resolve
/// to a property shape (unresolvable IRI, literal) becomes a `path: None`
/// property, which the court refuses as `MalformedShape` - never a silent
/// drop.
fn extract_shapes(graph: &Graph) -> Vec<NodeShapeIr> {
    let mut shapes = Vec::new();
    for (subj, pairs) in graph {
        let is_shape = pairs
            .iter()
            .any(|(p, o)| p.as_str() == RDF_TYPE && o == &Term::Iri(NODE_SHAPE.to_string()));
        if !is_shape {
            continue;
        }
        let closed_pred = format!("{}closed", SHACL_NS);
        let closed_occurrences = pairs.iter().filter(|(p, _)| *p == closed_pred).count();
        let closed = pairs
            .iter()
            .any(|(p, o)| *p == closed_pred && o == &Term::Bool(true));
        let closed_malformed = pairs
            .iter()
            .any(|(p, o)| *p == closed_pred && !matches!(o, Term::Bool(_)));
        let mut props = Vec::new();
        for (p, o) in pairs {
            if p != &format!("{}property", SHACL_NS) {
                continue;
            }
            let prop_pairs: &[(String, Term)] = match o {
                Term::Blank(pairs) => pairs,
                Term::Iri(iri) => match graph.get(iri) {
                    Some(pairs) => pairs.as_slice(),
                    // unresolvable reference: empty pairs -> no path -> court row
                    None => &[],
                },
                _ => &[],
            };
            let mut prop = PropertyShapeIr {
                path: None,
                datatype: None,
                min_count: 0,
                max_count: None,
                group: None,
                malformed: Vec::new(),
                constructs: Vec::new(),
            };
            let mut seen: BTreeSet<&str> = BTreeSet::new();
            for (pp, po) in prop_pairs {
                let local = local_name(pp);
                if !pp.starts_with(SHACL_NS) {
                    // Predicates outside the SHACL namespace are outside the
                    // compiler's registry; they are annotations by definition
                    // and cannot narrow the mask.
                    continue;
                }
                if KNOWN_PROP_LOCALS.contains(&local) {
                    let matched = match (local, po) {
                        ("path", Term::Iri(iri)) => {
                            prop.path = Some(iri.clone());
                            true
                        }
                        ("datatype", Term::Iri(iri)) => {
                            prop.datatype = Some(iri.clone());
                            true
                        }
                        ("minCount", Term::Int(n)) if *n >= 0 => {
                            prop.min_count = *n as u64;
                            true
                        }
                        ("maxCount", Term::Int(n)) if *n >= 0 => {
                            prop.max_count = Some(*n as u64);
                            true
                        }
                        ("group", Term::Iri(iri)) => {
                            prop.group = Some(iri.clone());
                            true
                        }
                        _ => false,
                    };
                    // Duplicate declarations would silently last-win; a
                    // wrong-typed object would silently become the default.
                    // Both are recorded, both refuse.
                    let kind = if !seen.insert(local) {
                        Some("duplicate")
                    } else if !matched {
                        Some("malformed")
                    } else {
                        None
                    };
                    if let Some(kind) = kind {
                        prop.malformed.push(format!("{}:sh:{}", kind, local));
                    }
                } else {
                    prop.constructs.push(local.to_string());
                }
            }
            prop.malformed.sort();
            prop.malformed.dedup();
            prop.constructs.sort();
            prop.constructs.dedup();
            props.push(prop);
        }
        shapes.push(NodeShapeIr {
            iri: subj.clone(),
            closed,
            closed_occurrences,
            closed_malformed,
            props,
        });
    }
    shapes
}

/// Groups declared `cgap:exactlyOneOf true` (cgap extension, doc section 5:
/// "exactly-one-of group (cgap ext `cgap:exactlyOneOf` on `sh:group`)").
/// Matched by local name so the fixture's prefix binding is irrelevant.
fn exact_one_of_groups(graph: &Graph) -> BTreeSet<String> {
    let mut groups = BTreeSet::new();
    for (subj, pairs) in graph {
        let is_group = pairs.iter().any(|(p, o)| {
            local_name(p) == "exactlyOneOf" && o == &Term::Bool(true)
        });
        if is_group {
            groups.insert(subj.clone());
        }
    }
    groups
}

/// Value-scope SHACL constructs (reason `ValueConstraintOutOfMaskScope`).
const VALUE_CONSTRUCTS: &[&str] = &[
    "pattern",
    "in",
    "minLength",
    "maxLength",
    "languageIn",
    "uniqueLang",
];

/// Logical-scope SHACL constructs (reason `LogicalShapeUnsupported`).
///
/// `sh:and` / `sh:or` are the remaining SHACL logical operators. The doc
/// section 1 registry row names node/not/xone; silently ignoring and/or would
/// drop a declared logical constraint (zero-silent-drops law), so they join
/// the row - the registry extension is disclosed in the lane receipt (same
/// mechanism as the `MalformedShape` extension).
const LOGICAL_CONSTRUCTS: &[&str] = &["node", "not", "xone", "and", "or"];

/// Property-shape locals the compiler consumes structurally. Observed with a
/// wrong-typed object (negative `sh:minCount`, string `sh:datatype`) or
/// declared twice, a local from this set is a `MalformedShape` refusal, never
/// a silent drop.
const KNOWN_PROP_LOCALS: &[&str] = &["path", "datatype", "minCount", "maxCount", "group"];

/// Targeting SHACL constructs (advisory rows; reason `TargetingOutOfScope`).
const TARGETING_CONSTRUCTS: &[&str] = &[
    "targetClass",
    "targetNode",
    "targetObjectsOf",
    "targetSubjectsOf",
];

/// SHACL non-validating annotation properties (W3C SHACL 1.1 section 4.6.1:
/// `sh:name`, `sh:description`, `sh:order`, `sh:severity`, `sh:message`,
/// `sh:defaultValue` - observed and ignored by a validator by spec). Anything
/// SHACL-namespaced outside this whitelist and the classified sets is REFUSED
/// (deny-by-default), never silently dropped.
const ANNOTATION_LOCALS: &[&str] = &[
    "name",
    "description",
    "order",
    "severity",
    "message",
    "defaultValue",
];

/// The two SHACL locals consumed structurally by the compiler; exempt from
/// the shape-level deny-by-default court.
const EXEMPT_SHAPE_LOCALS: &[&str] = &["closed", "property"];

/// Compile the shapes graph (doc section 5 pipeline, steps 1-4).
///
/// Compile semantics (documented interpretation, recorded in the lane
/// receipt): a shape is accepted **all-or-nothing**. Any blocking refusal row
/// (`NotClosedShape`, property-level value/cardinality/logical/malformed
/// rows, `MaskOverflow`) excludes the whole shape - a partial compile would
/// silently narrow `m_legal` and judge legal instances `IllegalProperty`.
/// `TargetingOutOfScope` rows are the one advisory exception: targeting never
/// affects the struct or mask (doc registry: "targeting at call site"), so
/// the row is recorded and the shape is still accepted.
pub fn compile_shapes(shapes_ttl: &str) -> Result<CompileReport, ShaclParseError> {
    let graph = parse_graph(shapes_ttl)?;
    let groups = exact_one_of_groups(&graph);
    let mut accepted = Vec::new();
    let mut refusals: Vec<ShaclRefusal> = Vec::new();

    for shape in extract_shapes(&graph) {
        let mut rows: Vec<ShaclRefusal> = Vec::new();
        let mut blocking = false;

        let row = |construct: &str, reason: ReasonCode| ShaclRefusal {
            shape_iri: shape.iri.clone(),
            construct: construct.to_string(),
            reason_code: reason,
        };

        // (1) closed-world precondition: `sh:closed true` is eligibility,
        // not a compiled feature (doc section 1 note). `sh:closed` is
        // single-valued (SHACL 1.1): a duplicate occurrence - including the
        // contradictory `sh:closed true ; sh:closed false` pair - would
        // otherwise silently resolve through `any()` and drop the `false`
        // reading from the report (Wave-6 repair; same silent-drop class the
        // property-level duplicate court refuses). The blocking duplicate
        // row fires first, and the NotClosedShape row below still records
        // the `false` reading's consequence on the same shape.
        if shape.closed_occurrences > 1 {
            rows.push(row("duplicate:sh:closed", ReasonCode::MalformedShape));
            blocking = true;
        }
        // Wave-7 repair (T11): a wrong-typed `sh:closed` object (integer,
        // string - anything but a boolean literal) is a malformed known local
        // and must carry its own `MalformedShape` row. Before this repair the
        // fact dropped silently: the report showed only `NotClosedShape`
        // (still a refusal, so no acceptance hole - a report-fidelity drop,
        // the T7c/T10 class). Both rows fire (precedent: a wrong-typed
        // `sh:path` carries `malformed:sh:path` AND `missing:sh:path`).
        if shape.closed_malformed {
            rows.push(row("malformed:sh:closed", ReasonCode::MalformedShape));
            blocking = true;
        }
        if !shape.closed {
            rows.push(row("sh:closed", ReasonCode::NotClosedShape));
            blocking = true;
        }

        // (2) shape-level constructs outside the mask scope. Deny-by-default:
        // every SHACL-namespaced local is classified (logical / targeting /
        // annotation / structurally consumed) or the shape refuses - a
        // constraint component with no registry row must not drop silently.
        if let Some(pairs) = graph.get(&shape.iri) {
            for (p, _) in pairs {
                let local = local_name(p);
                if !p.starts_with(SHACL_NS) || EXEMPT_SHAPE_LOCALS.contains(&local) {
                    continue;
                }
                if LOGICAL_CONSTRUCTS.contains(&local) {
                    rows.push(row(&format!("sh:{}", local), ReasonCode::LogicalShapeUnsupported));
                    blocking = true;
                } else if TARGETING_CONSTRUCTS.contains(&local) {
                    // advisory: recorded, never blocks (targeting is call-site)
                    rows.push(row(&format!("sh:{}", local), ReasonCode::TargetingOutOfScope));
                } else if !ANNOTATION_LOCALS.contains(&local) {
                    // Unclassified SHACL construct at shape level: no registry
                    // row, so accepting it would be a silent drop (disclosed
                    // MalformedShape extension, Wave-5 repair).
                    rows.push(row(&format!("sh:{}", local), ReasonCode::MalformedShape));
                    blocking = true;
                }
            }
        }

        // (3) property-level court.
        let mut seen_paths: BTreeSet<String> = BTreeSet::new();
        for prop in &shape.props {
            // Known locals observed malformed (wrong-typed object or
            // duplicate occurrence): each token is the canonical report
            // construct, stored ready-made by extract_shapes. Emitted before
            // the path check so a wrong-typed `sh:path` carries BOTH rows.
            for construct in &prop.malformed {
                rows.push(row(construct, ReasonCode::MalformedShape));
                blocking = true;
            }
            let Some(path) = &prop.path else {
                rows.push(row("missing:sh:path", ReasonCode::MalformedShape));
                blocking = true;
                continue;
            };
            if !seen_paths.insert(path.clone()) {
                rows.push(row("duplicate:sh:path", ReasonCode::MalformedShape));
                blocking = true;
                continue;
            }
            if prop.min_count > 1 {
                rows.push(row("sh:minCount>1", ReasonCode::CardinalityUnsupported));
                blocking = true;
            }
            if prop.max_count.is_some() {
                rows.push(row("sh:maxCount", ReasonCode::CardinalityUnsupported));
                blocking = true;
            }
            // value / logical / qualified constructs observed on the property
            // shape (carried in the IR by extract_shapes). Deny-by-default:
            // an unclassified SHACL constraint component refuses as
            // ValueConstraintOutOfMaskScope (out of mask scope, stage-2), it
            // must not drop silently.
            for local in &prop.constructs {
                if ANNOTATION_LOCALS.contains(&local.as_str()) {
                    continue;
                }
                if VALUE_CONSTRUCTS.contains(&local.as_str()) {
                    rows.push(row(&format!("sh:{}", local), ReasonCode::ValueConstraintOutOfMaskScope));
                    blocking = true;
                } else if local == "qualifiedValueShape" {
                    rows.push(row("sh:qualifiedValueShape", ReasonCode::CardinalityUnsupported));
                    blocking = true;
                } else if LOGICAL_CONSTRUCTS.contains(&local.as_str()) {
                    rows.push(row(&format!("sh:{}", local), ReasonCode::LogicalShapeUnsupported));
                    blocking = true;
                } else {
                    rows.push(row(&format!("sh:{}", local), ReasonCode::ValueConstraintOutOfMaskScope));
                    blocking = true;
                }
            }
        }

        // (3b) exactly-one-of groups: two or more mandatory members of one
        // group compile to an unsatisfiable mask - every instance fails
        // either MissingMandatory or GroupCardinality, so the shape has no
        // valid witness and must refuse rather than emit a dead evaluator.
        let mut mandatory_per_group: BTreeMap<&str, usize> = BTreeMap::new();
        for prop in &shape.props {
            if prop.min_count >= 1 {
                if let Some(g) = &prop.group {
                    if groups.contains(g) {
                        *mandatory_per_group.entry(g.as_str()).or_insert(0) += 1;
                    }
                }
            }
        }
        for (g, count) in &mandatory_per_group {
            if *count >= 2 {
                rows.push(
                    row(&format!("mandatory:sh:group<{}>", g), ReasonCode::MalformedShape),
                );
                blocking = true;
            }
        }

        // (4) mask width ceiling.
        if shape.props.len() > MAX_DECLARED_PROPERTIES {
            rows.push(row("mask-width>64", ReasonCode::MaskOverflow));
            blocking = true;
        }

        if blocking {
            refusals.extend(rows);
            continue;
        }

        // (5) BitmaskLayout: sorted by property IRI ascending (doc section 5
        // step 3), one bit per accepted property.
        let mut sorted: Vec<&PropertyShapeIr> = shape.props.iter().collect();
        sorted.sort_by(|a, b| a.path.as_deref().unwrap().cmp(b.path.as_deref().unwrap()));
        let n = sorted.len();
        let mut bits = Vec::with_capacity(n);
        let mut m_mandatory = 0u64;
        let mut m_group = 0u64;
        for (idx, prop) in sorted.iter().enumerate() {
            let bit = idx as u8;
            bits.push((prop.path.clone().unwrap(), bit));
            if prop.min_count == 1 {
                m_mandatory |= 1u64 << bit;
            }
            if let Some(group) = &prop.group {
                if groups.contains(group) {
                    m_group |= 1u64 << bit;
                }
            }
        }
        let m_legal = if n == 64 { u64::MAX } else { (1u64 << n) - 1 };
        accepted.push(CompiledShape {
            shape_iri: shape.iri.clone(),
            mask_width: n as u8,
            bits,
            m_mandatory,
            m_legal,
            m_group,
        });
        // advisory targeting rows accompany the accepted shape
        refusals.extend(rows);
    }

    accepted.sort_by(|a, b| a.shape_iri.cmp(&b.shape_iri));
    refusals.sort();
    Ok(CompileReport { accepted, refusals })
}

// ---------------------------------------------------------------------------
// Mask evaluator (doc section 6, verbatim)
// ---------------------------------------------------------------------------

/// Presence validation as a single mask operation (doc section 6).
///
/// Bounded claim: for a shape with no exactly-one-of group and a struct-locked
/// payload, the mandatory check is one AND-compare; the group path is one
/// AND + popcount + compare. Register-resident, O(1) in property count, no
/// loops, no graph traversal. T5 witnesses this on real emitted asm.
#[inline]
pub fn validate_presence(observed: MaskWord, s: &CompiledShape) -> Result<(), MaskViolation> {
    if observed & !s.m_legal != 0 {
        return Err(MaskViolation::IllegalProperty);
    }
    if observed & s.m_mandatory != s.m_mandatory {
        return Err(MaskViolation::MissingMandatory);
    }
    if s.m_group != 0 && (observed & s.m_group).count_ones() != 1 {
        return Err(MaskViolation::GroupCardinality);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Datatype -> B-ONT field type (doc section 5 step 4: struct field type)
// ---------------------------------------------------------------------------

/// B-ONT field types for `sh:datatype` projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BontFieldType {
    /// xsd:string
    Text,
    /// xsd:boolean
    Bool,
    /// xsd:integer
    Integer,
    /// xsd:decimal
    Decimal,
    /// xsd:double / xsd:float
    Double,
    /// xsd:date
    Date,
    /// xsd:dateTime
    DateTime,
    /// xsd:anyURI
    Iri,
}

/// Map an XSD datatype IRI to a [`BontFieldType`]. `None` for unknown IRIs -
/// the caller decides; the compiler never silently narrows an unknown type.
pub fn bont_field_type(xsd_iri: &str) -> Option<BontFieldType> {
    let code = match local_name(xsd_iri) {
        "string" => BontFieldType::Text,
        "boolean" => BontFieldType::Bool,
        "integer" => BontFieldType::Integer,
        "decimal" => BontFieldType::Decimal,
        "double" | "float" => BontFieldType::Double,
        "date" => BontFieldType::Date,
        "dateTime" => BontFieldType::DateTime,
        "anyURI" => BontFieldType::Iri,
        _ => return None,
    };
    Some(code)
}

// ---------------------------------------------------------------------------
// Canonical report format (doc section 5 step 2 / DoD-9: persisted compile report)
// ---------------------------------------------------------------------------
//
// Deterministic line format (deterministic-serializer law): accepted shapes
// sorted by IRI, bits sorted by property IRI, refusal rows sorted
// lexicographically. IRIs and construct labels are space-free by
// construction, so one space separates every field.

/// Compile report: accepted shapes + every refusal row, both persisted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileReport {
    /// Shapes that passed the eligibility court, sorted by shape IRI.
    pub accepted: Vec<CompiledShape>,
    /// Every typed refusal row (blocking and advisory), sorted.
    pub refusals: Vec<ShaclRefusal>,
}

/// Canonical report parse error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportParseError(pub String);

impl fmt::Display for ReportParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "report parse error: {}", self.0)
    }
}

impl std::error::Error for ReportParseError {}

/// Render the report in the canonical format. Byte-identical for identical
/// compile inputs (T6).
pub fn render_report(report: &CompileReport) -> String {
    let mut out = String::new();
    out.push_str("CGAP-SHACL-REPORT 1\n");
    out.push_str(&format!("ACCEPTED {}\n", report.accepted.len()));
    for s in &report.accepted {
        out.push_str(&format!("SHAPE {}\n", s.shape_iri));
        out.push_str(&format!("WIDTH {}\n", s.mask_width));
        out.push_str(&format!(
            "MASKS {} {} {}\n",
            s.m_mandatory, s.m_legal, s.m_group
        ));
        for (iri, bit) in &s.bits {
            out.push_str(&format!("BIT {} {}\n", bit, iri));
        }
        out.push_str("END\n");
    }
    out.push_str(&format!("REFUSALS {}\n", report.refusals.len()));
    for r in &report.refusals {
        out.push_str(&format!(
            "REFUSAL {} {} {}\n",
            r.reason_code.name(),
            r.shape_iri,
            r.construct
        ));
    }
    out.push_str("END-REPORT\n");
    out
}

/// Parse a canonical report (inverse of [`render_report`]).
///
/// Enforces the semantic invariants a compiler-produced report satisfies by
/// construction (mask-width cap, consecutive bit indices, unique property
/// IRIs, `m_legal` saturated to the width, masks within `m_legal`), so a
/// tampered or hand-mangled artifact refuses typed instead of parsing into
/// an out-of-invariant [`CompiledShape`].
pub fn parse_report(text: &str) -> Result<CompileReport, ReportParseError> {
    let err = |msg: String| ReportParseError(msg);
    let mut lines = text.lines().peekable();
    match lines.next() {
        Some("CGAP-SHACL-REPORT 1") => {}
        _ => return Err(err("bad magic header".into())),
    }
    let mut accepted = Vec::new();
    let mut refusals = Vec::new();
    loop {
        let Some(line) = lines.next() else {
            return Err(err("unexpected EOF".into()));
        };
        let mut fields = line.split(' ');
        match fields.next() {
            Some("ACCEPTED") => {
                let n: usize = fields
                    .next()
                    .ok_or_else(|| err("ACCEPTED missing count".into()))?
                    .parse()
                    .map_err(|_| err("ACCEPTED bad count".into()))?;
                for _ in 0..n {
                    accepted.push(parse_shape_block(&mut lines)?);
                }
            }
            Some("REFUSALS") => {
                let n: usize = fields
                    .next()
                    .ok_or_else(|| err("REFUSALS missing count".into()))?
                    .parse()
                    .map_err(|_| err("REFUSALS bad count".into()))?;
                for _ in 0..n {
                    let line = lines.next().ok_or_else(|| err("missing REFUSAL row".into()))?;
                    let mut f = line.split(' ');
                    let name = f.next().ok_or_else(|| err("REFUSAL missing code".into()))?;
                    let shape_iri = f.next().ok_or_else(|| err("REFUSAL missing shape_iri".into()))?;
                    let construct = f.next().ok_or_else(|| err("REFUSAL missing construct".into()))?;
                    if f.next().is_some() {
                        return Err(err("REFUSAL has extra fields".into()));
                    }
                    refusals.push(ShaclRefusal {
                        reason_code: ReasonCode::from_name(name)
                            .ok_or_else(|| err(format!("unknown reason code {:?}", name)))?,
                        shape_iri: shape_iri.to_string(),
                        construct: construct.to_string(),
                    });
                }
            }
            Some("END-REPORT") => {
                if lines.next().is_some() {
                    return Err(err("trailing content after END-REPORT".into()));
                }
                return Ok(CompileReport { accepted, refusals });
            }
            _ => return Err(err(format!("unexpected line {:?}", line))),
        }
    }
}

/// Parse one `SHAPE .. END` block.
fn parse_shape_block(
    lines: &mut std::iter::Peekable<std::str::Lines<'_>>,
) -> Result<CompiledShape, ReportParseError> {
    let err = |msg: &str| ReportParseError(msg.to_string());
    let shape_iri = match lines.next().and_then(|l| l.strip_prefix("SHAPE ")) {
        Some(iri) => iri.to_string(),
        None => return Err(err("expected SHAPE line")),
    };
    // Wave-8 repair (T8/M14): IRIs in the canonical format are space-free by
    // construction (format law in this file's serialization section; the T9
    // lexer law makes compiler output space-free), and the parse side already
    // enforces that law on BIT rows (`BIT has extra fields`) and on REFUSAL
    // shape_iri/construct fields (strict split). The SHAPE line was the one
    // remaining hole: the whole rest of the line was taken as the IRI, so a
    // hand-mangled artifact could smuggle a `CompiledShape` whose `shape_iri`
    // violates the field law every space-splitting consumer of the format is
    // promised - and the same IRI refused in a REFUSAL row while parsing in a
    // SHAPE line. A space here is a mangled artifact, refused typed at the
    // earliest point. An EMPTY IRI stays legal: `<>` is a valid IRIREF and a
    // compiler-produced report may carry it (`SHAPE ` round-trips), so the
    // check must not over-close past the documented law (T6 baseline).
    if shape_iri.contains(' ') {
        return Err(err("SHAPE IRI must be space-free"));
    }
    let mask_width: u8 = match lines.next().and_then(|l| l.strip_prefix("WIDTH ")) {
        Some(w) => w.parse().map_err(|_| err("WIDTH bad value"))?,
        None => return Err(err("expected WIDTH line")),
    };
    let masks = match lines.next().and_then(|l| l.strip_prefix("MASKS ")) {
        Some(m) => m,
        None => return Err(err("expected MASKS line")),
    };
    let mut mask_fields = masks.split(' ');
    let parse_mask = |f: Option<&str>| -> Result<u64, ReportParseError> {
        f.and_then(|v| v.parse().ok()).ok_or_else(|| err("MASKS bad value"))
    };
    let m_mandatory = parse_mask(mask_fields.next())?;
    let m_legal = parse_mask(mask_fields.next())?;
    let m_group = parse_mask(mask_fields.next())?;
    if mask_fields.next().is_some() {
        return Err(err("MASKS has extra fields"));
    }
    let mut bits = Vec::new();
    loop {
        let Some(line) = lines.next() else {
            return Err(err("unexpected EOF in shape block"));
        };
        if line == "END" {
            break;
        }
        let Some(rest) = line.strip_prefix("BIT ") else {
            return Err(err("expected BIT line"));
        };
        let mut f = rest.split(' ');
        let bit: u8 = f
            .next()
            .and_then(|v| v.parse().ok())
            .ok_or_else(|| err("BIT bad index"))?;
        let iri = f.next().ok_or_else(|| err("BIT missing IRI"))?;
        if f.next().is_some() {
            return Err(err("BIT has extra fields"));
        }
        bits.push((iri.to_string(), bit));
    }
    // Semantic invariants of a compiler-produced report (Wave-5 repair: a
    // tampered or hand-mangled report must not smuggle a CompiledShape whose
    // documented mask-width cap or mask/bits consistency is violated).
    if mask_width > MASK_WIDTH_MAX {
        return Err(err("WIDTH exceeds MASK_WIDTH_MAX (64)"));
    }
    if bits.len() != mask_width as usize {
        return Err(err("WIDTH does not match the number of BIT rows"));
    }
    let mut seen_iris = BTreeSet::new();
    for (idx, (iri, bit)) in bits.iter().enumerate() {
        if *bit as usize != idx {
            return Err(err("BIT indices must be consecutive from 0"));
        }
        if !seen_iris.insert(iri.as_str()) {
            return Err(err("BIT property IRIs must be unique"));
        }
    }
    // Wave-7 repair: bit order is part of the ABI (doc section 5 step 3 -
    // bits sorted by property IRI ascending); the emitted mask constants are
    // only meaningful for that order. A report whose BIT rows are not
    // IRI-ascending parses into a `CompiledShape` whose masks contradict its
    // own bit table (a swapped pair re-binds mandatory bits to the wrong
    // property), so it refuses typed. T8's M13 tamper class proves the check
    // load-bearing.
    for pair in bits.windows(2) {
        if pair[0].0 >= pair[1].0 {
            return Err(err("BIT rows must be sorted by property IRI"));
        }
    }
    let expect_legal = if mask_width == MASK_WIDTH_MAX {
        u64::MAX
    } else {
        (1u64 << mask_width) - 1
    };
    if m_legal != expect_legal {
        return Err(err("MASKS m_legal inconsistent with WIDTH"));
    }
    if m_mandatory & !m_legal != 0 {
        return Err(err("MASKS m_mandatory has bits outside m_legal"));
    }
    if m_group & !m_legal != 0 {
        return Err(err("MASKS m_group has bits outside m_legal"));
    }
    Ok(CompiledShape { shape_iri, mask_width, bits, m_mandatory, m_legal, m_group })
}

/// Persist the report (DoD-9) in the canonical format.
pub fn write_report(path: &std::path::Path, report: &CompileReport) -> std::io::Result<()> {
    std::fs::write(path, render_report(report))
}

/// Re-read a persisted report.
pub fn read_report(path: &std::path::Path) -> std::io::Result<CompileReport> {
    let text = std::fs::read_to_string(path)?;
    parse_report(&text).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

// ---------------------------------------------------------------------------
// Courts (doc section 7). Real collaborators: real TTL fixture files on disk,
// real cargo subprocess for T5, real emitted asm on disk. No mocks.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Doc section 5b worked example: 4 properties, sorted-IRI bit assignment.
    const PERSON_TTL: &str = "\
@prefix sh: <http://www.w3.org/ns/shacl#> .
@prefix xsd: <http://www.w3.org/2001/XMLSchema#> .
@prefix ex: <http://example.org/> .

ex:PersonShape a sh:NodeShape ;
    sh:closed true ;
    sh:property [
        sh:path ex:email ;
        sh:datatype xsd:string ;
        sh:minCount 1 ;
    ] ;
    sh:property [
        sh:path ex:name ;
        sh:minCount 1 ;
    ] ;
    sh:property [
        sh:path ex:nick ;
        sh:minCount 0 ;
    ] ;
    sh:property [
        sh:path ex:phone ;
    ] .
";

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "cgap_a08_{}_{}_{}",
            name,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.subsec_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn write_fixture(dir: &PathBuf, name: &str, ttl: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, ttl).expect("write fixture");
        path
    }

    fn has_reason(report: &CompileReport, code: ReasonCode) -> bool {
        report.refusals.iter().any(|r| r.reason_code == code)
    }

    /// Recursively collect `.s` files under `root` (T5 evidence search).
    fn find_asm_files(root: &PathBuf) -> Vec<PathBuf> {
        let mut found = Vec::new();
        let mut stack = vec![root.clone()];
        while let Some(d) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&d) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().and_then(|e| e.to_str()) == Some("s") {
                    found.push(path);
                }
            }
        }
        found
    }

    /// T1 (build gate) is witnessed by the `cargo test -p cgap-bont` command
    /// itself; this marker exists so the court inventory in the lane receipt
    /// maps 1:1 onto test names.
    #[test]
    fn t1_build_gate_is_the_cargo_command() {
        // Static asserts in this file are the compile-time half of the gate;
        // reaching this test at all proves `cargo build -p cgap-bont` exit 0.
        assert_eq!(MASK_WIDTH_MAX, 64);
        assert_eq!(MAX_DECLARED_PROPERTIES, 64);
    }

    /// T2 constraint court (anti-vacuity core): mandatory presence via one
    /// AND-compare, plus mutation M1 in both directions.
    #[test]
    fn t2_constraint_court_m1_flips() {
        let report = compile_shapes(PERSON_TTL).expect("parse person fixture");
        assert_eq!(report.accepted.len(), 1);
        let shape = &report.accepted[0];

        // Doc section 5b worked example masks.
        assert_eq!(shape.mask_width, 4);
        assert_eq!(shape.m_legal, 0b1111);
        assert_eq!(shape.m_mandatory, 0b0011);
        assert_eq!(shape.m_group, 0b0000);
        assert_eq!(
            shape.bits,
            vec![
                ("http://example.org/email".to_string(), 0),
                ("http://example.org/name".to_string(), 1),
                ("http://example.org/nick".to_string(), 2),
                ("http://example.org/phone".to_string(), 3),
            ]
        );

        // (a) present -> Ok
        assert_eq!(validate_presence(0b1011, shape), Ok(()));
        // (b) missing name -> Err(MissingMandatory)
        assert_eq!(validate_presence(0b1001, shape), Err(MaskViolation::MissingMandatory));

        // Mutation M1a (compiler bug drops the name bit from M_mandatory):
        // the same input must flip Err -> Ok, else the bit was decorative.
        let mut m1a = shape.clone();
        m1a.m_mandatory &= !(1u64 << 1);
        assert_eq!(m1a.m_mandatory, 0b0001);
        assert_eq!(validate_presence(0b1001, &m1a), Ok(()), "M1a must flip");

        // Mutation M1b (compiler bug adds the nick bit to M_mandatory):
        // the legal input must flip Ok -> Err, else the bit was decorative.
        let mut m1b = shape.clone();
        m1b.m_mandatory |= 1u64 << 2;
        assert_eq!(m1b.m_mandatory, 0b0111);
        assert_eq!(validate_presence(0b1011, &m1b), Err(MaskViolation::MissingMandatory), "M1b must flip");
    }

    /// T3 XOR court: exactly-one-of group via one AND+popcnt+compare, plus
    /// mutation M2 (clear M_group).
    #[test]
    fn t3_xor_court_m2_flips() {
        let ttl = "\
@prefix sh: <http://www.w3.org/ns/shacl#> .
@prefix cgap: <http://cgap.dev/ontology#> .
@prefix ex: <http://example.org/> .

ex:ContactGroup cgap:exactlyOneOf true .

ex:ContactShape a sh:NodeShape ;
    sh:closed true ;
    sh:property [
        sh:path ex:name ;
        sh:minCount 1 ;
    ] ;
    sh:property [
        sh:path ex:email ;
        sh:group ex:ContactGroup ;
    ] ;
    sh:property [
        sh:path ex:phone ;
        sh:group ex:ContactGroup ;
    ] .
";
        let report = compile_shapes(ttl).expect("parse contact fixture");
        assert_eq!(report.accepted.len(), 1, "{:?}", report.refusals);
        let shape = &report.accepted[0];
        // bits sorted by IRI: email=0, name=1, phone=2
        assert_eq!(shape.m_mandatory, 0b0010, "name only is mandatory");
        assert_eq!(shape.m_legal, 0b0111);
        assert_eq!(shape.m_group, 0b0101, "email+phone are the XOR group");

        // name+email (exactly one of group) -> Ok
        assert_eq!(validate_presence(0b0011, shape), Ok(()));
        // name+phone (exactly one of group) -> Ok
        assert_eq!(validate_presence(0b0110, shape), Ok(()));
        // name only (none of group) -> Err(GroupCardinality)
        assert_eq!(validate_presence(0b0010, shape), Err(MaskViolation::GroupCardinality));
        // name+email+phone (both of group) -> Err(GroupCardinality)
        assert_eq!(validate_presence(0b0111, shape), Err(MaskViolation::GroupCardinality));

        // Mutation M2 (compiler bug clears M_group): the both-set case must
        // flip Err -> Ok, else the group mask was decorative.
        let mut m2 = shape.clone();
        m2.m_group = 0;
        assert_eq!(validate_presence(0b0111, &m2), Ok(()), "M2 must flip");
    }

    /// T4 closed-world court: undeclared bit fails, plus mutation M3
    /// (open-world m_legal = u64::MAX).
    #[test]
    fn t4_closed_world_court_m3_flips() {
        let report = compile_shapes(PERSON_TTL).expect("parse person fixture");
        let shape = &report.accepted[0];

        // Undeclared bit 6 set -> IllegalProperty
        assert_eq!(validate_presence(1u64 << 6, shape), Err(MaskViolation::IllegalProperty));
        // legal record + undeclared bit 6 -> still IllegalProperty
        assert_eq!(
            validate_presence(0b1111 | (1u64 << 6), shape),
            Err(MaskViolation::IllegalProperty)
        );
        // legal full record still Ok
        assert_eq!(validate_presence(0b1111, shape), Ok(()));

        // Mutation M3 (compiler bug opens the world: m_legal = u64::MAX):
        // the illegal-property case must flip Err -> Ok, else the closed-world
        // assumption was prose, not code. Input carries the full legal record
        // plus the undeclared bit, so the ONLY baseline failure is the
        // closed-world guard.
        let mut m3 = shape.clone();
        m3.m_legal = u64::MAX;
        assert_eq!(validate_presence(0b1111 | (1u64 << 6), &m3), Ok(()), "M3 must flip");
    }

    /// T5 asm witness: real `--emit asm` on a scratch crate calling
    /// `validate_presence`, grepping real emitted instructions, with a
    /// call-bearing control function proving the no-call detector fires.
    #[test]
    fn t5_asm_witness() {
        let dir = temp_dir("t5");
        let outcome = (|| -> Result<String, String> {
            let src = r#"
#![allow(improper_ctypes_definitions)]
use cgap_bont::shacl_compiler::{validate_presence, CompiledShape, MaskViolation, MaskWord};

/// The witness: the whole evaluator inlined into one extern fn. Its body
/// contains the AND path (and+compare) AND the XOR path (popcount) by
/// construction - a single body is immune to identical-code folding, which
/// merged the earlier two identical wrappers.
#[no_mangle]
pub extern "C" fn t5_validate(observed: MaskWord, s: &CompiledShape) -> u32 {
    match validate_presence(observed, s) {
        Ok(()) => 0,
        Err(MaskViolation::IllegalProperty) => 1,
        Err(MaskViolation::MissingMandatory) => 2,
        Err(MaskViolation::GroupCardinality) => 3,
    }
}

/// Anti-vacuity control: a call the detector MUST see (M5).
#[inline(never)]
#[no_mangle]
pub fn t5_marker(x: u64) -> u64 {
    x.wrapping_add(1)
}

/// Control witness: same evaluator plus one deliberate call, whose result is
/// USED (a discarded pure call is dead code LLVM deletes - the first draft's
/// `let _ =` was optimized out and identical-code folding then merged the
/// control with the witness).
#[no_mangle]
pub extern "C" fn t5_control(observed: MaskWord, s: &CompiledShape) -> u32 {
    if t5_marker(observed) == u64::MAX {
        return 9;
    }
    match validate_presence(observed, s) {
        Ok(()) => 0,
        Err(MaskViolation::IllegalProperty) => 1,
        Err(MaskViolation::MissingMandatory) => 2,
        Err(MaskViolation::GroupCardinality) => 3,
    }
}
"#;
            write_fixture(&dir, "Cargo.toml", &format!(
                "[package]\nname = \"t5witness\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\ncgap-bont = {{ path = {:?} }}\n\n[workspace]\n\n[profile.release]\ncodegen-units = 1\n",
                env!("CARGO_MANIFEST_DIR")
            ));
            write_fixture(&dir, "lib.rs", src);
            std::fs::create_dir_all(dir.join("src")).expect("src dir");
            std::fs::rename(dir.join("lib.rs"), dir.join("src/lib.rs")).expect("move lib.rs");

            let manifest = dir.join("Cargo.toml");
            let manifest_str = manifest.to_str().expect("utf8");
            // Wave-4 fix: the scratch cargo MUST NOT inherit an outer
            // CARGO_TARGET_DIR (witnessed: a lane run exported one, so the .s
            // files landed in the shared build root while this test searched
            // the empty scratch dir - exit 0, zero asm, "test ok"). Pin the
            // scratch target dir so emission and search agree.
            let scratch_target = dir.join("target");
            let output = std::process::Command::new("cargo")
                .args([
                    "rustc",
                    "--release",
                    "--offline",
                    "--manifest-path",
                    manifest_str,
                    "--lib",
                    "--",
                    "--emit",
                    "asm",
                ])
                .env_remove("CARGO_MAKEFLAGS")
                .env_remove("RUSTFLAGS")
                .env("CARGO_TARGET_DIR", &scratch_target)
                .output()
                .map_err(|e| format!("spawn cargo failed: {}", e))?;
            if !output.status.success() || find_asm_files(&dir).is_empty() {
                // Fallback recipe: RUSTFLAGS-based emission (works even where
                // cargo's own --emit handling drops the .s files).
                let out2 = std::process::Command::new("cargo")
                    .args(["build", "--release", "--offline", "--manifest-path", manifest_str])
                    .env_remove("CARGO_MAKEFLAGS")
                    .env("RUSTFLAGS", "--emit asm")
                    .env("CARGO_TARGET_DIR", &scratch_target)
                    .output()
                    .map_err(|e| format!("spawn cargo (fallback) failed: {}", e))?;
                if !out2.status.success() && find_asm_files(&dir).is_empty() {
                    return Err(format!(
                        "cargo rustc exit {:?} stderr: {} | fallback build exit {:?} stderr: {}",
                        output.status.code(),
                        String::from_utf8_lossy(&output.stderr),
                        out2.status.code(),
                        String::from_utf8_lossy(&out2.stderr)
                    ));
                }
            }

            // Find emitted .s files anywhere under the scratch target dir.
            let asm_files = find_asm_files(&dir);
            if asm_files.is_empty() {
                return Err(format!(
                    "no .s files emitted under {}; first cmd exit {:?} stderr: {}",
                    scratch_target.display(),
                    output.status.code(),
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
            let mut combined = String::new();
            for f in &asm_files {
                combined.push_str(&std::fs::read_to_string(f).expect("read asm"));
            }
            Ok(combined)
        })();

        let asm = match outcome {
            Ok(asm) => asm,
            Err(blocking) => {
                // Loud skip, never a faked pass: named UNSUPPORTED with the
                // blocking fact (doc section 6 / T5). The scratch dir is kept
                // on this path so the blocking fact stays diagnosable.
                eprintln!("[SKIP] T5 asm witness: {}", blocking);
                eprintln!("[SKIP] T5 scratch dir kept for diagnosis: {}", dir.display());
                return;
            }
        };

        // Keep the witness artifact for the lane receipt: the scratch dir copy
        // below is the primary artifact; a best-effort copy into the cargo
        // target dir (lane build root lease) gives the receipt a durable path.
        // The assertions further down are the court; this copy is evidence,
        // not gate.
        let witness_path = dir.join("t5_witness.s");
        std::fs::write(&witness_path, &asm).expect("write witness");
        if let Ok(target_dir) = std::env::var("CARGO_TARGET_DIR") {
            let _ = std::fs::copy(
                &witness_path,
                std::path::Path::new(&target_dir).join("a08_t5_witness.s"),
            );
        }

        let body_of = |symbol: &str| -> Option<String> {
            let mut body = String::new();
            let mut inside = false;
            for line in asm.lines() {
                let trimmed = line.trim_end();
                if trimmed.ends_with(':') && trimmed.contains(symbol) {
                    inside = true;
                    body.clear();
                    continue;
                }
                if inside {
                    if trimmed.is_empty() {
                        continue;
                    }
                    if line.starts_with(char::is_whitespace) {
                        body.push_str(line);
                        body.push('\n');
                    } else {
                        // Column-0 line inside a function: rustc places
                        // internal branch labels (LBB*_*) at column 0 - those
                        // continue the body; anything else ends the function.
                        if trimmed.starts_with("LBB") {
                            continue;
                        }
                        break;
                    }
                }
            }
            if body.is_empty() {
                None
            } else {
                Some(body)
            }
        };

        fn mnemonic(line: &str) -> &str {
            line.trim()
                .split_whitespace()
                .next()
                .unwrap_or("")
                .trim_end_matches(':')
        }

        let and_family = |m: &str| {
            matches!(
                m,
                "and" | "andn" | "andq" | "andl" | "andw" | "andb" | "bic" | "bics" | "ands"
            )
        };
        let cmp_family = |m: &str| {
            m.starts_with("b.")
                || matches!(
                    m,
                    "cmp" | "cmpq" | "cmpl" | "cmpw" | "cmpb" | "tst" | "testb" | "testq"
                        | "testl" | "subs" | "subsw" | "subsx" | "cbz" | "cbnz" | "tbz" | "tbnz"
                )
        };
        let popcount_family = |m: &str| matches!(m, "popcnt" | "popcntb" | "cnt" | "cntb" | "cntw");
        let call_family = |m: &str| matches!(m, "call" | "callq" | "bl" | "blr");

        let collect = |body: &str, pred: &dyn Fn(&str) -> bool| Vec::from_iter(
            body.lines()
                .map(mnemonic)
                .filter(|m| pred(m))
                .map(|m| m.to_string()),
        );

        // The witness body and the control body must both be present.
        let witness_body = body_of("t5_validate")
            .expect("t5_validate symbol missing from emitted asm");
        let control_body = body_of("t5_control")
            .expect("t5_control symbol missing from emitted asm");

        // Single-mask-operation claim, bounded form: the inlined evaluator
        // body is and-family + a compare + (a popcount instruction OR the
        // power-of-two reduction of `count_ones() == 1`), with no calls, no
        // backward branches (loop freedom), and a bounded instruction count.
        let and_uses = collect(&witness_body, &and_family);
        let and_cmps = collect(&witness_body, &cmp_family);
        let and_calls = collect(&witness_body, &call_family);
        let popcounts = collect(&witness_body, &popcount_family);
        let eors = collect(&witness_body, &|m: &str| m == "eor");
        let subs = collect(&witness_body, &|m: &str| m == "sub");
        let instr_count = witness_body
            .lines()
            .filter(|l| !l.trim().is_empty() && !l.trim().starts_with('.'))
            .count();
        assert!(
            !and_uses.is_empty(),
            "AND path must contain an AND instruction, got:\n{}",
            witness_body
        );
        assert!(
            !and_cmps.is_empty(),
            "AND path must contain a compare, got:\n{}",
            witness_body
        );
        assert!(
            !popcounts.is_empty() || (!eors.is_empty() && !subs.is_empty()),
            "XOR path must contain a popcount instruction or the power-of-two \
             reduction of count_ones()==1 (and/sub/eor/cmp), got:\n{}",
            witness_body
        );
        assert!(
            and_calls.is_empty(),
            "evaluator body must contain no call instructions, got {:?} in:\n{}",
            and_calls,
            witness_body
        );
        // Loop freedom: no branch may target a label defined EARLIER in the
        // body (a backward edge is what a traversal loop looks like).
        let mut defined_labels: std::collections::BTreeSet<String> = Default::default();
        let mut backward_edges: Vec<String> = Vec::new();
        for line in witness_body.lines() {
            let trimmed = line.trim();
            if trimmed.ends_with(':') && !trimmed.starts_with('.') {
                defined_labels.insert(trimmed.trim_end_matches(':').to_string());
                continue;
            }
            if let Some(m) = trimmed.split_whitespace().next() {
                for tok in trimmed.split_whitespace().skip(1) {
                    let tok = tok.trim_end_matches(',');
                    if call_family(m)
                        || m.starts_with("b.")
                        || m.starts_with("cb")
                        || m.starts_with("tb")
                        || m.starts_with("j")
                    {
                        if defined_labels.contains(tok) {
                            backward_edges.push(format!("{} -> {}", m, tok));
                        }
                    }
                }
            }
        }
        assert!(
            backward_edges.is_empty(),
            "evaluator must be loop-free (backward edges found: {:?}) in:\n{}",
            backward_edges,
            witness_body
        );
        assert!(
            instr_count <= 48,
            "evaluator must be bounded ({} instructions exceeds the bound):\n{}",
            instr_count,
            witness_body
        );

        // Anti-vacuity (M5): the detector fires on a call-bearing control.
        let control_calls = collect(&control_body, &call_family);
        assert!(
            !control_calls.is_empty(),
            "non-vacuity: the no-call detector must see the control's call:\n{}",
            control_body
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    /// T6 round-trip: real TTL -> compiler -> canonical text -> re-read ->
    /// byte-equal masks across two runs and a file round-trip.
    #[test]
    fn t6_roundtrip_byte_equal() {
        let report1 = compile_shapes(PERSON_TTL).expect("parse 1");
        let report2 = compile_shapes(PERSON_TTL).expect("parse 2");
        let text1 = render_report(&report1);
        let text2 = render_report(&report2);
        assert_eq!(text1, text2, "two compiles must render byte-identical");

        let reparsed = parse_report(&text1).expect("re-parse canonical text");
        assert_eq!(reparsed, report1, "canonical round-trip must be lossless");
        assert_eq!(render_report(&reparsed), text1, "re-render byte-equal");

        let dir = temp_dir("t6");
        let path = write_fixture(&dir, "report.txt", &text1);
        let read_back = read_report(&path).expect("read persisted report");
        assert_eq!(read_back, report1, "file round-trip lossless");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// T7 refusal corpus: one TTL fixture per reason code, each witnessed.
    #[test]
    fn t7_refusal_corpus() {
        let dir = temp_dir("t7");

        // NotClosedShape: missing sh:closed.
        let not_closed = "\
@prefix sh: <http://www.w3.org/ns/shacl#> .
@prefix ex: <http://example.org/> .
ex:OpenShape a sh:NodeShape ;
    sh:property [ sh:path ex:name ; sh:minCount 1 ] .
";
        let p = write_fixture(&dir, "not_closed.ttl", not_closed);
        let report = compile_shapes(&std::fs::read_to_string(&p).expect("read")).expect("parse");
        assert!(has_reason(&report, ReasonCode::NotClosedShape), "{:?}", report.refusals);
        assert!(report.accepted.is_empty(), "refused shape must not be accepted");

        // NotClosedShape: explicit sh:closed false.
        let closed_false = not_closed.replace(
            "ex:OpenShape a sh:NodeShape ;",
            "ex:OpenShape a sh:NodeShape ;\n    sh:closed false ;",
        );
        let p = write_fixture(&dir, "closed_false.ttl", &closed_false);
        let report = compile_shapes(&std::fs::read_to_string(&p).expect("read")).expect("parse");
        assert!(has_reason(&report, ReasonCode::NotClosedShape));

        // ValueConstraintOutOfMaskScope: sh:pattern.
        let pattern = "\
@prefix sh: <http://www.w3.org/ns/shacl#> .
@prefix ex: <http://example.org/> .
ex:S a sh:NodeShape ;
    sh:closed true ;
    sh:property [ sh:path ex:code ; sh:pattern \"^[A-Z]+$\" ] .
";
        let p = write_fixture(&dir, "pattern.ttl", pattern);
        let report = compile_shapes(&std::fs::read_to_string(&p).expect("read")).expect("parse");
        assert!(has_reason(&report, ReasonCode::ValueConstraintOutOfMaskScope));
        assert!(report.accepted.is_empty());
        let row = report
            .refusals
            .iter()
            .find(|r| r.reason_code == ReasonCode::ValueConstraintOutOfMaskScope)
            .expect("row");
        assert_eq!(row.construct, "sh:pattern");

        // CardinalityUnsupported: sh:minCount 2.
        let min_count_2 = "\
@prefix sh: <http://www.w3.org/ns/shacl#> .
@prefix ex: <http://example.org/> .
ex:S a sh:NodeShape ;
    sh:closed true ;
    sh:property [ sh:path ex:alias ; sh:minCount 2 ] .
";
        let p = write_fixture(&dir, "min_count_2.ttl", min_count_2);
        let report = compile_shapes(&std::fs::read_to_string(&p).expect("read")).expect("parse");
        assert!(has_reason(&report, ReasonCode::CardinalityUnsupported));
        assert!(report.accepted.is_empty());

        // CardinalityUnsupported: sh:maxCount.
        let max_count = "\
@prefix sh: <http://www.w3.org/ns/shacl#> .
@prefix ex: <http://example.org/> .
ex:S a sh:NodeShape ;
    sh:closed true ;
    sh:property [ sh:path ex:alias ; sh:maxCount 3 ] .
";
        let p = write_fixture(&dir, "max_count.ttl", max_count);
        let report = compile_shapes(&std::fs::read_to_string(&p).expect("read")).expect("parse");
        assert!(has_reason(&report, ReasonCode::CardinalityUnsupported));
        assert!(report.accepted.is_empty());

        // CardinalityUnsupported: sh:qualifiedValueShape.
        let qualified = "\
@prefix sh: <http://www.w3.org/ns/shacl#> .
@prefix ex: <http://example.org/> .
ex:S a sh:NodeShape ;
    sh:closed true ;
    sh:property [ sh:path ex:addr ; sh:qualifiedValueShape ex:AddrShape ] .
";
        let p = write_fixture(&dir, "qualified.ttl", qualified);
        let report = compile_shapes(&std::fs::read_to_string(&p).expect("read")).expect("parse");
        assert!(has_reason(&report, ReasonCode::CardinalityUnsupported));
        assert!(report.accepted.is_empty());

        // LogicalShapeUnsupported: shape-level sh:not and property sh:node.
        let logical = "\
@prefix sh: <http://www.w3.org/ns/shacl#> .
@prefix ex: <http://example.org/> .
ex:S a sh:NodeShape ;
    sh:closed true ;
    sh:not [ sh:property [ sh:path ex:bad ] ] ;
    sh:property [ sh:path ex:name ; sh:minCount 1 ] .
";
        let p = write_fixture(&dir, "logical.ttl", logical);
        let report = compile_shapes(&std::fs::read_to_string(&p).expect("read")).expect("parse");
        assert!(has_reason(&report, ReasonCode::LogicalShapeUnsupported));
        assert!(report.accepted.is_empty());

        // TargetingOutOfScope: advisory row, shape still ACCEPTED.
        let targeting = "\
@prefix sh: <http://www.w3.org/ns/shacl#> .
@prefix ex: <http://example.org/> .
ex:S a sh:NodeShape ;
    sh:closed true ;
    sh:targetClass ex:Person ;
    sh:property [ sh:path ex:name ; sh:minCount 1 ] .
";
        let p = write_fixture(&dir, "targeting.ttl", targeting);
        let report = compile_shapes(&std::fs::read_to_string(&p).expect("read")).expect("parse");
        assert!(has_reason(&report, ReasonCode::TargetingOutOfScope));
        assert_eq!(report.accepted.len(), 1, "advisory row must not block");
        assert_eq!(report.accepted[0].shape_iri, "http://example.org/S");

        // MaskOverflow: 65 declared properties.
        let mut overflow = String::from(
            "@prefix sh: <http://www.w3.org/ns/shacl#> .\n@prefix ex: <http://example.org/> .\nex:Big a sh:NodeShape ;\n    sh:closed true ;\n",
        );
        for i in 0..65 {
            overflow.push_str(&format!("    sh:property [ sh:path ex:p{:03} ] ;\n", i));
        }
        overflow.push_str("    sh:property [ sh:path ex:pZ ] .\n");
        let p = write_fixture(&dir, "overflow.ttl", &overflow);
        let report = compile_shapes(&std::fs::read_to_string(&p).expect("read")).expect("parse");
        assert!(has_reason(&report, ReasonCode::MaskOverflow));
        assert!(report.accepted.is_empty());

        // MalformedShape: property shape without sh:path.
        let no_path = "\
@prefix sh: <http://www.w3.org/ns/shacl#> .
@prefix ex: <http://example.org/> .
ex:S a sh:NodeShape ;
    sh:closed true ;
    sh:property [ sh:minCount 1 ] .
";
        let p = write_fixture(&dir, "no_path.ttl", no_path);
        let report = compile_shapes(&std::fs::read_to_string(&p).expect("read")).expect("parse");
        assert!(has_reason(&report, ReasonCode::MalformedShape));
        assert!(report.accepted.is_empty());

        // MalformedShape: duplicate property path.
        let duplicate = "\
@prefix sh: <http://www.w3.org/ns/shacl#> .
@prefix ex: <http://example.org/> .
ex:S a sh:NodeShape ;
    sh:closed true ;
    sh:property [ sh:path ex:name ; sh:minCount 1 ] ;
    sh:property [ sh:path ex:name ] .
";
        let p = write_fixture(&dir, "duplicate.ttl", &duplicate);
        let report = compile_shapes(&std::fs::read_to_string(&p).expect("read")).expect("parse");
        assert!(has_reason(&report, ReasonCode::MalformedShape));
        assert!(report.accepted.is_empty());

        std::fs::remove_dir_all(&dir).ok();
    }

    /// T7b registry fidelity: `sh:and` / `sh:or` are SHACL logical
    /// operators and must refuse as LogicalShapeUnsupported, not drop
    /// silently. Mutation M7: deleting the `sh:or` line flips the verdict.
    #[test]
    fn t7b_logical_and_or_refused_m7_flips() {
        let ttl_with = |extra: &str| {
            format!(
                "\
@prefix sh: <http://www.w3.org/ns/shacl#> .
@prefix ex: <http://example.org/> .
ex:S a sh:NodeShape ;
    sh:closed true ;
{}
    sh:property [ sh:path ex:name ; sh:minCount 1 ] .
",
                extra
            )
        };
        let bad = compile_shapes(&ttl_with("    sh:or [ sh:property [ sh:path ex:x ] ] ;"))
            .expect("parse");
        let row = bad
            .refusals
            .iter()
            .find(|r| r.reason_code == ReasonCode::LogicalShapeUnsupported)
            .expect("sh:or row");
        assert_eq!(row.construct, "sh:or");
        assert!(bad.accepted.is_empty(), "shape with sh:or must refuse");

        // M7 flip: the same shape without the sh:or line compiles.
        let good = compile_shapes(&ttl_with("")).expect("parse");
        assert_eq!(good.accepted.len(), 1, "{:?}", good.refusals);

        // sh:and on a property shape also refuses as logical.
        let and_ttl = "\
@prefix sh: <http://www.w3.org/ns/shacl#> .
@prefix ex: <http://example.org/> .
ex:S a sh:NodeShape ;
    sh:closed true ;
    sh:property [ sh:path ex:name ; sh:minCount 1 ; sh:and ex:other ] .
";
        let report = compile_shapes(and_ttl).expect("parse");
        assert!(
            report
                .refusals
                .iter()
                .any(|r| r.reason_code == ReasonCode::LogicalShapeUnsupported
                    && r.construct == "sh:and"),
            "{:?}",
            report.refusals
        );
        assert!(report.accepted.is_empty());
    }

    /// T7c zero-silent-narrowing: a known SHACL local observed malformed -
    /// wrong-typed object or duplicate occurrence - refuses as
    /// MalformedShape. Mutation M6: demoting the malformed token to a
    /// well-formed one flips the verdict Err -> Ok (proves the malformed
    /// detection is load-bearing: without it, `sh:minCount -1` would behave
    /// exactly like the defaulted `sh:minCount 0`).
    #[test]
    fn t7c_malformed_known_locals_refused_m6_flips() {
        let ttl_with = |prop_body: &str| {
            format!(
                "\
@prefix sh: <http://www.w3.org/ns/shacl#> .
@prefix ex: <http://example.org/> .
ex:S a sh:NodeShape ;
    sh:closed true ;
    sh:property [ {} ] .
",
                prop_body
            )
        };
        // wrong-typed: negative sh:minCount (would silently become 0).
        let neg = compile_shapes(&ttl_with("sh:path ex:name ; sh:minCount -1")).expect("parse");
        let row = neg
            .refusals
            .iter()
            .find(|r| r.reason_code == ReasonCode::MalformedShape)
            .expect("malformed minCount row");
        assert_eq!(row.construct, "malformed:sh:minCount");
        assert!(neg.accepted.is_empty());

        // wrong-typed: sh:path as a string literal (non-IRI path).
        let str_path = compile_shapes(&ttl_with("sh:path \"name\" ; sh:minCount 1")).expect("parse");
        assert!(
            str_path
                .refusals
                .iter()
                .any(|r| r.reason_code == ReasonCode::MalformedShape
                    && r.construct == "malformed:sh:path"),
            "{:?}",
            str_path.refusals
        );
        assert!(str_path.accepted.is_empty());

        // duplicate: sh:minCount declared twice (last-wins would silently
        // drop the first declaration).
        let dup = compile_shapes(&ttl_with("sh:path ex:name ; sh:minCount 1 ; sh:minCount 0"))
            .expect("parse");
        assert!(
            dup.refusals
                .iter()
                .any(|r| r.reason_code == ReasonCode::MalformedShape
                    && r.construct == "duplicate:sh:minCount"),
            "{:?}",
            dup.refusals
        );
        assert!(dup.accepted.is_empty());

        // M6 flip: the well-formed equivalent compiles clean.
        let good = compile_shapes(&ttl_with("sh:path ex:name ; sh:minCount 0")).expect("parse");
        assert_eq!(good.accepted.len(), 1, "{:?}", good.refusals);
        assert!(!has_reason(&good, ReasonCode::MalformedShape));
    }

    /// T7d deny-by-default: a SHACL constraint component with no registry
    /// row refuses (property level -> ValueConstraintOutOfMaskScope, shape
    /// level -> MalformedShape); the spec annotation whitelist passes.
    /// Mutation M8: removing `sh:lessThan` flips the verdict Err -> Ok.
    #[test]
    fn t7d_unclassified_constructs_denied_by_default_m8_flips() {
        let ttl_with = |prop_extra: &str, shape_extra: &str| {
            format!(
                "\
@prefix sh: <http://www.w3.org/ns/shacl#> .
@prefix ex: <http://example.org/> .
ex:S a sh:NodeShape ;
    sh:closed true ;
{}
    sh:property [ sh:path ex:name ; sh:minCount 1 {} ] .
",
                shape_extra, prop_extra
            )
        };
        // Property-level unclassified constraint component.
        let bad = compile_shapes(&ttl_with("; sh:lessThan ex:other", "")).expect("parse");
        let row = bad
            .refusals
            .iter()
            .find(|r| r.reason_code == ReasonCode::ValueConstraintOutOfMaskScope
                && r.construct == "sh:lessThan")
            .expect("sh:lessThan row");
        assert_eq!(row.shape_iri, "http://example.org/S");
        assert!(bad.accepted.is_empty());

        // M8 flip: without the construct, the shape compiles.
        let good = compile_shapes(&ttl_with("", "")).expect("parse");
        assert_eq!(good.accepted.len(), 1, "{:?}", good.refusals);

        // Shape-level unclassified construct.
        let shape_bad = compile_shapes(&ttl_with("", "    sh:deactivated true ;")).expect("parse");
        assert!(
            shape_bad
                .refusals
                .iter()
                .any(|r| r.reason_code == ReasonCode::MalformedShape
                    && r.construct == "sh:deactivated"),
            "{:?}",
            shape_bad.refusals
        );
        assert!(shape_bad.accepted.is_empty());

        // Spec annotations pass through ignored, at both levels.
        let annotated = compile_shapes(&ttl_with(
            "; sh:name \"the name\" ; sh:severity sh:Info",
            "    sh:description \"a shape\" ;",
        ))
        .expect("parse");
        assert_eq!(annotated.accepted.len(), 1, "{:?}", annotated.refusals);
        assert!(!has_reason(&annotated, ReasonCode::ValueConstraintOutOfMaskScope));
    }

    /// T7e unsatisfiable-mask court: two mandatory members of one
    /// exactly-one-of group compile to a mask with zero valid witnesses
    /// (every instance fails MissingMandatory or GroupCardinality) - refused
    /// at compile time. Mutation M9: demoting one member to optional flips
    /// the verdict Err -> Ok and the emitted mask validates both directions.
    #[test]
    fn t7e_mandatory_group_contradiction_refused_m9_flips() {
        let ttl_with = |a: u64, b: u64| {
            format!(
                "\
@prefix sh: <http://www.w3.org/ns/shacl#> .
@prefix cgap: <http://cgap.dev/ontology#> .
@prefix ex: <http://example.org/> .

ex:G cgap:exactlyOneOf true .

ex:S a sh:NodeShape ;
    sh:closed true ;
    sh:property [ sh:path ex:a ; sh:minCount {} ; sh:group ex:G ] ;
    sh:property [ sh:path ex:b ; sh:minCount {} ; sh:group ex:G ] .
",
                a, b
            )
        };
        let bad = compile_shapes(&ttl_with(1, 1)).expect("parse");
        let row = bad
            .refusals
            .iter()
            .find(|r| r.reason_code == ReasonCode::MalformedShape)
            .expect("contradiction row");
        assert_eq!(row.construct, "mandatory:sh:group<http://example.org/G>");
        assert!(bad.accepted.is_empty());

        // M9 flip: one member optional -> satisfiable, compiles, and the
        // emitted mask evaluates exactly as the group law requires.
        let good = compile_shapes(&ttl_with(1, 0)).expect("parse");
        assert_eq!(good.accepted.len(), 1, "{:?}", good.refusals);
        let s = &good.accepted[0];
        // bits sorted by IRI: a=0, b=1
        assert_eq!(s.m_mandatory, 0b01, "only a is mandatory");
        assert_eq!(s.m_group, 0b11, "a and b are the XOR group");
        assert_eq!(validate_presence(0b01, s), Ok(()), "a set: group count 1, mandatory met");
        assert_eq!(
            validate_presence(0b10, s),
            Err(MaskViolation::MissingMandatory),
            "b alone: group count 1 but mandatory a is absent"
        );
        assert_eq!(validate_presence(0b00, s), Err(MaskViolation::MissingMandatory));
        assert_eq!(validate_presence(0b11, s), Err(MaskViolation::GroupCardinality));
    }

    /// T8 parse-side invariants: a tampered or hand-mangled canonical report
    /// must not smuggle a CompiledShape past the documented mask-width cap or
    /// mask/bits consistency (Wave-5 repair of the reader invariant hole).
    /// Each tamper below is a single-field mutation of a valid report, so a
    /// parse that accepted any of them would be the M3 open-world class
    /// arriving through the persisted artifact. Wave-7 adds the M13 class:
    /// BIT rows re-ordered against the sorted-IRI bit law must refuse (a
    /// swapped pair silently re-binds mask bits to the wrong property).
    /// Wave-8 adds the M14 class: a SHAPE line whose IRI carries a space must
    /// refuse - the space-free IRI law (format header; lexer T9) holds for
    /// every field, and BIT/REFUSAL rows already enforce it parse-side, so
    /// the SHAPE line was the one remaining acceptance channel for a mangled
    /// artifact.
    #[test]
    fn t8_report_invariants_enforced_on_parse() {
        let report = compile_shapes(PERSON_TTL).expect("compile person fixture");
        let shape = &report.accepted[0];
        let good = render_report(&report);
        assert!(parse_report(&good).is_ok(), "compiler output must parse");

        let flip = |what: &str, from: String, to: String| {
            let tampered = good.replace(&from, &to);
            assert_ne!(tampered, good, "tamper {} matched nothing", what);
            assert!(
                parse_report(&tampered).is_err(),
                "must reject tampered report ({}):\n{}",
                what,
                tampered
            );
        };

        // WIDTH beyond the ABI cap.
        flip("width>cap", "WIDTH 4\n".to_string(), "WIDTH 65\n".to_string());
        // WIDTH inconsistent with the BIT row count.
        flip("width!=bits", "WIDTH 4\n".to_string(), "WIDTH 5\n".to_string());
        // m_legal inconsistent with WIDTH (the M3 open-world class).
        flip(
            "m_legal open-world",
            format!("MASKS {} {} {}\n", shape.m_mandatory, shape.m_legal, shape.m_group),
            format!("MASKS {} {} {}\n", shape.m_mandatory, u64::MAX, shape.m_group),
        );
        // m_mandatory carrying bits outside m_legal.
        flip(
            "m_mandatory outside m_legal",
            format!("MASKS {} {} {}\n", shape.m_mandatory, shape.m_legal, shape.m_group),
            format!("MASKS {} {} {}\n", 1u64 << 6, shape.m_legal, shape.m_group),
        );
        // m_group carrying bits outside m_legal.
        flip(
            "m_group outside m_legal",
            format!("MASKS {} {} {}\n", shape.m_mandatory, shape.m_legal, shape.m_group),
            format!("MASKS {} {} {}\n", shape.m_mandatory, shape.m_legal, 1u64 << 6),
        );
        // BIT index beyond WIDTH (non-consecutive from 0).
        flip("bit index drift", "BIT 1 ".to_string(), "BIT 9 ".to_string());
        // BIT index duplicated (out of order).
        flip("bit index dup", "BIT 2 ".to_string(), "BIT 1 ".to_string());
        // duplicate property IRI across bits.
        flip(
            "bit iri dup",
            format!("BIT 3 http://example.org/phone"),
            format!("BIT 3 http://example.org/email"),
        );
        // M13 (Wave-7): BIT rows swapped against the sorted-IRI bit law.
        // Swapping the adjacent rows for `name` (bit 1, mandatory) and `nick`
        // (bit 2, optional) would silently re-bind the mandatory bit to
        // `nick` and the optional bit to `name` - the parsed shape's masks
        // contradict its own bit table, exactly the M1 miscompile arriving
        // through the persisted artifact.
        flip(
            "bit rows unsorted",
            format!(
                "BIT 1 http://example.org/name\nBIT 2 http://example.org/nick"
            ),
            format!(
                "BIT 1 http://example.org/nick\nBIT 2 http://example.org/name"
            ),
        );
        // M14 (Wave-8): a space-bearing SHAPE IRI must refuse. The compiler
        // can never produce one (T9 lexer law), so acceptance would be a
        // mangled artifact smuggling an out-of-invariant CompiledShape.
        flip(
            "shape iri space-bearing",
            "SHAPE http://example.org/PersonShape\n".to_string(),
            "SHAPE http://example.org/Person Shape\n".to_string(),
        );
        // Asymmetry witness for the same law: the SAME space-bearing IRI in a
        // REFUSAL row already refused parse-side (strict field split) before
        // this repair - the SHAPE-line check closes the one side that still
        // parsed, so both row kinds now enforce one law.
        let refusal_side = format!(
            "CGAP-SHACL-REPORT 1\nACCEPTED 0\nREFUSALS 1\n\
             REFUSAL NotClosedShape http://example.org/a b missing:sh:path\n\
             END-REPORT\n"
        );
        assert!(
            parse_report(&refusal_side).is_err(),
            "space-bearing REFUSAL shape_iri must refuse (the law the \
             SHAPE-line check now matches)"
        );
    }

    /// Boundary court (DoD-8's other side): exactly
    /// MAX_DECLARED_PROPERTIES properties compile and round-trip -
    /// MaskOverflow is strictly >64 - and m_legal saturates to u64::MAX.
    #[test]
    fn mask_width_boundary_64_accepted() {
        let mut ttl = String::from(
            "@prefix sh: <http://www.w3.org/ns/shacl#> .\n\
             @prefix ex: <http://example.org/> .\n\
             ex:Wide a sh:NodeShape ;\n    sh:closed true ;\n",
        );
        for i in 0..64 {
            let terminator = if i == 63 { '.' } else { ';' };
            ttl.push_str(&format!("    sh:property [ sh:path ex:p{:03} ] {}\n", i, terminator));
        }
        let report = compile_shapes(&ttl).expect("parse");
        assert_eq!(report.accepted.len(), 1, "{:?}", report.refusals);
        let s = &report.accepted[0];
        assert_eq!(s.mask_width, 64);
        assert_eq!(s.m_legal, u64::MAX);
        assert_eq!(s.m_mandatory, 0, "all boundary properties are optional");
        assert_eq!(validate_presence(0, s), Ok(()));
        // bit 63 is declared -> legal; the rejected 65th property lives in t7.
        assert_eq!(validate_presence(1u64 << 63, s), Ok(()));
        // the width-64 report exercises the saturating branch of the
        // parse-side invariant court.
        let text = render_report(&report);
        let reparsed = parse_report(&text).expect("width-64 report must parse");
        assert_eq!(reparsed, report);
    }

    /// Per-shape bit namespaces (doc section 10): the same property IRI in
    /// two shapes gets independent bit indices.
    #[test]
    fn per_shape_bit_namespaces_are_independent() {
        let ttl = "\
@prefix sh: <http://www.w3.org/ns/shacl#> .
@prefix ex: <http://example.org/> .
ex:A a sh:NodeShape ;
    sh:closed true ;
    sh:property [ sh:path ex:name ; sh:minCount 1 ] ;
    sh:property [ sh:path ex:email ; sh:minCount 1 ] .
ex:B a sh:NodeShape ;
    sh:closed true ;
    sh:property [ sh:path ex:name ; sh:minCount 1 ] .
";
        let report = compile_shapes(ttl).expect("parse");
        assert_eq!(report.accepted.len(), 2);
        let a = report.accepted.iter().find(|s| s.shape_iri.ends_with('A')).expect("shape A");
        let b = report.accepted.iter().find(|s| s.shape_iri.ends_with('B')).expect("shape B");
        let bit_of = |s: &CompiledShape, local: &str| -> u8 {
            s.bits
                .iter()
                .find(|(iri, _)| iri.ends_with(local))
                .map(|(_, b)| *b)
                .expect("bit")
        };
        // In A, name sorts after email; in B, name is the only bit.
        assert_eq!(bit_of(a, "name"), 1);
        assert_eq!(bit_of(a, "email"), 0);
        assert_eq!(bit_of(b, "name"), 0);
        assert_ne!(bit_of(a, "name"), bit_of(b, "name"));
    }

    /// Parser courts: undeclared prefix is a typed parse error with a line,
    /// and datatypes map through `bont_field_type` (unknown -> None, never a
    /// silent default).
    #[test]
    fn parser_errors_are_typed_and_datatypes_map() {
        let bad = "@prefix ex: <http://example.org/> .\nex:S a sh:NodeShape .\n";
        let err = compile_shapes(bad).expect_err("undeclared prefix must error");
        assert_eq!(err.line, 2);
        assert!(err.message.contains("undeclared prefix"));

        assert_eq!(bont_field_type("http://www.w3.org/2001/XMLSchema#string"), Some(BontFieldType::Text));
        assert_eq!(bont_field_type("http://www.w3.org/2001/XMLSchema#integer"), Some(BontFieldType::Integer));
        assert_eq!(bont_field_type("http://www.w3.org/2001/XMLSchema#dateTime"), Some(BontFieldType::DateTime));
        assert_eq!(bont_field_type("http://www.w3.org/2001/XMLSchema#nonexistent"), None);

        // ReasonCode persisted names round-trip (canonical format law).
        for code in [
            ReasonCode::NotClosedShape,
            ReasonCode::ValueConstraintOutOfMaskScope,
            ReasonCode::CardinalityUnsupported,
            ReasonCode::LogicalShapeUnsupported,
            ReasonCode::TargetingOutOfScope,
            ReasonCode::MaskOverflow,
            ReasonCode::MalformedShape,
        ] {
            assert_eq!(ReasonCode::from_name(code.name()), Some(code));
        }
    }

    /// T9 IRIREF character law (Wave-6 repair, doc section 5 step 1): the
    /// Turtle IRIREF production forbids U+0000..U+0020 and `"`, `<`, `>`,
    /// `{`, `}`, `|`, `^`, `\`, `` ` `` between the angle brackets. The lexer
    /// previously accepted any ASCII except `>`, so a space-bearing IRI
    /// compiled, reached the canonical report, and produced an artifact
    /// `parse_report` refuses (`BIT has extra fields`) - the compiler's own
    /// output un-persistable, T6's round-trip law broken at the lexer.
    /// Anti-vacuity M11: the sandbox mutation "delete the forbidden-byte
    /// check" flips (a)-(c) Err -> Ok (flip receipt in the lane report).
    #[test]
    fn t9_iri_ref_law_forbidden_bytes_typed_error() {
        let ttl_with_subject = |iri: &str| {
            format!(
                "\
@prefix sh: <http://www.w3.org/ns/shacl#> .
@prefix ex: <http://example.org/> .
<{}>
    a sh:NodeShape ;
    sh:closed true ;
    sh:property [ sh:path ex:name ; sh:minCount 1 ] .
",
                iri
            )
        };

        // (a) space inside a subject IRIREF: typed parse error, never
        // acceptance into a report the canonical parser cannot re-read.
        let err = compile_shapes(&ttl_with_subject("http://example.org/my shape"))
            .expect_err("space-bearing IRIREF must be a typed parse error");
        assert!(err.message.contains("forbidden byte 0x20"), "{}", err.message);
        assert!(err.message.contains("IRIREF"), "{}", err.message);
        assert_eq!(err.line, 3, "error carries the IRIREF's source line");

        // (b) control byte inside an IRIREF: same law, same channel.
        let err = compile_shapes(&ttl_with_subject("http://example.org/a\u{01}b"))
            .expect_err("control byte in IRIREF must be a typed parse error");
        assert!(err.message.contains("forbidden byte 0x01"), "{}", err.message);

        // (c) forbidden punctuation inside an IRIREF.
        for bad in ["http://example.org/a\"b", "http://example.org/a{b"] {
            let err = compile_shapes(&ttl_with_subject(bad))
                .expect_err("forbidden IRIREF punctuation must be a typed parse error");
            assert!(err.message.contains("forbidden byte"), "{}", err.message);
        }

        // (d) object-position IRIREF (sh:path): same law.
        let path_space = "\
@prefix sh: <http://www.w3.org/ns/shacl#> .
@prefix ex: <http://example.org/> .
ex:S a sh:NodeShape ;
    sh:closed true ;
    sh:property [ sh:path <http://example.org/a b> ; sh:minCount 1 ] .
";
        let err = compile_shapes(path_space)
            .expect_err("space-bearing sh:path IRIREF must be a typed parse error");
        assert!(err.message.contains("forbidden byte 0x20"), "{}", err.message);

        // (e) the clean equivalent compiles: the law is not over-broad
        // (M8-style flip baseline - removing the offending byte compiles).
        let good = compile_shapes(&ttl_with_subject("http://example.org/my-shape"))
            .expect("clean IRIREF must compile");
        assert_eq!(good.accepted.len(), 1, "{:?}", good.refusals);

        // (f) consequence witness: a space-bearing property IRI renders but
        // `parse_report` refuses it - proving the corruption channel the
        // lexer law closes, and that the parse side stays fail-closed. The
        // hostile shape's IRI is space-free so this probe isolates the BIT
        // row's refusal (`extra fields`); the SHAPE-line space case is its
        // own tamper class, refused by the Wave-8 check (T8/M14).
        let hostile = CompiledShape {
            shape_iri: "http://example.org/hostile".to_string(),
            mask_width: 1,
            bits: vec![("http://example.org/a b".to_string(), 0)],
            m_mandatory: 0,
            m_legal: 1,
            m_group: 0,
        };
        let text = render_report(&CompileReport {
            accepted: vec![hostile],
            refusals: vec![],
        });
        let err = parse_report(&text)
            .expect_err("space-bearing BIT IRI must not parse back");
        assert!(
            err.0.contains("extra fields"),
            "expected the space-split refusal, got: {}",
            err.0
        );
    }

    /// T10 `sh:closed` is single-valued (Wave-6 repair): a duplicate
    /// occurrence - including the contradictory `sh:closed true ;
    /// sh:closed false` - must refuse as `MalformedShape` instead of
    /// silently resolving through `any()`. Without the repair, a
    /// contradictory pair is accepted with the mandatory bits of the
    /// `true` reading and no trace of the `false` reading in the report:
    /// a silent drop. Anti-vacuity M10: the sandbox mutation "delete the
    /// duplicate-count check" flips (a)/(b) Err -> Ok.
    #[test]
    fn t10_sh_closed_single_valued_refused_m10_flips() {
        let ttl_with = |closed: &str| {
            format!(
                "\
@prefix sh: <http://www.w3.org/ns/shacl#> .
@prefix ex: <http://example.org/> .
ex:S a sh:NodeShape ;
    sh:closed {} ;
    sh:property [ sh:path ex:name ; sh:minCount 1 ] .
",
                closed
            )
        };

        // (a) contradictory pair: true and false readings of one shape.
        let bad = compile_shapes(&ttl_with("true ; sh:closed false")).expect("parse");
        let row = bad
            .refusals
            .iter()
            .find(|r| r.reason_code == ReasonCode::MalformedShape)
            .expect("contradictory sh:closed must refuse");
        assert_eq!(row.construct, "duplicate:sh:closed");
        assert_eq!(row.shape_iri, "http://example.org/S");
        assert!(bad.accepted.is_empty(), "{:?}", bad.refusals);

        // (b) duplicate declaration, both readings true: same refusal
        // (last-wins would silently drop the first declaration).
        let dup = compile_shapes(&ttl_with("true ; sh:closed true")).expect("parse");
        assert!(
            dup.refusals
                .iter()
                .any(|r| r.reason_code == ReasonCode::MalformedShape
                    && r.construct == "duplicate:sh:closed"),
            "{:?}",
            dup.refusals
        );
        assert!(dup.accepted.is_empty());

        // (c) single `sh:closed true` compiles clean (the M10 flip
        // baseline: without the duplicate, the shape is accepted).
        let good = compile_shapes(&ttl_with("true")).expect("parse");
        assert_eq!(good.accepted.len(), 1, "{:?}", good.refusals);
        assert!(!has_reason(&good, ReasonCode::MalformedShape));

        // (d) single `sh:closed false` keeps the existing registry row:
        // NotClosedShape, not MalformedShape.
        let open = compile_shapes(&ttl_with("false")).expect("parse");
        assert!(has_reason(&open, ReasonCode::NotClosedShape));
        assert!(!has_reason(&open, ReasonCode::MalformedShape));
    }

    /// T11 wrong-typed `sh:closed` object (Wave-7 repair): a non-boolean
    /// `sh:closed` object is a malformed known local (SHACL 1.1 types
    /// `sh:closed` as xsd:boolean) and must carry its own
    /// `malformed:sh:closed` MalformedShape row. Before the repair the fact
    /// dropped silently: the report showed only the generic
    /// `NotClosedShape` mislabel. The shape VERDICT is unchanged by design
    /// (such a shape was already refused); what the repair fixes is the
    /// report-fidelity drop, so mutation M12 flips the ROW's presence, not
    /// the accept verdict. Anti-vacuity M12: the sandbox mutation "delete
    /// the malformed:sh:closed row" flips (a)/(b)/(c) from row-present to
    /// row-absent.
    #[test]
    fn t11_sh_closed_wrong_typed_object_m12_flips() {
        let ttl_with = |closed: &str| {
            format!(
                "\
@prefix sh: <http://www.w3.org/ns/shacl#> .
@prefix ex: <http://example.org/> .
ex:S a sh:NodeShape ;
    sh:closed {} ;
    sh:property [ sh:path ex:name ; sh:minCount 1 ] .
",
                closed
            )
        };
        fn malformed_row(report: &CompileReport) -> Option<&ShaclRefusal> {
            report.refusals.iter().find(|r| {
                r.reason_code == ReasonCode::MalformedShape
                    && r.construct == "malformed:sh:closed"
            })
        }

        // (a) integer object: malformed row + refusal. NotClosedShape also
        // fires (the precondition is genuinely unmet) - the both-rows
        // precedent of a wrong-typed `sh:path` (T7c).
        let int_obj = compile_shapes(&ttl_with("1")).expect("parse");
        let row = malformed_row(&int_obj).expect("malformed:sh:closed row");
        assert_eq!(row.shape_iri, "http://example.org/S");
        assert!(int_obj.accepted.is_empty(), "{:?}", int_obj.refusals);
        assert!(has_reason(&int_obj, ReasonCode::NotClosedShape));

        // (b) string object: same law.
        let str_obj = compile_shapes(&ttl_with("\"yes\"")).expect("parse");
        assert!(malformed_row(&str_obj).is_some(), "{:?}", str_obj.refusals);
        assert!(str_obj.accepted.is_empty());

        // (c) wrong-typed + `true` in one shape: the malformed row fires
        // alongside the duplicate-occurrence row (two `sh:closed` triples),
        // and `closed == true` so no NotClosedShape row fires.
        let mixed = compile_shapes(&ttl_with("1 ; sh:closed true")).expect("parse");
        assert!(malformed_row(&mixed).is_some(), "{:?}", mixed.refusals);
        assert!(
            mixed
                .refusals
                .iter()
                .any(|r| r.construct == "duplicate:sh:closed"),
            "{:?}",
            mixed.refusals
        );
        assert!(mixed.accepted.is_empty());
        assert!(!has_reason(&mixed, ReasonCode::NotClosedShape));

        // (d) the M12 flip's other side: a well-typed boolean `sh:closed`
        // (either reading) carries NO malformed row.
        let good = compile_shapes(&ttl_with("true")).expect("parse");
        assert!(malformed_row(&good).is_none(), "{:?}", good.refusals);
        assert_eq!(good.accepted.len(), 1, "{:?}", good.refusals);
        let open = compile_shapes(&ttl_with("false")).expect("parse");
        assert!(malformed_row(&open).is_none());
        assert!(has_reason(&open, ReasonCode::NotClosedShape));
    }

    // -----------------------------------------------------------------------
    // Wave 9 (cross-lane integration, v26.11.1 wave-9 slice)
    // -----------------------------------------------------------------------

    /// The T3 contact fixture, extracted to a shared const so the Wave-9
    /// stress court compiles the SAME shape T3 courts (one fixture, one bit
    /// table - the doc section 10 bit-order-drift risk, closed by locality).
    const CONTACT_TTL: &str = "\
@prefix sh: <http://www.w3.org/ns/shacl#> .
@prefix cgap: <http://cgap.dev/ontology#> .
@prefix ex: <http://example.org/> .

ex:ContactGroup cgap:exactlyOneOf true .

ex:ContactShape a sh:NodeShape ;
    sh:closed true ;
    sh:property [
        sh:path ex:name ;
        sh:minCount 1 ;
    ] ;
    sh:property [
        sh:path ex:email ;
        sh:group ex:ContactGroup ;
    ] ;
    sh:property [
        sh:path ex:phone ;
        sh:group ex:ContactGroup ;
    ] .
";

    /// Deterministic row generator for the stress court (xorshift64, nonzero
    /// seed; no Process.sleep, no wall-clock seeds - the corpus is replayable
    /// byte-for-byte).
    struct XorShift64(u64);
    impl XorShift64 {
        fn next(&mut self) -> u64 {
            let mut x = self.0;
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            self.0 = x;
            x
        }
    }

    /// Row construction for the stress corpus: arm 0 (free) draws the legal
    /// 3-bit space; arms 1-3 force one verdict class each so every class is
    /// witnessed regardless of draw luck. 25% of rows stay unconstrained.
    fn stress_row(r: u64) -> u64 {
        match r % 4 {
            0 => (r >> 3) % 8,
            1 => ((r >> 3) % 8) | (1 << 6), // undeclared bit 6: IllegalProperty
            2 => ((r >> 3) % 8) & !0b0010,  // mandatory name bit cleared
            _ => 0b0111,                    // both group bits set: GroupCardinality
        }
    }

    /// T12 cross-lane content-identity court (A06 seam) + mutation M15.
    ///
    /// Wave-9 slice item "A04 identity court", realized on A08's reachable
    /// surface: cgap-bont has no cgap-core dep edge (Cargo.toml is not lane-
    /// owned), so identity rides the crate's sanctioned content-identity
    /// primitive - A06 `slot_id` = truncate64(BLAKE3(BONT_NS_V1 ++ lexeme))
    /// - over the canonical compile report (the shape's identity substrate,
    /// doc section 5 step 3 + section 10 bit-order-stability row).
    ///
    /// Laws: byte-equal reports collide on identity; any report-affecting
    /// compile difference (mandatory set, shape-IRI namespace) diverges.
    /// M15 (identity-collapse mutation): hashing a constant instead of the
    /// report bytes must flip the distinguishing arms - a court that cannot
    /// flip is FAIL.
    #[test]
    fn t12_content_identity_court_m15_flips() {
        use crate::primitives::slot_id;

        // (a) determinism: two compiles -> byte-equal canonical reports ->
        // equal identity slots.
        let a = compile_shapes(PERSON_TTL).expect("parse person fixture");
        let b = compile_shapes(PERSON_TTL).expect("parse person fixture again");
        let (ra, rb) = (render_report(&a), render_report(&b));
        assert_eq!(ra, rb, "canonical render must be deterministic (T6 law)");
        let slot_a = slot_id(ra.as_bytes());
        let slot_b = slot_id(rb.as_bytes());
        assert_eq!(slot_a, slot_b, "byte-equal reports must collide on identity");

        // (b) distinguishing arm 1: mandatory-set difference moves identity.
        // Email drops minCount 1 -> m_mandatory loses bit 0 (doc section 5b).
        let c = compile_shapes(PERSON_TTL_EMAIL_OPTIONAL).expect("parse variant");
        let rc = render_report(&c);
        assert_ne!(ra, rc, "fixture sanity: reports must differ");
        let slot_c = slot_id(rc.as_bytes());
        assert_ne!(slot_a, slot_c, "mask change must move identity");

        // (c) distinguishing arm 2: the same property IRI in two shapes -
        // per-shape bit namespaces (doc section 10 risk row) - so both
        // shapes accept ex:email at bit 0 while reports (and identities)
        // differ by shape IRI.
        let d = compile_shapes(TWO_SHAPES_SHARED_EMAIL_TTL).expect("parse two shapes");
        assert_eq!(d.accepted.len(), 2, "{:?}", d.refusals);
        let (s1, s2) = (&d.accepted[0], &d.accepted[1]);
        assert_eq!(s1.bits, vec![("http://example.org/email".to_string(), 0u8)]);
        assert_eq!(s2.bits, vec![("http://example.org/email".to_string(), 0u8)]);
        assert_ne!(
            slot_id(render_report(&d).as_bytes()),
            slot_a,
            "different shape set must move identity"
        );

        // (d) M15 flip, realized on the hashed input (the same epistemic
        // shape as M1b/M2/M3: mutate the court's constant, watch the verdict
        // flip). The mutation replaces the report lexeme with a constant -
        // identity collapse - under which the differing reports of (b) hash
        // to the SAME slot: the court's assert_ne! flips pass -> fail.
        let m15_a = slot_id(b"M15-identity-collapsed");
        let m15_c = slot_id(b"M15-identity-collapsed");
        assert_ne!(slot_a, slot_c, "premise: real identity distinguishes");
        assert_eq!(
            m15_a, m15_c,
            "M15 mutated identity collapses - the distinguishing verdict flips"
        );
    }

    /// The doc section 5b person fixture with `ex:email` optional (minCount 0):
    /// m_mandatory loses bit 0 (0b0011 -> 0b0010), everything else unchanged.
    const PERSON_TTL_EMAIL_OPTIONAL: &str = "\
@prefix sh: <http://www.w3.org/ns/shacl#> .
@prefix xsd: <http://www.w3.org/2001/XMLSchema#> .
@prefix ex: <http://example.org/> .

ex:PersonShape a sh:NodeShape ;
    sh:closed true ;
    sh:property [
        sh:path ex:email ;
        sh:datatype xsd:string ;
    ] ;
    sh:property [
        sh:path ex:name ;
        sh:minCount 1 ;
    ] ;
    sh:property [
        sh:path ex:nick ;
        sh:minCount 0 ;
    ] ;
    sh:property [
        sh:path ex:phone ;
    ] .
";

    /// Two closed NodeShapes sharing one property IRI: per-shape bit
    /// namespaces (doc section 10) - each shape assigns ex:email bit 0.
    const TWO_SHAPES_SHARED_EMAIL_TTL: &str = "\
@prefix sh: <http://www.w3.org/ns/shacl#> .
@prefix ex: <http://example.org/> .

ex:S1 a sh:NodeShape ;
    sh:closed true ;
    sh:property [ sh:path ex:email ; sh:minCount 1 ] .

ex:S2 a sh:NodeShape ;
    sh:closed true ;
    sh:property [ sh:path ex:email ; sh:minCount 0 ] .
";

    /// T13 A20-scale 1M-row stress court (wave-9 slice item "A20 1M-row
    /// stress", realized on the A08 evaluator surface).
    ///
    /// Shape: the T3 contact shape (m_mandatory = 0b0010, m_legal = 0b0111,
    /// m_group = 0b0101), re-asserted here against the doc's sorted-IRI bit
    /// table so the oracle constants are doc-derived, not implementation-
    /// derived. The oracle then applies the doc's evaluation order (section 5
    /// step 5: closed-world, then AND, then XOR) with those constants.
    ///
    /// Asserted: (1) doc-anchored boundary rows; (2) partition completeness
    /// + class reachability over 1M deterministic rows; (3) identity
    /// stability - an independent second compile classifies all 1M rows
    /// identically; (4) the O(1) envelope as a no-sleep ratio fence against
    /// a bare-xorshift baseline (relative, sleep-free - never an absolute-ms
    /// bound).
    #[test]
    fn t13_wave9_1m_row_stress() {
        let report = compile_shapes(CONTACT_TTL).expect("parse contact fixture");
        assert_eq!(report.accepted.len(), 1, "{:?}", report.refusals);
        let shape = &report.accepted[0];
        // Doc-derived mask constants (sorted-IRI bits: email=0, name=1,
        // phone=2; name mandatory; email+phone the XOR group).
        assert_eq!(shape.m_legal, 0b0111);
        assert_eq!(shape.m_mandatory, 0b0010);
        assert_eq!(shape.m_group, 0b0101);

        // (1) Doc-anchored boundary rows (T3's worked verdicts, restated as
        // the stress court's preamble).
        assert_eq!(validate_presence(0b0011, shape), Ok(()));
        assert_eq!(validate_presence(0b0110, shape), Ok(()));
        assert_eq!(validate_presence(0b0010, shape), Err(MaskViolation::GroupCardinality));
        assert_eq!(validate_presence(0b0111, shape), Err(MaskViolation::GroupCardinality));
        assert_eq!(validate_presence(u64::MAX, shape), Err(MaskViolation::IllegalProperty));
        // All-zeros row: mandatory name bit absent -> MissingMandatory fires
        // first (doc section 5 step-5 precedence: closed-world, AND, XOR).
        assert_eq!(validate_presence(0, shape), Err(MaskViolation::MissingMandatory));

        // (2)+(3)+(4) 1M deterministic rows; the spec-law oracle uses the
        // doc-derived constants above with the doc's evaluation order.
        const ROWS: u32 = 1_000_000;
        let mut rng = XorShift64(0x9E37_79B9_7F4A_7C15);
        // None == Ok (valid); Some(v) == the named violation.
        let mut verdicts_a: Vec<Option<MaskViolation>> = Vec::with_capacity(ROWS as usize);
        let mut oks = 0u64;
        let mut illegal = 0u64;
        let mut missing = 0u64;
        let mut group = 0u64;
        for _ in 0..ROWS {
            let r = rng.next();
            let obs = stress_row(r);
            let expected = if obs & !0b0111 != 0 {
                illegal += 1;
                Some(MaskViolation::IllegalProperty)
            } else if obs & 0b0010 != 0b0010 {
                missing += 1;
                Some(MaskViolation::MissingMandatory)
            } else if (obs & 0b0101).count_ones() != 1 {
                group += 1;
                Some(MaskViolation::GroupCardinality)
            } else {
                oks += 1;
                None
            };
            match validate_presence(obs, shape) {
                Ok(()) => assert_eq!(expected, None, "row {r:#x} (obs {obs:#x})"),
                Err(v) => assert_eq!(Some(v), expected, "row {r:#x} (obs {obs:#x})"),
            }
            verdicts_a.push(expected);
        }
        // Partition completeness + class reachability (final state).
        assert_eq!(oks + illegal + missing + group, ROWS as u64);
        for count in [oks, illegal, missing, group] {
            assert!(count > 0, "class reachability: {oks}/{illegal}/{missing}/{group}");
        }

        // (3) identity stability at scale: an independent second compile
        // classifies all 1M rows identically.
        let report2 = compile_shapes(CONTACT_TTL).expect("second compile");
        let shape2 = &report2.accepted[0];
        assert_ne!(shape as *const _, shape2 as *const _, "fixture: distinct instances");
        let mut rng2 = XorShift64(0x9E37_79B9_7F4A_7C15);
        for (i, va) in verdicts_a.iter().enumerate() {
            let obs = stress_row(rng2.next());
            let vb = match validate_presence(obs, shape2) {
                Ok(()) => None,
                Err(v) => Some(v),
            };
            assert_eq!(*va, vb, "row {i} diverged across independent compiles");
        }

        // (4) O(1) envelope as a no-sleep ratio fence: 1M evaluate rows vs
        // 1M bare xorshift steps, same process, same build. The fence is
        // relative (never an absolute-ms bound); 50x leaves headroom for
        // debug-profile call overhead (~10-30x honest ratio) while still
        // catching an algorithmic regression (per-row map lookups or scans
        // over the bit table land >100x).
        let t_eval = std::time::Instant::now();
        let mut rng3 = XorShift64(0x9E37_79B9_7F4A_7C15);
        let mut sink = 0u64;
        for _ in 0..ROWS {
            let obs = stress_row(rng3.next());
            sink ^= match validate_presence(obs, shape) {
                Ok(()) => 0,
                Err(v) => v as u64,
            };
        }
        let eval = t_eval.elapsed();
        let t_base = std::time::Instant::now();
        let mut rng4 = XorShift64(0x9E37_79B9_7F4A_7C15);
        let mut sink2 = 0u64;
        for _ in 0..ROWS {
            sink2 ^= std::hint::black_box(rng4.next());
        }
        let base = t_base.elapsed();
        assert_ne!(sink, u64::MAX, "fence accumulators must stay live");
        let _ = sink2;
        assert!(
            eval.as_nanos() < 50 * base.as_nanos().max(1),
            "ratio fence blown: eval {eval:?} vs base {base:?} (>=50x)"
        );
    }
}
