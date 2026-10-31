# avec

avec is a freestanding columnar substrate built on one thesis: Apache Arrow (in-memory
buffers), Apache Parquet DataPageV2 (encoded pages), and Git Packfile v2 (content-addressed
objects) are three physical renderings of the same byte continuum. One layout discipline —
64-byte-aligned buffers, LSB-first validity bitmaps, branch-free inner loops at Cyclomatic
Complexity M=1 — drives all three, and one receipt — a sovereign zero-dependency BLAKE3 tree
hash over the content-addressed object graph — binds them. Every claim on this page is
receipt-grounded: a command, an exit code, and a minted receipt under
`docs/jira/v26.10.31/receipts/`, never prose alone.

What actually ships (2026-10-31, HEAD `ead4032`, numbers re-read from receipts on disk):
twelve crates, 661 tests green with 0 failed and 2 ignored
(`docs/jira/v26.10.31/63-test-census.md`), the 10-gate verification ladder at 10/10 PASS
(`docs/jira/v26.10.31/verify-latest.md`), a real-git fsck acceptance court
(`crates/avec-git/tests/fsck_court.rs` feeds an avec-git-written pack to the real `git`
binary — `git fsck --full`, `verify-pack -v`, `cat-file -p` — with a corruption
anti-vacuity control), a real-BEAM NIF membrane (`crates/avec-beam`, bridge-contract
tests, receipt `gate-beam-bridge-v2` ALIVE), and a SHACL-conformant ontology with an
anti-vacuity perturbation court (`scripts/validate_ontology.sh`, receipt
`gate-shacl-v3` ALIVE).

The honest red: CTQ-06 (BLAKE3 throughput) measures 1.85 GiB/s/core on NEON
(`hash_many_simd`, 256 MiB min-of-5; cpk study n=100, mean 1.846241, CV 0.57%) against the
unratified 8.5 design target. The 8.5 limit is refuted on NEON — verdict of record:
"The 8.5 LSL as written is unsatisfiable; it is a spec error, not an implementation gap"
(`48-lsl-rederivation.md` section 3). The proposed replacement LSL is 1.0 GiB/s/core,
which 1.85 clears by +85%; ratification is an operator-only flip
(`const LSL_RATIFIED: Option<f64> = None` in `crates/bench/src/bin/ctq_report.rs`) and is
pending. Decision package: `docs/jira/v26.10.31/59-ratification-package.md`.

## The twelve crates

| crate | one-liner |
|---|---|
| `avec-arrow` | in-memory substrate (`no_std`, zero deps): 64 B-aligned `Buf`, SWAR, C ABI |
| `avec-parquet` | Parquet kernels (`no_std`, zero deps): RLE, FFOR, level-free, DataPageV2 |
| `avec-git` | Git Packfile v2 writer: stored blobs, `.idx` v2, in-crate SHA-1/CRC32/DEFLATE |
| `avec-blake3` | sovereign `no_std` BLAKE3 tree hash; 4-way NEON / 8-way AVX2 parent reduction |
| `avec-gpu` | Apple Silicon Metal unified-memory zero-copy bridge (`MetalSharedBuffer`) |
| `avec-ffi` | C Data Interface cdylib membrane (Gate 3 wire parity) |
| `avec-beam` | BEAM/AtomVM NIF membrane: GC-pinned resource binaries, POSIX SHM SPSC ring |
| `audit` | CC=1 branch-audit cdylib forcing kernels into the artifact |
| `bench` | gate harness: line-rate benches, alloc interception, `ctq_report` / `gage_rr` |
| `cli` | `avec-cli` verbs + `verify_v26` machine report + `status` dashboard bin |
| `integration` | cross-crate tests: C-ABI roundtrip, alignment law, admission e2e |
| `wasm-footprint` | `no_std` cdylib link glue; footprint subject of `scripts/footprint.sh` |

Scoping that keeps the claims honest: "branchless" always means compiled inner-loop
bodies branch-free — latches, dispatch, bounds checks, and fail paths are control flow,
itemized per symbol by `make branch-audit` (register of record:
`docs/BRANCH_REGISTER.md`). `avec-parquet` is minimal DataPageV2 page framing, not the
full Parquet format: no row groups, no footer, no compression.

## The triad

```text
        in-memory              page-encoded            content-addressed
   +----------------+      +----------------+     +----------------+
   |     ARROW      | ---> |    PARQUET     | --> |      GIT       |
   | 64B-aligned    |      | DataPageV2     |     | Packfile v2    |
   | Buf + bitmap   |      | RLE/bit-pack   |     | + .idx v2      |
   +----------------+      +----------------+     +----------------+
            ^                      ^                      ^
            |                      |                      |
            +----------------------+----------------------+
                 one BLAKE3 content receipt binds all three
              (avec-blake3 over the content-addressed graph)
```

## Quickstart

Five commands. Full walkthrough: `docs/GETTING_STARTED.md` (pending — wave-10 docs
lanes in flight; until it lands, this section is the walkthrough).

```console
$ cargo build --workspace          # build all 12 crates
$ make test                        # cargo test --workspace
$ just verify                      # scripts/verify_v26.10.31.sh — the 10-gate ladder
$ just status                      # crates/cli/src/bin/status.rs — milestone dashboard
$ just doctor                      # PENDING BIN — doctor.rs not on disk yet (wave-10)
```

## Milestone status

v26.10.31 PARTIAL_ALIVE. Ladder 10/10 PASS
(`docs/jira/v26.10.31/verify-latest.md`, 2026-10-31). Three operator decisions are open:
(a) ratify the CTQ-06 LSL, (b) ratify the CTQ-04 p99/p50 protocol, (c) approve the
integration commit — the whole milestone is one uncommitted diff
(`docs/jira/v26.10.31/65-coordinator-handoff.md`). Consolidated decision register:
`docs/OPERATOR_DECISIONS.md` (pending lane 16 — not yet on disk).
Observed drift: the generated version constant still reads 26.10.29
(`ontology/avec.ttl`, `crates/avec-arrow/src/generated_version.rs`) while the milestone
is named v26.10.31.

## License

MIT for the source code; use/execution of the substrate is governed by the
P28 Sovereign Invariant License (P28-SIL v2.1.0,
`urn:avec:license:p28-sil:v2.1.0-sovereign`): $50M universal safe harbor,
covenant not to sue upon commercial regularization, ASC 450 / SOX
302/404 evidentiary enforcement. See [LICENSE.md](LICENSE.md) and
[LICENSE-P28-SIL.md](LICENSE-P28-SIL.md).

## Documentation map

- `docs/GETTING_STARTED.md` — walkthrough (pending, wave-10 docs lanes; not yet on disk)
- `docs/TROUBLESHOOTING.md` — troubleshooting guide (pending lane 9; not yet on disk)
- `docs/diataxis/INDEX.md` — Diataxis suite (tutorials / how-to / reference /
  explanation); docs router landing pending lane 10
- `docs/FEATURE_MATRIX.md` — feature matrix (pending lane 13; not yet on disk)
- `docs/jira/v26.10.31/00-index.md` — milestone corpus hub
- `docs/DFLSS_CHARTER.md` — CTQ register and verification ladder of record
- `docs/CONTENT_ADDRESSING.md` — digest surface: what the fold claims and refuses

## Footprint

`wasm_footprint.wasm` cdylib: 4,309 B stripped, identical on `wasm32-unknown-unknown`
and `wasm32-wasip1`, from the standalone-manifest build (`crates/wasm-footprint` has its
own lockfile and is deliberately not a root-workspace member). Receipt:
`crates/wasm-footprint/NOTES-v26.10.31.md`. This corrects the earlier README cite of
`docs/wasm-footprint.txt` at 10,123 B — that was the v26.10.29-era combined artifact;
the v26.10.31 number of record is 4,309 B.

## Receipts

The receipt corpus of record is `docs/jira/v26.10.31/receipts/` (37 minted receipts at
writing time; `just receipts` tallies standing live). Each carries command, exit code,
input SHA, evidence digest, and standing; `ALIVE` requires exit 0 plus non-empty
captured evidence — a receipt is never silently minted from a failing run.

## See Also

- `docs/jira/v26.10.31/00-index.md` — milestone corpus
- `docs/jira/v26.10.31/63-test-census.md` — test census (661/0/2)
- `docs/jira/v26.10.31/65-coordinator-handoff.md` — operator decision detail
- `docs/DFLSS_CHARTER.md` — CTQ register
