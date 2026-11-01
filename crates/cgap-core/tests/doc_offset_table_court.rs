//! §9 DoD 9 court: "Doc §6 byte-offset table machine-checked against
//! `frame.rs` — a test parses the markdown table and asserts
//! offsets/sizes/total 128." Also covers the A17 §2.1 attestation table
//! (the authoritative P28LicenseAttestation layout this crate embeds).
//!
//! The lane docs are the normative layout contracts referenced byte-for-byte
//! by lanes A14/A17/A19; this court makes doc-vs-code drift a test failure
//! instead of a prose claim. A missing doc file fails loudly (never a skip).

use cgap_core::frame::{CgapFrameHeader, P28LicenseAttestation};

const A04_DOC: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/jira/v26.11.1/lanes/A04-triumvirate-isomorphism.md");
const A17_DOC: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/jira/v26.11.1/lanes/A17-p28-derivative.md");

/// Compiled truth for the frame header, by §6.1 field name:
/// (offset, size) straight from `offset_of!`/`size_of`, never literals.
fn pinned_header_layout(field: &str) -> Option<(usize, usize)> {
    Some(match field {
        "magic" => (core::mem::offset_of!(CgapFrameHeader, magic), 4),
        "version" => (core::mem::offset_of!(CgapFrameHeader, version), 1),
        "flags" => (core::mem::offset_of!(CgapFrameHeader, flags), 1),
        "reserved" => (core::mem::offset_of!(CgapFrameHeader, reserved), 2),
        "payload_byte_len" => (
            core::mem::offset_of!(CgapFrameHeader, payload_byte_len),
            core::mem::size_of::<u64>(),
        ),
        "oabi_digest" => (
            core::mem::offset_of!(CgapFrameHeader, oabi_digest),
            core::mem::size_of::<[u8; 32]>(),
        ),
        "attestation" => (
            core::mem::offset_of!(CgapFrameHeader, attestation),
            core::mem::size_of::<P28LicenseAttestation>(),
        ),
        _ => return None,
    })
}

/// Compiled truth for the attestation, by A17 §2.1 field name.
fn pinned_attestation_layout(field: &str) -> Option<(usize, usize)> {
    Some(match field {
        "license_magic" => (
            core::mem::offset_of!(P28LicenseAttestation, license_magic),
            core::mem::size_of::<[u8; 4]>(),
        ),
        "version" => (
            core::mem::offset_of!(P28LicenseAttestation, version),
            core::mem::size_of::<u16>(),
        ),
        "attestation_flags" => (
            core::mem::offset_of!(P28LicenseAttestation, attestation_flags),
            core::mem::size_of::<u16>(),
        ),
        "safe_harbor_ceiling_cents" => (
            core::mem::offset_of!(P28LicenseAttestation, safe_harbor_ceiling_cents),
            core::mem::size_of::<u64>(),
        ),
        "origin_authority_digest" => (
            core::mem::offset_of!(P28LicenseAttestation, origin_authority_digest),
            core::mem::size_of::<[u8; 32]>(),
        ),
        "upstream_receipt" => (
            core::mem::offset_of!(P28LicenseAttestation, upstream_receipt),
            core::mem::size_of::<[u8; 32]>(),
        ),
        _ => return None,
    })
}

/// Extract `(offset, size, field)` rows from the first markdown table under
/// `section_heading` (e.g. "### 6.1"). Non-data rows (header, separator,
/// trailing summary rows with a non-numeric size cell) are skipped.
fn parse_offset_table(doc: &str, section_heading: &str) -> Vec<(usize, usize, String)> {
    let section = doc
        .split(section_heading)
        .nth(1)
        .unwrap_or_else(|| panic!("section {section_heading:?} not found in doc"));
    let mut rows = Vec::new();
    for line in section.lines().skip(1) {
        if !line.trim_start().starts_with('|') {
            if !rows.is_empty() {
                break; // table ended
            }
            continue;
        }
        let cells: Vec<&str> = line.split('|').collect();
        // "| a | b | c |" splits into ["", " a ", " b ", " c ", ""]
        if cells.len() < 5 {
            continue;
        }
        let (off_c, size_c, field_c) = (cells[1].trim(), cells[2].trim(), cells[3].trim());
        if off_c.contains("--") || off_c == "offset" || off_c == "Offset" {
            continue; // separator / header row
        }
        let (Ok(offset), Ok(size)) = (off_c.parse::<usize>(), size_c.parse::<usize>())
        else {
            continue; // summary rows like "| 80 | — | end |"
        };
        let field = field_c.trim_matches('`').to_string();
        rows.push((offset, size, field));
    }
    rows
}

fn assert_table_matches(
    doc_name: &str,
    rows: &[(usize, usize, String)],
    pinned_for: fn(&str) -> Option<(usize, usize)>,
    total: usize,
) {
    assert!(!rows.is_empty(), "{doc_name}: table parsed to zero rows");
    let mut cursor = 0usize;
    let mut sum = 0usize;
    for &(offset, size, ref field) in rows.iter() {
        assert_eq!(
            offset, cursor,
            "{doc_name}: field {field:?} offset {offset} breaks contiguity (expected {cursor})"
        );
        // The compiled layout is the oracle; the doc must name it exactly.
        let pinned = pinned_for(field)
            .unwrap_or_else(|| panic!("{doc_name}: unparsed field name {field:?}"));
        assert_eq!(
            (offset, size),
            pinned,
            "{doc_name}: DRIFT — doc says field {field:?} at ({offset},{size}), \
             frame.rs compiles to {:?}",
            pinned
        );
        cursor = offset + size;
        sum += size;
    }
    assert_eq!(cursor, total, "{doc_name}: table end {cursor} != {total}");
    assert_eq!(sum, total, "{doc_name}: size sum {sum} != {total}");
}

#[test]
fn a04_doc_s61_table_matches_compiled_frame_layout() {
    let doc = std::fs::read_to_string(A04_DOC).expect("A04 lane doc must exist (DoD 9)");
    let rows = parse_offset_table(&doc, "### 6.1");
    assert_eq!(rows.len(), 7, "§6.1 must pin all 7 header fields");
    assert_table_matches("A04 §6.1", &rows, pinned_header_layout, 128);
}

#[test]
fn a17_doc_s21_table_matches_compiled_attestation_layout() {
    let doc = std::fs::read_to_string(A17_DOC).expect("A17 lane doc must exist");
    let rows = parse_offset_table(&doc, "### 2.1");
    assert_eq!(rows.len(), 6, "A17 §2.1 must pin all 6 attestation fields");
    assert_table_matches("A17 §2.1", &rows, pinned_attestation_layout, 80);
}
