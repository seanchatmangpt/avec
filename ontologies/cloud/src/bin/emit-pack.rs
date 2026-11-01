//! Build-time `.oabi` pack emitter — lane A10 (lane doc §3.3 emit component,
//! §6 layout, §7 courts basis).
//!
//! Deterministic, fail-closed producer for `ontologies/cloud/tosca_cimi_v1.oabi`:
//!
//! - the corpus is [`cgap_cloud::LEXEME_TABLE`] itself (single source of
//!   truth — the bin never restates lexemes);
//! - emit-time SlotId collision vetting through A06's real
//!   [`cgap_bont::primitives::CollisionDetector`] (A06 §5 producer
//!   obligation, lane DoD 9): any two distinct lexemes truncating to the
//!   same 64-bit prefix abort the emit naming both lexemes;
//! - emit-time golden-drift guard: every row's committed `slot`/`lex_hash`
//!   must equal the live `cgap_bont` derivation, else `GoldenDrift` (DoD 2/6
//!   fail-closed; zeros are never emittable);
//! - emit-time equivalence + enum-table validation (lane §6: aliases resolve,
//!   no self-loops; family ids inside the loaded EnumId space, variants dense);
//! - A06 §6 container bytes verbatim via `cgap_bont::oabi` types (cited,
//!   never redefined), then the finished bytes are re-loaded through A06's
//!   real loader ([`cgap_bont::oabi::OabiPackV1::from_bytes`]) BEFORE any
//!   write — a pack the authority loader refuses is never written;
//! - prints the BLAKE3 digest of the exact written bytes (becomes
//!   `PACK_DIGEST_HEX` in `src/lib.rs`, verified by court T1).
//!
//! Modes: default = emit + write + print digest; `--check` = regenerate and
//! byte-compare against the committed file without writing (exit 4 on
//! drift); `--print-table` = print the `LEXEME_TABLE` source block with
//! computed goldens (the golden bootstrap; ignores committed values).
//! Exit codes: 0 ok; 1 emit error; 4 `--check` drift.

use std::process::ExitCode;

use cgap_blake3::hash;
use cgap_bont::oabi::{OabiError, OabiHeaderV1, OabiPackV1, OabiRecordV1};
use cgap_bont::primitives::{truncate64, ClassId, CollisionDetector, EnumId, LexHash, SlotId};
use cgap_cloud::{
    CLASS_COUNT, ENUM_COUNT, ENUM_FAMILIES, ENUM_NONE, ENUM_FAMILY_CREDENTIAL_TYPE,
    ENUM_FAMILY_EVENT_SEVERITY, ENUM_FAMILY_JOB_STATUS, ENUM_FAMILY_MACHINE_STATE,
    ENUM_FAMILY_VOLUME_STATE, EQUIV_TABLE, LEXEME_TABLE,
};

/// Committed pack path (lane doc file list `tosca_cimi_v1.oabi`, emitted
/// inside the lane-owned `ontologies/cloud/` directory).
const OUT_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tosca_cimi_v1.oabi");

/// Typed emit failure; every variant names its subject.
#[derive(Debug, PartialEq, Eq)]
enum EmitError {
    /// A06 §5 hard producer obligation (DoD 9).
    SlotCollision(cgap_bont::primitives::CollisionError),
    /// Committed golden != live derivation (DoD 2/6, fail-closed).
    GoldenDrift { lexeme: &'static str, field: &'static str },
    /// Equivalence table integrity (T4 basis).
    EquivUnknownLexeme { lexeme: &'static str },
    EquivSelfLoop { alias: &'static str },
    /// Enum-family table integrity (record EnumId semantics).
    EnumFamilyOutOfBounds { id: u16 },
    EnumVariantNotDense { family: &'static str, name: &'static str, value: u16 },
    /// Record field outside the header spaces (loader would refuse).
    ClassOutOfRange { lexeme: &'static str, class: u32 },
    FamilyOutOfRange { lexeme: &'static str, family: u16 },
    /// The real A06 loader refused the freshly built bytes.
    LoaderRefused(OabiError),
}

impl core::fmt::Display for EmitError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::SlotCollision(e) => write!(f, "{e}"),
            Self::GoldenDrift { lexeme, field } => {
                write!(f, "golden drift: {field} of `{lexeme}` != live derivation")
            }
            Self::EquivUnknownLexeme { lexeme } => {
                write!(f, "equivalence row names unknown lexeme `{lexeme}`")
            }
            Self::EquivSelfLoop { alias } => write!(f, "equivalence self-loop on `{alias}`"),
            Self::EnumFamilyOutOfBounds { id } => {
                write!(f, "enum family id {id} outside [0, ENUM_COUNT)")
            }
            Self::EnumVariantNotDense { family, name, value } => {
                write!(f, "enum family `{family}` variant `{name}` value {value} is not dense")
            }
            Self::ClassOutOfRange { lexeme, class } => {
                write!(f, "lexeme `{lexeme}` class {class} >= CLASS_COUNT {CLASS_COUNT}")
            }
            Self::FamilyOutOfRange { lexeme, family } => {
                write!(f, "lexeme `{lexeme}` enum family {family} >= ENUM_COUNT {ENUM_COUNT}")
            }
            Self::LoaderRefused(e) => write!(f, "A06 loader refused fresh pack: {e}"),
        }
    }
}

fn run() -> Result<String, EmitError> {
    // --- golden + record derivation over the single-source table -----------
    let mut detector = CollisionDetector::new();
    let mut records = Vec::with_capacity(LEXEME_TABLE.len());
    for row in LEXEME_TABLE {
        let lexeme = row.lexeme.as_bytes();
        detector.insert(lexeme).map_err(EmitError::SlotCollision)?;
        let lex_hash = cgap_bont::primitives::lex_hash(lexeme);
        let slot = truncate64(&lex_hash);
        if row.lex_hash != lex_hash.0 {
            return Err(EmitError::GoldenDrift { lexeme: row.lexeme, field: "lex_hash" });
        }
        if row.slot != slot.0 {
            return Err(EmitError::GoldenDrift { lexeme: row.lexeme, field: "slot" });
        }
        if row.class >= CLASS_COUNT {
            return Err(EmitError::ClassOutOfRange { lexeme: row.lexeme, class: row.class });
        }
        if row.enum_family as u32 >= ENUM_COUNT {
            return Err(EmitError::FamilyOutOfRange { lexeme: row.lexeme, family: row.enum_family });
        }
        records.push(OabiRecordV1 {
            slot: SlotId(row.slot),
            class: ClassId(row.class),
            enum_: EnumId(row.enum_family),
            _pad: [0, 0],
            lex: LexHash(row.lex_hash),
        });
    }

    // --- equivalence-table integrity (T4 emit-side basis) ------------------
    for row in EQUIV_TABLE {
        let alias = LEXEME_TABLE.iter().find(|r| r.lexeme == row.alias_lexeme);
        let canonical = LEXEME_TABLE.iter().find(|r| r.lexeme == row.canonical_lexeme);
        let (alias, canonical) = match (alias, canonical) {
            (Some(a), Some(c)) => (a, c),
            (missing, _) if missing.is_none() => {
                return Err(EmitError::EquivUnknownLexeme { lexeme: row.alias_lexeme })
            }
            _ => return Err(EmitError::EquivUnknownLexeme { lexeme: row.canonical_lexeme }),
        };
        if alias.slot == canonical.slot {
            return Err(EmitError::EquivSelfLoop { alias: row.alias_lexeme });
        }
    }

    // --- enum-family table integrity ----------------------------------------
    for family in ENUM_FAMILIES {
        if family.id as u32 >= ENUM_COUNT || family.id == ENUM_NONE {
            return Err(EmitError::EnumFamilyOutOfBounds { id: family.id });
        }
        for (want, variant) in family.variants.iter().enumerate() {
            if variant.value as usize != want {
                return Err(EmitError::EnumVariantNotDense {
                    family: family.name,
                    name: variant.name,
                    value: variant.value,
                });
            }
        }
    }

    // --- container bytes through A06's real types, then the real loader ----
    let bytes = regen_bytes();
    OabiPackV1::from_bytes(&bytes).map_err(EmitError::LoaderRefused)?;

    Ok(digest_hex(&bytes))
}

/// Canonical pack bytes: A06 §6 header + field-wise LE records, encoded
/// through A06's real container types (`OabiHeaderV1::to_bytes` /
/// `OabiRecordV1::to_bytes`).
fn regen_bytes() -> Vec<u8> {
    let mut bytes = Vec::with_capacity(
        cgap_cloud::OABI_HEADER_LEN + LEXEME_TABLE.len() * cgap_cloud::OABI_RECORD_SIZE,
    );
    let header = OabiHeaderV1::new(CLASS_COUNT, ENUM_COUNT, LEXEME_TABLE.len() as u32);
    bytes.extend_from_slice(&header.to_bytes());
    for row in LEXEME_TABLE {
        let rec = OabiRecordV1 {
            slot: SlotId(row.slot),
            class: ClassId(row.class),
            enum_: EnumId(row.enum_family),
            _pad: [0, 0],
            lex: LexHash(row.lex_hash),
        };
        bytes.extend_from_slice(&rec.to_bytes());
    }
    bytes
}

fn digest_hex(bytes: &[u8]) -> String {
    let d = hash(bytes);
    let mut s = String::with_capacity(64);
    for b in d {
        s.push_str(&format!("{b:02X}"));
    }
    s
}

fn family_name(id: u16) -> &'static str {
    match id {
        ENUM_NONE => "ENUM_NONE",
        ENUM_FAMILY_MACHINE_STATE => "ENUM_FAMILY_MACHINE_STATE",
        ENUM_FAMILY_VOLUME_STATE => "ENUM_FAMILY_VOLUME_STATE",
        ENUM_FAMILY_EVENT_SEVERITY => "ENUM_FAMILY_EVENT_SEVERITY",
        ENUM_FAMILY_JOB_STATUS => "ENUM_FAMILY_JOB_STATUS",
        ENUM_FAMILY_CREDENTIAL_TYPE => "ENUM_FAMILY_CREDENTIAL_TYPE",
        _ => "<unknown>",
    }
}

fn print_table() {
    println!("// BEGIN GENERATED LEXEME_TABLE goldens (emit-pack --print-table)");
    for row in LEXEME_TABLE {
        let lex_hash = cgap_bont::primitives::lex_hash(row.lexeme.as_bytes());
        let slot = truncate64(&lex_hash);
        let lex: Vec<String> = lex_hash.0.iter().map(|b| format!("0x{b:02X}")).collect();
        println!(
            "    LexemeRow {{ lexeme: {:?}, class: {}, enum_family: {}, origin: Origin::{:?}, citation: {:?}, slot: 0x{:016X}, lex_hash: [{}] }},",
            row.lexeme, row.class, family_name(row.enum_family), row.origin, row.citation, slot.0, lex.join(", ")
        );
    }
    println!("// END GENERATED LEXEME_TABLE goldens");
}

fn main() -> ExitCode {
    let args = std::env::args().collect::<Vec<_>>();
    match args.get(1).map(|s| s.as_str()) {
        Some("--print-table") => {
            print_table();
            ExitCode::SUCCESS
        }
        Some("--check") => match std::fs::read(OUT_PATH) {
            Err(e) => {
                eprintln!("[emit-pack --check] cannot read {OUT_PATH}: {e}");
                ExitCode::from(4)
            }
            Ok(committed) => {
                let regen = regen_bytes();
                if committed == regen {
                    println!("[emit-pack --check] committed pack == regenerated bytes ({} B)", committed.len());
                    ExitCode::SUCCESS
                } else {
                    eprintln!(
                        "[emit-pack --check] DRIFT: committed {} B != regenerated {} B (BLAKE3 committed {}, regenerated {})",
                        committed.len(),
                        regen.len(),
                        digest_hex(&committed),
                        digest_hex(&regen),
                    );
                    ExitCode::from(4)
                }
            }
        },
        _ => match run() {
            Ok(digest) => {
                let bytes = regen_bytes();
                if std::fs::write(OUT_PATH, &bytes).is_err() {
                    eprintln!("[emit-pack] failed to write {OUT_PATH}");
                    return ExitCode::FAILURE;
                }
                println!("[emit-pack] wrote {OUT_PATH} ({} B)", bytes.len());
                println!("[emit-pack] BLAKE3 pack digest: {digest}");
                println!(
                    "[emit-pack] collision vetting: {} lexemes, zero collisions (A06 §5)",
                    LEXEME_TABLE.len()
                );
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("[emit-pack] REFUSED: {e}");
                ExitCode::FAILURE
            }
        },
    }
}
