//! Build-time `.oabi` pack emitter — lane A09 (docs/jira/v26.11.1/lanes/
//! A09-fibo-oabi.md §3.2 build-time half, §6, §8 T7/T11; DoD 3/4/11).
//!
//! Deterministic, fail-closed producer for `fibo_sec_dbt_v1.oabi`:
//!
//! - closed-world module allowlist (§2.1/§2.2): every corpus term is
//!   classified; anything outside the allowed modules is a typed
//!   `TermRefused { lexeme, section, module }` naming §2.2 — the refusal
//!   list is mechanical (T11), never review-time;
//! - BLAKE3 SlotId/LexHash derivation through the real in-repo substrate
//!   (`cgap_blake3::hash`, A06's cited hash basis) — no hand-minted values;
//! - emit-time SlotId collision check (A06 §5 producer obligation, DoD 11);
//! - emit-time re-derivation guard, twice: derived-vs-facade-consts (T9
//!   emit half) and derived-vs-wire-bytes (DoD 4);
//! - A06 §6 container bytes verbatim (cited, never redefined): 64 B header
//!   + `slot_count` x 48 B records, LE, LexHash flag set, reserved zero
//!   (A06 §6 wins over the lane doc §5.1 "reserved-free extension area"
//!   sketch — recorded divergence);
//! - deterministic: const corpus in frozen harvest order (§4), no clocks,
//!   no env, no hash-map iteration order; `--check` re-derives the bytes
//!   and compares with the committed pack on disk byte-for-byte.
//!
//! Modes (see `run()`; the cargo bin TARGET is `emit-fibo-pack` since the
//! Wave-6 rename — the Wave-2 name `emit-pack` collided with lane A10's
//! identically-named bin and let this lane's courts execute A10's binary
//! in joint workspace invocations; the source file keeps its name):
//! ```text
//!   emit-fibo-pack                    emit the pack (fails loud on drift, exit 4)
//!   emit-fibo-pack --check            committed pack == fresh emit? (drift -> 4)
//!   emit-fibo-pack --probe <lexeme>   closed-world refusal witness over ONE
//!                                     argv-supplied lexeme: refused -> exit 3,
//!                                     allowed -> exit 0. The probe corpus lives
//!                                     in the T11 court, never in this binary
//!                                     (Wave-7 quarantine repair)
//!   emit-fibo-pack --inject <lexeme>  literal T11 mutation: run the full emit
//!                                     pipeline with <lexeme> appended to the
//!                                     corpus; TermRefused -> exit 3; exit 0
//!                                     means the gate is open (court FAIL)
//! ```
//!
//! Exit codes: 0 ok; 1 emit error; 3 TermRefused (probe/inject); 4 drift.
//!
//! Wave 9 (2026-11-01, cross-lane integration slice): the first-emit write
//! path (committed pack absent) is now crash-consistent per the A05
//! power-cut discipline — full temporary write in the destination
//! directory, `sync_all`, then `rename(2)` onto the committed path. A cut
//! during emit leaves the committed path either absent or holding the
//! previous complete pack — never a torn pack at the identity surface
//! (previously a cut mid-`fs::write` tore the committed pack; loud later
//! via T7/`--check`, but torn on disk). Torn/orphaned `*.oabi.w9tmp`
//! temporaries are garbage outside the identity surface; every gate
//! refuses them (courts/wave9_integration_courts.rs power-cut sweep).
//! std-only best effort: directory-entry durability (dir fsync) is
//! outside `std::fs`; a lost rename is loud at the next `--check`/T7,
//! never silent.

use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;
use std::process::ExitCode;

use cgap_blake3::hash;

use cgap_fibo::{
    ACCT_RECV_SLOT, BOND_SLOT, COUPON_RATE_SLOT, CORP_BOND_SLOT, DEBT_INSTR_SLOT, EUR_SLOT,
    JPY_SLOT, LEX_ACCT_RECV, LEX_BOND, LEX_COUPON_RATE, LEX_CORP_BOND, LEX_DEBT_INSTR, LEX_EUR,
    LEX_JPY, LEX_MATURITY_DATE, LEX_USD, MATURITY_DATE_SLOT, USD_SLOT,
};

/// A06 §5 namespace tag (cited, restated as a producer input — the same
/// constant the Wave-1 derivation/witness courts restated).
const BONT_NS_V1: &[u8] = b"bont\x01";

/// `.oabi` v1 container constants (A06 §6, cited from the reader at
/// `crates/cgap-bont/src/oabi.rs:58-78` — range freshened and mechanically
/// pinned in wave 8 by `tests/wave8_courts.rs::a06_citation_freshness_court`,
/// which re-derives the decl lines from the real file on disk every run;
/// the wave-2-era `:44-65` citation had gone stale after A06's edits.
/// Restated as producer inputs — the reader courts own the values).
const OABI_MAGIC: [u8; 8] = *b"OABIv1\0\0";
const OABI_HEADER_LEN: usize = 64;
const OABI_RECORD_SIZE: usize = 48;
const OABI_VERSION_MAJOR: u16 = 1;
const OABI_VERSION_MINOR: u16 = 0;
const OABI_FLAG_LE: u32 = 1 << 0;
const OABI_FLAG_LEXHASH: u32 = 1 << 1;

/// Dense ClassId space (lane doc §4, frozen Wave 1; append-only at the end
/// per §10). Zero is the "no class row" sentinel carried by currency rows.
const CLASS_NONE: u32 = 0x0000_0000;
const CLASS_CORP_BOND: u32 = 0x0000_0001;
const CLASS_BOND: u32 = 0x0000_0002;
const CLASS_DEBT_INSTR: u32 = 0x0000_0003;
const CLASS_ACCT_RECV: u32 = 0x0000_0010;
/// Attribute rows carry ClassIds in the dense space too (frozen Wave 1).
const ATTR_MATURITY_DATE: u32 = 0x0000_0020;
const ATTR_COUPON_RATE: u32 = 0x0000_0021;

/// Currency EnumIds (lane doc §4 freeze values, verified both directions
/// by court T8).
const ENUM_USD: u16 = 0x0348;
const ENUM_EUR: u16 = 0x0349;
const ENUM_JPY: u16 = 0x034A;

/// `class_count` covers the frozen dense ClassId space: assigned ids end at
/// `0x21`, so the v1 header count is `0x22` (A06 §6: "ClassId space is
/// [0, class_count)"). Assigning id `0x22` rewrites the header -> new pack
/// bytes -> new digest -> pack version bump (§10), never an in-place patch.
const CLASS_COUNT: u32 = 0x0022;

/// `enum_count` covers the frozen currency EnumId space: max `0x034A`, so
/// `0x034B` (A06 §6: "EnumId space is [0, enum_count)"; sentinel rows carry
/// enum 0).
const ENUM_COUNT: u32 = 0x034B;

/// Closed-world module allowlist (lane doc §2.1). Anything outside these
/// modules is refused naming §2.2 — closed-world default-refuse.
const ALLOWED_MODULES: &[&str] = &["FndUtlCur", "SecDbt", "ClAcct"];

/// One harvested corpus term (lane doc §2.1 cut + §4 rows, in frozen
/// harvest order = Wave-1 corpus order).
struct Term {
    lexeme: Vec<u8>,
    /// Dense ClassId ([`CLASS_NONE`] for enum-family rows).
    class: u32,
    /// Currency EnumId (`None` -> wire carries the 0 sentinel).
    enum_id: Option<u16>,
}

/// The frozen corpus: 9 terms, harvest order (lane doc §4 table = Wave-1
/// corpus order: USD, EUR, JPY, CorporateBond, Bond, DebtInstrument,
/// AccountsReceivable, hasMaturityDate, hasCouponRate). Dense ClassIds are
/// order-frozen (§10): a renumber or a mid-space append is a pack version
/// bump, never a patch.
fn corpus() -> Vec<Term> {
    vec![
        Term { lexeme: b"fibo:FndUtlCur:USD".to_vec(), class: CLASS_NONE, enum_id: Some(ENUM_USD) },
        Term { lexeme: b"fibo:FndUtlCur:EUR".to_vec(), class: CLASS_NONE, enum_id: Some(ENUM_EUR) },
        Term { lexeme: b"fibo:FndUtlCur:JPY".to_vec(), class: CLASS_NONE, enum_id: Some(ENUM_JPY) },
        Term { lexeme: b"fibo:SecDbt:CorporateBond".to_vec(), class: CLASS_CORP_BOND, enum_id: None },
        Term { lexeme: b"fibo:SecDbt:Bond".to_vec(), class: CLASS_BOND, enum_id: None },
        Term { lexeme: b"fibo:SecDbt:DebtInstrument".to_vec(), class: CLASS_DEBT_INSTR, enum_id: None },
        Term { lexeme: b"fibo:ClAcct:AccountsReceivable".to_vec(), class: CLASS_ACCT_RECV, enum_id: None },
        Term { lexeme: b"fibo:SecDbt:hasMaturityDate".to_vec(), class: ATTR_MATURITY_DATE, enum_id: None },
        Term { lexeme: b"fibo:SecDbt:hasCouponRate".to_vec(), class: ATTR_COUPON_RATE, enum_id: None },
    ]
}

/// Committed facade witnesses, in harvest order (T9 emit half: the emitter
/// refuses to emit a corpus whose derivations drift from the committed
/// `src/lib.rs` consts — a hand-minted const fails at emit, not at test).
/// Const-evaluable values: `cgap_fibo::*` items are `const`.
const COMMITTED: &[(u64, [u8; 32])] = &[
    (USD_SLOT, LEX_USD),
    (EUR_SLOT, LEX_EUR),
    (JPY_SLOT, LEX_JPY),
    (CORP_BOND_SLOT, LEX_CORP_BOND),
    (BOND_SLOT, LEX_BOND),
    (DEBT_INSTR_SLOT, LEX_DEBT_INSTR),
    (ACCT_RECV_SLOT, LEX_ACCT_RECV),
    (MATURITY_DATE_SLOT, LEX_MATURITY_DATE),
    (COUPON_RATE_SLOT, LEX_COUPON_RATE),
];

/// Wave-7 quarantine repair (a07-wave6-audit-receipt.md §5 open item 1,
/// audit option (b)): the §2.2-refused probe corpus and the allowed
/// control are NOT compiled into this producer binary — refused vocabulary
/// has no place in a shipped artifact. The T11 court owns the corpus
/// (`tests/wave4_courts.rs`, restated independently there since Wave 4)
/// and drives every probe through `--probe <lexeme>` argv, so the
/// classifier runs on runtime-supplied bytes. What remains embedded is the
/// frozen corpus itself (the producer's inputs, ADR-002 PRODUCER role,
/// scoped by the gate's keyed EMITTER_BINARIES exemption).

/// Lane doc §2.2 — every TermRefused names this section.
const REFUSAL_SECTION: &str = "2.2";

/// The committed pack path (lane doc "Files in scope":
/// `ontologies/fibo/fibo_sec_dbt_v1.oabi`).
fn pack_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fibo_sec_dbt_v1.oabi")
}

fn lex_hash(lexeme: &[u8]) -> [u8; 32] {
    let mut tagged = Vec::with_capacity(BONT_NS_V1.len() + lexeme.len());
    tagged.extend_from_slice(BONT_NS_V1);
    tagged.extend_from_slice(lexeme);
    hash(&tagged)
}

/// Closed-world classification (T11). Lexeme shape: `fibo:<Module>:<Term>`.
fn classify(lexeme: &[u8]) -> Result<(), EmitError> {
    let text = core::str::from_utf8(lexeme).map_err(|_| EmitError::TermRefused {
        lexeme: format!("{lexeme:?}"),
        section: REFUSAL_SECTION,
        module: "<non-utf8>".to_string(),
    })?;
    let rest = text.strip_prefix("fibo:").ok_or_else(|| EmitError::TermRefused {
        lexeme: text.to_string(),
        section: REFUSAL_SECTION,
        module: "<no fibo: prefix>".to_string(),
    })?;
    let module = rest.split(':').next().unwrap_or("");
    if ALLOWED_MODULES.contains(&module) {
        Ok(())
    } else {
        Err(EmitError::TermRefused {
            lexeme: text.to_string(),
            section: REFUSAL_SECTION,
            module: module.to_string(),
        })
    }
}

/// Typed emit failure. Every variant names its subject — a refused term
/// names the refusing section (T11), a collision names both lexemes.
#[derive(Debug, PartialEq, Eq)]
enum EmitError {
    /// Closed-world refusal (lane doc §2.2, T11).
    TermRefused { lexeme: String, section: &'static str, module: String },
    /// A06 §5 producer obligation (DoD 11).
    SlotCollision { slot: u64, a: Vec<u8>, b: Vec<u8> },
    /// T9 emit half: derivation drifted from the committed facade consts.
    CommittedDrift { index: usize },
    /// DoD 4 second half: wire-encoded SlotId != SlotId implied by the wire
    /// LexHash.
    WireReDerivation { index: usize },
    /// --check / emit drift guard: committed pack bytes differ from a fresh
    /// deterministic emit.
    PackDrift { first_diff: Option<usize>, committed_len: usize, fresh_len: usize },
    /// Filesystem/argument failure (typed, never a silent pass).
    Io(String),
}

impl fmt::Display for EmitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EmitError::TermRefused { lexeme, section, module } => {
                write!(f, "TermRefused {lexeme:?} module={module} section={section}")
            }
            EmitError::SlotCollision { slot, a, b } => {
                let sa = String::from_utf8_lossy(a);
                let sb = String::from_utf8_lossy(b);
                write!(f, "SlotCollision {slot:#018x} between {sa:?} and {sb:?}")
            }
            EmitError::CommittedDrift { index } => {
                write!(f, "CommittedDrift corpus[{index}] drifted from src/lib.rs consts")
            }
            EmitError::WireReDerivation { index } => {
                write!(f, "WireReDerivation record {index}: wire SlotId != low64(wire LexHash)")
            }
            EmitError::PackDrift { first_diff, committed_len, fresh_len } => match first_diff {
                Some(at) => write!(
                    f,
                    "PackDrift committed pack != fresh emit (first differing byte @{at}; committed {committed_len} B vs fresh {fresh_len} B)"
                ),
                None => write!(
                    f,
                    "PackDrift committed pack != fresh emit (len {committed_len} != {fresh_len})"
                ),
            },
            EmitError::Io(m) => write!(f, "io: {m}"),
        }
    }
}

impl EmitError {
    fn exit_code(&self) -> u8 {
        match self {
            EmitError::TermRefused { .. } => 3,
            EmitError::PackDrift { .. } => 4,
            _ => 1,
        }
    }
}

/// A06 §6 field-wise LE header (64 B). Reserved stays zero — the loader
/// refuses nonzero reserved (A06 §6 loader checks), which is why the lane
/// doc §5.1 "reserved-free extension area" provenance slot is a recorded
/// divergence (provenance lives in `src/lib.rs` instead).
fn header_bytes(slot_count: u32) -> [u8; OABI_HEADER_LEN] {
    let mut b = [0u8; OABI_HEADER_LEN];
    b[0..8].copy_from_slice(&OABI_MAGIC);
    b[8..12].copy_from_slice(&(OABI_HEADER_LEN as u32).to_le_bytes());
    b[12..14].copy_from_slice(&OABI_VERSION_MAJOR.to_le_bytes());
    b[14..16].copy_from_slice(&OABI_VERSION_MINOR.to_le_bytes());
    b[16..20].copy_from_slice(&(OABI_FLAG_LE | OABI_FLAG_LEXHASH).to_le_bytes());
    b[20..24].copy_from_slice(&CLASS_COUNT.to_le_bytes());
    b[24..28].copy_from_slice(&ENUM_COUNT.to_le_bytes());
    b[28..32].copy_from_slice(&slot_count.to_le_bytes());
    b[32..36].copy_from_slice(&(OABI_RECORD_SIZE as u32).to_le_bytes());
    b
}

/// A06 §6 field-wise LE record (48 B): slot | class | enum | pad(0) | lex.
fn record_bytes(slot: u64, class: u32, enum_id: u16, lex: &[u8; 32]) -> [u8; OABI_RECORD_SIZE] {
    let mut b = [0u8; OABI_RECORD_SIZE];
    b[0..8].copy_from_slice(&slot.to_le_bytes());
    b[8..12].copy_from_slice(&class.to_le_bytes());
    b[12..14].copy_from_slice(&enum_id.to_le_bytes());
    // b[14..16] pad: zero by construction.
    b[16..48].copy_from_slice(lex);
    b
}

/// The full deterministic emit pipeline over `corpus`: classify -> derive
/// -> cross-check committed -> collision check -> encode -> wire
/// re-derivation -> header. Returns `(pack_bytes, blake3_digest)`.
fn build_pack(corpus: &[Term]) -> Result<(Vec<u8>, [u8; 32]), EmitError> {
    let mut seen: HashMap<u64, Vec<u8>> = HashMap::new();
    let mut records: Vec<[u8; OABI_RECORD_SIZE]> = Vec::with_capacity(corpus.len());
    for (i, term) in corpus.iter().enumerate() {
        classify(&term.lexeme)?;
        let lex = lex_hash(&term.lexeme);
        let slot = u64::from_le_bytes(lex[0..8].try_into().expect("32-byte digest"));
        // T9 emit half (only for the frozen corpus positions; injected
        // probe terms beyond the frozen 9 skip this join).
        if let Some((exp_slot, exp_lex)) = COMMITTED.get(i) {
            if slot != *exp_slot || lex != *exp_lex {
                return Err(EmitError::CommittedDrift { index: i });
            }
        }
        if let Some(prev) = seen.insert(slot, term.lexeme.clone()) {
            return Err(EmitError::SlotCollision { slot, a: prev, b: term.lexeme.clone() });
        }
        records.push(record_bytes(slot, term.class, term.enum_id.unwrap_or(0), &lex));
    }
    // DoD 4 second half: re-derive from the WIRE bytes (defense against an
    // encoding-order bug the derivation-side check cannot see).
    for (i, rec) in records.iter().enumerate() {
        let stored = u64::from_le_bytes(rec[0..8].try_into().expect("record window"));
        let implied = u64::from_le_bytes(rec[16..24].try_into().expect("record window"));
        if stored != implied {
            return Err(EmitError::WireReDerivation { index: i });
        }
    }
    let mut bytes = Vec::with_capacity(OABI_HEADER_LEN + records.len() * OABI_RECORD_SIZE);
    bytes.extend_from_slice(&header_bytes(records.len() as u32));
    for rec in &records {
        bytes.extend_from_slice(rec);
    }
    let digest = hash(&bytes);
    Ok((bytes, digest))
}

/// --probe: run the closed-world gate over ONE argv-supplied lexeme.
/// Refused -> prints `[REFUSED] ...` and exits 3; allowed -> prints
/// `[ALLOWED] ...` and exits 0. The probe corpus itself is court-owned
/// (tests/wave4_courts.rs) — this binary never embeds refused vocabulary
/// (Wave-7 quarantine repair, a07-wave6 §5 option (b)).
fn probe_one(lexeme: String) -> Result<(), EmitError> {
    match classify(lexeme.as_bytes()) {
        Err(e @ EmitError::TermRefused { .. }) => {
            println!("[REFUSED] {e}");
            Err(e)
        }
        Ok(()) => {
            println!("[ALLOWED] {lexeme} (control)");
            Ok(())
        }
        Err(e) => Err(e),
    }
}

fn drift_guard(committed: &[u8], fresh: &[u8]) -> Result<(), EmitError> {
    let first_diff = committed.iter().zip(fresh.iter()).position(|(a, b)| a != b);
    if first_diff.is_some() || committed.len() != fresh.len() {
        return Err(EmitError::PackDrift {
            first_diff,
            committed_len: committed.len(),
            fresh_len: fresh.len(),
        });
    }
    Ok(())
}

fn run() -> Result<(), EmitError> {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("--check") => {
            let (fresh, digest) = build_pack(&corpus())?;
            let committed = std::fs::read(pack_path()).map_err(|e| EmitError::Io(e.to_string()))?;
            drift_guard(&committed, &fresh)?;
            println!("OK {} bytes, digest {}", committed.len(), hex(&digest));
            Ok(())
        }
        Some("--probe") => {
            let lexeme = args
                .next()
                .ok_or_else(|| EmitError::Io("--probe needs a lexeme".to_string()))?;
            probe_one(lexeme)
        }
        Some("--inject") => {
            let lexeme = args
                .next()
                .ok_or_else(|| EmitError::Io("--inject needs a lexeme".to_string()))?;
            let mut full = corpus();
            full.push(Term { lexeme: lexeme.clone().into_bytes(), class: CLASS_NONE, enum_id: None });
            match build_pack(&full) {
                Err(e @ EmitError::TermRefused { .. }) => {
                    println!("[REFUSED] {e}");
                    Err(e)
                }
                Ok(_) => {
                    // Gate open for a refused-family term — loud, court FAIL.
                    println!("[ALLOWED] {lexeme} (GATE OPEN)");
                    Ok(())
                }
                Err(e) => Err(e),
            }
        }
        Some(other) => Err(EmitError::Io(format!("unknown mode {other:?} (see module docs)"))),
        None => {
            let (bytes, digest) = build_pack(&corpus())?;
            let path = pack_path();
            if path.exists() {
                let committed =
                    std::fs::read(&path).map_err(|e| EmitError::Io(e.to_string()))?;
                drift_guard(&committed, &bytes)?;
                println!("OK {} bytes (unchanged), digest {}", bytes.len(), hex(&digest));
                return Ok(());
            }
            // A05 crash-consistent first emit (wave 9): full temporary
            // write + `sync_all` + `rename(2)`; the committed path is
            // replaced atomically (module doc above for the law).
            let tmp = path.with_extension("oabi.w9tmp");
            let emit_tmp = || -> Result<(), EmitError> {
                use std::io::Write;
                let io = |ctx: &str, e: std::io::Error| {
                    EmitError::Io(format!("{ctx}: {e}"))
                };
                let mut f = std::fs::File::create(&tmp)
                    .map_err(|e| io("temp create", e))?;
                f.write_all(&bytes).map_err(|e| io("temp write", e))?;
                f.sync_all().map_err(|e| io("temp sync", e))?;
                drop(f);
                std::fs::rename(&tmp, &path)
                    .map_err(|e| io("atomic rename", e))
            };
            if let Err(e) = emit_tmp() {
                // Orphaned temporary is garbage outside the identity
                // surface; the destination law (absent or complete) holds.
                let _ = std::fs::remove_file(&tmp);
                return Err(e);
            }
            println!("emitted {} ({} bytes), digest {}", path.display(), bytes.len(), hex(&digest));
            Ok(())
        }
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(e.exit_code())
        }
    }
}
