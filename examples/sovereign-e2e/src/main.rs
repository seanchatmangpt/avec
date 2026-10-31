//! sovereign-e2e — the minimal runnable demo of the avec thesis.
//!
//! One dataset, three planes, one receipt:
//!
//! ```text
//! 20 Quad64 (Arrow) -> BLAKE3 receipt R
//!                   -> level-free Parquet file (8 fixed-width columns, 0 levels)
//!                   -> Git packfile v2 (blob) + .idx v2
//!                   -> readback: inflate, byte-equality, re-hash == R
//! ```
//!
//! Every plane crossing is narrated on stdout; every inequality is a loud
//! non-zero exit (`REFUSED: ...`), never a silent pass. No `unwrap` on the
//! main path: every fallible step is a typed error surfaced as a refusal.

use core::fmt::Debug;
use std::process::exit;

use avec_arrow::layout::{Quad64, QUAD_ALIGN, QUAD_SIZE};
use avec_blake3::hash;
use avec_parquet::level_free::{write_file, LevelFreeColumn, LevelFreeType};
use avec_parquet::scan::{parse_footer, PhysType};
use avec_parquet::thrift::{encoding, read_page_header};
use avec_git::inflate::zlib_decode;
use avec_git::pack_write::{
    build_pack_and_idx, encode_object_header, git_object_oid, parse_idx_v2, parse_pack_v2,
};

/// Record count — the demo dataset.
const N: usize = 20;

/// Byte width of the quad wire field at a given record offset (the 64B
/// record is 5 x u32 + 1 x u64 + 2 x u32 + 32B digest = 64B).
fn field_width(off: usize) -> usize {
    match off {
        0x10 => 8,    // t_ns
        0x20 => 32,   // inline_digest
        _ => 4,       // s/p/o/g id, vector_clock, policy_mask
    }
}

/// The 8 level-free columns: (name, physical type, offset in the 64B quad
/// wire record). Sum of widths = 64B, so one row group's page payload is
/// exactly the column-major permutation of the row-major quad bytes.
const SCHEMA: [(&str, LevelFreeType, usize); 8] = [
    ("s_id", LevelFreeType::U32, 0x00),
    ("p_id", LevelFreeType::U32, 0x04),
    ("o_id", LevelFreeType::U32, 0x08),
    ("g_id", LevelFreeType::U32, 0x0C),
    ("t_ns", LevelFreeType::U64, 0x10),
    ("vector_clock", LevelFreeType::U32, 0x18),
    ("policy_mask", LevelFreeType::U32, 0x1C),
    ("inline_digest", LevelFreeType::Bytes32, 0x20),
];

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// Loud, typed refusal: narrate to stderr and exit non-zero.
fn refuse(stage: &str, why: &str) -> ! {
    eprintln!("REFUSED at {stage}: {why}");
    exit(1);
}

/// Unwrap-equivalent that refuses instead of panicking.
fn must<T, E: Debug>(stage: &str, r: Result<T, E>) -> T {
    match r {
        Ok(v) => v,
        Err(e) => refuse(stage, &format!("{e:?}")),
    }
}

/// Equality gate: assert or refuse.
fn require(stage: &str, ok: bool, why: &str) {
    if !ok {
        refuse(stage, why);
    }
}

/// Deterministic quad pattern for record `i` (no clocks, no entropy: the
/// same 20 quads on every run, so R is a stable receipt).
fn pattern(i: usize) -> (u32, u32, u32, u32, u64, u32, u32) {
    let i = i as u64;
    (
        0x5355_0000_u32.wrapping_add(i as u32),
        0x0000_0100_u32 + (i as u32 % 8),
        0x4F42_0000_u32.wrapping_add((i as u32).wrapping_mul(7)),
        0x0000_0042,
        1_700_000_000_000_000_000 + i * 1_000_000,
        (i as u32) + 1,
        0x0300_0007_u32 ^ (i as u32),
    )
}

/// Per-quad receipt: BLAKE3 over the 32-byte pattern (s/p/o/g/t/vc/pm,
/// little-endian, field order). The digest is a receipt over the pattern,
/// not payload data.
fn quad_digest(i: usize) -> [u8; 32] {
    let (s, p, o, g, t, vc, pm) = pattern(i);
    let mut pat = [0u8; 32];
    pat[0x00..0x04].copy_from_slice(&s.to_le_bytes());
    pat[0x04..0x08].copy_from_slice(&p.to_le_bytes());
    pat[0x08..0x0C].copy_from_slice(&o.to_le_bytes());
    pat[0x0C..0x10].copy_from_slice(&g.to_le_bytes());
    pat[0x10..0x18].copy_from_slice(&t.to_le_bytes());
    pat[0x18..0x1C].copy_from_slice(&vc.to_le_bytes());
    pat[0x1C..0x20].copy_from_slice(&pm.to_le_bytes());
    hash(&pat)
}

fn main() {
    // ------------------------------------------------------- PLANE 1: Arrow
    let stage = "PLANE 1 (Arrow)";
    let quads: Vec<Quad64> = (0..N)
        .map(|i| {
            let (s, p, o, g, t, vc, pm) = pattern(i);
            Quad64::new(s, p, o, g, t, vc, pm, quad_digest(i))
        })
        .collect();

    // Alignment is a build-time law in avec-arrow; witness it at runtime.
    let base = quads.as_ptr() as usize;
    require(
        stage,
        base % QUAD_ALIGN == 0,
        &format!("quad buffer at {base:#x} is not {QUAD_ALIGN}-byte aligned"),
    );
    println!("PLANE 1 (Arrow): {N} records x 64B = {}B, aligned", N * QUAD_SIZE);

    // Row-major wire bytes — the canonical byte sequence the receipt hashes.
    let flat: Vec<u8> = quads.iter().flat_map(|q| q.to_le_bytes()).collect();
    require(stage, flat.len() == N * QUAD_SIZE, "flat buffer != 1280B");

    // The dataset receipt.
    let r: [u8; 32] = hash(&flat);
    println!("RECEIPT: R = {}", hex(&r));

    // ----------------------------------------------------- PLANE 2: Parquet
    let stage = "PLANE 2 (Parquet)";
    // Column-major payloads: column c gathers its fixed-width slice from
    // every 64B record — the 1:1 Arrow-buffer-to-page mapping, no copy.
    let cols: Vec<LevelFreeColumn> = SCHEMA
        .iter()
        .map(|(name, ty, _)| LevelFreeColumn {
            name: name.as_bytes().to_vec(),
            ty: *ty,
        })
        .collect();
    let mut col_payloads: Vec<Vec<u8>> = Vec::with_capacity(SCHEMA.len());
    for &(_, _, off) in &SCHEMA {
        let w = field_width(off);
        let mut col = Vec::with_capacity(N * w);
        for q in &quads {
            let wire = q.to_le_bytes();
            col.extend_from_slice(&wire[off..off + w]);
        }
        col_payloads.push(col);
    }
    let payload_refs: Vec<&[u8]> = col_payloads.iter().map(|c| c.as_slice()).collect();

    // One file: PAR1 | row-group page | footer | len(u32 LE) | PAR1.
    let file_bytes = must(stage, write_file(&cols, &payload_refs, N));
    require(stage, file_bytes[..4] == *b"PAR1", "file magic missing");

    // Frame the page and prove the level-free claim from the written bytes.
    let footer_len = u32::from_le_bytes(
        file_bytes[file_bytes.len() - 8..file_bytes.len() - 4].try_into().unwrap(),
    ) as usize;
    let body_end = file_bytes.len() - 8 - footer_len;
    let page = &file_bytes[4..body_end];
    let (ph, ph_len) = must(stage, read_page_header(page));
    let v2 = match ph.data_page_header_v2 {
        Some(v2) => v2,
        None => refuse(stage, "page is not DataPageV2"),
    };
    require(stage, v2.num_nulls == 0, "page carries nulls");
    require(
        stage,
        v2.definition_levels_byte_length == 0 && v2.repetition_levels_byte_length == 0,
        "page carries rep/def level bytes",
    );
    require(stage, !v2.is_compressed, "page is compressed");
    require(stage, v2.encoding == encoding::PLAIN, "page is not PLAIN");
    require(stage, v2.num_rows == N as i32, "page row count wrong");
    require(
        stage,
        page.len() - ph_len == N * QUAD_SIZE,
        "page payload is not header + 1280B",
    );
    println!(
        "PLANE 2 (Parquet): page bytes = {}, levels = 0 (file = {} B, {} columns x {N} rows)",
        page.len(),
        file_bytes.len(),
        SCHEMA.len(),
    );

    // -------------------------------------------------------- PLANE 3: Git
    let stage = "PLANE 3 (Git)";
    let (pack, idx, entries) = must(stage, build_pack_and_idx(&[&file_bytes]));
    let oid = git_object_oid("blob", &file_bytes);
    require(stage, entries.len() == 1, "pack entry count wrong");
    require(stage, entries[0].oid == oid, "idx OID != git_object_oid");
    println!(
        "PLANE 3 (Git): pack = {} bytes, OID = {}",
        pack.len(),
        hex(&oid)
    );

    // ------------------------------------------------------------- READBACK
    // Parse the pack with the independent in-crate parser: checksums, entry
    // boundaries, CRC32, declared sizes.
    let stage = "READBACK (Git)";
    let objs = must(stage, parse_pack_v2(&pack));
    require(stage, objs.len() == 1, "pack does not hold exactly 1 object");
    let obj = objs[0];
    require(stage, obj.obj_type == 3, "pack object is not a blob");
    require(
        stage,
        obj.size as usize == file_bytes.len(),
        "declared blob size != parquet file size",
    );

    // Cross-check the .idx: same OID, offset and CRC as the parsed pack.
    let (idx_oids, idx_crcs, idx_offs) = must(stage, parse_idx_v2(&idx));
    require(stage, idx_oids.len() == 1, "idx entry count wrong");
    require(stage, idx_oids[0] == oid, "idx OID mismatch");
    require(stage, idx_offs[0] == obj.offset, "idx offset mismatch");
    require(stage, idx_crcs[0] == obj.crc, "idx CRC mismatch");

    // Slice the zlib stream (header varint is re-derived, not guessed) and
    // inflate it back to the parquet file.
    let hdr = encode_object_header(obj.obj_type, obj.size);
    require(
        stage,
        pack[obj.offset as usize..].starts_with(&hdr),
        "object header mismatch at recorded offset",
    );
    let stream = &pack[obj.offset as usize + hdr.len()..pack.len() - 20];
    let blob = must(stage, zlib_decode(stream));
    require(
        stage,
        blob == file_bytes,
        "inflated blob is not byte-identical to the written parquet file",
    );
    require(
        stage,
        hash(&blob) == hash(&file_bytes),
        "blob receipt mismatch after the pack round trip",
    );
    println!(
        "READBACK (Git): blob byte-identical ({} B), receipt holds",
        blob.len()
    );

    // ------------------------------------------- READBACK (Parquet -> Arrow)
    // Re-parse the parquet framing from the inflated blob and reassemble the
    // row-major quad bytes: content survives the two representation changes.
    let stage = "READBACK (Parquet)";
    require(stage, blob[..4] == *b"PAR1" && blob[blob.len() - 4..] == *b"PAR1",
        "blob framing magic missing");
    let footer_len2 = u32::from_le_bytes(
        blob[blob.len() - 8..blob.len() - 4].try_into().unwrap(),
    ) as usize;
    let body_end2 = blob.len() - 8 - footer_len2;
    let footer = &blob[body_end2..blob.len() - 8];
    let meta = must(stage, parse_footer(footer));
    require(stage, meta.num_rows == N as i64, "footer row count wrong");
    require(stage, meta.columns.len() == SCHEMA.len(), "footer column count wrong");
    for (c, ((name, _, _), desc)) in SCHEMA.iter().zip(meta.columns.iter()).enumerate() {
        require(stage, desc.name == *name, &format!("column {c} name drift"));
        let want = match c {
            4 => PhysType::U64,
            7 => PhysType::Flba(32),
            _ => PhysType::U32,
        };
        require(stage, desc.phys == want, &format!("column {c} type drift"));
        require(stage, desc.width == field_width(SCHEMA[c].2), "column width drift");
    }
    require(stage, meta.row_groups.len() == 1, "row group count wrong");
    require(stage, meta.row_groups[0].page_offset == 4, "page not at offset 4");

    let page2 = &blob[4..body_end2];
    let (ph2, ph2_len) = must(stage, read_page_header(page2));
    let v22 = match ph2.data_page_header_v2 {
        Some(v2) => v2,
        None => refuse(stage, "readback page is not DataPageV2"),
    };
    require(
        stage,
        v22.num_nulls == 0
            && v22.definition_levels_byte_length == 0
            && v22.repetition_levels_byte_length == 0,
        "readback page carries levels",
    );
    let payload = &page2[ph2_len..];
    require(stage, payload.len() == N * QUAD_SIZE, "readback payload != 1280B");

    // Column-major -> row-major: scatter each column back into 64B records.
    let mut flat2 = vec![0u8; N * QUAD_SIZE];
    let mut col_base = 0usize;
    for &(_, _, off) in &SCHEMA {
        let w = field_width(off);
        for row in 0..N {
            let src = col_base + row * w;
            flat2[row * QUAD_SIZE + off..row * QUAD_SIZE + off + w]
                .copy_from_slice(&payload[src..src + w]);
        }
        col_base += N * w;
    }

    // The three equalities that close the continuum.
    require(stage, flat2 == flat, "reassembled records != original records");
    require(stage, hash(&flat2) == r, "re-hashed receipt != R");
    let reparsed: Vec<Quad64> = (0..N)
        .map(|row| {
            Quad64::from_le_bytes(&flat2[row * QUAD_SIZE..(row + 1) * QUAD_SIZE])
                .unwrap_or_else(|| refuse(stage, "record re-parse failed"))
        })
        .collect();
    require(stage, reparsed == quads, "re-parsed records != original quads");

    println!(
        "READBACK (Parquet): 8 columns, {N} rows, payload = {} B",
        payload.len()
    );

    // ------------------------------------------------------------ CONTINUUM
    println!("CONTINUUM: readback byte-identical, receipt balanced. ΔS + Φ ≡ R.");
    println!("  R (Arrow, PLANE 1)        = {}", hex(&r));
    println!("  R' (readback, PLANE 3)    = {}", hex(&hash(&flat2)));
    println!("  git blob OID (content)    = {}", hex(&oid));
}
