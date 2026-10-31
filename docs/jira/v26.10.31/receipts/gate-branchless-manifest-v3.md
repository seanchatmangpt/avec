# Receipt: gate-branchless-manifest-v3

| Field | Value |
|---|---|
| Receipt ID | gate-branchless-manifest-v3 |
| Date | 2026-10-31T05:00:01Z |
| Subject | minted by scripts/mint_receipt.sh |
| Command | `bash scripts/audit_branchless.sh` |
| Exit | 0 |
| Machine | ProductName:		macOS ProductVersion:		26.2 BuildVersion:		25C56 Mac15,9 |
| Input SHA (git HEAD) | ead40329a699878a91b070378066b5e50541ba33 |
| Tree state | dirty (70 entries) |
| Evidence digest (blake3 of stdout tail) | `a3ca8c00134180e3f369656a94976b0cd4fe6dc33b41b6da4af482fbbdec37d3` |
| Standing | ALIVE |

## Standing rule

`ALIVE` requires exit 0 **and** non-empty captured evidence. Anything else is
`FAILED` (with exit code) — a receipt is never silently minted from a failing
or evidence-free run.

<!-- auto-generated block; part of the receipt -->

## Evidence (stdout tail, last 40 lines)

```text
== disassembler: objdump (Apple LLVM version 17.0.0)
== positive control: FIRED (4 banned ops in branchy.o) — gate is not vacuous
== building avec-arrow, avec-blake3 (--profile audit: lto=false; the workspace release
   profile is thin-LTO and emits bitcode-only rlibs, which objdump cannot disassemble)
 into /var/folders/s6/jqyw48zs39z38b_3f6f_x2sc0000gn/T/tmp.1iSUsnPjw3/target
   Compiling avec-arrow v26.10.29 (/Users/sac/avec/crates/avec-arrow)
   Compiling avec-blake3 v26.10.29 (/Users/sac/avec/crates/avec-blake3)
    Finished `audit` profile [optimized] target(s) in 2.07s
== scope regex: (transpose|predicates)  (unanchored; audit limited to hot-kernel candidate symbols; see header)
== auditing 4 artifact(s)
   (skipped non-object archive member: __.SYMDEF [cannot open: Permission denied])
   /var/folders/s6/jqyw48zs39z38b_3f6f_x2sc0000gn/T/tmp.1iSUsnPjw3/target/audit/libavec_blake3.rlib: scanned=0 skipped(out-of-scope)=27116 hits=0
   (skipped non-object archive member: __.SYMDEF [cannot open: Permission denied])
   /var/folders/s6/jqyw48zs39z38b_3f6f_x2sc0000gn/T/tmp.1iSUsnPjw3/target/audit/deps/libavec_arrow-ac411ac6f23ea957.rlib: scanned=174 skipped(out-of-scope)=21946 hits=0
   (skipped non-object archive member: __.SYMDEF [cannot open: Permission denied])
   /var/folders/s6/jqyw48zs39z38b_3f6f_x2sc0000gn/T/tmp.1iSUsnPjw3/target/audit/deps/libavec_blake3-998827c5f4765d30.rlib: scanned=0 skipped(out-of-scope)=27116 hits=0
   (skipped non-object archive member: __.SYMDEF [cannot open: Permission denied])
   /var/folders/s6/jqyw48zs39z38b_3f6f_x2sc0000gn/T/tmp.1iSUsnPjw3/target/audit/libavec_arrow.rlib: scanned=174 skipped(out-of-scope)=21946 hits=0
== total: scanned_instructions=348 skipped_out_of_scope=98124 artifacts_with_hits=4
== hits present; adjudicating against manifest: /Users/sac/avec/scripts/branchless_manifest.toml
MANIFEST-OK  sym=/var/folders/s6/jqyw48zs39z38b_3f6f_x2sc0000gn/T/tmp.mT2d40Yk9A/avec_arrow-ac411ac6f23ea957.avec_arrow.704106cce5ea540f-cgu.07.rcgu.o:__RNvNtCs9DwzS8jbwzX_10avec_arrow10predicates15filter_quads_eq count=2 mnemonics:[ b.lo  b.ne  ] kernel="equality masks (quad equality)" (count<=2, mnemonics accepted) gate_mode=INFORMATIONAL
MANIFEST-OK  sym=/var/folders/s6/jqyw48zs39z38b_3f6f_x2sc0000gn/T/tmp.Z2f1gM13N0/avec_arrow-ac411ac6f23ea957.avec_arrow.704106cce5ea540f-cgu.11.rcgu.o:__RNvNtCs9DwzS8jbwzX_10avec_arrow9transpose9transpose count=12 mnemonics:[ cbz  b  b.ne  ] kernel="quad transpose / unpack" (count<=24, mnemonics accepted) gate_mode=INFORMATIONAL
MANIFEST-OK  sym=/var/folders/s6/jqyw48zs39z38b_3f6f_x2sc0000gn/T/tmp.Z2f1gM13N0/avec_arrow-ac411ac6f23ea957.avec_arrow.704106cce5ea540f-cgu.07.rcgu.o:__RNvNtCs9DwzS8jbwzX_10avec_arrow10predicates15filter_quads_eq count=2 mnemonics:[ b.lo  b.ne  ] kernel="equality masks (quad equality)" (count<=2, mnemonics accepted) gate_mode=INFORMATIONAL
MANIFEST-OK  sym=/var/folders/s6/jqyw48zs39z38b_3f6f_x2sc0000gn/T/tmp.mT2d40Yk9A/avec_arrow-ac411ac6f23ea957.avec_arrow.704106cce5ea540f-cgu.11.rcgu.o:__RNvNtCs9DwzS8jbwzX_10avec_arrow9transpose9transpose count=12 mnemonics:[ cbz  b  b.ne  ] kernel="quad transpose / unpack" (count<=24, mnemonics accepted) gate_mode=INFORMATIONAL
GATE PASS (all hits adjudicated MANIFEST-OK; zero unexplained hits)
```

<!-- Tree state: dirty (70 entries) -->
