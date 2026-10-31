//! Wave-14 lane 3 — systematic idx corruption court for the 16-stage lookup
//! ladder + `IdxV2` reader.
//!
//! Fixture: a valid 1,024-object `.idx` v2 built by the crate's own writer
//! (`build_pack_and_idx`), then systematically corrupted one byte class at a
//! time. Every mutation is classified by *observed* reader behavior; nothing
//! here asserts what the code "should" do — rows pin what it *does* do.
//!
//! GUARANTEE MATRIX (release build; live counts in per-test receipts).
//! Fanout byte indexing below is big-endian: byte0 = +2^24, byte1 = +2^16,
//! byte2 = +-256, byte3 = +-1.
//!
//! | class                            | parse            | lookup                                   | verify_idx_checksum |
//! |----------------------------------|------------------|------------------------------------------|---------------------|
//! | pristine                         | Ok               | all N hit (row, offset)                  | Ok                  |
//! | header magic/version byte        | Err(BadMagic)    | unreachable (parse gates)                | n/a                 |
//! | fanout byte0 (^0x01 = +2^24)     | Ok (e<255)       | Err(TooManyObjects) always               | Err(ChecksumMismatch) |
//! | fanout byte1 (^0x01 = +2^16)     | Ok (e<255)       | nonempty bucket: Err(TooManyObjects); empty bucket: **OOB PANIC** (defect) | Err |
//! | fanout byte2 (^0x01 = +-256)     | Ok (e<255)       | Hit self-consistent / Miss / Err(TooManyObjects|Truncated) on inversion | Err |
//! | fanout byte3 (^0x01 = +-1)       | Ok (e<255)       | Hit self-consistent / Miss / Err(TooManyObjects|Truncated) on inversion | Err |
//! | fanout hi<lo (non-monotonic)     | Ok               | Err(Truncated) — checked_sub refusal     | Err                 |
//! | fanout +4,096 (inside window)    | Ok               | **OOB PANIC** (defect witness)           | Err                 |
//! | OID table byte                   | Ok               | Hit self-consistent / Miss; never Err, never panic | Err       |
//! | OID rows swapped (unsorted)      | Ok               | Hit self-consistent / Miss; raw ladder b<=N, eq <=> rows[b]==t, 16 stages | Err |
//! | CRC table byte                   | Ok               | IDENTICAL to pristine baseline           | Err                 |
//! | offset LSB byte                  | Ok               | same row, SILENT wrong offset            | Err                 |
//! | offset MSB byte (^0x01 = +2^24)  | Ok               | same row, offset shifted by 2^24, silent | Err                 |
//! | offset MSB bit7 (bit-31 escape)  | Ok               | in-range escape: same row, exact 64-bit large-table offset; out-of-range escape: Err(Truncated) (this fixture: ALWAYS — no large table, min offset 12 > 4 in-range slots) | Err |
//! | trailer pack-ck / idx-ck byte    | Ok               | IDENTICAL to pristine baseline           | Err(ChecksumMismatch) |
//! | partition > 2^16 (under mutation)| Ok               | Err(TooManyObjects(exact part))          | n/a                 |
//!
//! Loudly (task definition, verified against the code): `IdxV2::lookup` does
//! NOT call `verify_idx_checksum` and never consults the per-entry CRC table.
//! Corruption detection is OPT-IN: a lookup on a corrupted table is answered
//! *from that table* — wrong offsets are real and silent. The court asserts
//! the invariant the ladder does guarantee (a hit row's bytes always equal
//! the query — no false hits) and asserts `Err(IdxChecksumMismatch)` from
//! `verify_idx_checksum` on every body mutation that actually changed a byte
//! (no-op assignments are counted, not asserted): detection exists but never
//! sits on the lookup path.
//!
//! Live defect pinned as a witness (see
//! `fanout_inflation_within_window_oob_panic_defect_witness`): `IdxV2::parse`
//! validates size only against `fanout[255]`; the ladder probes
//! `oid(lo + p - 1)` with `lo` taken from the possibly-corrupted fanout,
//! unclamped by the physical table. A fanout entry inflated into
//! [~1.4n, <2^16] drives probe indices past the OID table while staying
//! inside the partition window -> slice-index panic in release AND debug.
//! If the defect is fixed, the witness goes red on purpose — flip it to the
//! new typed guarantee and update the matrix rows.
//!
//! Run: `cargo test -p avec-git --test ladder_mutations --release`
//!
//! Substrate note: idx_read.rs lookup changed under this lane mid-session
//! (raw `hi - lo` replaced by `checked_sub` -> typed Truncated on a
//! non-monotonic fanout). All rows here are pinned against the CURRENT tree
//! (witness + sweep re-run green against it); the OOB defect witness still
//! reproduces, so the parse-side fanout trust remains unfixed.
//!
//! Re-derivation 2026-10-31 (wave-14 fix lane): idx_read.rs `offset()` fixed
//! (full-width u64 MSB select + typed `Err(Truncated)` on a corrupt escape;
//! witnesses were crates/avec-git/tests/large_offset.rs cases 4/6). The
//! "offset MSB bit7" matrix row and M10 were re-pinned against the fixed
//! reader: out-of-range escapes are now the typed refusal instead of the old
//! silent 0-or-garbage. The fanout and offset-LSB/MSB-byte rows are
//! unaffected.

use std::any::Any;
use std::panic::{catch_unwind, AssertUnwindSafe};

use avec_git::idx_read::IdxV2;
use avec_git::ladder::{ladder_search, LADDER_WINDOW};
use avec_git::pack_write::{build_pack_and_idx, crc32, parse_idx_v2, write_idx_v2, IdxEntry, Sha1};
use avec_git::{GitError, Oid};

/// Object count of the fixture table (inside MAX_OBJECTS; small enough to
/// sweep every mutation class with full-query checks).
const N: usize = 1_024;

/// Mirrors idx_read.rs section layout (magic 4 + version 4 + 256*4 fanout).
/// If the on-disk layout drifts, `pristine_baseline_ground_truth` fails on
/// the exact byte length and this court is forced to re-derive.
const FANOUT_OFF: usize = 8;
const OID_TABLE_OFF: usize = FANOUT_OFF + 256 * 4; // 1032

// ------------------------------------------------------------- fixture ----

fn blob(i: usize) -> Vec<u8> {
    let le = (i as u64).to_le_bytes();
    let mut b = Vec::with_capacity(64);
    for _ in 0..8 {
        b.extend_from_slice(&le);
    }
    b
}

/// (idx bytes, sorted (oid, offset) table) from the crate's real writer.
fn fixture() -> (Vec<u8>, Vec<(Oid, u64)>) {
    let blobs: Vec<Vec<u8>> = (0..N).map(blob).collect();
    let refs: Vec<&[u8]> = blobs.iter().map(|b| b.as_slice()).collect();
    let (_pack, idx, entries) =
        build_pack_and_idx(&refs).expect("writer emits pack+idx for 1024 blobs");
    assert_eq!(entries.len(), N);
    // write_idx_v2 sorts entries in place before serialization.
    let table: Vec<(Oid, u64)> = entries.iter().map(|e| (e.oid, e.offset as u64)).collect();
    assert!(
        table.windows(2).all(|w| w[0].0 < w[1].0),
        "writer sorts oids"
    );
    (idx, table)
}

fn true_oids(table: &[(Oid, u64)]) -> Vec<Oid> {
    table.iter().map(|e| e.0).collect()
}

/// One deterministic probe OID per first byte. Probes falling in empty
/// buckets are what let the sweep enter corrupted empty partitions — true
/// OIDs alone cannot. Probes colliding with real rows are dropped.
fn per_byte_probes(table: &[(Oid, u64)]) -> Vec<Oid> {
    let mut out = Vec::new();
    for b in 0..=255u8 {
        let mut o = [0x42u8; 20];
        o[0] = b;
        if table.binary_search_by(|e| e.0.cmp(&o)).is_err() {
            out.push(o);
        }
    }
    out
}

fn query_set(table: &[(Oid, u64)], true_stride: usize) -> Vec<Oid> {
    let mut q: Vec<Oid> = true_oids(table).into_iter().step_by(true_stride).collect();
    q.extend(per_byte_probes(table));
    q
}

// --------------------------------------------------------- mutation util --

fn flipped(base: &[u8], pos: usize, xor: u8) -> Vec<u8> {
    let mut b = base.to_vec();
    b[pos] ^= xor;
    b
}

/// First byte with the largest nonempty bucket among 0..=254 (leaves 255 as
/// the wrap-target neighbor).
fn biggest_bucket(table: &[(Oid, u64)]) -> (usize, u32) {
    let mut counts = [0u32; 256];
    for (o, _) in table {
        counts[o[0] as usize] += 1;
    }
    let mut bstar = 0;
    let mut best = 0u32;
    for (b, &count) in counts.iter().enumerate().take(255) {
        if count > best {
            best = count;
            bstar = b;
        }
    }
    assert!(best >= 1, "fixture must have a nonempty bucket in 0..=254");
    (bstar, best)
}

// ---------------------------------------------------------- observation ----

/// The invariant the ladder guarantees on ANY parse-accepted table: a hit
/// row's bytes equal the query (no false hits), and the returned offset is
/// whatever the table's own offset table says.
fn assert_hit_self_consistent(idx: &IdxV2, t: &Oid, i: u32, off: u64) {
    assert_eq!(
        &idx.oid(i),
        t,
        "FALSE HIT: row {i} bytes != query (ladder eq-mask contract broken)"
    );
    assert_eq!(
        idx.offset(i).expect("offset(i)"),
        off,
        "hit offset != table's own offset at row {i}"
    );
}

/// Run `queries` against one parsed corrupted table, asserting per-query:
/// Hit => self-consistent, Miss => ok, Typed => only TooManyObjects or
/// Truncated (both reachable under a corrupted fanout), Panic => forbidden
/// unless `break_on_panic` (then record + stop and return the message).
fn sweep_and_assert(
    idx: &IdxV2,
    queries: &[Oid],
    break_on_panic: bool,
) -> (u32, u32, u32, Option<String>) {
    let (mut hits, mut misses, mut typed) = (0u32, 0u32, 0u32);
    for t in queries {
        match catch_unwind(AssertUnwindSafe(|| idx.lookup(t))) {
            Ok(Ok(Some((i, off)))) => {
                assert_hit_self_consistent(idx, t, i, off);
                hits += 1;
            }
            Ok(Ok(None)) => misses += 1,
            Ok(Err(GitError::TooManyObjects(_))) | Ok(Err(GitError::Truncated)) => typed += 1,
            Ok(Err(other)) => panic!(
                "unexpected error variant from lookup on a parse-accepted table: {other:?} \
                 (only TooManyObjects/Truncated are reachable)"
            ),
            Err(payload) => {
                let msg = panic_msg(&payload);
                assert!(
                    !break_on_panic,
                    "LOOKUP PANICKED on a parse-accepted table (court forbids): {msg}"
                );
                return (hits, misses, typed, Some(msg));
            }
        }
    }
    (hits, misses, typed, None)
}

fn panic_msg(p: &Box<dyn Any + Send>) -> String {
    if let Some(s) = p.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = p.downcast_ref::<String>() {
        s.clone()
    } else {
        "opaque panic payload".to_string()
    }
}

// --------------------------------------------------------------- tests ----

/// M0: pristine ground truth. Grounds every mirrored offset in this file.
#[test]
fn pristine_baseline_ground_truth() {
    let (bytes, table) = fixture();
    assert_eq!(
        bytes.len(),
        OID_TABLE_OFF + 28 * N + 40,
        "idx byte length drifted from the mirrored layout; re-derive section offsets"
    );
    let idx = IdxV2::parse(&bytes).expect("parse pristine");
    assert_eq!(idx.len() as usize, N);
    idx.verify_idx_checksum().expect("pristine idx checksum");
    assert!(
        parse_idx_v2(&bytes).is_ok(),
        "independent parser accepts pristine"
    );
    for (i, (t, off)) in table.iter().enumerate() {
        match idx.lookup(t).expect("pristine partition in window") {
            Some((found, found_off)) => {
                assert_eq!(found as usize, i, "hit row must equal sorted position");
                assert_eq!(found_off, *off);
            }
            None => panic!("pristine miss on present OID at sorted index {i}"),
        }
    }
    for t in per_byte_probes(&table) {
        assert_eq!(idx.lookup(&t).expect("probes in window"), None);
    }
    println!("RECEIPT pristine: objects={N} all_hits_exact=true probes_miss=true");
}

/// M1-M4: every fanout byte, single-bit class, 1,024 mutations. Asserts:
/// parse errors confined to fanout[255] growth (exactly 4, all Truncated);
/// per-query Hit self-consistent / Miss / typed TooManyObjects|Truncated;
/// OOB panics ONLY in the byte-1 (+2^16) empty-bucket corner — counted and
/// pinned as the live defect, never silent.
#[test]
fn fanout_bit0_sweep_classification() {
    let (base, table) = fixture();
    let queries = query_set(&table, 32); // 32 true OIDs + 256 per-byte probes
    let mut parse_errs = 0u32;
    let mut verify_caught = 0u32;
    let mut typed_lookups = 0u32;
    let mut hit_lookups = 0u32;
    let mut panic_rows: Vec<String> = Vec::new();
    for e in 0..256usize {
        for byi in 0..4usize {
            let pos = FANOUT_OFF + e * 4 + byi;
            let bytes = flipped(&base, pos, 0x01);
            match IdxV2::parse(&bytes) {
                Err(err) => {
                    assert!(
                        e == 255,
                        "parse error outside fanout[255] growth: entry {e}.{byi} -> {err:?}"
                    );
                    assert_eq!(err, GitError::Truncated, "entry {e}.{byi}");
                    parse_errs += 1;
                }
                Ok(idx) => {
                    let (hits, _miss, typed, panic_msg) = sweep_and_assert(&idx, &queries, false);
                    hit_lookups += hits;
                    typed_lookups += typed;
                    if let Some(msg) = panic_msg {
                        assert!(
                            byi == 1,
                            "OOB panic escaped the byte-1 corner: entry {e}.{byi} -> {msg}"
                        );
                        assert!(
                            msg.contains("out of range for slice"),
                            "expected OOB slice panic class, got: {msg}"
                        );
                        panic_rows.push(format!("{e}.{byi}: {msg}"));
                    }
                    assert_eq!(
                        idx.verify_idx_checksum(),
                        Err(GitError::IdxChecksumMismatch),
                        "opt-in checksum must catch every fanout body flip (entry {e}.{byi})"
                    );
                    verify_caught += 1;
                }
            }
        }
    }
    assert_eq!(
        parse_errs, 4,
        "all four fanout[255] byte flips must truncate parse"
    );
    println!(
        "RECEIPT fanout_bit0: mutations=1024 parse_truncated_at_fanout255={parse_errs} \
         verify_caught={verify_caught} typed_lookups={typed_lookups} hit_lookups={hit_lookups} \
         oob_panic_mutations={} (defect, e.g. {:?})",
        panic_rows.len(),
        panic_rows.first()
    );
}

/// M5: fanout entry inflated by exactly 2^16 (byte-1 bit flip). Nonempty
/// bucket: the partition lands just over the window -> typed TooManyObjects
/// carrying the exact inflated partition size. The byte AFTER sees lo > hi
/// (non-monotonic fanout): lookup refuses via `checked_sub` with a typed
/// Err(Truncated). Pinned against the current tree; an earlier revision of
/// lookup relied on release wrap semantics and this row was re-pinned when
/// the substrate changed mid-session.
#[test]
fn fanout_inflation_window_and_inversion_typed_refusals() {
    let (base, table) = fixture();
    let (bstar, bucket) = biggest_bucket(&table);
    let bytes = flipped(&base, FANOUT_OFF + bstar * 4 + 1, 0x01); // +2^16
    let idx = IdxV2::parse(&bytes).expect("parse accepts inflated fanout");
    let lo = if bstar == 0 { 0 } else { idx.fanout(bstar - 1) };
    let hi = idx.fanout(bstar);
    let part = hi - lo;
    assert_eq!(part, 65_536 + bucket, "inflated partition must be exact");
    assert!(part > LADDER_WINDOW);
    let t = table
        .iter()
        .find(|(o, _)| o[0] as usize == bstar)
        .expect("bucket nonempty")
        .0;
    assert_eq!(
        idx.lookup(&t),
        Err(GitError::TooManyObjects(part as usize)),
        "partition > 2^16 under mutation must be a typed refusal"
    );
    // non-monotonic case: the byte AFTER the inflated entry sees lo > hi.
    let j = bstar + 1;
    let lo2 = idx.fanout(j - 1);
    let hi2 = idx.fanout(j);
    assert!(lo2 > hi2, "precondition: fanout inversion at byte {j}");
    let mut probe = [0x42u8; 20];
    probe[0] = j as u8;
    assert_eq!(
        idx.lookup(&probe),
        Err(GitError::Truncated),
        "non-monotonic fanout (lo > hi) must be a typed refusal, never a panic or wrap"
    );
    assert_eq!(
        idx.verify_idx_checksum(),
        Err(GitError::IdxChecksumMismatch)
    );
    println!(
        "RECEIPT fanout_window_inversion: bucket={bstar} part={part} (window={LADDER_WINDOW}) \
         inversion_byte={j} -> Err(Truncated)"
    );
}

/// M6: DEFECT WITNESS — fanout entry inflated by 4,096 (byte-2 ^= 0x10;
/// inside the 2^16 window) on a nonempty bucket. The ladder's first
/// in-bounds probe lands at index lo+4095 >= 1435, past the physical
/// 1,024-row OID table, and the reader panics with a slice range fault in
/// release AND debug. Root cause: IdxV2::parse validates size only against
/// fanout[255]; lookup never clamps probes to the physical table. Pinned so
/// the fix flips this test red — then replace with the typed guarantee.
#[test]
fn fanout_inflation_within_window_oob_panic_defect_witness() {
    let (base, table) = fixture();
    let (bstar, _) = biggest_bucket(&table);
    let bytes = flipped(&base, FANOUT_OFF + bstar * 4 + 2, 0x10); // +4,096
    let idx = IdxV2::parse(&bytes).expect("parse accepts the in-window inflated fanout");
    assert_eq!(
        idx.verify_idx_checksum(),
        Err(GitError::IdxChecksumMismatch)
    );
    let t = table
        .iter()
        .find(|(o, _)| o[0] as usize == bstar)
        .expect("bucket nonempty")
        .0;
    match catch_unwind(AssertUnwindSafe(|| idx.lookup(&t))) {
        Err(payload) => {
            let msg = panic_msg(&payload);
            assert!(
                msg.contains("out of range for slice"),
                "expected the OOB slice panic class, got: {msg}"
            );
            println!(
                "RECEIPT oob_witness: bucket={bstar} pinned panic: {msg} \
                 [DEFECT: idx_read.rs lookup probes unclamped by physical table]"
            );
        }
        Ok(r) => panic!(
            "DEFECT APPEARS FIXED: lookup returned {r:?} on an in-window inflated fanout. \
             Update this witness to assert the new typed behavior and flip matrix rows M4/M6."
        ),
    }
}

/// M7: OID table byte corruption. Strict row: lookup NEVER errors and NEVER
/// panics (fanout intact -> partitions in window; offset reads bounds-guarded),
/// hits are always self-consistent (no false hits), and the opt-in checksum
/// catches every flip. Pristine hits may silently vanish to Miss (counted).
#[test]
fn oid_byte_corruption_never_false_hits() {
    let (base, table) = fixture();
    let queries = true_oids(&table);
    let mut checked = 0u32;
    let mut noop = 0u32;
    let mut vanished_total = 0u32;
    for row in [0usize, 1, 512, N - 1] {
        for byi in [0usize, 10, 19] {
            for op in [0x01u8, 0xFF, 0x00] {
                let pos = OID_TABLE_OFF + row * 20 + byi;
                let mut bytes = base.clone();
                if op == 0x00 {
                    bytes[pos] = 0;
                } else {
                    bytes[pos] ^= op;
                }
                let idx = IdxV2::parse(&bytes).expect("parse must not consult table bytes");
                checked += 1;
                let changed = bytes[pos] != base[pos];
                if !changed {
                    noop += 1;
                }
                let mut vanished = 0u32;
                for t in &queries {
                    // direct call: a panic fails this test loudly (never-panics row)
                    match idx.lookup(t) {
                        Ok(Some((i, off))) => assert_hit_self_consistent(&idx, t, i, off),
                        Ok(None) => {
                            if table.binary_search_by(|e| e.0.cmp(t)).is_ok() {
                                vanished += 1; // pristine hit silently lost
                            }
                        }
                        Err(e) => panic!(
                            "OID corruption must never produce a typed lookup error, got {e:?}"
                        ),
                    }
                }
                vanished_total += vanished;
                if changed {
                    assert_eq!(
                        idx.verify_idx_checksum(),
                        Err(GitError::IdxChecksumMismatch),
                        "opt-in checksum must catch every OID flip"
                    );
                }
            }
        }
    }
    println!(
        "RECEIPT oid_corruption: mutations={checked} (noop_assignments={noop}) \
         vanished_pristine_hits={vanished_total} typed_errors=0 panics=0 false_hits=0"
    );
}

/// M8 (task item 2): sorted-order violation by exchanging two OID rows.
/// The ladder must still terminate in bounds on every input: raw
/// `ladder_search` over the corrupted table returns `b <= N` and
/// `eq != 0 <=> rows[b] == t` for every query; `IdxV2::lookup` never errors,
/// never panics, never false-hits. In debug builds the stage count stays
/// exactly 16 per call. Documented: with an inversion present, present OIDs
/// MAY silently miss and hits MAY relocate to the swapped row's position —
/// structurally self-consistent, semantically wrong; counts are receipts.
#[test]
fn oid_swap_sorted_order_violation_lower_bound_terminates() {
    let (base, table) = fixture();
    let queries = true_oids(&table);
    let mut receipts = Vec::new();
    for (a, b) in [(512usize, 513usize), (10usize, 1_000usize)] {
        let mut bytes = base.clone();
        let (lo_row, hi_row) = (a.min(b), a.max(b));
        let lo_pos = OID_TABLE_OFF + lo_row * 20;
        let hi_pos = OID_TABLE_OFF + hi_row * 20;
        let (left, right) = bytes.split_at_mut(hi_pos);
        left[lo_pos..lo_pos + 20].swap_with_slice(&mut right[0..20]);
        let idx = IdxV2::parse(&bytes).expect("parse must not consult sort order");
        let rows: Vec<[u8; 20]> = (0..N as u32).map(|i| idx.oid(i)).collect();
        assert!(
            rows.windows(2).any(|w| w[0] > w[1]),
            "swap must invert order"
        );
        let mut hits = 0u32;
        let mut lost = 0u32;
        let mut relocated = 0u32;
        for t in &queries {
            // reader path: hit/miss only, never error, never panic
            match idx.lookup(t) {
                Ok(Some((i, off))) => {
                    assert_hit_self_consistent(&idx, t, i, off);
                    hits += 1;
                    match queries.binary_search(t) {
                        Ok(orig) if i as usize != orig => relocated += 1,
                        _ => {}
                    }
                }
                Ok(None) => lost += 1,
                Err(e) => panic!("swap must never produce a typed error: {e:?}"),
            }
            // raw ladder path: bounded termination + exact eq contract
            #[cfg(debug_assertions)]
            let before = avec_git::ladder::debug_probes();
            let (lb, eq) = ladder_search(&rows, t);
            #[cfg(debug_assertions)]
            assert_eq!(
                avec_git::ladder::debug_probes() - before,
                16,
                "stage constancy must survive sorted-order violation"
            );
            assert!(lb as usize <= N, "lower bound escaped [0, N]: {lb}");
            assert_eq!(
                eq != 0,
                rows.get(lb as usize) == Some(t),
                "eq mask must hold exactly at the returned bound"
            );
        }
        assert_eq!(
            idx.verify_idx_checksum(),
            Err(GitError::IdxChecksumMismatch),
            "opt-in checksum must catch a swapped row pair"
        );
        receipts.push(format!(
            "swap({a},{b}) hits={hits} lost={lost} relocated={relocated}"
        ));
    }
    println!("RECEIPT oid_swap: {}", receipts.join("; "));
}

/// M9: CRC table corruption is invisible to lookup (the per-entry CRC fields
/// are never consulted on the lookup path). Results must be IDENTICAL to the
/// pristine baseline for the full query set; `crc()` reads the corrupted
/// bytes; only the opt-in checksums notice.
#[test]
fn crc_corruption_invisible_to_lookup() {
    let (base, table) = fixture();
    let queries = query_set(&table, 1);
    let pristine = IdxV2::parse(&base).expect("parse pristine");
    let crc_off = OID_TABLE_OFF + 20 * N;
    let mut checked = 0u32;
    let mut noop = 0u32;
    for row in [0usize, 512, N - 1] {
        for byi in [0usize, 3] {
            for op in [0x01u8, 0x00] {
                let pos = crc_off + row * 4 + byi;
                let mut bytes = base.clone();
                if op == 0x00 {
                    bytes[pos] = 0;
                } else {
                    bytes[pos] ^= op;
                }
                let idx = IdxV2::parse(&bytes).expect("parse must not consult CRC bytes");
                checked += 1;
                let changed = bytes[pos] != base[pos];
                if !changed {
                    noop += 1;
                }
                for t in &queries {
                    match idx.lookup(t) {
                        Ok(now) => {
                            let was = pristine.lookup(t).expect("pristine in window");
                            assert_eq!(now, was, "CRC corruption changed lookup for a probe OID");
                        }
                        Err(e) => panic!("CRC corruption must never error lookup: {e:?}"),
                    }
                }
                let want = u32::from_be_bytes(
                    bytes[crc_off + row * 4..crc_off + row * 4 + 4]
                        .try_into()
                        .expect("4"),
                );
                assert_eq!(idx.crc(row as u32), want, "crc() must read live bytes");
                if changed {
                    assert_eq!(
                        idx.verify_idx_checksum(),
                        Err(GitError::IdxChecksumMismatch),
                        "opt-in checksum must catch every CRC flip"
                    );
                    assert_eq!(
                        parse_idx_v2(&bytes),
                        Err(GitError::IdxChecksumMismatch),
                        "independent parser must reject a corrupted idx outright"
                    );
                }
            }
        }
    }
    println!(
        "RECEIPT crc_corruption: mutations={checked} (noop_assignments={noop}) \
         baseline_deltas=0 typed_errors=0 panics=0"
    );
}

/// M10: offset table corruption — the silent wrong-offset class, re-derived
/// 2026-10-31 after the `offset()` escape fix (full-width u64 MSB select +
/// typed `Err(Truncated)` on a corrupt escape). LSB and MSB-byte flips stay
/// SILENT: the ladder still hits the SAME row (OID table intact) and returns
/// whatever the corrupted offset table says — lookup verifies nothing.
/// MSB bit-7 flips are the escape class: an escape whose index is inside the
/// physical large-offset table resolves to that exact 64-bit entry
/// (self-consistent hit); an escape past it is the typed Err(Truncated)
/// refusal — never the old silent `Ok(0)`, never a panic. This fixture has
/// NO large table and every pristine offset >= 12 (the pack header), while
/// in-range escapes need index <= 4 (`large_base 29704 + (idx+1)*8 <= len
/// 29744`) — so all three escape mutations here MUST be typed refusals.
#[test]
fn offset_corruption_silent_wrong_offset() {
    let (base, table) = fixture();
    let off_off = OID_TABLE_OFF + 24 * N;
    let mut checked = 0u32;
    let mut escape_refusals = 0u32;
    for row in [0usize, 512, N - 1] {
        let pristine_off = table[row].1;
        for (name, mutate) in [
            ("lsb_xor01", 0u8),
            ("lsb_set_ff", 1u8),
            ("msb_xor01", 2u8),
            ("bit31_escape", 3u8),
        ] {
            let mut bytes = base.clone();
            match mutate {
                0 => bytes[off_off + row * 4 + 3] ^= 0x01, // LSB +-1
                1 => bytes[off_off + row * 4 + 3] = 0xFF,  // LSB forced
                2 => bytes[off_off + row * 4] ^= 0x01,     // MSB bit0: +2^24
                _ => bytes[off_off + row * 4] ^= 0x80,     // MSB bit7: bit-31 escape
            }
            let idx = IdxV2::parse(&bytes).expect("parse must not consult offset bytes");
            let t = table[row].0;
            checked += 1;
            match idx.lookup(&t) {
                Ok(Some((i, off))) => {
                    assert_eq!(i as usize, row, "hit row must survive offset corruption");
                    match name {
                        "bit31_escape" => {
                            // In-range escape (unreachable in this fixture —
                            // the assertion below pins that): the hit stays
                            // exact and self-consistent with offset().
                            let expect = idx.offset(i).expect("offset(i) in bounds");
                            assert_eq!(off, expect, "escape must resolve self-consistently");
                            println!("RECEIPT bit31 escape row {row}: in-range offset={off}");
                        }
                        "msb_xor01" => {
                            assert_eq!(off, pristine_off + 0x0100_0000, "+2^24 must be additive");
                        }
                        _ => {
                            assert_ne!(off, pristine_off, "LSB flip must change the offset");
                            let want = u32::from_be_bytes(
                                bytes[off_off + row * 4..off_off + row * 4 + 4]
                                    .try_into()
                                    .expect("4"),
                            );
                            assert_eq!(off, want as u64, "offset must be the live corrupted bytes");
                        }
                    }
                }
                Ok(None) => panic!("offset corruption must not lose the hit"),
                // Re-derived row: out-of-range escape -> typed refusal. Only
                // this class may error; every silent class must still answer.
                Err(GitError::Truncated) if name == "bit31_escape" => escape_refusals += 1,
                Err(e) => panic!("offset corruption must never error lookup: {e:?}"),
            }
            assert_eq!(
                idx.verify_idx_checksum(),
                Err(GitError::IdxChecksumMismatch),
                "opt-in checksum must catch every offset flip"
            );
            assert_eq!(
                parse_idx_v2(&bytes),
                Err(GitError::IdxChecksumMismatch),
                "independent parser must reject corrupted offsets outright"
            );
        }
    }
    assert_eq!(
        escape_refusals, 3,
        "every bit-31 escape in this no-large-table fixture must be the typed \
         Truncated refusal; if the fixture grew an in-range escape, re-derive \
         this row (see the M10 doc arithmetic)"
    );
    println!(
        "RECEIPT offset_corruption: mutations={checked} \
         wrong_offsets_silent={} escape_typed_refusals={escape_refusals} panics=0",
        checked - escape_refusals
    );
}

/// M11: trailer corruption (pack checksum at len-40, idx checksum at len-20).
/// Neither region is on the lookup path: results IDENTICAL to pristine.
/// Both regions sit inside verify_idx_checksum's covered body (len-20).
#[test]
fn trailer_corruption_detection() {
    let (base, table) = fixture();
    let queries = query_set(&table, 1);
    let pristine = IdxV2::parse(&base).expect("parse pristine");
    let len = base.len();
    let mut checked = 0u32;
    for region_off in [len - 40, len - 20] {
        for byi in [0usize, 7, 19] {
            for op in [0x01u8, 0xFF] {
                let bytes = flipped(&base, region_off + byi, op);
                let idx = IdxV2::parse(&bytes).expect("parse must not verify checksums");
                checked += 1;
                for t in &queries {
                    match idx.lookup(t) {
                        Ok(now) => {
                            let was = pristine.lookup(t).expect("pristine in window");
                            assert_eq!(
                                now, was,
                                "trailer corruption changed lookup for a probe OID"
                            );
                        }
                        Err(e) => panic!("trailer corruption must never error lookup: {e:?}"),
                    }
                }
                assert_eq!(
                    idx.verify_idx_checksum(),
                    Err(GitError::IdxChecksumMismatch),
                    "checksum byte flip at {} must trip the verifier",
                    region_off + byi
                );
                assert_eq!(
                    parse_idx_v2(&bytes),
                    Err(GitError::IdxChecksumMismatch),
                    "independent parser must reject trailer corruption"
                );
            }
        }
    }
    println!(
        "RECEIPT trailer_corruption: mutations={checked} baseline_deltas=0 \
              verify_caught={checked} panics=0"
    );
}

/// M12: header bytes are parse-gated. Any magic or version byte flip is
/// rejected outright with BadMagic before any table is read.
#[test]
fn header_corruption_typed_rejection() {
    let (base, _table) = fixture();
    for pos in [0usize, 2, 4, 7] {
        for op in [0x01u8, 0xFF] {
            let bytes = flipped(&base, pos, op);
            match IdxV2::parse(&bytes) {
                Err(GitError::BadMagic) => {}
                other => panic!("header flip at {pos} must be BadMagic, got {other:?}"),
            }
        }
    }
    println!("RECEIPT header_corruption: mutations=8 all BadMagic");
}

/// M13 (task item 3): the 2^16 boundary, re-asserted on this court's own
/// footing. (a) The public writer refuses 65,537 entries outright. (b) The
/// writer accepts exactly 65,536 and the ladder handles a full-window
/// partition (inclusive boundary) with exact hits. (c) A hand-serialized
/// 65,537-row table parses and lookup refuses with the exact partition size.
#[test]
fn too_many_objects_writer_cap_and_reader_partition_reassert() {
    assert_eq!(LADDER_WINDOW, 65_536);
    assert_eq!(avec_git::pack_write::MAX_OBJECTS, 65_536);

    // (a) writer cap
    let over: Vec<IdxEntry> = (0..=65_536usize)
        .map(|i| IdxEntry {
            oid: zero_prefix_oid(i),
            offset: 12,
            crc: 0,
        })
        .collect();
    match write_idx_v2(&mut over.clone(), &[0u8; 20]) {
        Err(GitError::TooManyObjects(n)) => assert_eq!(n, 65_537),
        other => panic!(
            "writer must refuse >2^16 entries, got {:?}",
            other.map(|_| ())
        ),
    }

    // (b) inclusive boundary: 65,536 rows all in bucket 0 — writer accepts,
    // reader ladder runs the full window and hits exactly
    let at: Vec<IdxEntry> = (0..65_536usize)
        .map(|i| IdxEntry {
            oid: zero_prefix_oid(i),
            offset: (i * 12) as u32,
            crc: crc32(&zero_prefix_oid(i)),
        })
        .collect();
    let idx_bytes = write_idx_v2(&mut at.clone(), &[0u8; 20]).expect("writer admits exactly 2^16");
    let idx = IdxV2::parse(&idx_bytes).expect("parse at window");
    for k in [0usize, 32_768, 65_535] {
        let (found, off) = idx
            .lookup(&at[k].oid)
            .expect("full-window partition is inside the inclusive boundary")
            .expect("hit");
        assert_eq!(found as usize, k);
        assert_eq!(off, (k * 12) as u64);
    }
    assert_eq!(
        idx.lookup(&zero_prefix_oid(65_536)).expect("in window"),
        None,
        "a miss still runs the ladder over the full window"
    );

    // (c) reader partition cap on a hand-serialized 65,537-row table (the
    // public writer refuses to build it, by design)
    let over_table: Vec<(Oid, u64)> = (0..=65_536usize)
        .map(|i| (zero_prefix_oid(i), (i * 12) as u64))
        .collect();
    let bytes = serialize_idx(&over_table);
    let idx = IdxV2::parse(&bytes).expect("parse of oversized table");
    assert_eq!(idx.len(), 65_537);
    assert_eq!(
        idx.lookup(&over_table[0].0),
        Err(GitError::TooManyObjects(65_537)),
        "reader must refuse a partition exceeding the 2^16 window"
    );
    println!(
        "RECEIPT too_many_objects: writer_refuses_65537=yes writer_admits_65536=yes \
         reader_refuses_65537_partition=yes"
    );
}

fn zero_prefix_oid(i: usize) -> Oid {
    let mut o = [0u8; 20];
    o[1..5].copy_from_slice(&(i as u32).to_be_bytes());
    o[5] = 0xAA;
    o
}

/// Minimal local serializer mirroring the writer's §1 layout — used only for
/// the >cap reader court where `write_idx_v2` refuses by design.
fn serialize_idx(entries: &[(Oid, u64)]) -> Vec<u8> {
    let n = entries.len();
    let mut out = Vec::with_capacity(OID_TABLE_OFF + 28 * n + 40);
    out.extend_from_slice(&[0xFF, 0x74, 0x4F, 0x63]);
    out.extend_from_slice(&2u32.to_be_bytes());
    let mut fanout = [0u32; 256];
    for (o, _) in entries {
        fanout[o[0] as usize] += 1;
    }
    for i in 1..256 {
        fanout[i] += fanout[i - 1];
    }
    for f in fanout {
        out.extend_from_slice(&f.to_be_bytes());
    }
    for (o, _) in entries {
        out.extend_from_slice(o);
    }
    for (o, _) in entries {
        out.extend_from_slice(&crc32(o).to_be_bytes());
    }
    for (_, off) in entries {
        out.extend_from_slice(&(*off as u32).to_be_bytes());
    }
    out.extend_from_slice(&[0u8; 20]); // pack checksum placeholder
    let mut h = Sha1::new();
    h.update(&out);
    out.extend_from_slice(&h.finalize());
    out
}
