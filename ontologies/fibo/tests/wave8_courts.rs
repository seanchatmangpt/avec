//! Wave-8 courts (lane A09) — defect repair driven by the wave-7-era court
//! receipts (a07-wave6-audit-receipt.md §5 / open item 1; the a08-wave7
//! audit receipt's D-A08-1 absent-artifact class). Two repairs, each with a
//! flipping court, zero new features beyond the courts:
//!
//! - **D1 citation-rot repair** (`a06_citation_freshness_court`): the lane
//!   doc-comment citations of A06 reader line numbers had gone stale a
//!   SECOND time (lib.rs cited `oabi.rs:47`, the const lives at `:61` as
//!   of the wave-8 run; emit-pack.rs cited `:44-65`, the constants live at
//!   `:58-78` as of the same run) — the same line-citation rot class the
//!   a07-wave5 audit receipt names, which this lane already hand-repaired
//!   once in wave 6 (`:48` -> `:47`). Hand freshening rots again by
//!   construction, so the repair is mechanical: the court re-derives each
//!   constant's declaration line from the REAL
//!   `crates/cgap-bont/src/oabi.rs` on disk every run and joins it against
//!   the citations parsed from this lane's own files. Either side moving
//!   flips the court loudly.
//! - **D2 dangling-receipt repair** (T13 flip re-witness): the T13
//!   quarantine court cited a "Wave-7 lane receipt" file that does not
//!   exist on disk (the absent-artifact class the a08-wave7 audit receipt
//!   registers as D-A08-1). The flip witness is re-executed in a /tmp
//!   sandbox with the named mutation class (embed a refused lexeme in the
//!   emitter's referenced data — the Wave-2..6 corpus shape; an unused
//!   const is NOT codegen'd and flips nothing, also witnessed). The T13
//!   docstring no longer cites the phantom receipt.

use std::path::PathBuf;

/// The cited reader (A06-owned, read-only to this lane). Relative to this
/// crate's manifest dir — same layout in the canonical checkout and in the
/// mirrored /tmp sandbox used by shim protocols.
const OABI_RS: &str = "../../crates/cgap-bont/src/oabi.rs";

/// The lane-owned files whose A06 line citations this court pins.
const LIB_RS: &str = "src/lib.rs";
const EMIT_RS: &str = "src/bin/emit-pack.rs";

fn read_lane_file(rel: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {}", path.display(), e))
}

fn read_oabi_rs() -> String {
    read_lane_file(OABI_RS)
}

/// Truth side: 1-based line of the first line whose trimmed text starts
/// with `decl` — where `pub const <NAME>` actually lives on disk today.
fn decl_line(text: &str, decl: &str, name: &str) -> usize {
    match text.lines().position(|l| l.trim_start().starts_with(decl)) {
        Some(i) => i + 1,
        None => panic!("A06 reader must declare {} (missing: {})", name, decl),
    }
}

/// Parse the digits of the single `src/oabi.rs:<line>` citation.
fn parse_single_citation(text: &str) -> usize {
    let marker = "src/oabi.rs:";
    let n = text.matches(marker).count();
    assert_eq!(
        n, 1,
        "expected exactly one numbered src/oabi.rs: citation, found {}",
        n
    );
    let i = text.find(marker).unwrap();
    let rest = &text[i + marker.len()..];
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().expect("citation line number parses")
}

/// Parse the digits of the `src/oabi.rs:<lo>-<hi>` range citation.
fn parse_range_citation(text: &str) -> (usize, usize) {
    let marker = "src/oabi.rs:";
    let n = text.matches(marker).count();
    assert_eq!(
        n, 1,
        "expected exactly one numbered src/oabi.rs: citation, found {}",
        n
    );
    let i = text.find(marker).unwrap();
    let rest = &text[i + marker.len()..];
    let b = rest.as_bytes();
    let mut j = 0;
    while j < b.len() && b[j].is_ascii_digit() {
        j += 1;
    }
    let lo: usize = rest[..j].parse().expect("range start parses");
    assert!(j < b.len() && b[j] == b'-', "range citation must be lo-hi");
    let rest2 = &rest[j + 1..];
    let b2 = rest2.as_bytes();
    let mut k = 0;
    while k < b2.len() && b2[k].is_ascii_digit() {
        k += 1;
    }
    let hi: usize = rest2[..k].parse().expect("range end parses");
    (lo, hi)
}

#[test]
fn a06_citation_freshness_court() {
    // --- Truth side: re-derived from the REAL reader on disk, every run ---
    let oabi_rs = read_oabi_rs();
    let magic_line = decl_line(&oabi_rs, "pub const OABI_MAGIC: [u8; 8]", "OABI_MAGIC");
    let header_len_line =
        decl_line(&oabi_rs, "pub const OABI_HEADER_LEN: usize", "OABI_HEADER_LEN");
    let record_size_line =
        decl_line(&oabi_rs, "pub const OABI_RECORD_SIZE: usize", "OABI_RECORD_SIZE");
    assert!(
        magic_line < header_len_line && header_len_line < record_size_line,
        "A06 constant order regressed"
    );

    // --- Citation side: parsed from this lane's own doc comments ----------
    let lib_rs = read_lane_file(LIB_RS);
    let emit_rs = read_lane_file(EMIT_RS);

    // D1 pin: lib.rs cites OABI_HEADER_LEN's CURRENT line.
    let lib_cite = parse_single_citation(&lib_rs);
    assert_eq!(
        lib_cite, header_len_line,
        "lib.rs OABI_HEADER_LEN citation is stale (cites {}, const lives at {}) — wave-8 D1",
        lib_cite, header_len_line
    );

    // D1 pin: emit-pack.rs cites a range covering the container constants.
    let (lo, hi) = parse_range_citation(&emit_rs);
    assert!(
        lo <= magic_line && hi > record_size_line,
        "emit-pack.rs range citation {}-{} no longer covers the container constants — wave-8 D1",
        lo, hi
    );

    // --- Value joins: the producer's restated constants are byte-identical
    // --- (modulo `pub `) to A06's on-disk declarations ("cited, not
    // --- redefined", enforced mechanically).
    assert!(oabi_rs.contains("pub const OABI_MAGIC: [u8; 8] = *b\"OABIv1\\0\\0\";"));
    assert!(emit_rs.contains("const OABI_MAGIC: [u8; 8] = *b\"OABIv1\\0\\0\";"));
    assert!(oabi_rs.contains("pub const OABI_HEADER_LEN: usize = 64;"));
    assert!(emit_rs.contains("const OABI_HEADER_LEN: usize = 64;"));
    assert!(oabi_rs.contains("pub const OABI_RECORD_SIZE: usize = 48;"));
    assert!(emit_rs.contains("const OABI_RECORD_SIZE: usize = 48;"));
    assert!(oabi_rs.contains("pub const OABI_VERSION_MAJOR: u16 = 1;"));
    assert!(emit_rs.contains("const OABI_VERSION_MAJOR: u16 = 1;"));
    assert!(oabi_rs.contains("pub const OABI_VERSION_MINOR: u16 = 0;"));
    assert!(emit_rs.contains("const OABI_VERSION_MINOR: u16 = 0;"));

    // --- Mutation witness (in-repo, mechanical): a shifted citation flips
    // --- this court — the line AFTER the true decl does not carry the
    // --- decl, so any +/-1-stale line number fails the line-exact join.
    let shifted = oabi_rs
        .lines()
        .nth(header_len_line) // 0-based nth(54) == 1-based line 55
        .expect("line after the decl must exist");
    assert!(
        !shifted.trim_start().starts_with("pub const OABI_HEADER_LEN: usize"),
        "line-exactness witness vacuous: the next line also matches the decl"
    );

    // --- Stale-history witnesses: the pre-repair values must not pass.
    assert_ne!(
        lib_cite, 47,
        "stale-history witness: lib.rs still cites the pre-repair value 47"
    );
    assert!(
        !(lo == 44 && hi == 65),
        "stale-history witness: emit-pack.rs still cites the pre-repair range 44-65"
    );
}
