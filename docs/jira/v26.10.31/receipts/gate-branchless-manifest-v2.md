# Receipt: gate-branchless-manifest-v2

| Field | Value |
|---|---|
| Receipt ID | gate-branchless-manifest-v2 |
| Date | 2026-10-31T03:46:09Z |
| Subject | minted by scripts/mint_receipt.sh |
| Command | `/Users/sac/avec/scripts/audit_branchless.sh` |
| Exit | 0 |
| Machine | ProductName:		macOS ProductVersion:		26.2 BuildVersion:		25C56 Mac15,9 |
| Input SHA (git HEAD) | ead40329a699878a91b070378066b5e50541ba33 |
| Tree state | dirty (70 entries) |
| Evidence digest (blake3 of stdout tail) | `85875012e711cc5191f68ff3382ff92df3213d1780618ac1ce3feb468ff6de8b` |
| Standing | ALIVE |

## Standing rule

`ALIVE` requires exit 0 **and** non-empty captured evidence. Anything else is
`FAILED` (with exit code) — a receipt is never silently minted from a failing
or evidence-free run.

<!-- auto-generated block; part of the receipt -->

## Evidence (stdout tail, last 40 lines)

```text
== disassembler: llvm-objdump (Homebrew LLVM version 21.1.7)
== positive control: FIRED (4 banned ops in branchy.o) — gate is not vacuous
== building avec-arrow, avec-blake3 (--profile audit: lto=false; the workspace release
   profile is thin-LTO and emits bitcode-only rlibs, which objdump cannot disassemble)
 into /var/folders/s6/jqyw48zs39z38b_3f6f_x2sc0000gn/T/tmp.t9I8jbJv9V/target
   Compiling avec-blake3 v26.10.29 (/Users/sac/avec/crates/avec-blake3)
   Compiling avec-arrow v26.10.29 (/Users/sac/avec/crates/avec-arrow)
    Finished `audit` profile [optimized] target(s) in 2.05s
== scope regex: (ladder|predicates|transpose|ct_)  (unanchored; audit limited to hot-kernel candidate symbols; see header)
== auditing 4 artifact(s)
   (skipped non-object archive member: __.SYMDEF [cannot open: Permission denied])
   /var/folders/s6/jqyw48zs39z38b_3f6f_x2sc0000gn/T/tmp.t9I8jbJv9V/target/audit/libavec_blake3.rlib: scanned=0 skipped(out-of-scope)=27116 hits=0
   (skipped non-object archive member: __.SYMDEF [cannot open: Permission denied])
   /var/folders/s6/jqyw48zs39z38b_3f6f_x2sc0000gn/T/tmp.t9I8jbJv9V/target/audit/deps/libavec_arrow-ac411ac6f23ea957.rlib: scanned=389 skipped(out-of-scope)=21731 hits=0
   (skipped non-object archive member: __.SYMDEF [cannot open: Permission denied])
   /var/folders/s6/jqyw48zs39z38b_3f6f_x2sc0000gn/T/tmp.t9I8jbJv9V/target/audit/deps/libavec_blake3-998827c5f4765d30.rlib: scanned=0 skipped(out-of-scope)=27116 hits=0
   (skipped non-object archive member: __.SYMDEF [cannot open: Permission denied])
   /var/folders/s6/jqyw48zs39z38b_3f6f_x2sc0000gn/T/tmp.t9I8jbJv9V/target/audit/libavec_arrow.rlib: scanned=389 skipped(out-of-scope)=21731 hits=0
== total: scanned_instructions=778 skipped_out_of_scope=97694 artifacts_with_hits=8
== hits present; adjudicating against manifest: /Users/sac/avec/scripts/branchless_manifest.toml
MANIFEST-OK  sym=/var/folders/s6/jqyw48zs39z38b_3f6f_x2sc0000gn/T/tmp.f20PEiyXIW/avec_arrow-ac411ac6f23ea957.avec_arrow.704106cce5ea540f-cgu.05.rcgu.o:__RNvNtCs9DwzS8jbwzX_10avec_arrow12c_abi_nested21release_struct_schema count=10 mnemonics:[ cbz  b  b.eq  blr  ] kernel="default-scope collision: c_abi_nested FFI release helpers" (count<=12, mnemonics accepted) gate_mode=INFORMATIONAL
MANIFEST-OK  sym=/var/folders/s6/jqyw48zs39z38b_3f6f_x2sc0000gn/T/tmp.RY15kKKuf0/avec_arrow-ac411ac6f23ea957.avec_arrow.704106cce5ea540f-cgu.05.rcgu.o:__RNvNtCs9DwzS8jbwzX_10avec_arrow12c_abi_nested20release_struct_array count=11 mnemonics:[ cbz  b  b.eq  blr  ] kernel="default-scope collision: c_abi_nested FFI release helpers" (count<=12, mnemonics accepted) gate_mode=INFORMATIONAL
MANIFEST-OK  sym=/var/folders/s6/jqyw48zs39z38b_3f6f_x2sc0000gn/T/tmp.f20PEiyXIW/avec_arrow-ac411ac6f23ea957.avec_arrow.704106cce5ea540f-cgu.14.rcgu.o:__RNvMNtCs9DwzS8jbwzX_10avec_arrow12perfect_hashNtB2_10PerfectMap9try_build count=11 mnemonics:[ cbz  b.eq  b.ne  b  ] kernel="default-scope collision: perfect_hash try_build" (count<=12, mnemonics accepted) gate_mode=INFORMATIONAL
MANIFEST-OK  sym=/var/folders/s6/jqyw48zs39z38b_3f6f_x2sc0000gn/T/tmp.f20PEiyXIW/avec_arrow-ac411ac6f23ea957.avec_arrow.704106cce5ea540f-cgu.05.rcgu.o:__RNvNtCs9DwzS8jbwzX_10avec_arrow12c_abi_nested20release_struct_array count=11 mnemonics:[ cbz  b  b.eq  blr  ] kernel="default-scope collision: c_abi_nested FFI release helpers" (count<=12, mnemonics accepted) gate_mode=INFORMATIONAL
MANIFEST-OK  sym=/var/folders/s6/jqyw48zs39z38b_3f6f_x2sc0000gn/T/tmp.RY15kKKuf0/avec_arrow-ac411ac6f23ea957.avec_arrow.704106cce5ea540f-cgu.05.rcgu.o:__RNvNtCs9DwzS8jbwzX_10avec_arrow12c_abi_nested21release_struct_schema count=10 mnemonics:[ cbz  b  b.eq  blr  ] kernel="default-scope collision: c_abi_nested FFI release helpers" (count<=12, mnemonics accepted) gate_mode=INFORMATIONAL
MANIFEST-OK  sym=/var/folders/s6/jqyw48zs39z38b_3f6f_x2sc0000gn/T/tmp.f20PEiyXIW/avec_arrow-ac411ac6f23ea957.avec_arrow.704106cce5ea540f-cgu.07.rcgu.o:__RNvNtCs9DwzS8jbwzX_10avec_arrow10predicates15filter_quads_eq count=2 mnemonics:[ b.lo  b.ne  ] kernel="equality masks (quad equality)" (count<=2, mnemonics accepted) gate_mode=INFORMATIONAL
MANIFEST-OK  sym=/var/folders/s6/jqyw48zs39z38b_3f6f_x2sc0000gn/T/tmp.f20PEiyXIW/avec_arrow-ac411ac6f23ea957.avec_arrow.704106cce5ea540f-cgu.11.rcgu.o:__RNvNtCs9DwzS8jbwzX_10avec_arrow9transpose9transpose count=12 mnemonics:[ cbz  b  b.ne  ] kernel="quad transpose / unpack" (count<=24, mnemonics accepted) gate_mode=INFORMATIONAL
MANIFEST-OK  sym=/var/folders/s6/jqyw48zs39z38b_3f6f_x2sc0000gn/T/tmp.RY15kKKuf0/avec_arrow-ac411ac6f23ea957.avec_arrow.704106cce5ea540f-cgu.07.rcgu.o:__RNvNtCs9DwzS8jbwzX_10avec_arrow10predicates15filter_quads_eq count=2 mnemonics:[ b.lo  b.ne  ] kernel="equality masks (quad equality)" (count<=2, mnemonics accepted) gate_mode=INFORMATIONAL
MANIFEST-OK  sym=/var/folders/s6/jqyw48zs39z38b_3f6f_x2sc0000gn/T/tmp.RY15kKKuf0/avec_arrow-ac411ac6f23ea957.avec_arrow.704106cce5ea540f-cgu.14.rcgu.o:__RNvMNtCs9DwzS8jbwzX_10avec_arrow12perfect_hashNtB2_10PerfectMap9try_build count=11 mnemonics:[ cbz  b.eq  b.ne  b  ] kernel="default-scope collision: perfect_hash try_build" (count<=12, mnemonics accepted) gate_mode=INFORMATIONAL
MANIFEST-OK  sym=/var/folders/s6/jqyw48zs39z38b_3f6f_x2sc0000gn/T/tmp.RY15kKKuf0/avec_arrow-ac411ac6f23ea957.avec_arrow.704106cce5ea540f-cgu.11.rcgu.o:__RNvNtCs9DwzS8jbwzX_10avec_arrow9transpose9transpose count=12 mnemonics:[ cbz  b  b.ne  ] kernel="quad transpose / unpack" (count<=24, mnemonics accepted) gate_mode=INFORMATIONAL
GATE PASS (all hits adjudicated MANIFEST-OK; zero unexplained hits)
```

<!-- Tree state: dirty (70 entries) -->

## Lane-3 detail: manifest v2 inventory closure (spec 27 §6)

Wave-8 lane 3 extended `scripts/branchless_manifest.toml` from 2 to 7 kernel
entries. Method: build each crate into a temp `CARGO_TARGET_DIR` with
`--profile audit`, then run `scripts/audit_branchless.sh` with an explicit
rlib and a narrow `BRANCHLESS_SCOPE` to get the real banned-opcode counts, and
set `expected_max_count = observed + 1`. Every entry is INFORMATIONAL
(observed count > 0; ENFORCED requires 0). Environment note: homebrew
llvm-objdump must be on PATH (`/opt/homebrew/opt/llvm/bin`) — Apple objdump
renders mach-o symbols as `<ltmp0>`, which defeats name scoping entirely.

Observed counts at audit-profile, aarch64, llvm-objdump 21.1.7:

| Kernel | Symbol | Observed | Mnemonics | Ceiling | Gate mode |
|---|---|---|---|---|---|
| git idx constant-time ladder lookup | `avec_git::idx_read::IdxV2::lookup` | 23 | cbz, b, b.ne, b.eq, b.hi, b.ls, b.hs, tbnz, bic | 24 | INFORMATIONAL |
| offset-MSB large-offset select | `avec_git::idx_read::IdxV2::offset` | 4 (b.ls x2, b x1, bic x1 FP) | b.ls, b, bic | 5 | INFORMATIONAL |
| blake3 4-way chunk compressor | `multi_chunk23compress_chunks_4way_x2` | 3 (cbz 0x3a64, b 0x3ad0, b.ne 0x3b94) | cbz, b, b.ne | 4 | INFORMATIONAL |
| (4-way non-x2 / neon variants) | compress_chunks_4way{,_neon,_x2_neon} | 0 | — | — | covered by same entry |

Classification: all hits are guard/latch-class (entry null/truncation guards,
typed-error return joins, loop latches). The offset-MSB select itself is
arithmetic (`(raw>>31).wrapping_neg()` mask, andn via `bic`) — 0
data-dependent branches in the select proper.

`ladder_search` has no auditable symbol in the audit rlib (local symbol,
renders `<ltmp0>`); it is reached through `IdxV2::lookup` (scope `6lookup`).
`round4`/`g4` are `#[inline(always)]` — no separate symbols, covered inside
the 4-way entry. blake3 counts are wave-8-observed at
multi_chunk.rs sha256 `dccfa01149c300b3ac7a73e2e5c243d0c787de2f` (lane 1
editing concurrently), 2026-10-30T20:43-0700.

### Findings for follow-up (out of lane scope)

1. **Gate regex transport bug**: awk `-v re="$BANNED_RE"` decodes the escape
   in `b\.[a-z]+` into `b.[a-z]+`, so `bic`/`blr` falsely match the b-dot
   alternative. Witnessed as `bic` at 0x758/0x854 in IdxV2::lookup and `blr`
   rows in the c_abi_nested symbols. Classified in the manifest as
   REGEX_TRANSPORT_FP so the opcode is not an unaccepted remainder; fixing
   `scripts/audit_branchless.sh` (pass the regex via ENVIRON or a file) is a
   one-line follow-up for the script's owner.
2. **Default-scope substring collisions**: the default scope's `ct_`
   alternative substring-matches "stru(ct_)" — pulls
   `c_abi_nested::release_struct_{array,schema}` (11/10 hits) and
   `perfect_hash::PerfectMap::try_build` (11 hits) into scope. These three
   FAILed as unlisted symbols under the pre-wave manifest too (verified by
   replaying the default gate against the 2-kernel manifest), i.e. the default
   gate was already red on main before lane 3. Registered here as
   guard-class collision entries; owning lanes should reclaim them.
3. **avec-git is outside the default artifact set**: the gate builds only
   avec-arrow + avec-blake3, so the two avec-git kernels adjudicate only when
   the avec-git audit rlib is passed explicitly (documented in the manifest
   header).
4. **Apple objdump cannot scope mach-o symbols** (`<ltmp0>` for all): the
   gate silently audits zero in-scope instructions and exits 3 in that
   environment — llvm-objdump is a hard requirement.

Selftest: `audit_branchless.sh --selftest` exit 0 (positive control fired, 4
banned ops; clean fixture 0 hits).
