# Dependency + Supply-Chain Audit

Standing: ALIVE — every claim carries a command + exit receipt run against
`/Users/sac/avec` on 2026-10-31 (wave-14 adversarial lane 17). Verdict: **zero
vulnerabilities, zero runtime external dependencies, zero network surface**. The
entire external crate surface (24 crates) traces to exactly two declared roots,
both non-runtime: `divan` (bench dev-dep) and `cc` (avec-gpu build-dep). One
config gap: no `deny.toml`, so `cargo deny check licenses` fails on an empty
allowlist, not on any actual licensing problem.

## Verdict Table

| Check | Result | Receipt |
|---|---|---|
| cargo-audit vulns | 0 vulns, 0 warnings | `cargo audit --json --no-fetch` exit 0; count=0 |
| advisory DB | 1352 advisories; last sync 2026-10-24 | `--stale` printed no warning |
| deny: advisories | ok | summary line `advisories ok` |
| deny: bans | ok | summary line `bans ok` |
| deny: sources | ok | summary line `sources ok` |
| deny: licenses | FAILED, 13 rejections | see license note below |
| lockfile | 35 pkgs = 11 members + 24 external | `grep -c '^\[\[package\]\]' Cargo.lock` → 35 |
| runtime externals | 0 | manifest sweep, next section |
| network surface | 0 real hits | P28-SIL section |

License note: no `deny.toml` exists in the repo, so the licenses check runs
with an empty allowlist and rejects everything not explicitly allowed. All 13
rejections (11 workspace crates + cc/shlex/find-msvc-tools) are MIT OR
Apache-2.0 class. Config gap, not a licensing defect.

## Tooling

`which cargo-audit cargo-deny` → both installed. Both were run; neither
installed by this lane. `--no-fetch` keeps the lane read-only (no writes
outside the repo, no registry/git network mutation). The local advisory DB
was fresh enough that no staleness warning fired.

## External-Crate Inventory

Members (11): avec-arrow, avec-audit, avec-beam, avec-blake3, avec-cli,
avec-ffi, avec-git, avec-gpu, avec-integration, avec-parquet, bench.
Format: `name version pre1.0? <- pulled-by` (D = divan tree / dev-only,
B = cc tree / build-only).

```text
anstyle 1.0.14 no <- clap_builder (D)
bitflags 2.13.2 no <- rustix (D)
cc 1.10.0 no <- avec-gpu build-dep (B)
cfg-if 1.0.5 no <- divan (D)
clap 4.6.7 no <- divan (D)
clap_builder 4.6.7 no <- clap (D)
clap_lex 1.1.1 no <- clap_builder (D)
condtype 1.3.0 no <- divan (D)
divan 0.1.21 YES <- bench dev-dep (root)
divan-macros 0.1.21 YES <- divan (D)
errno 0.3.14 YES <- rustix (D)
find-msvc-tools 0.1.15 YES <- cc (B)
libc 0.2.191 YES <- divan, errno, rustix (D)
linux-raw-sys 0.12.1 YES <- rustix (D)
proc-macro2 1.0.107 no <- divan-macros, quote, syn (D)
quote 1.0.47 no <- divan-macros, syn (D)
regex-lite 0.1.9 YES <- divan (D)
rustix 1.1.6 no <- terminal_size (D)
shlex 2.0.1 no <- cc (B)
syn 2.0.119 no <- divan-macros (D)
terminal_size 0.4.4 YES <- clap_builder (D)
unicode-ident 1.0.27 no <- proc-macro2, syn (D)
windows-link 0.2.1 YES <- windows-sys (D)
windows-sys 0.61.2 YES <- errno, rustix, terminal_size (D)
```

Count: 21 D + 3 B = 24 external. Pre-1.0: 10/24 (divan, divan-macros, errno,
find-msvc-tools, libc, linux-raw-sys, regex-lite, terminal_size, windows-link,
windows-sys). Yanked-check: UNVERIFIED offline — crates.io index queries are
network operations, not attempted by this read-only lane.

## Supply-Chain Posture: Zero-Dependency Claim, Precisely

| Root | Declared in | Tree | Scope |
|---|---|---|---|
| `divan = "0.1.21"` | bench dev-deps | 21 crates | benches only, never in release artifacts |
| `cc = "1.0"` | avec-gpu build-deps | 3 crates | Metal shim compile only |

Per-crate zero-external-dep truth (`E` = declared external deps):

```text
avec-arrow        E=0  zero-claim TRUE   (publish=true)
avec-blake3       E=0  zero-claim TRUE   (publish=true)
avec-parquet      E=0  zero-claim TRUE   (publish=true)
avec-git          E=0  zero-claim TRUE   (publish=true)
avec-ffi          E=0  path-only
avec-beam         E=0  path-only; build.rs has no build-deps
avec-gpu          E=1  cc build-dep; zero-claim FALSE for "all-zeros"
avec-audit        E=0  path-only
avec-cli          E=0  path-only; no clap — hand-rolled std arg parsing
avec-integration  E=0  path-only
bench             E=1  divan dev-dep; zero-claim FALSE for "all-zeros"
wasm-footprint    E=0  path-only; own workspace+lock (4 pkgs)
sovereign-e2e     E=0  path-only; own workspace+lock (5 pkgs)
```

- The four publish=true crates have empty `[dependencies]`; the
  zero-dependency claim is exact for them, and they are exactly the
  publishable set.
- hash-wasm: ABSENT (grep over Cargo.lock + all manifests: zero hits for
  `hash-wasm`, `wasm-bindgen`, `getrandom`). BLAKE3 is in-tree avec-blake3.
- No `[patch]`/`[replace]`, no git deps, no registry deps in any member
  `[dependencies]` — every member dep is a workspace path dep.

## P28-SIL: No Telemetry/Network/Phoning-Home Surface

Grep over `crates/*/src` (12 crate dirs), read-only:

| Pattern | Raw hits | Classification |
|---|---|---|
| Tcp/Udp/net/reqwest/hyper/ureq/curl/http | 4 | see grep note 1 |
| socket/telemetry/phone-home/beacon/analytics/track | 0 real | see grep note 2 |

- Grep note 1: the 4 hits are 2 BLAKE3 test-vector URL comments and 2
  rustup.rs hint strings in cli doctor output. Zero code hits.
- Grep note 2: all matches are `Tracking`/`tracked` in avec-audit's alloc
  counter and comments. False positives, zero real hits.

- No crate declares a network-capable dependency; the only externals (cc tree,
  divan tree) are build/bench tools.
- Build scripts (the only arbitrary-code execution surface in the build):
  - `crates/avec-beam/build.rs` (11 lines): macOS-only
    `-Wl,-undefined,dynamic_lookup` link-arg. No deps, no fs, no net. Benign.
  - `crates/avec-gpu/build.rs`: cc-compiles `metal_shim.m`, links Metal +
    Foundation, macOS-only. Benign; the sole justification for the cc dep.

## Build Reproducibility

- Root `Cargo.lock` (v4, 35 packages) committed, exact versions pinned.
  Standalone workspaces carry committed locks too:
  `crates/wasm-footprint/Cargo.lock` (4 pkgs),
  `examples/sovereign-e2e/Cargo.lock` (5 pkgs).
- Vendoring (offline rebuild), one-liner split for line width:

  ```sh
  cargo vendor vendor/ && mkdir -p .cargo && \
    printf '[source.crates-io]\nreplace-with = "vendored-sources"\n\
  [source.vendored-sources]\ndirectory = "vendor"\n' > .cargo/config.toml
  ```

  24 crates to vendor, a few MB; fully offline rebuildable afterwards.
- Advisory DB provenance: `~/.cargo/advisory-db` commit `21d909e4`,
  `Synchronize IDs (2026-10-24)`; 7d old at run, `--stale` did not fire.

## Findings Register

| # | Finding | Severity | Close condition |
|---|---|---|---|
| F1 | No `deny.toml`; empty allowlist, 13 rejections | low | Commit a deny.toml allowlist |
| F2 | divan tree not license-checked (dev-scope excluded) | info | Enable dev-scope in deny.toml |
| F3 | 10/24 externals pre-1.0; yanked unverifiable offline | info | re-run audit with fetch |
| F4 | Zero vulns / zero network / zero runtime externals | pass | Re-run cargo audit each wave |

- F1 detail: 13 rejections = 11 workspace crates + cc/shlex/find-msvc-tools,
  all MIT OR Apache-2.0 class. Suggested allowlist: MIT, Apache-2.0,
  BSD-3-Clause, Unicode-3.0.
- F2 detail: cargo-deny's default scope excludes dev-deps, so the 21-crate
  divan tree was not license-checked by the tool; those crates are well-known
  MIT OR Apache-2.0 — UNVERIFIED-by-tool.
- F4 detail: verdict holds as of 2026-10-31; re-run with the --no-fetch
  caveat recorded (DB was 7d old).
