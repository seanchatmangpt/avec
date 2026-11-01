//! Lane A19 — libcgap sovereign C-ABI surface: the Wave 1 ABI freeze,
//! hardened in Wave 4 (C-side `CgapStatus` width pin, M11/M12 mutation
//! corpus, C++ compile leg with its own rename-flip witness), repaired in
//! Wave 7 per the wave-6 court receipts (ledger walk excludes lane build
//! roots), re-tightened in Wave 8 per the wave-7 court receipt
//! (a08-wave7: anchor uniqueness + isolated-flip discipline; in-code
//! receipt block below), and extended in Wave 9 with the cross-lane
//! integration slice: five real-subprocess courts executing the named
//! campaign integration surfaces (A04 identity, A14 dam over A04 frames,
//! A20 1M-row stress, A05 power-cut sweep, A07 zero-string gate over all
//! built crates) from the membrane's seat — receipts in-code below.
//!
//! Spec: `docs/jira/v26.11.1/lanes/A19-libcgap-ffi-xip.md` §2 (normative C
//! signatures, `CgapStatus` register + reachability law, caller/callee
//! ownership table). Wave 1 slice: struct/const definitions plus size/align
//! static asserts only — zero logic. The four sovereign bodies are owned by
//! implementation lanes (§2 owner table) and land behind cfg gates in later
//! waves (DoD 5):
//!
//! | Entry point         | Implementation owner |
//! |---------------------|----------------------|
//! | `cgap_admit_frame`  | A14 + A16            |
//! | `cgap_eval_bn3`     | A12 + A13            |
//! | `cgap_commit_parquet` | A02 + A03          |
//! | `cgap_verify_p28`   | A17                  |
//!
//! C-side twin: `include/cgap.h` (frozen in the same wave; the §6.1 header
//! court landed in Wave 3 below — a mechanical parity gate between the two,
//! non-vacuous by its mutation corpus).
//!
//! Upstream wire layouts are cited, not redefined here: 128 B
//! `CgapFrameHeader` (A04 §6, `crates/cgap-core/src/frame.rs`) and 80 B
//! `P28LicenseAttestation` (A17 §2, `crates/cgap-p28/src/attestation.rs`).
//! The sovereign surface carries neither struct across the ABI — admit and
//! verify see raw `bytes`/`evidence`; the only view crossing the boundary is
//! `CgapFrameView` below. `.oabi` tables (A06 §6) and `.bn3` networks (A11)
//! stay flash-rodata bytes under `bn3_rodata`.
//!
//! Cross-lane note: `cgap-gate`'s Wave-1 penstock (A14) pins copies of the
//! status register values below ("frozen copy pending Wave-3 re-export");
//! this module is the canonical A19 register those copies cite. Wave 3
//! enforces the copy parity mechanically (status court, below).

#![allow(dead_code)] // Wave 1 freezes the ABI; the logic that reads it lands later.

// ---------------------------------------------------------------------------
// ABI version (§2 "ABI versioning")
// ---------------------------------------------------------------------------

/// Explicit version gate for the four sovereigns (§2). Mirrors
/// `#define CGAP_ABI_VERSION 1u` in `include/cgap.h`; the §6.1 header court
/// asserts parity. A future incompatible change bumps both together;
/// `cgap_version()` remains the human-readable triple for the frozen slot
/// surface and is NOT this version.
pub const CGAP_ABI_VERSION: u32 = 1;

const _: () = assert!(CGAP_ABI_VERSION == 1);

// ---------------------------------------------------------------------------
// CgapStatus register (§2 "C signatures (normative)"; C: `typedef uint32_t
// CgapStatus` + anonymous enum, mirrored name-for-name and value-for-value)
// ---------------------------------------------------------------------------

/// The sovereign status word (§2). Every value below MUST stay reachable
/// through a real code path on some supported target (§2 reachability law;
/// enforced by the §6.2 status court — an enum value no injected fault can
/// produce is untested decoration).
pub type CgapStatus = u32;

/// §2: admitted.
pub const CGAP_OK: CgapStatus = 0x0000_0000;
/// §2: magic/format gate refusal (`'CG' + 01`). Producers: A14+A16.
pub const CGAP_ERR_INVALID_MAGIC: CgapStatus = 0x4347_0001;
/// §2: OABI rule-table version mismatch (`'CG' + 02`; tables: A06 §6).
/// Producers: A12+A13.
pub const CGAP_ERR_OABI_MISMATCH: CgapStatus = 0x4347_0002;
/// §2: SHACL shape violation (`'CG' + 03`). Producers: A14+A16.
pub const CGAP_ERR_SHACL_VIOLATION: CgapStatus = 0x4347_0003;
/// §2: documented lease table exhausted (`'CG' + 04`). Producers: A02+A03.
pub const CGAP_ERR_LEASE_EXHAUSTED: CgapStatus = 0x4347_0004;
/// §2: P28 spoliation — digest/license predicate failure (`'P28' + FF`;
/// attestation layout: A17 §2). Producers: A17.
pub const CGAP_ERR_P28_SPOLIATION: CgapStatus = 0x5032_38FF;

const _: () = assert!(core::mem::size_of::<CgapStatus>() == 4);

// ---------------------------------------------------------------------------
// CgapFrameView (§2: header-defined, non-owning, zero-copy; C twin:
// `cgap_frame_view` in include/cgap.h)
// ---------------------------------------------------------------------------

/// Header-defined, non-owning, zero-copy view into an admitted frame (§2).
///
/// `out_view.payload` points INTO the caller's `bytes` — no allocation, no
/// copy; the view dies with `bytes` (§2 ownership table). This is the only
/// structure the sovereign surface defines: admit produces a view, never a
/// buffer (§2 zero-copy guarantees), and eval reads `view->payload` in place
/// (on XIP, straight through the flash cache — no SRAM staging buffer in the
/// budget).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CgapFrameView {
    /// Points INTO the caller's bytes — zero copy (§2).
    pub payload: *const u8,
    /// Length in bytes of the visible payload.
    pub payload_len: usize,
    /// Schema id of the admitted frame (wire tier header: A04 §6).
    pub schema_id: u32,
    /// Rule count of the admitted frame (.oabi tables: A06 §6).
    pub rule_count: u32,
}

// ---------------------------------------------------------------------------
// Wave 1 ABI freeze pins. Core-only const asserts, no `static_assertions`
// crate (zero-dependency rule). C twins: `_Static_assert` in include/cgap.h.
//
// The view is pointer-width dependent by construction, so BOTH forms are
// pinned explicitly: host LP64 and XIP ILP32 (ESP32-S3 / ESP32-C3 rv32imc).
//
// Anti-vacuity: these are the flip-on-mutation court for the ABI freeze —
// widening `CgapFrameView` (adding/reordering/removing a field) or changing
// each pinned constant MUST fail `cargo build -p cgap-ffi`. A build that
// still passes after such a mutation is a VACUOUS GATE.
// ---------------------------------------------------------------------------

#[cfg(target_pointer_width = "64")]
const _: () = {
    assert!(core::mem::size_of::<CgapFrameView>() == 24);
    assert!(core::mem::align_of::<CgapFrameView>() == 8);
    assert!(core::mem::offset_of!(CgapFrameView, payload) == 0);
    assert!(core::mem::offset_of!(CgapFrameView, payload_len) == 8);
    assert!(core::mem::offset_of!(CgapFrameView, schema_id) == 16);
    assert!(core::mem::offset_of!(CgapFrameView, rule_count) == 20);
};

#[cfg(target_pointer_width = "32")]
const _: () = {
    assert!(core::mem::size_of::<CgapFrameView>() == 16);
    assert!(core::mem::align_of::<CgapFrameView>() == 4);
    assert!(core::mem::offset_of!(CgapFrameView, payload) == 0);
    assert!(core::mem::offset_of!(CgapFrameView, payload_len) == 4);
    assert!(core::mem::offset_of!(CgapFrameView, schema_id) == 8);
    assert!(core::mem::offset_of!(CgapFrameView, rule_count) == 12);
};

// ---------------------------------------------------------------------------
// Rust-side signature freeze (§2 normative C signatures)
//
// fn-pointer type aliases mirror the frozen C prototypes one-to-one so the
// owner lanes implement against the frozen signature and the §6.1 header
// court has a mechanical Rust-side diff target — without emitting symbols
// (bodies land with their owner lanes, DoD 5) and without colliding with the
// future `#[no_mangle]` exports in this crate.
// ---------------------------------------------------------------------------

/// `cgap_admit_frame` — bytes enter; magic/format/SHACL gate or typed refusal.
///
/// `out_view` is caller-allocated and MAY be NULL (probe-only call);
/// `out_view.payload` borrows the caller's `bytes` (zero copy). Divergence
/// note: the lane-doc §2 sketch writes `const cgap_frame_view* out_view`,
/// which no callee could write through — the frozen header/Rust signature
/// uses a non-const out-parameter, per the doc's own ownership table
/// ("callee (pointer only)" fills `out_view.payload`).
pub type CgapAdmitFrameFn = unsafe extern "C" fn(
    bytes: *const u8,
    len: usize,
    out_view: *mut CgapFrameView,
) -> CgapStatus;

/// `cgap_eval_bn3` — admitted bytes evaluate deterministically against a
/// flash-resident `.bn3` network (A11 layout); zero copies of the input.
/// The callee never writes past `lanes_cap`; the ONLY copy in the whole
/// surface is the `out_lanes` write-out, caller-owned (§2).
pub type CgapEvalBn3Fn = unsafe extern "C" fn(
    view: *const CgapFrameView,
    bn3_rodata: *const u8,
    bn3_len: usize,
    out_lanes: *mut i32,
    lanes_cap: usize,
    out_lane_count: *mut usize,
) -> CgapStatus;

/// `cgap_commit_parquet` — evaluated batches commit to Parquet; HOST targets
/// only. An XIP/no_std build omits this symbol entirely (cfg), so it never
/// enters the SRAM budget (§2 ownership rules, §5 size budget). Callee
/// allocates here and only here (§2: "The callee performs no allocation in
/// admit/eval/verify. Commit allocates on host targets only.").
pub type CgapCommitParquetFn = unsafe extern "C" fn(
    lanes: *const i32,
    lane_count: usize,
    path: *const core::ffi::c_char,
) -> CgapStatus;

/// `cgap_verify_p28` — P28 spoliation evidence verification; digest +
/// license predicate over the embedded 80-byte `P28LicenseAttestation`
/// (A17 §2); read-only, no allocation. `out_digest` MAY be NULL.
pub type CgapVerifyP28Fn = unsafe extern "C" fn(
    evidence: *const u8,
    len: usize,
    out_digest: *mut u64,
) -> CgapStatus;

// ===========================================================================
// Wave 3 — lane-doc §6 courts. This section is the Wave-3 slice: §6.1 header
// court, §6.2 status court, §6.4 zero-copy witness. All three live in
// `#[cfg(test)]` modules (test-only; the ABI surface above stays byte-stable).
//
// Court-wide discipline (lane contract): real collaborators (the checked-in
// header on disk, the cc/clang subprocess, real heap buffers, raw pointer
// writes), assert final state, no mocks. Anti-vacuity law: a court that
// cannot flip is FAIL — every court carries its own mutation corpus or
// negative control, and a mutation that goes undetected fails the court as a
// VACUOUS GATE.
//
// Owner-lane reality (§2 owner table; recorded per DoD 5, not silently
// assumed): the four sovereign BODIES are owned by A14+A16 / A12+A13 /
// A02+A03 / A17 and have NOT landed in this crate — there are no
// `#[no_mangle]` sovereign exports yet. The courts below pin the exact
// current reality and flip loudly when it moves.
//
// Wave 7 — IN-CODE RECEIPT (defect repair driven by the wave-6 court
// receipts, re-read in full this wave: a07-wave6, a16-wave6). Findings
// repaired here:
//   1. a07-wave6 §5 (coverage hygiene: two gates, two coverage sets) —
//      the §6.2 ledger walk descended into lane build roots (`target*`,
//      e.g. `crates/cgap-bont/target-a06/` and
//      `ontologies/fibo/target-a09/`, both on the wave-7 tree with ZERO
//      .rs files inside — latent, not live). Repair: walk skips `target*`,
//      the real tree is pinned pollution-free in the ledger, and
//      `ledger_walk_build_root_pollution_court` executes the named mutation
//      (skip deleted = `walk_unfiltered`) on a real temp tree EVERY run.
//   2. a16-wave6 D-A16-1 (evidence durability: /tmp flip logs are
//      ephemeral; no waves/ receipt file exists for A19 — that directory is
//      coordinator-owned). Repair: this block IS the durable A19 wave
//      receipt, living in the owned artifact; the lane report carries the
//      exact commands + exits. Method transfers applied:
//      `cargo test --no-fail-fast` (a16-wave6 note: fail-fast hides a red
//      target's results).
// No include/cgap.h change in Wave 7: the header court is green and no
// wave-6 finding implicates the header (diff-minimal repair).
//
// Wave 8 — defect repair driven by the wave-7 court receipt (a08-wave7,
// the only wave-7 receipt on disk; re-read in full this wave). Findings
// repaired here, all in the §6.1 header court (diff-minimal; include/
// cgap.h itself is UNCHANGED this wave):
//   1. a08-wave7 §"Court attack" (flip attribution: the wave-7 named
//      courts M12 and M13 "flip in isolation", i.e. the flip names its own
//      court, and "anchor count asserted == 1 before every write") — the
//      §6.1 mutation corpus asserted detection only, with NEITHER anchor
//      uniqueness NOR flip attribution: a multi-site anchor makes a
//      mutation ambiguous, and a "detected somewhere" result cannot say
//      WHICH court leg flipped, so a collateral-only flip (the named leg
//      dead, detection via an unrelated leg) would have passed silently.
//      Repair: every corpus anchor is asserted UNIQUE (count == 1) in the
//      normalized header before mutation, and every mutation now carries a
//      named detecting leg (4th column of MUTATIONS) that the failure text
//      must name — an off-leg flip fails the court as unattributable.
//   2. Same method transfer, both compile legs: the C11 layout-mutation
//      anchor is pinned count==1, and the C++ rename anchor is scoped to
//      the PROTOTYPE site (`CgapStatus cgap_verify_p28(`) instead of the
//      blanket symbol string — the raw header mentions the symbol twice
//      (file-top comment + prototype), so the old blanket anchor was
//      multi-site and the rename unattributable.
//   3. D-A08-1/D-A16-1 continuity (receipt durability): this block remains
//      the durable A19 receipt; waves/ is coordinator-owned.
// The repair's own non-vacuity is witnessed in the lane report: a wrong-
// leg shim (M4's leg renamed to a non-detecting leg) flips this court with
// the COLLATERAL message, then is restored byte-exact.
// No include/cgap.h change in Wave 8 either: no wave-7 finding implicates
// the header (diff-minimal repair).
// ===========================================================================

#[cfg(test)]
mod wave3_courts {
    use super::{
        CgapAdmitFrameFn, CgapCommitParquetFn, CgapEvalBn3Fn, CgapFrameView,
        CgapStatus, CgapVerifyP28Fn, CGAP_ABI_VERSION, CGAP_ERR_INVALID_MAGIC,
        CGAP_ERR_LEASE_EXHAUSTED, CGAP_ERR_OABI_MISMATCH, CGAP_ERR_P28_SPOLIATION,
        CGAP_ERR_SHACL_VIOLATION, CGAP_OK,
    };
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    // -- Harness ------------------------------------------------------------

    /// Absolute repo root, derived from the crate manifest (never cwd).
    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .expect("repo root canonicalize")
    }

    /// The checked-in header, read fresh from disk every run — the court
    /// always tests the file a consumer would actually compile.
    fn header_source() -> String {
        fs::read_to_string(repo_root().join("include/cgap.h"))
            .expect("include/cgap.h readable from repo root")
    }

    /// Strip /* ... */ comments. The header has no nested or string-literal
    /// comments, so a flat scanner is exact here.
    fn strip_comments(src: &str) -> String {
        let bytes = src.as_bytes();
        let mut out = Vec::with_capacity(bytes.len());
        let mut i = 0;
        while i < bytes.len() {
            if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'*' {
                i += 2;
                while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                    i += 1;
                }
                i = (i + 2).min(bytes.len());
            } else {
                out.push(bytes[i]);
                i += 1;
            }
        }
        String::from_utf8(out).expect("header is UTF-8")
    }

    /// Collapse whitespace runs to single spaces so multi-line prototypes
    /// normalize to one line for parsing.
    fn normalize(text: &str) -> String {
        let mut out = String::with_capacity(text.len());
        let mut in_ws = false;
        for ch in text.chars() {
            if ch.is_whitespace() {
                if !in_ws {
                    out.push(' ');
                }
            in_ws = true;
            } else {
                out.push(ch);
                in_ws = false;
            }
        }
        out
    }

    // -- Parsers: all Result-returning so the mutation corpus observes an
    //    Err (detection), never a panic-as-pass. ----------------------------

    fn parse_version(text: &str) -> Result<u32, String> {
        let needle = "#define CGAP_ABI_VERSION";
        let i = text
            .find(needle)
            .ok_or("CGAP_ABI_VERSION define absent")?;
        let rest = &text[i + needle.len()..];
        let tok = rest
            .split(|c: char| c.is_whitespace() || c == ';')
            .find(|t| !t.is_empty())
            .ok_or("CGAP_ABI_VERSION has no value")?;
        tok.trim_end_matches('u')
            .parse()
            .map_err(|e| format!("CGAP_ABI_VERSION unparsable: {e}"))
    }

    fn parse_enum(text: &str) -> Result<Vec<(String, u32)>, String> {
        let start = text.find("enum {").ok_or("status enum absent")?;
        let body_start = start + "enum {".len();
        let close = text[body_start..]
            .find("};")
            .ok_or("status enum unterminated")?;
        let body = &text[body_start..body_start + close];
        let mut out = Vec::new();
        for member in body.split(',') {
            let member = member.trim();
            if member.is_empty() {
                continue;
            }
            let Some((name, val)) = member.split_once('=') else {
                continue;
            };
            let name = name.trim();
            if name.is_empty() || name.contains(char::is_whitespace) {
                continue;
            }
            let val = val.trim().trim_end_matches('u');
            let v = u32::from_str_radix(val.trim_start_matches("0x"), 16)
                .map_err(|e| format!("enum {name} unparsable: {e}"))?;
            out.push((name.to_string(), v));
        }
        if out.is_empty() {
            return Err("status enum parsed to zero members".into());
        }
        Ok(out)
    }

    fn parse_view_fields(text: &str) -> Result<Vec<(String, String)>, String> {
        let start = text
            .find("typedef struct {")
            .ok_or("cgap_frame_view typedef absent")?;
        let body_start = start + "typedef struct {".len();
        let end = text[body_start..]
            .find("} cgap_frame_view;")
            .ok_or("cgap_frame_view typedef unterminated")?;
        let body = &text[body_start..body_start + end];
        let mut out = Vec::new();
        // Normalized text is one line; split declarations on ';'.
        for decl in body.split(';') {
            let decl = decl.trim();
            if decl.is_empty() || decl.contains('{') || decl.contains('}') {
                continue;
            }
            let Some((ty, name)) = decl.rsplit_once(char::is_whitespace) else {
                continue;
            };
            let (ty, name) = (ty.trim(), name.trim());
            if ty.is_empty() || name.is_empty() || name.contains(char::is_whitespace) {
                continue;
            }
            out.push((name.to_string(), ty.to_string()));
        }
        if out.is_empty() {
            return Err("cgap_frame_view parsed to zero fields".into());
        }
        Ok(out)
    }

    fn parse_prototypes(
        text: &str,
    ) -> Result<BTreeMap<String, (String, Vec<(String, String)>)>, String> {
        let mut out = BTreeMap::new();
        for name in [
            "cgap_admit_frame",
            "cgap_eval_bn3",
            "cgap_commit_parquet",
            "cgap_verify_p28",
        ] {
            let needle = format!("{name}(");
            let i = text.find(&needle).ok_or(format!("prototype {name} absent"))?;
            let open = i + needle.len() - 1; // index of '('
            let close = text[open..]
                .find(')')
                .ok_or(format!("prototype {name} paren mismatch"))?;
            let params = &text[open + 1..open + close];
            // Return type: the nearest preceding `CgapStatus` with only
            // whitespace between it and the name (empty filter match fails).
            text[..i]
                .rfind("CgapStatus")
                .filter(|p| text[p + "CgapStatus".len()..i].trim().is_empty())
                .ok_or(format!("prototype {name}: return not CgapStatus"))?;
            let mut parsed: Vec<(String, String)> = Vec::new();
            for param in params.split(',') {
                let param = param.trim();
                if param.is_empty() {
                    continue;
                }
                let (ty, arg) = param
                    .rsplit_once(char::is_whitespace)
                    .ok_or(format!("prototype {name}: param '{param}' has no name"))?;
                parsed.push((arg.trim().to_string(), ty.trim().to_string()));
            }
            out.insert(name.to_string(), ("CgapStatus".to_string(), parsed));
        }
        Ok(out)
    }

    // -- Rust-side mirror (frozen spec §2; three-way freeze) -----------------
    //
    // The mirror is the literal type text the court demands from the header.
    // Its correctness against the RUST side is pinned by
    // `rust_side_signature_guards_match_aliases`, which coerces fn items
    // carrying the literal types to the public fn-pointer aliases — a
    // mismatch anywhere fails to COMPILE. Chain: aliases <-> guard literals
    // <-> mirror <-> parsed header; any drift flips something.

    const MIRROR_PARAMS: &[(&str, &[&str])] = &[
        (
            "cgap_admit_frame",
            &["const uint8_t* bytes", "size_t len", "cgap_frame_view* out_view"],
        ),
        (
            "cgap_eval_bn3",
            &[
                "const cgap_frame_view* view",
                "const uint8_t* bn3_rodata",
                "size_t bn3_len",
                "int32_t* out_lanes",
                "size_t lanes_cap",
                "size_t* out_lane_count",
            ],
        ),
        (
            "cgap_commit_parquet",
            &["const int32_t* lanes", "size_t lane_count", "const char* path"],
        ),
        (
            "cgap_verify_p28",
            &["const uint8_t* evidence", "size_t len", "uint64_t* out_digest"],
        ),
    ];

    const MIRROR_VIEW: &[(&str, &str)] = &[
        ("payload", "const uint8_t*"),
        ("payload_len", "size_t"),
        ("schema_id", "uint32_t"),
        ("rule_count", "uint32_t"),
    ];

    const MIRROR_ENUM: &[(&str, u32)] = &[
        ("CGAP_OK", CGAP_OK),
        ("CGAP_ERR_INVALID_MAGIC", CGAP_ERR_INVALID_MAGIC),
        ("CGAP_ERR_OABI_MISMATCH", CGAP_ERR_OABI_MISMATCH),
        ("CGAP_ERR_SHACL_VIOLATION", CGAP_ERR_SHACL_VIOLATION),
        ("CGAP_ERR_LEASE_EXHAUSTED", CGAP_ERR_LEASE_EXHAUSTED),
        ("CGAP_ERR_P28_SPOLIATION", CGAP_ERR_P28_SPOLIATION),
    ];

    // -- §6.1 header parity (§6.1 mechanical diff) ---------------------------

    /// Wave-4 leg: the status word typedef must be `uint32_t` verbatim
    /// (normalized) — the C register width is pinned by `_Static_assert` in
    /// the header (C11) and by const-assert in Rust; this leg pins the
    /// DECLARATION text so a silent width change flips the parser court even
    /// where `_Static_assert` is unavailable (C89/C99 consumers).
    fn parse_status_typedef(text: &str) -> Result<(), String> {
        let needle = "typedef uint32_t CgapStatus;";
        if text.contains(needle) {
            return Ok(());
        }
        Err(format!(
            "CgapStatus typedef drift: expected `{needle}` (got none) — width pin violated"
        ))
    }

    fn header_parity(text: &str) -> Result<(), String> {
        let norm = normalize(&strip_comments(text));
        let version = parse_version(&norm)?;
        if version != CGAP_ABI_VERSION {
            return Err(format!("ABI version header={version} rust={CGAP_ABI_VERSION}"));
        }
        parse_status_typedef(&norm)?;
        let got: BTreeMap<String, u32> = parse_enum(&norm)?.into_iter().collect();
        let want: BTreeMap<String, u32> =
            MIRROR_ENUM.iter().map(|(n, v)| (n.to_string(), *v)).collect();
        if got != want {
            return Err(format!("status enum drift: header={got:?} rust={want:?}"));
        }
        let got_view = parse_view_fields(&norm)?;
        let want_view: Vec<(String, String)> =
            MIRROR_VIEW.iter().map(|(n, t)| (n.to_string(), t.to_string())).collect();
        if got_view != want_view {
            return Err(format!("view drift: header={got_view:?} rust={want_view:?}"));
        }
        let protos = parse_prototypes(&norm)?;
        for (name, want_params) in MIRROR_PARAMS {
            let Some((ret, params)) = protos.get(*name) else {
                return Err(format!("prototype {name} absent"));
            };
            if ret != "CgapStatus" {
                return Err(format!("{name}: return {ret} != CgapStatus"));
            }
            let want: Vec<(String, String)> = want_params
                .iter()
                .map(|p| {
                    let (t, a) = p
                        .rsplit_once(char::is_whitespace)
                        .expect("mirror param shape");
                    (a.to_string(), t.to_string())
                })
                .collect();
            if params != &want {
                return Err(format!("{name} params: header={params:?} rust={want:?}"));
            }
        }
        Ok(())
    }

    // -- Anti-vacuity corpus (§6.1: a court that cannot flip is FAIL).
    // Each mutation is applied to a COPY of the normalized header text — the
    // checked-in file is never modified — and MUST be detected ON ITS NAMED
    // LEG (4th column: the court leg whose error text must appear; a08-wave7
    // method — the wave-7 named courts flip IN ISOLATION, and every shim
    // anchor is asserted count==1 before write). An undetected mutation
    // fails the court as a VACUOUS GATE; a flip on a collateral leg fails it
    // as unattributable.
    const MUTATIONS: &[(&str, &str, &str, &str)] = &[
        ("M1 param-type flip (§6.1 literal)", "const uint8_t* bytes", "const uint32_t* bytes", "cgap_admit_frame params"),
        ("M2 param drop (admit len)", "bytes, size_t len,", "bytes,", "cgap_admit_frame params"),
        ("M3 return-type change (commit)", "CgapStatus cgap_commit_parquet", "int cgap_commit_parquet", "cgap_commit_parquet: return"),
        ("M4 status value flip (P28)", "0x503238FFu", "0x503238FEu", "status enum drift"),
        ("M5 ABI version bump", "#define CGAP_ABI_VERSION 1u", "#define CGAP_ABI_VERSION 2u", "ABI version"),
        ("M6 out-param const flip (doc divergence note)", "cgap_frame_view* out_view", "const cgap_frame_view* out_view", "cgap_admit_frame params"),
        ("M7 view field type (layout pin)", "size_t payload_len;", "uint32_t payload_len;", "view drift"),
        ("M8 view field drop (rule_count)", "uint32_t rule_count;", "", "view drift"),
        ("M9 symbol rename (verify)", "cgap_verify_p28", "cgap_verify_p2x", "prototype cgap_verify_p28 absent"),
        ("M10 enum member rename (OK)", "CGAP_OK =", "CGAP_SUCCESS =", "status enum drift"),
        // Wave-4 additions:
        ("M11 status typedef width flip", "typedef uint32_t CgapStatus;", "typedef uint16_t CgapStatus;", "CgapStatus typedef drift"),
        ("M12 enum member drop (INVALID_MAGIC)", "CGAP_ERR_INVALID_MAGIC =", "", "status enum drift"),
    ];

    /// Fold through a view's payload IN PLACE — the house rotate-xor word
    /// fold (`crate::fold_word`, the exact fold `cgap_digest_of` serves and
    /// the `digest_ffi` court parity-pins to `cgap_arrow::digest`). LE word
    /// order, odd tail zero-extended: platform-pinned, replayable.
    fn fold_view(view: &CgapFrameView) -> u64 {
        assert!(!view.payload.is_null(), "fold_view: null payload");
        let bytes =
            unsafe { core::slice::from_raw_parts(view.payload, view.payload_len) };
        let mut acc: u64 = 0;
        let mut i = 0;
        while i + 8 <= bytes.len() {
            let mut w = [0u8; 8];
            w.copy_from_slice(&bytes[i..i + 8]);
            acc = crate::fold_word(acc, u64::from_le_bytes(w));
            i += 8;
        }
        if i < bytes.len() {
            let mut tail = [0u8; 8];
            tail[..bytes.len() - i].copy_from_slice(&bytes[i..]);
            acc = crate::fold_word(acc, u64::from_le_bytes(tail));
        }
        acc
    }

    fn cc_available() -> bool {
        Command::new("cc")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    // -- §6.1 header court ---------------------------------------------------

    #[test]
    fn rust_side_signature_guards_match_aliases() {
        // Fn items carrying the literal frozen types must coerce to the
        // public fn-pointer aliases: any drift in alias or guard fails to
        // COMPILE. Chain leg: aliases <-> guard literals (the mirror table
        // <-> header leg is the parity test below).
        unsafe extern "C" fn sig_admit(_: *const u8, _: usize, _: *mut CgapFrameView) -> CgapStatus { 0 }
        unsafe extern "C" fn sig_eval(
            _: *const CgapFrameView,
            _: *const u8,
            _: usize,
            _: *mut i32,
            _: usize,
            _: *mut usize,
        ) -> CgapStatus { 0 }
        unsafe extern "C" fn sig_commit(_: *const i32, _: usize, _: *const core::ffi::c_char) -> CgapStatus { 0 }
        unsafe extern "C" fn sig_verify(_: *const u8, _: usize, _: *mut u64) -> CgapStatus { 0 }
        let _ = sig_admit as CgapAdmitFrameFn;
        let _ = sig_eval as CgapEvalBn3Fn;
        let _ = sig_commit as CgapCommitParquetFn;
        let _ = sig_verify as CgapVerifyP28Fn;
    }

    #[test]
    fn header_court_parity_green_on_checked_in_header() {
        let text = header_source();
        header_parity(&text).unwrap_or_else(|e| {
            panic!("checked-in include/cgap.h drifted from the Rust ABI freeze: {e}")
        });
    }

    #[test]
    fn header_court_mutation_corpus_all_detected() {
        let norm = normalize(&strip_comments(&header_source()));
        for (label, old, new, leg) in MUTATIONS {
            // a08-wave7 shim discipline: the anchor is asserted UNIQUE
            // (count == 1) before the mutation is applied — a multi-site
            // anchor makes the mutation ambiguous and its flip
            // unattributable.
            let hits = norm.matches(old).count();
            assert_eq!(
                hits, 1,
                "corpus anchor not unique (count={hits}): {label} — mutation ambiguous, flip unattributable (a08-wave7 count==1 shim discipline)"
            );
            let mutated = norm.replacen(old, new, 1);
            assert_ne!(mutated, norm, "{label}: mutation did not change the text");
            match header_parity(&mutated) {
                Err(e) => assert!(
                    e.contains(leg),
                    "mutation '{label}' flipped on a COLLATERAL leg (unattributable): expected the error to name {leg:?}, got: {e}"
                ),
                Ok(()) => panic!(
                    "VACUOUS GATE: mutation '{label}' NOT detected — the header court cannot flip"
                ),
            }
            println!("DETECTED  {label} -> {leg} (isolated flip)");
        }
        println!(
            "HEADER MUTATION CORPUS: {} mutations, every anchor unique, every flip on its named leg (a08-wave7 method)",
            MUTATIONS.len()
        );
    }

    #[test]
    fn header_court_c11_compile_real_and_pins_fire_on_mutation() {
        if !cc_available() {
            println!("[SKIP] cc absent — C11 compile leg unexecuted (parser legs still ran)");
            return;
        }
        let dir =
            std::env::temp_dir().join(format!("cgap-header-court-{}", std::process::id()));
        fs::create_dir_all(&dir).expect("temp dir");
        // Explicit-typed fn pointers: if a header prototype drifts, these
        // lines fail to type-match and the TU refuses to compile — a C-side
        // cross-check independent of the Rust-side parser.
        let tu = r#"#include "cgap.h"
static CgapStatus (*const p_admit)(const uint8_t*, size_t, cgap_frame_view*) = cgap_admit_frame;
static CgapStatus (*const p_eval)(const cgap_frame_view*, const uint8_t*, size_t, int32_t*, size_t, size_t*) = cgap_eval_bn3;
static CgapStatus (*const p_commit)(const int32_t*, size_t, const char*) = cgap_commit_parquet;
static CgapStatus (*const p_verify)(const uint8_t*, size_t, uint64_t*) = cgap_verify_p28;
int main(void) {
  cgap_frame_view v;
  v.payload = 0; v.payload_len = 0; v.schema_id = 0; v.rule_count = 0;
  return (int)(CGAP_OK + (CgapStatus)CGAP_ABI_VERSION) + (int)sizeof(cgap_frame_view)
       + ((p_admit && p_eval && p_commit && p_verify) ? 0 : 1);
}
"#;
        fs::write(dir.join("tu.c"), tu).expect("write TU");
        let run = |inc: &Path, out: &str| {
            Command::new("cc")
                .arg("-std=c11")
                .arg("-Wall")
                .arg("-I")
                .arg(inc)
                .arg(dir.join("tu.c"))
                .arg("-c")
                .arg("-o")
                .arg(dir.join(out))
                .output()
                .expect("spawn cc")
        };
        let ok = run(&repo_root().join("include"), "real.o");
        assert!(
            ok.status.success(),
            "checked-in cgap.h must compile standalone C11:\n{}",
            String::from_utf8_lossy(&ok.stderr)
        );
        let mut_inc = dir.join("mutated-include");
        fs::create_dir_all(&mut_inc).expect("mutated include dir");
        let real = header_source();
        // a08-wave7 count==1 shim discipline: assert the anchor is unique in
        // the raw header before mutating.
        let anchor = "size_t payload_len;";
        assert_eq!(
            real.matches(anchor).count(),
            1,
            "C11 layout-mutation anchor not unique — compile leg vacuous"
        );
        let mutated = real.replacen(anchor, "uint32_t payload_len;", 1);
        assert_ne!(mutated, real, "mutation anchor absent — compile leg vacuous");
        fs::write(mut_inc.join("cgap.h"), mutated).expect("write mutated header");
        let bad = run(&mut_inc, "mutated.o");
        assert!(
            !bad.status.success(),
            "VACUOUS GATE: mutated header compiled — the C layout pins never fire"
        );
        let err = String::from_utf8_lossy(&bad.stderr);
        assert!(
            err.contains("cgap ABI freeze"),
            "compile failed but NOT on the layout pin:\n{err}"
        );
        println!("C11 COMPILE COURT: real header compiles; mutated header fails ON the _Static_assert pin");
        let _ = fs::remove_dir_all(&dir);
    }

    // -- Wave-4 hardening: the extern "C" C++ branch of cgap.h is part of the
    //    frozen surface (header doc: "C++ consumers get the same declarations
    //    without the pins") but was unwitnessed by the Wave-3 court. The leg
    //    compiles the SAME TU as C++ against the real header (MUST succeed —
    //    fn-pointer pins type-check, no _Static_asserts fire) and against the
    //    M9-mutated header (MUST FAIL — the rename must flip the C++ leg too,
    //    otherwise the C++ branch is a gate that cannot flip = VACUOUS).
    #[test]
    fn header_court_cpp_compile_leg_real_passes_and_rename_mutant_fails() {
        if !cc_available() {
            println!("[SKIP] cc absent — C++ compile leg unexecuted (parser legs still ran)");
            return;
        }
        let dir =
            std::env::temp_dir().join(format!("cgap-header-court-cpp-{}", std::process::id()));
        fs::create_dir_all(&dir).expect("temp dir");
        let tu = r#"#include "cgap.h"
static CgapStatus (*const p_admit)(const uint8_t*, size_t, cgap_frame_view*) = cgap_admit_frame;
static CgapStatus (*const p_eval)(const cgap_frame_view*, const uint8_t*, size_t, int32_t*, size_t, size_t*) = cgap_eval_bn3;
static CgapStatus (*const p_commit)(const int32_t*, size_t, const char*) = cgap_commit_parquet;
static CgapStatus (*const p_verify)(const uint8_t*, size_t, uint64_t*) = cgap_verify_p28;
int main() {
  cgap_frame_view v{};
  return (int)(CGAP_OK + (CgapStatus)CGAP_ABI_VERSION) + (int)sizeof(cgap_frame_view)
       + ((p_admit && p_eval && p_commit && p_verify) ? 0 : 1);
}
"#;
        fs::write(dir.join("tu.cpp"), tu).expect("write TU");
        let run = |inc: &Path, out: &str| {
            Command::new("cc")
                .arg("-x")
                .arg("c++")
                .arg("-std=c++17")
                .arg("-Wall")
                .arg("-I")
                .arg(inc)
                .arg(dir.join("tu.cpp"))
                .arg("-c")
                .arg("-o")
                .arg(dir.join(out))
                .output()
                .expect("spawn cc")
        };
        let ok = run(&repo_root().join("include"), "real_cpp.o");
        assert!(
            ok.status.success(),
            "checked-in cgap.h must compile as C++17 (extern \"C\" branch):\n{}",
            String::from_utf8_lossy(&ok.stderr)
        );
        let mut_inc = dir.join("mutated-include");
        fs::create_dir_all(&mut_inc).expect("mutated include dir");
        let real = header_source();
        // a08-wave7 count==1 shim discipline + precise anchor: rename ONLY
        // the prototype site. The raw header mentions the symbol TWICE
        // (file-top comment + prototype), so a blanket-symbol anchor is
        // multi-site and the rename unattributable; the prototype-scoped
        // anchor is count==1.
        let anchor = "CgapStatus cgap_verify_p28(";
        assert_eq!(
            real.matches(anchor).count(),
            1,
            "C++ rename anchor not unique (must be the prototype site only; the file-top comment mention must NOT be renamed) — compile leg vacuous"
        );
        let mutated = real.replacen(anchor, "CgapStatus cgap_verify_p2x(", 1);
        assert_ne!(mutated, real, "rename anchor absent — C++ leg vacuous");
        fs::write(mut_inc.with_file_name("cgap.h"), mutated).expect("write mutated header");
        let bad = run(&mut_inc, "mutated_cpp.o");
        assert!(
            !bad.status.success(),
            "VACUOUS GATE: renamed-symbol header compiled as C++ — the C++ leg cannot flip"
        );
        println!("C++ COMPILE COURT: real header compiles as C++17; symbol-rename mutant fails");
        let _ = fs::remove_dir_all(&dir);
    }

    // -- Ledger walk (module-level so the Wave-7 pollution court can arm its
    //    named mutation as a second arm) --------------------------------------

    /// Bounded recursive walk of workspace `.rs` sources. Wave-7: directory
    /// names starting with `target` are NEVER descended (lane build roots are
    /// leases, not workspace source; see the §6.2 ledger comment).
    fn walk(dir: &Path, depth: usize, files: &mut Vec<PathBuf>) {
        walk_impl(dir, depth, files, true);
    }

    /// The pollution court's NAMED MUTATION arm: the Wave-7 `target*` skip
    /// deleted. It exists so the court can prove the skip is load-bearing
    /// every run (same pattern as the header court's M-corpus: the mutation
    /// is executed, and an undetected mutation fails the court).
    fn walk_unfiltered(dir: &Path, depth: usize, files: &mut Vec<PathBuf>) {
        walk_impl(dir, depth, files, false);
    }

    fn walk_impl(dir: &Path, depth: usize, files: &mut Vec<PathBuf>, skip_build_roots: bool) {
        if depth > 6 {
            return;
        }
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                let is_build_root = skip_build_roots
                    && p.file_name().map_or(false, |n| n.to_string_lossy().starts_with("target"));
                if !is_build_root {
                    walk_impl(&p, depth + 1, files, skip_build_roots);
                }
            } else if p.extension().map(|x| x == "rs").unwrap_or(false) {
                files.push(p);
            }
        }
    }

    // -- §6.2 status court ---------------------------------------------------

    #[test]
    fn status_court_register_hygiene_and_header_value_parity() {
        let vals = [
            CGAP_OK,
            CGAP_ERR_INVALID_MAGIC,
            CGAP_ERR_OABI_MISMATCH,
            CGAP_ERR_SHACL_VIOLATION,
            CGAP_ERR_LEASE_EXHAUSTED,
            CGAP_ERR_P28_SPOLIATION,
        ];
        for i in 0..vals.len() {
            for j in (i + 1)..vals.len() {
                assert_ne!(vals[i], vals[j], "register values must be pairwise distinct");
            }
        }
        // Exact pins (§2).
        assert_eq!(CGAP_OK, 0x0000_0000);
        assert_eq!(CGAP_ERR_INVALID_MAGIC, 0x4347_0001);
        assert_eq!(CGAP_ERR_OABI_MISMATCH, 0x4347_0002);
        assert_eq!(CGAP_ERR_SHACL_VIOLATION, 0x4347_0003);
        assert_eq!(CGAP_ERR_LEASE_EXHAUSTED, 0x4347_0004);
        assert_eq!(CGAP_ERR_P28_SPOLIATION, 0x5032_38FF);
        // Value classes: 'CG' (0x4347) + nonzero nn; 'P28' (0x503238) + FF.
        for v in [
            CGAP_ERR_INVALID_MAGIC,
            CGAP_ERR_OABI_MISMATCH,
            CGAP_ERR_SHACL_VIOLATION,
            CGAP_ERR_LEASE_EXHAUSTED,
        ] {
            assert_eq!(v & 0xFFFF_0000, 0x4347_0000, "'CG' class");
            assert_ne!(v & 0x0000_FFFF, 0, "'CG' sequence nonzero");
        }
        assert_eq!(CGAP_ERR_P28_SPOLIATION & 0xFFFF_FF00, 0x5032_3800, "'P28' class");
        assert_eq!(CGAP_ERR_P28_SPOLIATION & 0xFF, 0xFF, "'P28' tail FF");
        // Header enum parity, asserted independently of the §6.1 court so
        // the status court never depends on another test's passage.
        let norm = normalize(&strip_comments(&header_source()));
        let got: BTreeMap<String, u32> =
            parse_enum(&norm).expect("header enum parse").into_iter().collect();
        for (name, v) in MIRROR_ENUM {
            assert_eq!(got.get(*name), Some(v), "header enum value for {name}");
        }
        assert_eq!(got.len(), 6, "header enum pins exactly the six register values");
    }

    #[test]
    fn status_court_reachability_ledger() {
        let names = [
            "CGAP_ERR_INVALID_MAGIC",
            "CGAP_ERR_OABI_MISMATCH",
            "CGAP_ERR_SHACL_VIOLATION",
            "CGAP_ERR_LEASE_EXHAUSTED",
            "CGAP_ERR_P28_SPOLIATION",
        ];
        // Bounded walk of ALL workspace-member .rs sources (real on-disk
        // files): `crates/` AND `ontologies/` — fibo/cloud are workspace
        // members (root Cargo.toml), so a producer landing in ontologies/ is
        // as landed as one in crates/. Wave-6 repair: the walk previously
        // covered `crates/` only, so a producer landing in ontologies/ would
        // have been SILENTLY INVISIBLE — the pinned-skip guard below could
        // not flip on a state it cannot see. Driven by the Wave-5 audit
        // method (a20-wave5 receipt divergence 3: sites outside the scan are
        // uncounted, not nonexistent). Wave-7 repair (a07-wave6 §5: two
        // gates, two coverage sets): lane build roots — directory names
        // starting with `target`, e.g. `crates/cgap-bont/target-a06/` and
        // `ontologies/fibo/target-a09/`, both present on the wave-7 tree —
        // are NEVER descended: build roots are leases, not workspace source
        // (same-checkout-fanout cleanup law), so build-artifact text can
        // never count as a workspace production. The skip is proven
        // load-bearing by `ledger_walk_build_root_pollution_court` below.
        let mut files: Vec<PathBuf> = Vec::new();
        for root in ["crates", "ontologies"] {
            walk(&repo_root().join(root), 0, &mut files);
        }
        assert!(
            files.len() > 50,
            "workspace walk found {} .rs files — the walk is broken, ledger vacuous",
            files.len()
        );
        // Wave-7 hygiene pin on the REAL tree: the ledger walk is workspace
        // source only. If a lane build root ever contains .rs files, the
        // ledger refuses loudly instead of silently counting pollution (the
        // same silent-invisibility class a07-wave6 §5 flagged for gates with
        // mismatched coverage sets).
        let polluted: Vec<String> = files
            .iter()
            .filter(|f| {
                f.components()
                    .any(|c| c.as_os_str().to_string_lossy().starts_with("target"))
            })
            .map(|f| f.display().to_string())
            .collect();
        assert!(
            polluted.is_empty(),
            "ledger walk found .rs files inside lane build roots (target*): {polluted:?} — build roots are leases, not workspace source (same-checkout-fanout cleanup law); delete or exclude them, never count them"
        );
        // Wave-6 repair pin — the anti-vacuity of the widening itself: the
        // named ontologies member source must be present in the walked set,
        // so narrowing the walk scope back to crates/-only flips THIS assert
        // (flip witnessed in the /tmp court sandbox this wave, receipt in the
        // lane report). Exact-path membership, not a suffix match, so a stray
        // nested copy cannot satisfy the pin.
        let ontologies_member = repo_root().join("ontologies/cloud/src/lib.rs");
        assert!(
            files.iter().any(|f| f == &ontologies_member),
            "ledger walk lost ontologies/cloud/src/lib.rs — walk scope regressed below the Wave-6 repair (workspace members are crates/ + ontologies/)"
        );
        let self_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/sovereign.rs");

        // Producer = a `return CGAP_ERR_<NAME>;` site outside this file
        // (definitions are not productions).
        let mut sites: BTreeMap<&str, Vec<String>> = BTreeMap::new();
        for f in &files {
            if f == &self_path {
                continue;
            }
            let Ok(text) = fs::read_to_string(f) else { continue };
            for n in names {
                let c = text.matches(&format!("return {n}")).count();
                if c > 0 {
                    sites.entry(n).or_default().push(format!(
                        "{} ({c} site{})",
                        f.display(),
                        if c == 1 { "" } else { "s" }
                    ));
                }
            }
        }

        // Landed pins: the A14 penstock produces three of the five fault
        // values; the ledger cites its on-disk sites mechanically.
        let penstock = files
            .iter()
            .find(|f| f.ends_with("cgap-gate/src/penstock.rs"))
            .expect("cgap-gate/src/penstock.rs in workspace");
        let ptext = fs::read_to_string(penstock).expect("penstock.rs readable");
        for n in [
            "CGAP_ERR_INVALID_MAGIC",
            "CGAP_ERR_OABI_MISMATCH",
            "CGAP_ERR_P28_SPOLIATION",
        ] {
            let s = sites
                .get(n)
                .unwrap_or_else(|| panic!("ledger: {n} has NO landed producer"));
            assert!(s.iter().any(|x| x.contains("penstock.rs")), "{n} must cite penstock.rs");
            println!("LANDED  {n}: {}", s.join(", "));
        }
        // Cross-lane frozen-copy parity (Wave-1 note): penstock pins the
        // register by value, this module is canonical.
        for hex in ["0x0000_0000", "0x4347_0001", "0x4347_0002", "0x5032_38FF"] {
            assert!(ptext.contains(hex), "penstock register copy {hex} missing");
        }
        assert!(ptext.contains("CGAP_OK"), "penstock success value absent");
        println!("LANDED  CGAP_OK: success value of the landed producers (cgap-gate::ingress success tail)");

        // Loud skips for the unlanded, PINNED as current reality: if an
        // owner lane lands a producer mid-wave, this assert fires and the
        // pin + receipt must be updated — progress must not pass silently.
        let mut skipped: Vec<&str> = Vec::new();
        for (n, owner) in [
            ("CGAP_ERR_SHACL_VIOLATION", "A14+A16"),
            ("CGAP_ERR_LEASE_EXHAUSTED", "A02+A03"),
        ] {
            if sites.get(n).map_or(true, |v| v.is_empty()) {
                println!(
                    "[SKIP] {n}: no landed producer in crates/ (owners {owner}) — reachability UNVERIFIED this wave"
                );
                skipped.push(n);
            }
        }
        assert_eq!(
            skipped,
            ["CGAP_ERR_SHACL_VIOLATION", "CGAP_ERR_LEASE_EXHAUSTED"],
            "reachability reality moved — update the Wave-3 pin + lane receipt"
        );
        println!(
            "STATUS COURT: 3/5 fault values fault-reachable on landed producers (cgap-gate::ingress); 2 loud [SKIP] (owners cited); CGAP_OK success path landed"
        );
    }

    // -- Wave-7: the walk's `target*` skip is load-bearing. Named mutation =
    //    the skip deleted, armed as `walk_unfiltered` and executed EVERY run
    //    (a16-wave6 M-corpus pattern: the mutation is executed in-court, and
    //    an undetected mutation fails the court as VACUOUS GATE).

    #[test]
    fn ledger_walk_build_root_pollution_court() {
        // Real temp tree (Chicago: real files, real walk, no mocks):
        //   <root>/crates/m/src/real.rs              — real workspace source
        //   <root>/crates/m/target-lane7/artifact.rs — build-root pollution
        let dir = std::env::temp_dir().join(format!("cgap-walk-court-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let src = dir.join("crates/m/src");
        let bld = dir.join("crates/m/target-lane7");
        fs::create_dir_all(&src).expect("temp src dir");
        fs::create_dir_all(&bld).expect("temp build dir");
        fs::write(src.join("real.rs"), "fn real() {}").expect("write real source");
        fs::write(bld.join("artifact.rs"), "fn artifact() {}").expect("write build artifact");

        let has = |files: &[PathBuf], name: &str| {
            files
                .iter()
                .any(|f| f.file_name().map_or(false, |n| n.to_string_lossy() == name))
        };
        let mut filtered = Vec::new();
        let mut unfiltered = Vec::new();
        walk(&dir, 0, &mut filtered);
        walk_unfiltered(&dir, 0, &mut unfiltered);

        // Premises: both arms really walk (the real source is found by both)
        // and the pollution is real (an unfiltered walk sees it — otherwise
        // this court could never detect a broken skip and would be vacuous).
        assert!(
            has(&filtered, "real.rs") && has(&unfiltered, "real.rs"),
            "walk broken: real source not visited by both arms — court vacuous"
        );
        assert!(
            has(&unfiltered, "artifact.rs"),
            "NEGATIVE CONTROL FAILED: the unfiltered (mutation) arm cannot see the planted build-root source — the court could never detect a deleted skip (vacuous)"
        );
        // The repair: the filtered walk must not count build-root pollution.
        // This is the leg that FLIPS if the Wave-7 skip is deleted or broken.
        assert!(
            !has(&filtered, "artifact.rs"),
            "VACUOUS GATE / repair reverted: the filtered walk counted build-root pollution — the target* skip is gone (named mutation FIRED and was detected, court red as designed)"
        );
        println!(
            "WALK POLLUTION COURT: filtered walk excludes target* build roots; unfiltered (mutation) arm sees the planted artifact — skip proven load-bearing"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    // -- §6.4 zero-copy witness ----------------------------------------------

    #[test]
    fn zero_copy_witness_view_borrows_caller_buffer() {
        // §6.4: >=1 MB host frame; the view must BORROW the caller's bytes.
        const LEN: usize = 1024 * 1024 + 7; // odd tail word included
        let mut buf = vec![0u8; LEN];
        let mut seed: u64 = 0x2463_4100_A19E_0001;
        for chunk in buf.chunks_mut(8) {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            let bytes = seed.to_le_bytes();
            let take = chunk.len();
            chunk.copy_from_slice(&bytes[..take]);
        }
        let view = CgapFrameView {
            payload: buf.as_ptr(),
            payload_len: LEN,
            schema_id: 1,
            rule_count: 1,
        };
        assert_eq!(
            view.payload, buf.as_ptr(),
            "view.payload must alias the caller buffer (zero-copy borrow, §2)"
        );
        assert_eq!(view.payload_len, LEN);
        // Negative control: an independent copy observed through its own
        // view must NOT move when the caller buffer mutates — proving any
        // change through `view` comes from the borrow, not fold drift.
        let snapshot = buf.clone();
        let view_copy = CgapFrameView {
            payload: snapshot.as_ptr(),
            payload_len: LEN,
            schema_id: 1,
            rule_count: 1,
        };

        let d1 = fold_view(&view);
        let d1c = fold_view(&view_copy);
        assert_eq!(d1, d1c, "fold must be deterministic over equal bytes");
        assert_ne!(d1, 0, "1 MiB+7 pseudorandom bytes folded to 0 — fold broken");
        for site in [LEN / 2, 0, LEN - 1] {
            // SAFETY: site < LEN, single-threaded test-local buffer;
            // volatile defeats elision of the mutation.
            let orig = unsafe { core::ptr::read_volatile(buf.as_ptr().add(site)) };
            unsafe { core::ptr::write_volatile(buf.as_mut_ptr().add(site), !orig) };
            let d2 = fold_view(&view);
            assert_ne!(
                d2, d1,
                "site {site}: one-byte flip must change the fold through the view"
            );
            assert_eq!(
                fold_view(&view_copy),
                d1c,
                "site {site}: snapshot fold moved — change did not come via the borrow"
            );
            unsafe { core::ptr::write_volatile(buf.as_mut_ptr().add(site), orig) };
            assert_eq!(
                fold_view(&view),
                d1,
                "site {site}: restore must return the original fold"
            );
        }
        println!(
            "ZERO-COPY WITNESS: {LEN} B frame; 3 mutate/restore sites through the view; snapshot negative control held"
        );
    }
}

// ===========================================================================
// Wave 9 — cross-lane integration slice. Dispatch: "Wave 9 - cross-lane
// integration: A04 identity court, A14 dam over A04 frames, A20 1M-row
// stress, A05 power-cut sweep, A07 zero-string gate over all built crates."
//
// A19's seat is the C-ABI membrane, so this wave's contribution is NOT new
// logic in those lanes (their owner lanes landed it: A04 identity court =
// crates/cgap-core/tests/identity_court.rs; A14 dam over A04 frames =
// crates/cgap-core/tests/cross_lane_gate_court.rs; A20 verdict suite =
// crates/cgap-court [[test]] court_verdict; A05 sweep =
// crates/cgap-storage/tests/court_sweep.rs; A07 gate = scripts/
// zero_string_gate.py). It is the boundary's own witness: this module
// EXECUTES the five surfaces as real subprocesses on the real tree, this
// session, every run — other lanes' receipts are hearsay here
// (receipt-cited counts are stale by default; re-read at use time).
//
// IN-CODE WAVE-9 RECEIPT (durable receipt lives in the owned artifact; the
// waves/ directory is coordinator-owned, per the Wave-7 continuity note).
// Session receipts, this wave (commands + exits re-run this session):
//   - A04: `cargo test -p cgap-core --test identity_court` -> exit 0,
//     "4 passed; 0 failed" (courts also re-witnessed through the nested
//     court, below).
//   - A14: `cargo test -p cgap-core --test cross_lane_gate_court` -> exit 0,
//     "8 passed; 0 failed".
//   - A05: `cargo test -p cgap-storage --test court_sweep` -> exit 0,
//     "2 passed; 0 failed" in 4.33s.
//   - A07: `python3 scripts/zero_string_gate.py` (default scan = ALL built
//     crates) -> exit 0, "SUMMARY scanned=45 crates=22 hits=0 emitter_hits=2
//     adv_hits=0" (scanned grew to 49 by end of wave as lanes landed —
//     floors are 10/20, both far under); `--self-check` -> exit 0,
//     "SELF-CHECK RESULT PASS", poison fired=True. Both also re-executed
//     inside the A07 court every run.
//   - A20: `cargo test -p cgap-court --test court_verdict` -> exit 0,
//     12/12, three consecutive runs at end of wave. TWO live transients
//     were witnessed and are recorded, not rounded over:
//     (1) a shared-crate compile freeze: cgap-bn3 (arena.rs:632,685,697-
//     699 referenced `BN3_STATEMENT_SIZE` without importing it; sigil.rs:81
//     defines it) was red 08:25-08:31 while the bn3 seam lanes (A11+A12+A13)
//     were actively editing (arena.rs mtime 08:26:18 -> fix landed 08:31:53,
//     exit 0 re-witnessed) — the freeze resolved INSIDE the SLA window by
//     its owner; this lane applied NO unblock edit to bn3.
//     (2) one red witness of `a05_interleave_engine` (court_verdict.rs:1044
//     assert, "11 passed; 1 failed") against the A20 owner's MID-EDIT tree
//     (tests/court_verdict.rs mtime 08:34:41, sampled during the edit);
//     the settled tree is 12/12 x3 (the edit added a 12th court). The court
//     asserting the tree's REAL run-time state is the court working — a
//     mid-edit tree fails loudly, never silently.
//
// Cross-lane divergence disclosures: NONE this wave. No file outside the
// lane's two owned files was written (bn3 self-repaired inside the SLA
// window; the A20 edit belongs to its owner lane).
//
// Court-wide discipline (lane contract, unchanged): real collaborators (the
// real suites on the real tree, real exits, real output), assert final
// state, no mocks. Anti-vacuity: every leg must be able to FLIP — a suite
// that executes ZERO tests is the compile-abort class and is refused by the
// `require_summary` floor (negative control executed every run in
// `wave9_result_parser_negative_control`), a red suite fails its court by
// exit, the A07 court additionally requires the gate's own poison/clean
// self-check to pass (a gate that cannot detect poison cannot vouch for
// hits=0), and every nested build root is a LEASE deleted by the last
// leaseholder (cleanup law; deletion asserted in the negative-control
// court on a real scratch root).
//
// No include/cgap.h change in Wave 9: the header court is green and no
// wave-8/wave-9 finding implicates the header (diff-minimal, waves 7/8
// method).
// ===========================================================================

#[cfg(test)]
mod wave9_courts {
    use std::path::{Path, PathBuf};
    use std::process::{Command, Stdio};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Mutex, OnceLock};
    use std::time::{Duration, Instant};

    /// Absolute repo root, derived from the crate manifest (never cwd).
    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .expect("repo root canonicalize")
    }

    // -- Bounded subprocess runner ------------------------------------------
    //
    // No-sleep law: the wait below is a BOUNDED POLL on a real observable
    // (child exit status) with a deadline; the poll step lives only inside
    // this one audited poll implementation (SLEEP-CLASS: POLL-STEP) and a
    // deadline breach kills the child and fails the court — time never
    // substitutes for the observation.

    /// Poll step of the audited bounded poll (never a coordination sleep).
    const POLL_STEP: Duration = Duration::from_millis(250);
    /// Cargo nested-suite deadline. Generous on purpose (first run builds
    /// the whole closure in a fresh lease), still bounded and enforced.
    const CARGO_DEADLINE: Duration = Duration::from_secs(900);
    /// Python gate deadline (the scan is strings(1)-bound, seconds-class).
    const GATE_DEADLINE: Duration = Duration::from_secs(300);

    struct Outcome {
        status: Option<std::process::ExitStatus>,
        stdout: String,
        stderr: String,
    }

    fn run_bounded(
        program: &str,
        args: &[&str],
        envs: &[(&str, String)],
        deadline: Duration,
    ) -> Result<Outcome, String> {
        let mut cmd = Command::new(program);
        cmd.args(args);
        for (k, v) in envs {
            cmd.env(k, v);
        }
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = cmd
            .spawn()
            .map_err(|e| format!("spawn {program} {args:?}: {e}"))?;
        let out_pipe = child.stdout.take().expect("stdout piped");
        let err_pipe = child.stderr.take().expect("stderr piped");
        let read_out = std::thread::spawn(move || {
            let mut s = String::new();
            let _ = std::io::Read::read_to_string(&mut { out_pipe }, &mut s);
            s
        });
        let read_err = std::thread::spawn(move || {
            let mut s = String::new();
            let _ = std::io::Read::read_to_string(&mut { err_pipe }, &mut s);
            s
        });
        let started = Instant::now();
        let mut timed_out = false;
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break Some(status),
                Ok(None) => {
                    if started.elapsed() >= deadline {
                        timed_out = true;
                        let _ = child.kill();
                        let _ = child.wait();
                        break None;
                    }
                    std::thread::sleep(POLL_STEP); // SLEEP-CLASS: POLL-STEP (audited bounded poll)
                }
                Err(e) => return Err(format!("try_wait {program}: {e}")),
            }
        };
        let stdout = read_out.join().expect("stdout reader thread");
        let stderr = read_err.join().expect("stderr reader thread");
        Ok(Outcome {
            status: if timed_out { None } else { status },
            stdout,
            stderr,
        })
    }

    fn tail(text: &str) -> String {
        let len = text.chars().count();
        if len <= 2000 {
            text.to_string()
        } else {
            text.chars().skip(len - 2000).collect()
        }
    }

    // -- Nested cargo plumbing ----------------------------------------------
    //
    // The outer `cargo test` holds the PRIMARY target dir for the whole
    // invocation; a nested cargo on the same dir would deadlock against its
    // own parent. Every nested suite therefore builds into a fresh LEASE
    // root (same-checkout-fanout: build roots are leases, not assets) —
    // shared per test-process, deleted by the LAST leaseholder out.

    fn lease_root() -> &'static PathBuf {
        static ROOT: OnceLock<PathBuf> = OnceLock::new();
        ROOT.get_or_init(|| {
            let r = std::env::temp_dir().join(format!(
                "cgap-a19-w9-nested-target-{}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&r); // stale lease from a crashed prior run
            std::fs::create_dir_all(&r).expect("create nested build-root lease");
            r
        })
    }

    /// Open lease count for the shared nested build root.
    fn open_leases() -> &'static AtomicUsize {
        static OPEN: OnceLock<AtomicUsize> = OnceLock::new();
        OPEN.get_or_init(|| AtomicUsize::new(0))
    }

    /// The cleanup law, factored so BOTH legs are executable (a law only
    /// its happy path can express cannot be witnessed non-vacuously): the
    /// LAST holder out (`holders_after_release == 0`) deletes the
    /// build-root lease; an earlier release must leave it standing.
    fn release_lease_if_last(root: &Path, holders_after_release: usize) {
        if holders_after_release == 0 {
            let _ = std::fs::remove_dir_all(root);
        }
    }

    struct Lease;

    impl Lease {
        fn acquire() -> Lease {
            open_leases().fetch_add(1, Ordering::SeqCst);
            Lease
        }
    }

    impl Drop for Lease {
        fn drop(&mut self) {
            let before = open_leases().fetch_sub(1, Ordering::SeqCst);
            release_lease_if_last(lease_root(), before - 1);
        }
    }

    /// In-process serializer: parallel cargo courts would queue on the
    /// lease's own file lock and burn their deadlines waiting; serialized
    /// here instead so each court gets its full budget. Poison-tolerant on
    /// purpose — a panicking predecessor must not turn successors'
    /// failures into "mutex poisoned".
    fn cargo_serializer() -> std::sync::MutexGuard<'static, ()> {
        static SERIALIZER: OnceLock<Mutex<()>> = OnceLock::new();
        SERIALIZER
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|p| p.into_inner())
    }

    fn nested_cargo_envs() -> Vec<(&'static str, String)> {
        vec![
            (
                "CARGO_TARGET_DIR",
                lease_root().display().to_string(),
            ),
            // Wave-receipt method (a20-wave5/a20-wave9): dev opt-level 2 so
            // the 1M-row stress and the BLAKE3 closures run at measurable
            // speed; dev opt-0 is not the receipted configuration.
            ("CARGO_PROFILE_DEV_OPT_LEVEL", "2".to_string()),
            // Deterministic resolution: every dep in these closures is a
            // workspace path dep (E=0 externals, gate --deps receipt).
            ("CARGO_NET_OFFLINE", "true".to_string()),
        ]
    }

    /// The shared body of the four cargo suite courts: run ONE nested
    /// `cargo test -p <pkg> --test <target>` on a fresh lease, assert
    /// exit 0, and require a libtest summary with `0 failed` and at least
    /// `min_passed` executed tests (the zero-executed/compile-abort class
    /// must never read as a pass).
    fn cargo_suite_court(label: &str, pkg: &str, test_target: &str, min_passed: u64) {
        // Held to scope end: the LAST court out of the process deletes the
        // lease root (Drop impl below).
        let _lease = Lease::acquire();
        let _serial = cargo_serializer();
        let envs = nested_cargo_envs();
        let started = Instant::now();
        let outcome = run_bounded(
            "cargo",
            &["test", "-p", pkg, "--test", test_target],
            &envs,
            CARGO_DEADLINE,
        )
        .unwrap_or_else(|e| panic!("{label}: nested runner failed: {e}"));
        let elapsed = started.elapsed();
        let status = outcome.status.unwrap_or_else(|| {
            panic!(
                "{label}: deadline of {}s exceeded — child killed; stderr tail:\n{}",
                CARGO_DEADLINE.as_secs(),
                tail(&outcome.stderr)
            )
        });
        assert!(
            status.success(),
            "{label}: `cargo test -p {pkg} --test {test_target}` exited {status}; stdout tail:\n{}\nstderr tail:\n{}",
            tail(&outcome.stdout),
            tail(&outcome.stderr)
        );
        let (passed, failed) = require_summary(&outcome.stdout, min_passed).unwrap_or_else(|e| {
            panic!(
                "{label}: {e}; stdout tail:\n{}",
                tail(&outcome.stdout)
            )
        });
        println!(
            "WAVE9 INTEGRATION  {label}: cargo test -p {pkg} --test {test_target} exit=0 passed={passed} failed={failed} elapsed={elapsed:?} (lease {})",
            lease_root().display()
        );
    }

    // -- libtest summary parsing (the court's flip sensor) -------------------

    /// Parse EVERY `test result:` line in `out`. Ok((passed, failed)) only
    /// when: at least one summary line exists (a cargo invocation that ran
    /// no test binary is the compile-abort class — never a pass), every
    /// line is `ok`, `failed == 0`, and `passed >= min_passed`.
    fn require_summary(out: &str, min_passed: u64) -> Result<(u64, u64), String> {
        let mut lines = 0usize;
        let (mut passed, mut failed) = (0u64, 0u64);
        for raw in out.lines() {
            let Some(rest) = raw.trim_start().strip_prefix("test result: ") else {
                continue;
            };
            lines += 1;
            let ok = rest.starts_with("ok.");
            let mut num: Option<u64> = None;
            let (mut p, mut f) = (None, None);
            for tok in rest.split_whitespace() {
                if tok == "passed;" || tok == "passed" {
                    p = num;
                } else if tok == "failed;" || tok == "failed" {
                    f = num;
                }
                num = tok.parse::<u64>().ok();
            }
            let (Some(p), Some(f)) = (p, f) else {
                return Err(format!("unparsable libtest summary: {raw:?}"));
            };
            if !ok {
                return Err(format!("libtest summary not ok: {raw:?}"));
            }
            passed += p;
            failed += f;
        }
        if lines == 0 {
            return Err(
                "no `test result:` line — no test binary executed (compile-abort class), refused"
                    .into(),
            );
        }
        if failed != 0 {
            return Err(format!("{failed} test(s) FAILED"));
        }
        if passed < min_passed {
            return Err(format!(
                "only {passed} test(s) executed (< floor {min_passed}) — a suite gutted to zero executed tests must flip this court, never pass it"
            ));
        }
        Ok((passed, failed))
    }

    /// Anti-vacuity, executed EVERY run (the walk_unfiltered pattern): the
    /// refusal paths above are real legs of the real parser, armed against
    /// fabricated outputs on every run. An undetectable failure class is a
    /// court that cannot flip = VACUOUS GATE; these four flips are the
    /// proof it can. Also witnesses the lease cleanup law on a real
    /// scratch root (deletion by the last leaseholder, observed).
    #[test]
    fn wave9_result_parser_negative_control() {
        let ok3 = "test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out";
        assert_eq!(require_summary(ok3, 3).unwrap(), (3, 0));
        let refusals: Vec<(&str, &str)> = vec![
            ("zero-executed", "test result: ok. 0 passed; 0 failed; 0 ignored"),
            ("red suite", "test result: FAILED. 2 passed; 1 failed; 0 ignored"),
            ("no summary (compile-abort)", "error: could not compile `cgap-x` (lib)"),
        ];
        for (label, fabricated) in refusals {
            assert!(
                require_summary(fabricated, 1).is_err(),
                "negative control '{label}' was ACCEPTED — parser cannot flip, VACUOUS GATE"
            );
        }
        // Floor refusal is its own leg: a green 1-test summary must refuse
        // at floor 2 but pass at floor 1.
        let one = "test result: ok. 1 passed; 0 failed";
        assert!(require_summary(one, 2).is_err(), "floor leg cannot flip");
        assert_eq!(require_summary(one, 1).unwrap(), (1, 0));

        // Lease cleanup law, both legs, executed on a real scratch root:
        // a NON-last release must leave the root standing; the LAST
        // release must delete it. (Lease::drop routes through the same
        // factored law — if either leg breaks, the shared nested build
        // root either never disappears or vanishes under a live court.)
        let scratch = std::env::temp_dir().join(format!("cgap-a19-w9-lease-court-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&scratch);
        std::fs::create_dir_all(&scratch).expect("scratch lease");
        assert!(scratch.exists(), "scratch lease root must exist before release");
        release_lease_if_last(&scratch, 1); // one holder remains
        assert!(
            scratch.exists(),
            "non-last release DELETED the lease root — cleanup law broken (live courts would lose their build root)"
        );
        release_lease_if_last(&scratch, 0); // last holder out
        assert!(
            !scratch.exists(),
            "last-holder release did NOT delete the lease root — cleanup law broken (orphaned build root)"
        );
    }

    // -- The five integration-surface courts ---------------------------------

    /// A04 — the wire/memory identity court over the 128 B `CgapFrameHeader`
    /// (owner lane A04). The sovereign surface hands RAW frame bytes across
    /// the ABI; this witness keeps the upstream identity law green from the
    /// membrane's seat.
    #[test]
    fn wave9_a04_identity_court_green() {
        cargo_suite_court(
            "A04 identity court",
            "cgap-core",
            "identity_court",
            4, // receipted this wave: 4 passed
        );
    }

    /// A14 — the dam (cgap-gate `penstock::ingress`) over REAL A04 frames:
    /// the Wave-9 cross-lane tier court (owner lanes A04+A14). The A19
    /// register values it imports are parity-pinned against THIS module in
    /// the wave-3 status court and penstock's own three-way purity court.
    #[test]
    fn wave9_a14_dam_over_a04_frames_court_green() {
        cargo_suite_court(
            "A14 dam over A04 frames",
            "cgap-core",
            "cross_lane_gate_court",
            8, // receipted this wave: 8 passed
        );
    }

    /// A20 — the court-verdict suite including the 1M-row interleave stress
    /// (owner lane A20). Receipted method: dev opt-level 2.
    #[test]
    fn wave9_a20_one_m_row_stress_court_green() {
        cargo_suite_court(
            "A20 1M-row stress (court verdict)",
            "cgap-court",
            "court_verdict",
            11, // receipted: 11/11 courts
        );
    }

    /// A05 — the power-cut sweep: torn-write/rollback court over the frozen
    /// WAL (owner lane A05).
    #[test]
    fn wave9_a05_power_cut_sweep_court_green() {
        cargo_suite_court(
            "A05 power-cut sweep",
            "cgap-storage",
            "court_sweep",
            2, // receipted this wave: 2 passed
        );
    }

    /// A07 — the zero-string gate over ALL BUILT CRATES: the default scan
    /// mode enumerates every non-excluded workspace member's built
    /// artifacts and REFUSES if any member has zero artifacts (structural
    /// anti-vacuity precondition) or if any forbidden string surface
    /// scores a hit. Two legs, both real subprocesses:
    ///   1. the scan itself (hits=0 over every built crate),
    ///   2. the gate's OWN poison/clean self-check (`--self-check`, the
    ///      T4+T12 anti-vacuity probe) — the gate must be able to DETECT
    ///      poison for its hits=0 to carry information.
    #[test]
    fn wave9_a07_zero_string_gate_over_all_built_crates() {
        if !python3_available() {
            println!("[SKIP] python3 absent — A07 zero-string gate unexecuted (loud skip, never a silent pass)");
            return;
        }
        // Leg 1: the real scan over all built crates (defaults: root = the
        // repo this test compiles in, target = <root>/target, profile =
        // debug — the same invocation the session receipt pinned). The
        // script path is ABSOLUTE: cargo runs test binaries with cwd =
        // the package dir, and a relative path resolves there, not at the
        // repo root (witnessed: exit 2, "can't open file .../cgap-ffi/
        // scripts/zero_string_gate.py" on the first court run).
        let script = repo_root().join("scripts/zero_string_gate.py");
        let script = script.to_str().expect("repo script path is UTF-8");
        let started = Instant::now();
        let outcome = run_bounded("python3", &[script], &[], GATE_DEADLINE)
            .unwrap_or_else(|e| panic!("A07 gate: runner failed: {e}"));
        let elapsed = started.elapsed();
        let status = outcome.status.unwrap_or_else(|| {
            panic!(
                "A07 gate: deadline of {}s exceeded — child killed; stderr tail:\n{}",
                GATE_DEADLINE.as_secs(),
                tail(&outcome.stderr)
            )
        });
        assert!(
            status.success(),
            "A07 gate over all built crates exited {status}; stdout tail:\n{}\nstderr tail:\n{}",
            tail(&outcome.stdout),
            tail(&outcome.stderr)
        );
        let summary = parse_gate_summary(&outcome.stdout).unwrap_or_else(|e| {
            panic!("A07 gate: {e}; stdout tail:\n{}", tail(&outcome.stdout))
        });
        // Leg 2: the gate's own anti-vacuity probe must pass — a gate that
        // cannot detect its poison fixture cannot vouch for hits=0 above.
        let probe = run_bounded("python3", &[script, "--self-check"], &[], GATE_DEADLINE)
            .unwrap_or_else(|e| panic!("A07 gate self-check: runner failed: {e}"));
        let probe_status = probe.status.unwrap_or_else(|| {
            panic!("A07 gate self-check: deadline exceeded; stderr tail:\n{}", tail(&probe.stderr))
        });
        assert!(
            probe_status.success(),
            "A07 gate self-check (poison/clean probe) exited {probe_status} — the gate cannot flip, its scan is untrustworthy; stdout tail:\n{}",
            tail(&probe.stdout)
        );
        println!(
            "WAVE9 INTEGRATION  A07 zero-string gate: scan exit=0 scanned={} crates={} hits=0 + self-check exit=0 elapsed={elapsed:?}",
            summary.scanned, summary.crates
        );
    }

    struct GateSummary {
        scanned: u64,
        crates: u64,
    }

    /// Mechanical read of the gate's SUMMARY line. Refuses: no SUMMARY
    /// (the gate exited 0 without scanning — vacuous), any `hits=` or
    /// `adv_hits=` other than 0, or a scan narrower than the receipted
    /// breadth (45 artifacts / 22 crates this wave; floors 10/20 allow
    /// benign churn, refuse the gutting class).
    fn parse_gate_summary(out: &str) -> Result<GateSummary, String> {
        let line = out
            .lines()
            .find(|l| l.trim_start().starts_with("SUMMARY "))
            .ok_or("no SUMMARY line — the gate exited 0 without scanning (vacuous)")?;
        let mut scanned = None;
        let mut crates = None;
        for tok in line.split_whitespace() {
            if let Some(v) = tok.strip_prefix("scanned=") {
                scanned = v.parse::<u64>().ok();
            } else if let Some(v) = tok.strip_prefix("crates=") {
                crates = v.parse::<u64>().ok();
            } else if let Some(v) = tok.strip_prefix("hits=") {
                if v != "0" {
                    return Err(format!("zero-string HIT in built crates: {tok}"));
                }
            } else if let Some(v) = tok.strip_prefix("adv_hits=") {
                if v != "0" {
                    return Err(format!("advisory hits nonzero: {tok}"));
                }
            }
        }
        let (Some(scanned), Some(crates)) = (scanned, crates) else {
            return Err(format!("SUMMARY line unparsable: {line:?}"));
        };
        if scanned < 10 {
            return Err(format!("scanned={scanned} < floor 10 — scan breadth gutted"));
        }
        if crates < 20 {
            return Err(format!("crates={crates} < floor 20 — not all built crates scanned"));
        }
        Ok(GateSummary { scanned, crates })
    }

    fn python3_available() -> bool {
        Command::new("python3")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// The parser's own negative control, executed every run: a SUMMARY
    /// with a hit, a hit-free summary below the breadth floors, and a
    /// missing SUMMARY must all REFUSE. If any is accepted, the A07 court
    /// cannot flip = VACUOUS GATE.
    #[test]
    fn wave9_a07_gate_parser_negative_control() {
        let good = "SUMMARY scanned=45 crates=22 hits=0 emitter_hits=2 adv_hits=0 unknown=316943 allowed=6661 gate=1.4.0";
        let s = parse_gate_summary(good).expect("receipted summary must parse");
        assert_eq!((s.scanned, s.crates), (45, 22));
        let refusals = vec![
            ("hit present", "SUMMARY scanned=45 crates=22 hits=1 adv_hits=0"),
            ("advisory hit", "SUMMARY scanned=45 crates=22 hits=0 adv_hits=3"),
            ("breadth gutted", "SUMMARY scanned=4 crates=22 hits=0 adv_hits=0"),
            ("crate coverage gutted", "SUMMARY scanned=45 crates=2 hits=0 adv_hits=0"),
        ];
        for (label, fabricated) in refusals {
            assert!(
                parse_gate_summary(fabricated).is_err(),
                "negative control '{label}' was ACCEPTED — A07 parser cannot flip, VACUOUS GATE"
            );
        }
        assert!(
            parse_gate_summary("no summary here").is_err(),
            "missing-SUMMARY leg cannot flip, VACUOUS GATE"
        );
    }
}
