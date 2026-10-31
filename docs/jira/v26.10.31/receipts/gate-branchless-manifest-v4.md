# Receipt: gate-branchless-manifest-v4

| Field | Value |
|---|---|
| Receipt ID | gate-branchless-manifest-v4 |
| Date | 2026-10-31T05:28:35Z |
| Subject | minted by scripts/mint_receipt.sh |
| Command | `env BRANCHLESS_SCOPE=transpose|untranspose PATH=/opt/homebrew/opt/llvm/bin:/Users/sac/.m2/mvnd/bin:/Users/sac/.sdkman/candidates/mvnd/current/bin:/Users/sac/.sdkman/candidates/maven/current/bin:/Users/sac/.sdkman/candidates/java/current/bin:/Users/sac/.local/bin:/Users/sac/.erlmcp/otp-28.3.1/bin:/opt/homebrew/bin:/opt/homebrew/sbin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:/opt/pmk/env/global/bin:/Library/Apple/usr/bin:/Library/TeX/texbin:/Applications/VMware Fusion.app/Contents/Public:/usr/local/go/bin:/Users/sac/.cargo/bin:/Users/sac/.claude/plugins/cache/claude-plugins-official/lumen/0.0.42/bin bash /Users/sac/avec/scripts/audit_branchless.sh` |
| Exit | 0 |
| Machine | ProductName:		macOS ProductVersion:		26.2 BuildVersion:		25C56 Mac15,9 |
| Input SHA (git HEAD) | ead40329a699878a91b070378066b5e50541ba33 |
| Tree state | dirty (116 entries) |
| Evidence digest (blake3 of stdout tail) | `9d15091762476fa8f4dea33a3ae758a29a6f3dcee701b662626d8f86672d220e` |
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
 into /var/folders/s6/jqyw48zs39z38b_3f6f_x2sc0000gn/T/tmp.JRArpURfyG/target
   Compiling avec-blake3 v26.10.29 (/Users/sac/avec/crates/avec-blake3)
   Compiling avec-arrow v26.10.29 (/Users/sac/avec/crates/avec-arrow)
    Finished `audit` profile [optimized] target(s) in 2.18s
== scope regex: (transpose|untranspose)  (unanchored; audit limited to hot-kernel candidate symbols; see header)
== auditing 4 artifact(s)
   (skipped non-object archive member: __.SYMDEF [cannot open: Permission denied])
   /var/folders/s6/jqyw48zs39z38b_3f6f_x2sc0000gn/T/tmp.JRArpURfyG/target/audit/libavec_blake3.rlib: scanned=0 skipped(out-of-scope)=27122 hits=0
   (skipped non-object archive member: __.SYMDEF [cannot open: Permission denied])
   /var/folders/s6/jqyw48zs39z38b_3f6f_x2sc0000gn/T/tmp.JRArpURfyG/target/audit/deps/libavec_arrow-ac411ac6f23ea957.rlib: scanned=128 skipped(out-of-scope)=22015 hits=0
   (skipped non-object archive member: __.SYMDEF [cannot open: Permission denied])
   /var/folders/s6/jqyw48zs39z38b_3f6f_x2sc0000gn/T/tmp.JRArpURfyG/target/audit/deps/libavec_blake3-998827c5f4765d30.rlib: scanned=0 skipped(out-of-scope)=27122 hits=0
   (skipped non-object archive member: __.SYMDEF [cannot open: Permission denied])
   /var/folders/s6/jqyw48zs39z38b_3f6f_x2sc0000gn/T/tmp.JRArpURfyG/target/audit/libavec_arrow.rlib: scanned=128 skipped(out-of-scope)=22015 hits=0
== total: scanned_instructions=256 skipped_out_of_scope=98274 artifacts_with_hits=2
== hits present; adjudicating against manifest: /Users/sac/avec/scripts/branchless_manifest.toml
MANIFEST-OK  sym=/var/folders/s6/jqyw48zs39z38b_3f6f_x2sc0000gn/T/tmp.uGRzfhwnd6/avec_arrow-ac411ac6f23ea957.avec_arrow.704106cce5ea540f-cgu.11.rcgu.o:__RNvNtCs9DwzS8jbwzX_10avec_arrow9transpose9transpose count=12 mnemonics:[ cbz  b  b.ne  ] kernel="quad transpose / unpack" (count<=24, mnemonics accepted) gate_mode=INFORMATIONAL
MANIFEST-OK  sym=/var/folders/s6/jqyw48zs39z38b_3f6f_x2sc0000gn/T/tmp.IXOea7kVHe/avec_arrow-ac411ac6f23ea957.avec_arrow.704106cce5ea540f-cgu.11.rcgu.o:__RNvNtCs9DwzS8jbwzX_10avec_arrow9transpose9transpose count=12 mnemonics:[ cbz  b  b.ne  ] kernel="quad transpose / unpack" (count<=24, mnemonics accepted) gate_mode=INFORMATIONAL
GATE PASS (all hits adjudicated MANIFEST-OK; zero unexplained hits)
```

<!-- Tree state: dirty (116 entries) -->
