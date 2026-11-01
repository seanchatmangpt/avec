# How-to: hash, packfile, and parquet recipes

Working recipes for the `cgap-blake3`, `cgap-git`, and `cgap-parquet` kernels,
strictly from the public APIs in `crates/*/src`. Every snippet was compiled and
run against this checkout (path deps, 2026-10-31); per-recipe commands and
exits are listed with each recipe.

## Hash a large file with the SIMD tree-hash paths

`cgap-blake3` has no incremental/streaming hasher — `hash`, `hash_wide_simd`,
and `hash_many_simd` all take the whole input as one `&[u8]`. Read the file
into memory, then call any of the three: they are byte-identical, so the SIMD
paths are drop-in upgrades.

```rust
use std::fs;

fn main() -> std::io::Result<()> {
    let path = std::env::args().nth(1).expect("usage: r1_large_file <path>");
    let meta = fs::metadata(&path)?;
    let size = meta.len() as usize;
    if size > 64 * 1024 * 1024 {
        return Err(std::io::Error::other("file exceeds the 64 MiB recipe cap"));
    }
    let data = fs::read(&path)?;
    debug_assert_eq!(data.len(), size);

    // Reference digest over the whole slice (0..=1024 bytes: single-chunk root).
    let digest = cgap_blake3::hash(&data);

    // Level-batched 64-chunk SIMD parent reduction; byte-identical to hash().
    let wide = cgap_blake3::hash_wide_simd(&data);

    // Leaf kernels: 8-chunks-at-a-time NEON/AVX2 batch, byte-identical again.
    let many = cgap_blake3::hash_many_simd(&data);

    assert_eq!(digest, wide);
    assert_eq!(digest, many);
    for b in digest { print!("{b:02x}"); }
    println!();
    Ok(())
}
```

Verified: `cargo run --bin r1_large_file -- /tmp/cgap-howto-1m.bin` (1 MiB
file) printed
`488de202f73bd976de4e7048f4e1f39a776d86d582b7348ff53bf432b987fca8`,
exit 0.

## Choose the right hash entry point per payload size

Two thresholds govern dispatch:

- `is_small_batch_eligible(len)` — true for 4..=64 whole 1 KiB chunks
  (4 KiB..=64 KiB, whole-chunk length, chunk count a multiple of 4); this is
  the window where the leaf SIMD kernel (`hash_many_simd`) is routed in
  automatically.
- `should_dispatch_gpu(len)` — true at `>= GPU_CROSSOVER_BYTES`
  (16 MiB), where batched GPU dispatch amortizes its launch overhead.

```rust
use cgap_blake3::{GPU_CROSSOVER_BYTES, is_small_batch_eligible, should_dispatch_gpu};

fn main() {
    for len in [512usize, 4 * 1024, 64 * 1024, 16 * 1024 * 1024] {
        let gpu = should_dispatch_gpu(len);
        let small = is_small_batch_eligible(len);
        println!("len={len} small_batch={small} gpu_dispatch={gpu}");
    }
    assert_eq!(GPU_CROSSOVER_BYTES, 16 * 1024 * 1024);
    assert!(!is_small_batch_eligible(4 * 1024 - 1));
    assert!(is_small_batch_eligible(4 * 1024));
    assert!(!should_dispatch_gpu(GPU_CROSSOVER_BYTES - 1));
    assert!(should_dispatch_gpu(GPU_CROSSOVER_BYTES));
    println!("ok");
}
```

Target selection is compile-time, not runtime:

- `aarch64`: NEON is implied — `multi_chunk::compress_chunks_4way` and the
  4-way parent kernel are always available.
- `x86_64`: the AVX2 8-way parent kernel in `reduce_level_simd` only engages
  when `target_feature = "avx2"` is enabled at compile time
  (`#[cfg(target_feature = "avx2")]`); otherwise the scalar tail runs.

Verified: `cargo run --bin r2_simd_choice` — `ok`, exit 0.

## Write and verify a packfile index

One call builds the pack, the `.idx`, and the per-object
`(oid, offset, crc)` table; the crate's independent parser verifies both
artifacts and the constant-time ladder looks up every OID. Note that
`build_pack_and_idx` sorts the returned entry table by OID (the writer sorts
in place), so entry order is idx order, not blob order.

```rust
use cgap_git::idx_read::IdxV2;
use cgap_git::pack_write::{build_pack_and_idx, parse_pack_v2};

fn main() -> Result<(), cgap_git::GitError> {
    let blobs: Vec<Vec<u8>> = (0..16u32)
        .map(|i| (0..256u32).map(|j| ((j * 17 + i * 37 + 1) % 256) as u8).collect())
        .collect();
    let refs: Vec<&[u8]> = blobs.iter().map(|b| b.as_slice()).collect();

    // One pass: pack bytes, idx bytes, per-object (oid, offset, crc).
    let (pack, idx, entries) = build_pack_and_idx(&refs)?;
    println!(
        "pack={} bytes, idx={} bytes, objects={}",
        pack.len(),
        idx.len(),
        entries.len()
    );

    // Independent read-back: verify the pack trailer SHA-1 and every body.
    let parsed = parse_pack_v2(&pack)?;
    assert_eq!(parsed.len(), blobs.len());

    // Parse the idx, verify its own trailer, then ladder-look-up each OID.
    let index = IdxV2::parse(&idx)?;
    index.verify_idx_checksum()?;
    for (i, e) in entries.iter().enumerate() {
        let (found, offset) = index.lookup(&e.oid)?.expect("oid present");
        assert_eq!(found as usize, i);
        assert_eq!(offset, e.offset as u64);
        assert_eq!(index.crc(found), e.crc);
    }
    println!("ok");
    Ok(())
}
```

Verified: `cargo run --bin r3_packfile` —
`pack=4512 bytes, idx=1520 bytes, objects=16` then `ok`, exit 0.

## Grow a snapshot incrementally with PackBuilder

`PackBuilder` mirrors the spec's append-then-regenerate flow: `push` blobs in
any order, `finish` regenerates pack + idx from the full pending set.

```rust
use cgap_git::pack_write::{PackBuilder, parse_pack_v2};

fn main() -> Result<(), cgap_git::GitError> {
    let mut builder = PackBuilder::new();
    for chunk in 0..5u32 {
        let blob: Vec<u8> = (0..128u32).map(|j| (chunk * 128 + j) as u8).collect();
        builder.push(&blob);
    }
    assert_eq!(builder.len(), 5);

    // Regenerate pack + idx from the full pending set on each finish() call.
    let (pack, idx) = builder.finish()?;

    let parsed = parse_pack_v2(&pack)?;
    assert_eq!(parsed.len(), 5);
    println!("pack={} bytes, idx={} bytes, ok", pack.len(), idx.len());
    Ok(())
}
```

Verified: `cargo run --bin r4_builder` — `pack=750 bytes, idx=1212 bytes, ok`,
exit 0.

## Decode a Parquet data page into a cgap-arrow buffer

The bridge is the documented consumption pattern, not a dedicated function:
`decode_page_into` writes lanes in place into caller-owned memory, and
`cgap_arrow::buffer::Buf` supplies the 64-byte aligned owner. `Buf` has a
typed read view (`as_typed::<u32>`) but no typed mutable view, so a `&mut`
consumer rebinds the aligned byte window as u32 lanes (the same rebind the
crate's own wiring court writes in `crates/cgap-parquet/src/rle.rs`).

```rust
use cgap_arrow::buffer::Buf;
use cgap_parquet::page::{decode_page_into, write_data_page_v2};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let values: Vec<u32> = (0..64u32).map(|i| (i * 31 + 5) & 0xFF).collect();
    let page = write_data_page_v2(&values, 8, values.len() as u32);

    // 64-byte aligned caller-owned buffer (charter M-03); u32 lanes fit 1:1.
    let buf = Buf::new(values.len() * 4);

    // Buf has no typed mutable view, so rebind the (aligned, sized) byte
    // window as u32 lanes — the same rebind every &mut consumer writes.
    // SAFETY: 64-byte base + len a multiple of 4 => aligned for u32.
    let lanes: &mut [u32] = unsafe {
        core::slice::from_raw_parts_mut(buf.as_mut_ptr().cast::<u32>(), values.len())
    };

    // Decode lanes in place: no intermediate Vec, untouched tail preserved.
    let n = decode_page_into(&page, lanes)?;
    assert_eq!(n, values.len());

    // Verify through the typed read view.
    assert_eq!(buf.as_typed::<u32>(), values.as_slice());
    println!("decoded {} lanes into a 64-byte-aligned Buf", n);
    Ok(())
}
```

Bounds on this recipe, stated honestly: `cgap-parquet` implements a minimal
`DataPageV2`-style framing only (RLE encoding, uncompressed, zero level-byte
lengths — see the module docs in `crates/cgap-parquet/src/page.rs`); pages
outside that shape are typed refusals
(`ParquetError::UnsupportedBitWidth`/`UnexpectedEof`), not misparses. Bit
widths are the SWAR kernel set `{1, 2, 4, 8}`.

Verified: `cargo run --bin r5_parquet_buf` —
`decoded 64 lanes into a 64-byte-aligned Buf`, exit 0.
