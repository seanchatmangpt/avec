# Tutorial: hash bytes and write a git packfile

This tutorial chains two cgap kernels: you will hash a payload with
`cgap-blake3`, then write a real Git packfile v2 containing that payload as a
blob with `cgap-git` — and verify the pack with `cgap-git`'s independent
parser and with real `git index-pack`. The working program lives in a scratch
crate at `/tmp/cgap-tut02`; everything you build here runs end to end.

By the end you will have:

- A 32-byte BLAKE3 digest of your payload.
- A conformant `sample.pack` / `sample.idx` pair on disk (92 + 1100 bytes).
- Proof the pack is real: `git index-pack` accepts it and `git cat-file`
  recovers your original bytes.

Prerequisites: Rust (cargo) and git on your PATH.

## Step 1: create the scratch crate

cgap crates are plain path dependencies — no registry, no patching.

```console
$ mkdir -p /tmp/cgap-tut02/src
$ cd /tmp/cgap-tut02
```

Create `Cargo.toml`:

```toml
[package]
name = "cgap-tut02"
version = "0.1.0"
edition = "2021"

[dependencies]
cgap-blake3 = { path = "/Users/sac/cgap/crates/cgap-blake3" }
cgap-git = { path = "/Users/sac/cgap/crates/cgap-git" }
```

Adjust the two `path` values if your cgap checkout lives elsewhere.

## Step 2: write the program

Create `src/main.rs`. Note one API detail: `cgap-git` is a `#![no_std]` crate
with `alloc`, so `GitError` does not implement `std::error::Error` — we map
errors to `String` instead of using `Box<dyn Error>`.

```rust
use cgap_blake3::hash;
use cgap_git::pack_write::{build_pack_and_idx, parse_idx_v2, parse_pack_v2};

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn main() -> Result<(), String> {
    // 1. The payload: a git blob's raw content.
    let content = b"Hello, cgap! This blob will be hashed and packed.\n";

    // 2. Hash it with cgap-blake3.
    let digest = hash(content);
    println!("blake3 = {}", hex(&digest));

    // 3. Write a git packfile (v2) + idx containing the blob.
    let (pack, idx, entries) = build_pack_and_idx(&[content.as_slice()])
        .map_err(|e| e.to_string())?;
    std::fs::write("/tmp/cgap-tut02/sample.pack", &pack).map_err(|e| e.to_string())?;
    std::fs::write("/tmp/cgap-tut02/sample.idx", &idx).map_err(|e| e.to_string())?;
    println!("pack bytes = {}", pack.len());
    println!("idx bytes  = {}", idx.len());

    // 4. Verify: parse both back and cross-check.
    let parsed = parse_pack_v2(&pack).map_err(|e| e.to_string())?;
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].obj_type, 3); // OBJBlob
    assert_eq!(parsed[0].size, content.len() as u64);
    let (oids, crcs, offs) = parse_idx_v2(&idx).map_err(|e| e.to_string())?;
    assert_eq!(oids.len(), 1);
    assert_eq!(offs[0], entries[0].offset);
    assert_eq!(crcs[0], entries[0].crc);
    println!(
        "verified: 1 blob object, type 3, size {}, offset {}",
        content.len(),
        offs[0]
    );
    Ok(())
}
```

## Step 3: run it

```console
$ cargo run
   Compiling cgap-blake3 v0.1.0
   Compiling cgap-git v0.1.0
   Compiling cgap-tut02 v0.1.0
    Finished `dev` profile [unoptimized + debuginfo] target(s)
     Running `target/debug/cgap-tut02`
blake3 = 48a97b95ef917709bbac04de044b901d650b0a2d9bb50948921a26cd69e9acbd
pack bytes = 92
idx bytes  = 1100
verified: 1 blob object, type 3, size 50, offset 12
$ echo $?
0
```

What happened:

- `cgap_blake3::hash` returned the 32-byte BLAKE3 digest of the 50-byte
  payload. BLAKE3 is content-addressed and deterministic — the same bytes
  always produce `48a97b95...acbd`.
- `build_pack_and_idx` emitted a pack v2 (`PACK`, version 2, 1 object: a
  varint header with type 3 (blob) + zlib-compressed body + SHA-1 trailer)
  and an idx v2 (fan-out table, sorted OIDs, CRC32s, offsets, checksums).
  `92 = 12 (header) + object bytes + 20 (trailer)`.

Note the two hashes answer different questions: BLAKE3 (`cgap-blake3`) is the
content fingerprint; the object name in the idx is a SHA-1 over
`"blob 50\0<content>"` — git's own naming rule, computed inside `cgap-git`.

## Step 4: verify with real git

The pack is not "pack-shaped bytes" — it is a pack. Prove it:

```console
$ mkdir -p /tmp/cgap-tut02/vgit && cd /tmp/cgap-tut02/vgit
$ git init -q
$ git index-pack --stdin < ../sample.pack
pack	eeb7a0f8972e732ff212e0b3deeecc60de3b9bc5
$ git cat-file -p 129edad27c6e8b4a209447cb4ef663ccc59c5a95
Hello, cgap! This blob will be hashed and packed.
```

`git index-pack` built an object store from your pack (exit 0, no warnings),
and `git cat-file` recovered the exact original bytes under the OID real git
computes for `blob 50`. This is the same round-trip `cgap-git`'s in-crate
`parse_pack_v2`/`parse_idx_v2` performed in step 4 of the program — two
independent readers agreeing on one artifact.

## Step 5: where to go next

- Hash larger payloads: `cgap_blake3::hash` handles multi-chunk inputs with a
  SIMD Merkle reduction; `hash_wide_simd` and `hash_many_simd` are
  byte-identical alternates. `cgap-blake3`'s tests pin official BLAKE3
  vectors.
- Pack many blobs: pass a slice of `&[u8]` to `build_pack_and_idx` (up to
  `cgap_git::pack_write::MAX_OBJECTS` = 65,536 objects per pack).
- Inspect an existing pack: `parse_pack_v2` returns type/size/offset/CRC per
  object; `parse_idx_v2` returns the sorted OID, CRC, and offset tables and
  rejects corrupted trailers with `GitError::PackChecksumMismatch` /
  `GitError::IdxChecksumMismatch`.
