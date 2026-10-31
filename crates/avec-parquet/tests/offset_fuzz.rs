//! Adversarial mmap-scan offset fuzz for `scan::ParquetFile` (wave-14 lane 2).
//!
//! Property under test: a hostile or corrupt file surface can never turn the
//! mmap scanner into a panic, a hang, or an untyped failure. Every outcome is
//! either a typed [`ScanError`] or the correct (aliased, pristine) bytes.
//!
//! Method (Chicago school: real files on disk, real mmap syscalls, assert
//! final state; no mocks — the one thread-per-case deadline harness exists to
//! convert a *hang* into a named finding, which a direct call cannot do):
//!
//! 1. **Footer/page offset sweep** — a valid level-free file is built by
//!    `scan::write_fixture_file`; its page region is then re-framed under a
//!    hand-written footer whose `data_page_offset` is each hostile value in
//!    {0, 1, 3, len-1, len, len+1, len-8, i64::MAX, negative, wire-u64::MAX,
//!    mid-payload, mid-header, footer-start, len-12}. The thrift writers of
//!    `crate::thrift` (the same primitives the fixture writer uses) keep the
//!    footer otherwise wire-exact, so the ONLY variable is the offset.
//!    Structurally-deterministic refusals are pinned to their exact
//!    `ScanError` variant; parse-dependent ones are held to the universal
//!    typed-or-correct law.
//! 2. **Truncation at 64 B boundaries** — every 64-byte-multiple prefix (plus
//!    0, len-1, and the full-length control) must fail `open()` with a typed
//!    error. Exact variants are pinned: `EmptyFile` at 0, `MalformedFooter`
//!    otherwise — sound because the fixture asserts it carries no stray
//!    `PAR1` inside the body, so every short prefix fails the trailing-magic
//!    check deterministically.
//! 3. **200 LCG random byte corruptions** — deterministic Knuth LCG
//!    (`x = x*6364136223846793005 + 1442695040888963407`), one fresh
//!    reproducible seed per case, stratified round-robin across six file
//!    regions (leading magic, page header, column a/b/c payload, footer) so
//!    "across header/payload/footer" has a hard coverage floor instead of a
//!    probability. Universal invariants on every case: no panic, no hang, and
//!    a resolved column slice is exactly `declared_rows * declared_width`
//!    bytes whose row-0 typed accessor agrees. Where sound (a case flipped
//!    only column j's payload region), every other resolved column must be
//!    byte-identical to the pristine payload — the "correct bytes" branch.
//!
//! A panic anywhere is THE finding: it surfaces as a `Panic` ledger outcome,
//! fails the corpus assert, and names its case seed.

#![cfg(feature = "std")]

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use avec_parquet::scan::{
    write_fixture_file, FixtureColumn, ParquetFile, PhysType, ScanError,
};
use avec_parquet::thrift::{
    ctype, read_page_header, write_binary_field, write_field_header, write_i32_field,
    write_i64_field,
};

/// Rows in the fixture corpus.
const ROWS: usize = 64;
/// Per-column value widths of the fixture corpus (asserted against the real
/// `PhysType::width()` so the constant cannot drift from the fixture).
const WIDTHS: [usize; 3] = [4, 8, 32];
/// Master seed. Bump only when deliberately re-drawing the corpus; per-case
/// seeds derive deterministically from this.
const MASTER_SEED: u64 = 0x5EED_2026_1031_0002;
/// Number of LCG corruption cases (mandate: 200).
const LCG_CASES: usize = 200;
/// Per-case deadline. A scan that fails to return is a HANG — a fail-closed
/// violation just as much as a panic (deadline-bounded receive, not a sleep).
const CASE_TIMEOUT: Duration = Duration::from_secs(5);

// ---------------------------------------------------------------------------
// Fixture corpus + file layout
// ---------------------------------------------------------------------------

/// Three-column level-free corpus (u32 a, u64 b, FLBA(32) c), mirroring the
/// scan.rs unit-test corpus. Returns the fixture columns and the pristine
/// per-column payload bytes used as the "correct bytes" reference.
fn corpus() -> (Vec<FixtureColumn>, Vec<Vec<u8>>) {
    let a: Vec<u32> = (0..ROWS as u32).map(|i| i.wrapping_mul(0x9E37_79B9)).collect();
    let b: Vec<u64> =
        (0..ROWS as u64).map(|i| i.wrapping_mul(0xC2B2_AE3D_27D4_4EBF)).collect();
    let c: Vec<u8> = (0..ROWS * 32).map(|i| ((i * 7 + 3) & 0xFF) as u8).collect();
    let cols = vec![
        FixtureColumn {
            name: "a".into(),
            phys: PhysType::U32,
            bytes: a.iter().flat_map(|v| v.to_le_bytes()).collect(),
        },
        FixtureColumn {
            name: "b".into(),
            phys: PhysType::U64,
            bytes: b.iter().flat_map(|v| v.to_le_bytes()).collect(),
        },
        FixtureColumn {
            name: "c".into(),
            phys: PhysType::Flba(32),
            bytes: c,
        },
    ];
    for (i, w) in WIDTHS.iter().enumerate() {
        assert_eq!(cols[i].phys.width(), *w, "WIDTHS[{i}] drift vs corpus");
    }
    let originals = cols.iter().map(|col| col.bytes.clone()).collect();
    (cols, originals)
}

/// Absolute byte spans of the fixture file's regions. The fixture writer
/// emits `[PAR1 | page header | payload (col-major a,b,c) | FileMetaData |
/// meta_len (u32 LE) | PAR1]`; this struct pins that tiling so region
/// stratification targets real regions, not guesses.
struct Layout {
    len: usize,
    header_len: usize,
    payload_len: usize,
    footer_start: usize,
    meta_len: usize,
    col_spans: [(usize, usize); 3],
}

fn layout_of(file: &[u8]) -> Layout {
    let len = file.len();
    assert!(len > 128, "fixture too small for 64 B boundary truncation: {len}");
    let header_len = match read_page_header(&file[4..]) {
        Ok((_, n)) => n,
        Err(e) => panic!("fixture page header must parse: {e:?}"),
    };
    let payload_len: usize = ROWS * WIDTHS.iter().sum::<usize>();
    let meta_len =
        u32::from_le_bytes(file[len - 8..len - 4].try_into().expect("4 bytes")) as usize;
    assert!(meta_len > 4, "fixture meta_len implausible: {meta_len}");
    let footer_start = len - 8 - meta_len;
    // The tiling is load-bearing for every region assertion below.
    assert_eq!(
        footer_start,
        4 + header_len + payload_len,
        "fixture layout: footer must start where the page region ends"
    );
    let mut col_spans = [(0usize, 0usize); 3];
    let mut cum = 4 + header_len;
    for (i, w) in WIDTHS.iter().enumerate() {
        col_spans[i] = (cum, cum + ROWS * w);
        cum += ROWS * w;
    }
    Layout { len, header_len, payload_len, footer_start, meta_len, col_spans }
}

/// The six corruption regions of the LCG corpus, stratified round-robin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Region {
    /// Leading `PAR1` magic, bytes 0..4.
    Magic,
    /// The Thrift page header, 4..4+header_len.
    PageHeader,
    /// Column i's payload slice of the shared page.
    Col(usize),
    /// FileMetaData + meta_len prefix + trailing `PAR1`.
    Footer,
}

impl Region {
    fn span(self, lay: &Layout) -> (usize, usize) {
        match self {
            Region::Magic => (0, 4),
            Region::PageHeader => (4, 4 + lay.header_len),
            Region::Col(i) => lay.col_spans[i],
            Region::Footer => (lay.footer_start, lay.len),
        }
    }
}

// ---------------------------------------------------------------------------
// Re-framing helper: same wire shape as the fixture writer, parameterized
// data_page_offset. Built only from the crate's public thrift writers.
// ---------------------------------------------------------------------------

/// `PhysType` -> parquet.thrift `Type` id (mirrors the scan-side mapping).
fn phys_id(p: PhysType) -> i32 {
    match p {
        PhysType::U32 => 1,
        PhysType::U64 => 2,
        PhysType::Flba(_) => 7,
    }
}

/// Compact-protocol list header (`size << 4 | elem` below 15, else varint) —
/// the same encoding scan.rs writes; kept local because scan.rs's helper is
/// private and the fixture never exceeds 15 elements.
fn list_hdr(dst: &mut Vec<u8>, len: usize, elem_type: u8) {
    if len < 15 {
        dst.push(((len as u8) << 4) | elem_type);
    } else {
        dst.push(0xF0 | elem_type);
        let mut v = len as u64;
        loop {
            let mut b = (v & 0x7F) as u8;
            v >>= 7;
            if v != 0 {
                b |= 0x80;
            }
            dst.push(b);
            if v == 0 {
                break;
            }
        }
    }
}

/// Re-frame the fixture's page region under a hand-written minimal footer
/// whose single ColumnChunk declares `data_page_offset = page_offset`.
/// Everything else (schema, page bytes, framing) is the fixture's own, so the
/// offset is the only variable in the sweep.
fn build_file_with_page_offset(
    columns: &[FixtureColumn],
    num_rows: usize,
    page: &[u8],
    page_offset: i64,
) -> Vec<u8> {
    // SchemaElement list: root + one REQUIRED leaf per column.
    let mut root = Vec::new();
    write_binary_field(&mut root, 4, 0, b"schema");
    write_i32_field(&mut root, 5, 4, columns.len() as i32);
    root.push(ctype::STOP);
    let mut schema_wire = Vec::new();
    schema_wire.extend_from_slice(&root);
    for c in columns {
        let mut e = Vec::new();
        let mut prev = 0i16;
        write_i32_field(&mut e, 1, prev, phys_id(c.phys));
        prev = 1;
        if let PhysType::Flba(n) = c.phys {
            write_i32_field(&mut e, 2, prev, n as i32);
            prev = 2;
        }
        write_i32_field(&mut e, 3, prev, 0); // REQUIRED
        prev = 3;
        write_binary_field(&mut e, 4, prev, c.name.as_bytes());
        e.push(ctype::STOP);
        schema_wire.extend_from_slice(&e);
    }

    // RowGroup -> ColumnChunk -> ColumnMetaData{data_page_offset}.
    let mut md = Vec::new();
    write_i64_field(&mut md, 9, 0, page_offset);
    md.push(ctype::STOP);
    let mut cc = Vec::new();
    write_field_header(&mut cc, 3, 0, ctype::STRUCT);
    cc.extend_from_slice(&md);
    cc.push(ctype::STOP);
    let mut rg = Vec::new();
    write_field_header(&mut rg, 1, 0, ctype::LIST);
    list_hdr(&mut rg, 1, ctype::STRUCT);
    rg.extend_from_slice(&cc);
    write_i64_field(&mut rg, 3, 1, num_rows as i64);
    rg.push(ctype::STOP);

    // FileMetaData.
    let mut fmd = Vec::new();
    write_i32_field(&mut fmd, 1, 0, 1); // version
    write_field_header(&mut fmd, 2, 1, ctype::LIST);
    list_hdr(&mut fmd, columns.len() + 1, ctype::STRUCT);
    fmd.extend_from_slice(&schema_wire);
    write_i64_field(&mut fmd, 3, 2, num_rows as i64);
    write_field_header(&mut fmd, 4, 3, ctype::LIST);
    list_hdr(&mut fmd, 1, ctype::STRUCT);
    fmd.extend_from_slice(&rg);
    fmd.push(ctype::STOP);

    let mut out = Vec::with_capacity(4 + page.len() + fmd.len() + 8);
    out.extend_from_slice(b"PAR1");
    out.extend_from_slice(page);
    out.extend_from_slice(&fmd);
    out.extend_from_slice(&(fmd.len() as u32).to_le_bytes());
    out.extend_from_slice(b"PAR1");
    out
}

// ---------------------------------------------------------------------------
// Deterministic LCG (Knuth MMIX constants) — one fresh, printable seed per
// case so any finding is reproducible from its label alone.
// ---------------------------------------------------------------------------

struct Lcg(u64);

impl Lcg {
    fn new(seed: u64) -> Self {
        Lcg(seed)
    }
    fn next_u64(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0
    }
    fn byte(&mut self) -> u8 {
        (self.next_u64() >> 33) as u8
    }
    fn below(&mut self, n: usize) -> usize {
        ((self.next_u64() >> 33) as usize) % n.max(1)
    }
}

fn case_seed(i: usize) -> u64 {
    MASTER_SEED ^ (i as u64).wrapping_mul(0xD1B5_4A32_D192_ED03).wrapping_add(1)
}

// ---------------------------------------------------------------------------
// Temp file (real disk, unique per-run path, removed on drop)
// ---------------------------------------------------------------------------

struct TempFile {
    path: PathBuf,
}

impl TempFile {
    fn write(tag: &str, bytes: &[u8]) -> TempFile {
        use std::sync::atomic::{AtomicU64, Ordering};
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let seq = SEQ.fetch_add(1, Ordering::Relaxed);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let mut p = std::env::temp_dir();
        p.push(format!(
            "avec_offset_fuzz_{tag}_{}_{}_{}",
            std::process::id(),
            nanos,
            seq
        ));
        std::fs::write(&p, bytes).expect("write fuzz fixture");
        TempFile { path: p }
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

// ---------------------------------------------------------------------------
// Probe harness: one case = write file -> mmap open -> resolve all three
// columns, on a throwaway thread with a deadline. Panic/hang are named
// outcomes, never test-process crashes that hide the case.
// ---------------------------------------------------------------------------

#[derive(Debug)]
enum Outcome {
    /// `open()` refused: typed ScanError.
    OpenErr(ScanError),
    /// `open()` succeeded, column resolve refused: typed ScanError.
    ColErr { col: usize, err: ScanError },
    /// All three columns resolved: their aliased bytes.
    Resolved { cols: Vec<(usize, Vec<u8>)> },
    /// Universal-invariant breach short of a panic (typed outcomes that must
    /// not co-occur, e.g. an accessor refusing a column it already resolved).
    Invariant(String),
    Panic(String),
    Hang,
}

fn probe(bytes: Vec<u8>) -> Outcome {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let outcome =
            catch_unwind(AssertUnwindSafe(move || probe_inner(bytes)));
        let outcome = match outcome {
            Ok(o) => o,
            Err(payload) => Outcome::Panic(panic_message(&payload)),
        };
        let _ = tx.send(outcome);
    });
    match rx.recv_timeout(CASE_TIMEOUT) {
        Ok(o) => o,
        Err(_) => Outcome::Hang,
    }
}

fn probe_inner(bytes: Vec<u8>) -> Outcome {
    let tmp = TempFile::write("probe", &bytes);
    let pf = match ParquetFile::open(&tmp.path) {
        Ok(pf) => pf,
        Err(e) => return Outcome::OpenErr(e),
    };
    let declared_rows = pf.row_groups()[0].num_rows;
    let mut resolved = Vec::new();
    for col in 0..3usize {
        let slice = match pf.column_bytes(0, col) {
            Ok(s) => s,
            Err(err) => return Outcome::ColErr { col, err },
        };
        // Exact-width law: a resolved column is exactly
        // declared_rows * declared_width bytes of the mapping.
        let Some(desc) = pf.meta().columns.get(col) else {
            return Outcome::Invariant(format!(
                "column {col} resolved but the footer declares only {} columns",
                pf.meta().columns.len()
            ));
        };
        let want_len = declared_rows as usize * desc.width;
        if slice.len() != want_len {
            return Outcome::Invariant(format!(
                "column {col} resolved to {} bytes, want {want_len} (declared_rows * declared_width)",
                slice.len()
            ));
        }
        // Typed-accessor agreement on row 0 — strict when the (possibly
        // mutated) declared schema still matches the fixture corpus.
        let schema_matches = declared_rows == ROWS && desc.width == WIDTHS[col];
        match col {
            0 if schema_matches => {
                let want = u32::from_le_bytes(slice[0..4].try_into().expect("4"));
                match pf.value_u32(0, 0, 0) {
                    Ok(v) if v == want => {}
                    Ok(v) => {
                        return Outcome::Invariant(format!(
                            "value_u32(0,0,0)={v:#x} disagrees with aliased bytes {want:#x}"
                        ))
                    }
                    Err(e) => {
                        return Outcome::Invariant(format!(
                            "value_u32(0,0,0) refused ({e}) after column_bytes resolved the same column"
                        ))
                    }
                }
            }
            1 if schema_matches => {
                let want = u64::from_le_bytes(slice[0..8].try_into().expect("8"));
                match pf.value_u64(0, 1, 0) {
                    Ok(v) if v == want => {}
                    Ok(v) => {
                        return Outcome::Invariant(format!(
                            "value_u64(0,1,0)={v:#x} disagrees with aliased bytes {want:#x}"
                        ))
                    }
                    Err(e) => {
                        return Outcome::Invariant(format!(
                            "value_u64(0,1,0) refused ({e}) after column_bytes resolved the same column"
                        ))
                    }
                }
            }
            2 if schema_matches => match pf.value_flba(0, 2, 0, 32) {
                Ok(v) if v == &slice[0..32] => {}
                Ok(_) => {
                    return Outcome::Invariant(
                        "value_flba(0,2,0,32) disagrees with aliased bytes".into(),
                    )
                }
                Err(e) => {
                    return Outcome::Invariant(format!(
                        "value_flba(0,2,0,32) refused ({e}) after column_bytes resolved the same column"
                    ))
                }
            },
            // Mutated schema: the accessor must still return SOME typed
            // outcome for row 0; call it so a panic here is the finding.
            0 => {
                let _ = pf.value_u32(0, 0, 0);
            }
            1 => {
                let _ = pf.value_u64(0, 1, 0);
            }
            _ => {
                let _ = pf.value_flba(0, 2, 0, desc.width);
            }
        }
        resolved.push((col, slice.to_vec()));
    }
    Outcome::Resolved { cols: resolved }
}

fn panic_message(payload: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else {
        "opaque panic payload".to_string()
    }
}

// ---------------------------------------------------------------------------
// Ledger
// ---------------------------------------------------------------------------

/// What a class demands beyond the universal invariants (no panic, no hang,
/// no invariant breach).
enum Expect {
    /// Exactly this typed refusal (deterministic structural path).
    Exact(ScanError),
    /// Typed-or-correct; only universal invariants apply.
    Universal,
}

struct Ledger {
    class: &'static str,
    cases: usize,
    open_err: usize,
    col_err: usize,
    resolved: usize,
    failures: Vec<String>,
}

impl Ledger {
    fn new(class: &'static str) -> Self {
        Ledger {
            class,
            cases: 0,
            open_err: 0,
            col_err: 0,
            resolved: 0,
            failures: Vec::new(),
        }
    }

    fn check(&mut self, label: &str, outcome: &Outcome, expect: &Expect) {
        self.cases += 1;
        let mut v: Vec<String> = Vec::new();
        match outcome {
            Outcome::Panic(msg) => v.push(format!("PANIC: {msg}")),
            Outcome::Hang => {
                v.push(format!("HANG: no return within {CASE_TIMEOUT:?}"))
            }
            Outcome::Invariant(msg) => v.push(format!("INVARIANT: {msg}")),
            Outcome::OpenErr(e) => {
                self.open_err += 1;
                if let Expect::Exact(want) = expect {
                    if e != want {
                        v.push(format!(
                            "expected Err({want:?}) at open, got Err({e:?})"
                        ));
                    }
                }
            }
            Outcome::ColErr { col, err } => {
                self.col_err += 1;
                if let Expect::Exact(want) = expect {
                    if err != want {
                        v.push(format!(
                            "expected Err({want:?}) at column {col}, got Err({err:?})"
                        ));
                    }
                }
            }
            Outcome::Resolved { .. } => {
                self.resolved += 1;
                if matches!(expect, Expect::Exact(_)) {
                    v.push("expected a typed refusal, resolved instead".to_string());
                }
            }
        }
        if !v.is_empty() {
            self.failures
                .push(format!("[class={}] [case={label}] => {}", self.class, v.join("; ")));
        }
    }
}

// ---------------------------------------------------------------------------
// 1. Footer/page offset corruption sweep
// ---------------------------------------------------------------------------

#[test]
fn footer_page_offset_corruption_sweep() {
    let started = Instant::now();
    let (cols, originals) = corpus();
    let fixture = write_fixture_file(&cols, ROWS);
    let lay = layout_of(&fixture);
    let full_len = lay.len;
    // The sweep re-frames THIS page region under hostile footers.
    let page: Vec<u8> =
        fixture[4..4 + lay.header_len + lay.payload_len].to_vec();

    // Non-vacuity control: the sweep builder, given the LAWFUL offset (4 =
    // right after the leading PAR1), must behave exactly like the fixture
    // file — open, resolve, byte-equal every column. Without this the whole
    // sweep could be proving things about malformed footers.
    let lawful = build_file_with_page_offset(&cols, ROWS, &page, 4);
    match probe(lawful) {
        Outcome::Resolved { cols: got } => {
            let got: Vec<&Vec<u8>> = got.iter().map(|(_, b)| b).collect();
            let want: Vec<&Vec<u8>> = originals.iter().collect();
            assert_eq!(
                got, want,
                "sweep-builder control at the lawful offset must be byte-exact"
            );
        }
        other => panic!("sweep-builder control must resolve, got {other:?}"),
    }

    let mut led = Ledger::new("page_offset_sweep");

    // Structurally-deterministic refusals (pinned to the exact variant):
    // page_slice refuses offsets inside the leading magic and offsets whose
    // 12-byte minimum cannot fit before EOF; the i64 zigzag gate refuses
    // negative page offsets at footer-parse time. `u64::MAX` is unreachable
    // AS an i64-carried offset — on the wire it is i64::MIN's zigzag (raw
    // varint u64::MAX), which decodes negative and is refused FooterOverflow.
    let sweep: Vec<(&str, i64, Expect)> = vec![
        ("zero", 0, Expect::Exact(ScanError::OffsetMismatch)),
        ("one", 1, Expect::Exact(ScanError::OffsetMismatch)),
        ("three (last magic byte)", 3, Expect::Exact(ScanError::OffsetMismatch)),
        ("len-1", full_len as i64 - 1, Expect::Exact(ScanError::OffsetMismatch)),
        ("len", full_len as i64, Expect::Exact(ScanError::OffsetMismatch)),
        ("len+1", full_len as i64 + 1, Expect::Exact(ScanError::OffsetMismatch)),
        (
            "len-8 (the meta_len prefix)",
            full_len as i64 - 8,
            Expect::Exact(ScanError::OffsetMismatch),
        ),
        (
            "i64::MAX (2^63-1, past the tail bound)",
            i64::MAX,
            Expect::Exact(ScanError::OffsetMismatch),
        ),
        (
            "wire-u64::MAX (i64::MIN zigzag)",
            i64::MIN,
            Expect::Exact(ScanError::FooterOverflow),
        ),
        ("negative -1", -1, Expect::Exact(ScanError::FooterOverflow)),
        // Parse-dependent outcomes: held to the universal typed-or-correct
        // law, not to a pinned variant.
        (
            "mid-payload (page start + header + 128)",
            (4 + lay.header_len + 128) as i64,
            Expect::Universal,
        ),
        (
            "mid-page-header",
            (4 + lay.header_len / 2) as i64,
            Expect::Universal,
        ),
        ("footer-start", lay.footer_start as i64, Expect::Universal),
        (
            "len-12 (4 bytes of declared meta_len)",
            full_len as i64 - 12,
            Expect::Universal,
        ),
    ];

    let mut pins = 0usize;
    for (label, off, expect) in sweep {
        let bytes = build_file_with_page_offset(&cols, ROWS, &page, off);
        let outcome = probe(bytes);
        if matches!(expect, Expect::Exact(_)) {
            pins += 1;
        }
        led.check(&format!("offset={label} ({off})"), &outcome, &expect);
        println!("  offset={label:<44} -> {outcome:?}");
    }

    // Footer-length-prefix corruption (the footer's own "offset" of the
    // metadata window): pinned where the arithmetic is structural.
    let mut led_ml = Ledger::new("footer_len_prefix");
    for (label, ml, expect) in [
        ("0", 0u32, Some(Expect::Exact(ScanError::FooterOverflow))),
        ("u32::MAX", u32::MAX, Some(Expect::Exact(ScanError::FooterOverflow))),
        ("true-1", (lay.meta_len - 1) as u32, None),
        ("true+1", (lay.meta_len + 1) as u32, None),
    ] {
        let mut bytes = fixture.clone();
        bytes[full_len - 8..full_len - 4].copy_from_slice(&ml.to_le_bytes());
        let expect =
            expect.unwrap_or(Expect::Universal);
        led_ml.check(&format!("meta_len={label}"), &probe(bytes), &expect);
    }

    let failures: Vec<String> = led
        .failures
        .iter()
        .chain(led_ml.failures.iter())
        .cloned()
        .collect();
    println!(
        "  offset sweep: {} cases ({pins} exact-variant pins, {} parse-dependent) \
         + meta_len family {} cases; {} open-refused / {} column-refused / {} resolved; \
         {} violations in {:?}",
        led.cases,
        led.cases - pins,
        led_ml.cases,
        led.open_err,
        led.col_err,
        led.resolved,
        failures.len(),
        started.elapsed()
    );
    assert_eq!(led.cases, 14, "offset sweep corpus size pinned");
    assert_eq!(led_ml.cases, 4, "meta_len family size pinned");
    assert!(
        failures.is_empty(),
        "FAIL-CLOSED VIOLATIONS ({}):\n  {}",
        failures.len(),
        failures.join("\n  ")
    );
}

// ---------------------------------------------------------------------------
// 2. Truncation at 64 B boundaries
// ---------------------------------------------------------------------------

#[test]
fn truncate_at_64b_boundaries_open_fails_typed() {
    let started = Instant::now();
    let (cols, originals) = corpus();
    let file = write_fixture_file(&cols, ROWS);
    let len = file.len();

    // Precondition for the exact pins below: no stray `PAR1` inside the body,
    // so every proper prefix fails the trailing-magic check deterministically.
    assert!(
        !file[4..len - 4].windows(4).any(|w| w == b"PAR1"),
        "fixture precondition: no stray PAR1 inside the file body"
    );

    let mut lengths: Vec<(usize, &'static str)> = vec![(0, "zero")];
    let mut n = 64;
    while n < len {
        lengths.push((n, "64B boundary"));
        n += 64;
    }
    lengths.push((len - 1, "len-1"));
    lengths.push((len, "full-length control"));

    let mut led = Ledger::new("truncation_64b");
    for (n, kind) in &lengths {
        let prefix = file[..*n].to_vec();
        let expect = if *n == 0 {
            Expect::Exact(ScanError::EmptyFile)
        } else if *n == len {
            // The control must NOT be a refusal — assert that separately
            // below; here a resolution is the passing shape.
            Expect::Universal
        } else {
            Expect::Exact(ScanError::MalformedFooter)
        };
        let outcome = probe(prefix);
        led.check(&format!("len={n} ({kind})"), &outcome, &expect);
    }

    // Full-length control: real open, every column byte-identical, typed
    // accessors spot-checked across the row range.
    let tmp = TempFile::write("trunc_control", &file);
    let pf = ParquetFile::open(&tmp.path).expect("full-length control opens");
    assert_eq!(pf.num_rows(), ROWS as i64);
    assert_eq!(pf.meta().columns.len(), 3);
    for (col, original) in originals.iter().enumerate() {
        assert_eq!(
            pf.column_bytes(0, col).expect("control column"),
            &original[..],
            "control column {col} must be the pristine payload"
        );
    }
    for row in [0usize, 17, ROWS - 1] {
        assert!(pf.value_u32(0, 0, row).is_ok());
        assert!(pf.value_u64(0, 1, row).is_ok());
        assert!(pf.value_flba(0, 2, row, 32).is_ok());
    }

    println!(
        "  truncation: {} lengths ({} at 64 B boundaries) in {:?}; \
         {} open-refused / {} resolved; {} violations",
        led.cases,
        lengths.iter().filter(|(_, k)| *k == "64B boundary").count(),
        started.elapsed(),
        led.open_err,
        led.resolved,
        led.failures.len()
    );
    assert!(led.cases >= 10, "truncation corpus shrank: {}", led.cases);
    assert!(
        led.failures.is_empty(),
        "FAIL-CLOSED VIOLATIONS ({}):\n  {}",
        led.failures.len(),
        led.failures.join("\n  ")
    );
}

// ---------------------------------------------------------------------------
// 3. 200 LCG random byte corruptions
// ---------------------------------------------------------------------------

#[test]
fn lcg_random_byte_corruption_corpus() {
    let started = Instant::now();
    let (cols, originals) = corpus();
    let file = write_fixture_file(&cols, ROWS);
    let lay = layout_of(&file);
    assert_eq!(lay.col_spans[2].1, lay.footer_start, "payload tiling");

    let cycle = [
        Region::Magic,
        Region::PageHeader,
        Region::Col(0),
        Region::Col(1),
        Region::Col(2),
        Region::Footer,
    ];
    let mut led = Ledger::new("lcg_corruption");
    let mut region_hits = [0usize; 6];
    let mut equality_checks = 0usize;

    for i in 0..LCG_CASES {
        let seed = case_seed(i);
        let mut rng = Lcg::new(seed);
        let region = cycle[i % cycle.len()];
        let k = 1 + rng.below(3); // 1..=3 flipped bytes per case
        let mut bytes = file.clone();
        let mut positions = Vec::new();
        let (lo, hi) = region.span(&lay);
        for _ in 0..k {
            let pos = lo + rng.below(hi - lo);
            bytes[pos] ^= rng.byte().max(1); // never a no-op flip
            positions.push(pos);
        }
        region_hits[i % cycle.len()] += 1;
        let label =
            format!("case={i} seed=0x{seed:016x} region={region:?} k={k} pos={positions:?}");

        // A flipped leading magic must be a typed MalformedFooter at open
        // (deterministic: the magic equality check is structural).
        let expect = if region == Region::Magic {
            Expect::Exact(ScanError::MalformedFooter)
        } else {
            Expect::Universal
        };
        let outcome = probe(bytes);
        led.check(&label, &outcome, &expect);

        // "Correct bytes" branch: when a case touched ONLY column j's
        // payload region, the footer, page header, and every other column's
        // bytes are pristine — so any column that resolves must be
        // byte-identical to the pristine payload. (Page-header flips may
        // shift the parsed header length and footer flips may rewrite the
        // offsets/schema, so equality is not sound there and is not pinned.)
        if let (Region::Col(j), Outcome::Resolved { cols: resolved }) = (region, &outcome)
        {
            for (col, got) in resolved {
                if *col != j {
                    equality_checks += 1;
                    if got != &originals[*col] {
                        led.failures.push(format!(
                            "[{}] resolved column {col} differs from the pristine \
                             payload though only column {j}'s region was touched",
                            label
                        ));
                    }
                }
            }
        }
    }

    // Coverage floor: stratification guarantees every region is exercised;
    // assert it so an edited region list can never silently un-cover one.
    for (region, hits) in cycle.iter().zip(region_hits.iter()) {
        assert!(
            *hits >= 30,
            "region {region:?} covered only {hits} times — stratification broke"
        );
    }

    println!(
        "  lcg_corruption: {} cases in {:?}; regions hit {:?}; \
         {} open-refused / {} column-refused / {} resolved; \
         {equality_checks} byte-equality checks; {} violations",
        led.cases,
        started.elapsed(),
        region_hits,
        led.open_err,
        led.col_err,
        led.resolved,
        led.failures.len()
    );
    assert_eq!(led.cases, LCG_CASES, "corpus size pinned at the mandate");
    assert!(
        led.failures.is_empty(),
        "FAIL-CLOSED VIOLATIONS ({}):\n  {}",
        led.failures.len(),
        led.failures.join("\n  ")
    );
}
