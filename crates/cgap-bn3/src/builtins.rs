//! BN3 native builtins (Wave 2, core engines): the closed u8 opcode
//! registry, the 4-byte `BuiltinPayload` register-addressing layout, the
//! zero-indirection `match` dispatch, and the typed-refusal surface
//! (spec: `docs/jira/v26.11.1/lanes/A13-bn3-builtins.md` Sections 1-3, 5,
//! 6 DoD 2/3/4/5/7/9).
//!
//! Wave 1 (prior run) froze the ABI: opcode consts, `BuiltinPayload`, and
//! the compile-time layout asserts. This wave lands the dispatch `match`
//! (spec Section 2 ADR), the §5.1 known-answer tests, the §5.2 dispatch
//! courts with their anti-vacuity mutations, and the DoD-7 typed refusals.
//!
//! Authority notes:
//! - Sigil authority is A11 (spec "Read-only receipt basis"): this module
//!   consumes `sigil::Sigil::BuiltinOp` (0x04) and defines only the opcode
//!   space that sigil indexes into. The coupling is one-way — opcodes live
//!   inside the 56-bit payload, so an A11 sigil renumber does not move the
//!   opcode space (spec Section 7).
//! - Register discipline is A12's (spec Section 3 preamble): this lane
//!   *addresses* `R0..R255` through `&mut [u64; 256]` and defines no
//!   register-file type. Until A12 accepts the register-addressing scheme,
//!   the §5 courts are provisional (spec Section 7, Wave 3 sequencing).
//! - `BuiltinError` is lane-local. The prior Wave-1 header deferred the
//!   refusal variants pending "A11's `BN3Error`" — but `sigil.rs` declares
//!   no such type (verified this session; only doc comments name it), so
//!   deferral would leave the lane with no error surface. The enum is
//!   named `BuiltinError` (not `BN3Error`) so A11 can land its crate-level
//!   enum later; a one-way `From` bridge is the seam, not a fork.
//!
//! Zero-indirection court receipt (spec 2.1/5.3; re-run Wave 5,
//! 2026-11-01): `cargo rustc --release -p cgap-bn3 --lib -- --emit=asm`,
//! dispatch region `__RNvNtC...8cgap_bn38builtins8dispatch` (281 asm
//! lines, bounded by the adjacent `BuiltinError` Display::fmt): 0 `blr`,
//! 0 `br xN` — the opcode `match` emits a direct comparison tree; the
//! only calls are direct `bl` (`cgap_blake3::hash`, `hash_many_simd`,
//! `alloc::raw_vec grow_one` — the trace sink's growth path); the one
//! `adrp` feeds an anonymous constant-pool data load (`add @PAGEOFF`),
//! not a function-pointer table. The six `br x3` sites elsewhere in the
//! object belong to the adjacent `sigil::BN3Error` Display::fmt (A11's
//! function), not dispatch. The §2.1 claim is ALIVE as an emitted-code
//! fact, re-verified this session.
//!
//! Panic hygiene (DoD 7): `dispatch` has no panic path. Register ids are
//! `u8` into a 256-slot array (total index); operand ranges resolve
//! through `checked_add` + an in-`u64` bounds guard before any `usize`
//! narrowing (Wave 4: truncation cannot launder an out-of-bounds range on
//! a 32-bit target) + `Option<&[u8]>` slicing (bounds-checked by Rust,
//! never a wild read, spec Section 7); arithmetic is `checked_*`
//! plus an explicit 56-bit ceiling check (Section 7 top correctness
//! risk). No `unwrap`/`expect`/`panic!`/`assert!` on runtime paths, and
//! the eval counter increments with `wrapping_add` (Wave 4: a `+=`
//! increment is a debug-profile panic at 2^64 dispatches).
//!
//! Wave 4 (this run) closed the DoD items by execution: exhaustive
//! 256-opcode registry sweep, refusal write-isolation court, eval-counter
//! wrap court, the three Section 5.2 anti-vacuity mutations executed with
//! byte-exact restore, `-D warnings` gate, and a re-run of the
//! zero-indirection court against the current release asm (receipt below
//! in §5.3 court notes).
//!
//! Wave 5 (defect repair from the court receipts): repaired the crate's
//! one body-level `std::` path (`impl core::error::Error`, per the
//! a11-wave4 receipt's no_std direction) and added the module-hygiene
//! courts pinning it. The prior-session court claims (three §5.2
//! mutations, zero-indirection asm court, `-D warnings` gate) are
//! re-executed THIS session — a prior-session summary is hearsay — with
//! results in the wave receipt, not here.
//!
//! Wave 6 (defect repair from the wave-5 court receipts, 2026-11-01):
//! the wave-5 receipts' one code-level defect class is a03-wave5 F-1 —
//! "debug-profile arithmetic overflow panic on hostile bytes in a public
//! parser of untrusted input violates the typed-refusal contract."
//! `dispatch` is exactly such a parser (untrusted payload word + register
//! file + slab range), so the class was audited against this module: the
//! body arithmetic is checked/wrapping (reading), and the new
//! `hostile_domain_sweep_court` proves it by execution — all 256 opcodes
//! x hostile register domains x hostile slab ranges, plus a single-bit
//! walk over all 64 decode words, run in the debug profile where
//! arithmetic overflow actually panics. The prior-session claims were
//! re-executed again this session (7/7 named mutations flip incl. the two
//! wave-6 must-kills, zero-indirection asm court reproduced, `-D warnings`
//! green) — receipts in the A13 wave-6 lane report; the wave-receipt FILE
//! is still absent from `docs/jira/v26.11.1/waves/` (the same
//! cited-receipt-missing finding a03/a07/a11 recorded) and is a
//! coordinator open item.
//!
//! Wave 7 (defect repair from the wave-6 court receipts, 2026-11-01): every
//! wave-6 claim re-executed THIS session (a prior-session summary is
//! hearsay) and one doc-mechanism defect repaired — the §5.2 Mutation-3 note
//! below claimed a const renumber "reroutes dispatch"; observed this
//! session, the reroute is refused at BUILD time by the ABI-freeze
//! const-assert lattice (M1/M3: E0080, rc=101), and even the
//! const+literal-assert-consistent swap (M3c probe) dies on the namespace
//! equality/gapping asserts (`MATH_NS_FIRST == OP_MATH_GREATER_THAN`,
//! `MATH_NS_LAST < CRYPTO_NS_FIRST`) — 0x10 (math) and 0x20 (crypto) sit in
//! different asserted namespaces, so the swap has no buildable consistent
//! form. Dispatch-reroute kill at TEST level is separately observed: M2's
//! arm deletion reroutes 0x13 to `UnknownOpcode` and the KAT flips.
//! Receipts (sandbox /tmp/a13-w7, byte-exact restores, corrupt-restore
//! self-check detected): 7/7 must-kills flip (M1 E0080; M2
//! `known_answer_math_min` + `registry_exhaustively_closed_sweep`; M3 E0080;
//! M4 nostd hygiene court; M5 panic hygiene court; M6
//! `hostile_domain_sweep_court`; M7 `payload_encode_decode_roundtrip`);
//! M3c: build fail, 0 tests run; zero-indirection asm court re-run against
//! the release artifact: dispatch region 238 asm lines, 0 `blr`, 0 `br xN`,
//! 0 `brk`, 1 `adrp` (anonymous constant-pool data load), direct `bl` only
//! (`cgap_blake3::hash`, `cgap_blake3::hash_many_simd`,
//! `alloc::raw_vec::grow_one`); `-D warnings` fresh compile green; 0
//! `unsafe`. The A13 wave-6/7 receipt FILE remains absent from
//! `docs/jira/v26.11.1/waves/` — coordinator open item, unchanged.
//!
//! Wave 8 (defect repair from the wave-7 court receipts, 2026-11-01):
//! every wave-7 claim re-executed THIS session (a prior-session summary
//! is hearsay); the audit found NO module-body defect to repair — the
//! wave 4-7 repairs held under re-execution. Receipts (sandbox
//! /tmp/a13w8, sources sha256-identical to canonical `c615bcf5…`,
//! byte-exact sha256-fail-closed restores, corrupt-restore self-check
//! detected an injected corruption): 8/8 mutations flip — M1 E0080
//! (`OP_MATH_SUM == 0x11`); M2 three courts (`known_answer_math_min`,
//! `all_registered_opcodes_dispatch`,
//! `registry_exhaustively_closed_sweep`; 84/3); M3 E0080
//! (`OP_MATH_GREATER_THAN == 0x10`); M3c E0080 on the namespace lattice
//! (`MATH_NS_FIRST == OP_MATH_GREATER_THAN && …`,
//! `CRYPTO_NS_FIRST == OP_CRYPTO_BLAKE3 && …`) — the "no buildable
//! consistent form" finding reproduces; M4 `nostd_hygiene_court…`
//! (86/1); M5 `panic_hygiene_court…` (86/1); M6
//! `hostile_domain_sweep_court` (86/1, the 56-bit KATs stay green); M7
//! three courts (`hostile_decode_word_court`,
//! `payload_encode_decode_roundtrip`, `refusal_write_isolation_court`;
//! 84/3). Zero-indirection asm court re-derived against the release
//! artifact: dispatch region 238 lines (bounded by `BuiltinError`
//! Display::fmt), 0 `blr`, 0 `br xN`, 0 `brk`, 1 `adrp` (anonymous
//! constant-pool load, `add …@PAGEOFF` — data, not a function-pointer
//! table), 3 direct `bl` (`cgap_blake3::hash`, `hash_many_simd`,
//! `alloc::raw_vec grow_one`); the object's 6 `br x3` sites all belong
//! to `sigil::BN3Error` Display::fmt. `-D warnings` fresh compile exit
//! 0; repo suite exit 0 87/87 this session. The A13 wave-6/7/8 receipt
//! FILE remains absent from `docs/jira/v26.11.1/waves/` (coordinator
//! open item, unchanged). Auditor self-defects, recorded (a20-wave5
//! precedent): two garbled output blocks were emitted mid-session; the
//! first M3 anchor-join and the first M5/M6 mutation forms were
//! malformed — the runner's count==1 assert and the build's type/syntax
//! refusals caught all three, so no vacuous pass resulted; corrected
//! and re-run to completion.
//!
//! Wave 9 (restart re-execution, 2026-11-01): the wave-9 adversarial audit
//! (`docs/jira/v26.11.1/waves/a13-wave9-audit-receipt.md`, verdict PARTIAL
//! only on the then-missing receipt FILE, which now exists) is re-executed
//! THIS session line for line — a prior-session summary is hearsay.
//! Baseline: `cargo test -p cgap-bn3` exit 0 (87/87 across lib +
//! integration targets); `RUSTFLAGS="-D warnings" cargo check -p cgap-bn3
//! --lib` exit 0; workspace `cargo build` exit 0 (DoD 1). Fresh release
//! asm court (`cargo rustc --release -p cgap-bn3 --lib -- --emit=asm`):
//! dispatch region (bounded by the adjacent `BuiltinError` Display::fmt)
//! 239 non-empty lines, 0 `blr`, 0 `br xN`, 0 `brk`, 1 `adrp` (anonymous
//! constant-pool data load), 3 direct `bl` (`cgap_blake3::hash`,
//! `cgap_blake3::hash_many_simd`, `alloc::raw_vec::grow_one`); the
//! object's 6 `br x3` sites all sit inside `sigil::BN3Error`
//! Display::fmt (A11's function). Anti-vacuity matrix re-run in a /tmp
//! sandbox (pre-condition sandbox==canonical==pristine sha256
//! `5060b702…`; the fail-closed pre-condition caught a leftover mutation
//! from a crashed first runner run and refused to proceed): 8/8 named
//! mutations flip — M1/M3/M3c build-refused (E0080, rc=101); M2 84/3
//! (`known_answer_math_min`, `all_registered_opcodes_dispatch`,
//! `registry_exhaustively_closed_sweep`); M4 86/1 (nostd hygiene); M5
//! 85/2 (panic hygiene + hostile sweep); M6 86/1 (hostile sweep); M7
//! 84/3 (decode-word + roundtrip + refusal isolation); corrupt-restore
//! self-check detected an injected corruption and the final restore was
//! byte-exact. Zero-string gate (the A13 share of the wave-9 cross-lane
//! slice) over ALL built crates: exit 0, 26 artifacts / 22 crates,
//! fail-set hits=0, 2 loud EMITTER-EXEMPT-HIT producer rows. Canonical
//! `builtins.rs` was byte-unchanged by the re-verification runs (sha256
//! `5060b702…` before == after): wave 9 found NO module-body defect; the
//! only delta of this wave is this comment block. Coordinator open items
//! unchanged: the lane doc still reads "PROPOSED / does not exist yet"
//! and §5.1 still carries the SHA-256("abc") literal where BLAKE3 is
//! meant (audit-receipt divergences 3/4).

use crate::sigil::{Sigil, PAYLOAD_BITS};

// --- Closed opcode registry (spec Section 3) ----------------------------
// Dense assignment, namespace-gapped: log 0x01-0x03, math 0x10-0x13,
// crypto 0x20-0x21. 9 of 256 u8 codes; 247 remain with no renumber
// pressure (spec Section 3). A duplicate opcode literal in the dispatch
// `match` is a hard compile error (unreachable pattern, an error under
// `-D warnings`), so opcode uniqueness is compiler-guaranteed on top of
// the registry court below.

/// `0x01` `log:trace` — trace-sink write; result = input, identity; cannot
/// fail (spec 3.1).
pub const OP_LOG_TRACE: u8 = 0x01;
/// `0x02` `log:assert` — `R[dst]==R[exp]` else `AssertFailed`; ok -> dst=1
/// (spec 3.1).
pub const OP_LOG_ASSERT: u8 = 0x02;
/// `0x03` `log:counters` — latch eval counter into dst; identity; cannot
/// fail (spec 3.1).
pub const OP_LOG_COUNTERS: u8 = 0x03;
/// `0x10` `math:greater_than` — `R[dst] = (a > b) as u64`; total (spec 3.2).
pub const OP_MATH_GREATER_THAN: u8 = 0x10;
/// `0x11` `math:sum` — `R[dst]=a+b` checked against the 56-bit ceiling;
/// `Overflow { a, b }` (spec 3.2, 7).
pub const OP_MATH_SUM: u8 = 0x11;
/// `0x12` `math:mul10` — `R[dst]=a*10` checked; `b` must be 0 else
/// `ReservedOperand` (spec 3.2).
pub const OP_MATH_MUL10: u8 = 0x12;
/// `0x13` `math:min` — `R[dst] = min(a, b)`; total (spec 3.2).
pub const OP_MATH_MIN: u8 = 0x13;
/// `0x20` `crypto:blake3` — BLAKE3-256 via `cgap_blake3::hash`; `(ptr,len)`
/// must resolve in-bounds else `InvalidRange` (spec 3.3, 7).
pub const OP_CRYPTO_BLAKE3: u8 = 0x20;
/// `0x21` `crypto:blake3_wide` — `cgap_blake3::hash_many_simd`, result
/// equals 0x20's (spec 3.3).
pub const OP_CRYPTO_BLAKE3_WIDE: u8 = 0x21;

/// First opcode of the `log:` namespace (spec 3.1).
pub const LOG_NS_FIRST: u8 = 0x01;
/// Last opcode of the `log:` namespace (spec 3.1).
pub const LOG_NS_LAST: u8 = 0x03;
/// First opcode of the `math:` namespace (spec 3.2).
pub const MATH_NS_FIRST: u8 = 0x10;
/// Last opcode of the `math:` namespace (spec 3.2).
pub const MATH_NS_LAST: u8 = 0x13;
/// First opcode of the `crypto:` namespace (spec 3.3).
pub const CRYPTO_NS_FIRST: u8 = 0x20;
/// Last opcode of the `crypto:` namespace (spec 3.3).
pub const CRYPTO_NS_LAST: u8 = 0x21;

/// Width of the opcode field: the whole registry fits one `u8` (spec
/// Section 2 ADR).
pub const OPCODE_SPACE: usize = 256;
/// Assigned opcodes: 3 `log:` + 4 `math:` + 2 `crypto:` (spec Section 3).
pub const OPCODE_COUNT: usize = 9;
/// Unassigned opcodes: `256 - 9`; namespaces may claim these without
/// renumbering anything (spec Section 3).
pub const FREE_OPCODE_COUNT: usize = OPCODE_SPACE - OPCODE_COUNT;

/// The full registry as one flat table, ascending. Spec 5.2 requires this
/// to live in the same module as the dispatch `match` so the no-duplicate
/// court and the match share one source of truth.
pub const ALL_OPCODES: [u8; OPCODE_COUNT] = [
    OP_LOG_TRACE,
    OP_LOG_ASSERT,
    OP_LOG_COUNTERS,
    OP_MATH_GREATER_THAN,
    OP_MATH_SUM,
    OP_MATH_MUL10,
    OP_MATH_MIN,
    OP_CRYPTO_BLAKE3,
    OP_CRYPTO_BLAKE3_WIDE,
];

// --- Register-file addressing (spec Sections 1, 3) -----------------------
// `dst`, `a`, `b` are register ids into A11/A12's `R0..R255` file; this
// lane addresses, A12 adjudicates semantics (spec Section 3 preamble).

/// Register count of the addressed file: `R0..R255` (spec Section 1).
pub const REGISTER_COUNT: usize = 256;
/// Highest addressable register id (spec Section 1); exactly `u8::MAX`, so
/// each `BuiltinPayload` register byte is a total, truncation-free id.
pub const MAX_REGISTER_ID: u8 = u8::MAX;

// --- Payload layout (spec Section 1) --------------------------------------
// BUILTIN_OP word: sigil<<56 | payload(56 bits); payload bytes:
// [opcode:u8 | dst:u8 | a:u8 | b:u8].

/// Byte offset of the opcode field inside the builtin payload (spec
/// Section 1).
pub const PAYLOAD_BYTE_OPCODE: usize = 0;
/// Byte offset of the destination-register field (spec Section 1).
pub const PAYLOAD_BYTE_DST: usize = 1;
/// Byte offset of the first operand-register field (spec Section 1).
pub const PAYLOAD_BYTE_A: usize = 2;
/// Byte offset of the second operand-register field (spec Section 1).
pub const PAYLOAD_BYTE_B: usize = 3;

/// `repr(C)` so the payload bytes are at fixed, assert-pinned offsets and
/// the FFI/codec surface can reason byte-wise without a codec (A11
/// `BN3Statement` precedent, spec 4.3 rationale).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BuiltinPayload {
    /// Byte 0: the dispatch opcode (one of `ALL_OPCODES`).
    pub opcode: u8,
    /// Byte 1: destination register id, `R0..R255`.
    pub dst: u8,
    /// Byte 2: first operand register id, `R0..R255`.
    pub a: u8,
    /// Byte 3: second operand register id, `R0..R255`.
    pub b: u8,
}

/// Frozen exit-gate size of `BuiltinPayload` in bytes: opcode + three
/// register ids, one byte each (spec Section 1).
pub const BUILTIN_PAYLOAD_SIZE: usize = 4;

/// Reserved payload bits: the term payload is 56 bits (A11), the builtin
/// layout consumes the low 32; the high 24 must be zero on encode
/// (`decode` refuses dirty bits as `ReservedOperand`).
pub const RESERVED_PAYLOAD_BITS: u32 = PAYLOAD_BITS - (BUILTIN_PAYLOAD_SIZE as u32) * 8;

// --- ABI freeze: compile-time layout laws (spec Sections 1-3) ------------
// Mutation that MUST fail the build: renumber any opcode constant below;
// widen any `BuiltinPayload` field; reorder the payload fields; mint a
// tenth opcode without widening `OPCODE_COUNT`.
const _: () = assert!(core::mem::size_of::<BuiltinPayload>() == BUILTIN_PAYLOAD_SIZE);
const _: () = assert!(core::mem::align_of::<BuiltinPayload>() == 1);
const _: () = assert!(core::mem::offset_of!(BuiltinPayload, opcode) == PAYLOAD_BYTE_OPCODE);
const _: () = assert!(core::mem::offset_of!(BuiltinPayload, dst) == PAYLOAD_BYTE_DST);
const _: () = assert!(core::mem::offset_of!(BuiltinPayload, a) == PAYLOAD_BYTE_A);
const _: () = assert!(core::mem::offset_of!(BuiltinPayload, b) == PAYLOAD_BYTE_B);
const _: () = assert!(OP_LOG_TRACE == 0x01);
const _: () = assert!(OP_LOG_ASSERT == 0x02);
const _: () = assert!(OP_LOG_COUNTERS == 0x03);
const _: () = assert!(OP_MATH_GREATER_THAN == 0x10);
const _: () = assert!(OP_MATH_SUM == 0x11);
const _: () = assert!(OP_MATH_MUL10 == 0x12);
const _: () = assert!(OP_MATH_MIN == 0x13);
const _: () = assert!(OP_CRYPTO_BLAKE3 == 0x20);
const _: () = assert!(OP_CRYPTO_BLAKE3_WIDE == 0x21);
const _: () = assert!(LOG_NS_FIRST == OP_LOG_TRACE && LOG_NS_LAST == OP_LOG_COUNTERS);
const _: () = assert!(MATH_NS_FIRST == OP_MATH_GREATER_THAN && MATH_NS_LAST == OP_MATH_MIN);
const _: () = assert!(
    CRYPTO_NS_FIRST == OP_CRYPTO_BLAKE3 && CRYPTO_NS_LAST == OP_CRYPTO_BLAKE3_WIDE
);
const _: () = assert!(LOG_NS_LAST < MATH_NS_FIRST);
const _: () = assert!(MATH_NS_LAST < CRYPTO_NS_FIRST);
const _: () = assert!(ALL_OPCODES.len() == OPCODE_COUNT);
const _: () = assert!(OPCODE_COUNT == 3 + 4 + 2);
const _: () = assert!(FREE_OPCODE_COUNT == 247);
const _: () = assert!(REGISTER_COUNT == 256);
const _: () = assert!(MAX_REGISTER_ID == u8::MAX);
const _: () = assert!(BUILTIN_PAYLOAD_SIZE == 4);
const _: () = assert!(RESERVED_PAYLOAD_BITS == 24);
// One-way coupling to A11's sigil authority (spec Section 7): the builtin
// sigil is 0x04 and the builtin payload fits the 56-bit term payload.
const _: () = assert!(Sigil::BuiltinOp as u8 == 0x04);
const _: () = assert!((BUILTIN_PAYLOAD_SIZE as u32) * 8 <= PAYLOAD_BITS);

// --- Wave 2: typed refusals (DoD 7) ---------------------------------------

/// The 56-bit ceiling of the register-value domain: math operands and
/// results are 56-bit unsigned payloads (spec 3.2 preamble, Section 7).
/// Cited, not redefined: this is A11's `MAX_PAYLOAD` (the term-payload
/// ceiling) re-exported under the register-domain name.
pub use crate::sigil::MAX_PAYLOAD as PAYLOAD_CEILING;

/// Typed refusal surface of the builtin dispatch (spec DoD 7). Every
/// failure is one of these five variants; there is no panic path, no
/// fallback-to-identity, no lossy coercion.
///
/// Seam note: A11 may later land a crate-level `BN3Error`; this enum stays
/// lane-local so that landing is an additive `From` bridge, not a fork.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuiltinError {
    /// The opcode byte is not in `ALL_OPCODES` — a closed-registry refusal,
    /// never a fallback (spec 5.2 Mutation 2 pins this for 0x13).
    UnknownOpcode(u8),
    /// `math:sum`/`math:mul10` result exceeds the 56-bit ceiling. `a` is
    /// the first operand value; `b` is the second operand value, or the
    /// fixed multiplier 10 for `math:mul10` (spec 3.2, Section 7).
    Overflow {
        /// First operand value (the multiplier for `math:mul10`'s `b`).
        a: u64,
        /// Second operand value, or the fixed multiplier 10 for mul10.
        b: u64,
    },
    /// `log:assert` observed `R[dst] != R[exp]`; `actual` is the observed
    /// register value (spec 3.1, 5.1).
    AssertFailed {
        /// The failing register value, carried for the refusal court.
        actual: u64,
    },
    /// A crypto `(ptr, len)` operand pair does not resolve to a real byte
    /// range of the slab: out of bounds, or `ptr + len` wraps u64 (spec
    /// 3.3, Section 7).
    InvalidRange {
        /// The raw pointer value from the operand register.
        ptr: u64,
        /// The raw length value from the operand register.
        len: u64,
    },
    /// A reserved field carried a nonzero value: `math:mul10`'s `b`
    /// register (spec 3.2), or dirty bits 32..56 of a payload word on
    /// `decode` (the 24 reserved payload bits).
    ReservedOperand {
        /// The opcode whose operand was refused.
        opcode: u8,
        /// Which field: `"b"` for mul10, `"reserved_payload_bits"` on
        /// decode.
        field: &'static str,
    },
}

impl core::fmt::Display for BuiltinError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            BuiltinError::UnknownOpcode(op) => write!(f, "unknown builtin opcode 0x{op:02X}"),
            BuiltinError::Overflow { a, b } => {
                write!(f, "56-bit overflow: {a} +* {b} exceeds 0x{PAYLOAD_CEILING:X}")
            }
            BuiltinError::AssertFailed { actual } => {
                write!(f, "log:assert failed: actual register value {actual}")
            }
            BuiltinError::InvalidRange { ptr, len } => {
                write!(f, "crypto range [{ptr}, {ptr}+{len}) outside the slab")
            }
            BuiltinError::ReservedOperand { opcode, field } => {
                write!(f, "opcode 0x{opcode:02X}: reserved field `{field}` must be zero")
            }
        }
    }
}

// Wave 5 repair (a11-wave4 audit receipt §2 + open item 3): `core` not
// `std` — the crate's scaffold direction is `#![no_std]` (core/alloc
// only; A11-owned files carry zero `std::` paths) and `std::error::Error`
// is the `core::error::Error` re-export, so the impl needs no std. The
// hygiene courts below pin this from source bytes, not prose.
impl core::error::Error for BuiltinError {}

/// Real eval-loop state the `log:` builtins observe (spec 5.1: "counters
/// latch observed through the real eval-loop counter struct"). The unifier
/// (A12) will own the production context; until then this lane provides
/// the struct the `log:` arms are tested against — it is the honest
/// substrate, not a stub.
#[derive(Debug, Default)]
pub struct BuiltinContext {
    /// Dispatch-invocation count. Incremented on every `dispatch` entry,
    /// including dispatches that refuse — refusals are evaluations.
    /// `log:counters` latches this value (inclusive of its own dispatch).
    /// Wave 4 (DoD 7): the increment is `wrapping_add`, so a counter at
    /// `u64::MAX` wraps to 0 instead of panicking under debug
    /// overflow-checks — dispatch has no panic path in either profile.
    pub eval_count: u64,
    /// Trace sink: `log:trace` appends the observed register value.
    pub trace: Vec<u64>,
    /// Full 32-byte digest of the most recent crypto builtin (0x20/0x21).
    /// A register holds 56 bits, not 256, so the digest's register landing
    /// (first 7 bytes, little-endian — see `digest_word56`) is a reduction;
    /// the byte-for-byte known-answer court (spec 5.1) reads this field.
    pub last_digest: [u8; 32],
}

// --- Wave 2: payload encode/decode (Section 1) ----------------------------

impl BuiltinPayload {
    /// Total constructor: any four bytes are a legal payload word.
    pub const fn new(opcode: u8, dst: u8, a: u8, b: u8) -> Self {
        Self {
            opcode,
            dst,
            a,
            b,
        }
    }

    /// Pack the payload into the low 32 bits of the 56-bit term payload.
    /// Total: the high 24 reserved bits are zero by construction (four
    /// bytes cannot dirty them).
    pub const fn encode(&self) -> u64 {
        self.opcode as u64
            | (self.dst as u64) << 8
            | (self.a as u64) << 16
            | (self.b as u64) << 24
    }

    /// Unpack a 56-bit term payload into a builtin payload. Refuses dirty
    /// bits 32..56 (the reserved bits) with `ReservedOperand` — a typed
    /// refusal, never a silent mask-off.
    pub fn decode(word: u64) -> Result<Self, BuiltinError> {
        if word >> 32 != 0 {
            return Err(BuiltinError::ReservedOperand {
                opcode: (word & 0xFF) as u8,
                field: "reserved_payload_bits",
            });
        }
        Ok(Self {
            opcode: word as u8,
            dst: (word >> 8) as u8,
            a: (word >> 16) as u8,
            b: (word >> 24) as u8,
        })
    }
}

/// The crypto register landing: the first 7 digest bytes, little-endian —
/// a 56-bit-safe reduction of BLAKE3-256. The full 32 bytes land in
/// `BuiltinContext::last_digest`; the reduction is what `R[dst]` holds.
fn digest_word56(digest: &[u8; 32]) -> u64 {
    let mut word = 0u64;
    for (i, byte) in digest[..7].iter().enumerate() {
        word |= (*byte as u64) << (8 * i);
    }
    word
}

/// Dispatch one builtin application: one `match` on the u8 opcode,
/// executed directly on the register file. No `dyn`, no `fn` values, no
/// registry object (spec Section 2 ADR — the closed match is the whole
/// registry; duplicate opcode arms are a compile error under
/// `-D warnings`).
///
/// `context.eval_count` increments on entry — including refusals.
///
/// Semantics per arm: spec Section 3 tables. The crypto arms resolve
/// `(R[a], R[b])` as `(ptr, len)` against `slab` (A11's Wave-2 arena slab;
/// until it lands, the caller supplies the real byte range — tests use
/// real `&[u8]` slabs, never a fake) and land the full digest in
/// `context.last_digest`, with the first 7 bytes little-endian in
/// `R[dst]` (`digest_word56`).
pub fn dispatch(
    payload: BuiltinPayload,
    regs: &mut [u64; REGISTER_COUNT],
    slab: &[u8],
    context: &mut BuiltinContext,
) -> Result<u64, BuiltinError> {
    // Wave 4 hardening (DoD 7): wrapping, not `+=` — a `+=` increment is a
    // debug-profile panic path at 2^64 dispatches, which would falsify the
    // "no panic path" law in the profile tests actually run under. Perf
    // counters wrap; the wrap point is unreachable in any real eval loop.
    context.eval_count = context.eval_count.wrapping_add(1);
    // Total index: every payload register byte is < 256 == REGISTER_COUNT.
    let dst = payload.dst as usize;
    match payload.opcode {
        OP_LOG_TRACE => {
            // (dst) trace-sink write; result = input (identity). a/b unused.
            let value = regs[dst];
            context.trace.push(value);
            Ok(value)
        }
        OP_LOG_ASSERT => {
            // (dst, exp): exp is the `a` register. R[dst]==R[a] else
            // AssertFailed carrying the actual value; ok -> dst=1.
            let actual = regs[dst];
            let expected = regs[payload.a as usize];
            if actual == expected {
                regs[dst] = 1;
                Ok(1)
            } else {
                Err(BuiltinError::AssertFailed { actual })
            }
        }
        OP_LOG_COUNTERS => {
            // (dst) latch the eval counter (inclusive of this dispatch).
            let n = context.eval_count;
            regs[dst] = n;
            Ok(n)
        }
        OP_MATH_GREATER_THAN => {
            // (dst, a, b): R[dst] = (R[a] > R[b]) as u64. Total.
            let x = regs[payload.a as usize];
            let y = regs[payload.b as usize];
            let r = u64::from(x > y);
            regs[dst] = r;
            Ok(r)
        }
        OP_MATH_SUM => {
            // (dst, a, b): R[dst] = R[a] + R[b], checked twice (spec
            // Section 7 top correctness risk): checked_add never sees
            // 56-bit-domain overflow (sum < 2^57 always fits u64), so the
            // ceiling check is the load-bearing arm.
            let a = regs[payload.a as usize];
            let b = regs[payload.b as usize];
            let sum = a.checked_add(b).ok_or(BuiltinError::Overflow { a, b })?;
            if sum > PAYLOAD_CEILING {
                return Err(BuiltinError::Overflow { a, b });
            }
            regs[dst] = sum;
            Ok(sum)
        }
        OP_MATH_MUL10 => {
            // (dst, a): R[dst] = R[a] * 10, checked; b register must be 0
            // (reserved operand, spec 3.2).
            if payload.b != 0 || regs[payload.b as usize] != 0 {
                return Err(BuiltinError::ReservedOperand {
                    opcode: OP_MATH_MUL10,
                    field: "b",
                });
            }
            let a = regs[payload.a as usize];
            let prod = a.checked_mul(10).ok_or(BuiltinError::Overflow { a, b: 10 })?;
            if prod > PAYLOAD_CEILING {
                return Err(BuiltinError::Overflow { a, b: 10 });
            }
            regs[dst] = prod;
            Ok(prod)
        }
        OP_MATH_MIN => {
            // (dst, a, b): R[dst] = min(R[a], R[b]). Total.
            let x = regs[payload.a as usize];
            let y = regs[payload.b as usize];
            let r = if x < y { x } else { y };
            regs[dst] = r;
            Ok(r)
        }
        OP_CRYPTO_BLAKE3 => {
            // (dst, ptr, len): BLAKE3-256 via the real cgap_blake3::hash.
            let bytes = crypto_range(payload, regs, slab)?;
            let digest = cgap_blake3::hash(bytes);
            Ok(land_digest(payload.dst as usize, digest, regs, context))
        }
        OP_CRYPTO_BLAKE3_WIDE => {
            // (dst, ptr, len): SIMD hash_many_simd; result equals 0x20's.
            let bytes = crypto_range(payload, regs, slab)?;
            let digest = cgap_blake3::hash_many_simd(bytes);
            Ok(land_digest(payload.dst as usize, digest, regs, context))
        }
        // Closed-registry refusal: unassigned opcodes are typed errors,
        // never a fallback-to-identity (spec 5.2 Mutation 2).
        _ => Err(BuiltinError::UnknownOpcode(payload.opcode)),
    }
}

/// Resolve the `(ptr, len)` crypto operands against the slab. The range
/// check is total: `ptr + len` must not wrap, and `[ptr, ptr+len)` must be
/// in-bounds — `InvalidRange`, never a wild read (spec Section 7; the
/// returned slice is bounds-checked by Rust). Returns the validated byte
/// range so the dispatch arms carry no indexing at all.
///
/// Wave 4 hardening: the bounds comparison runs in `u64` against
/// `slab.len()` *before* any `usize` narrowing, so a 32-bit target cannot
/// launder an out-of-bounds range through `usize` truncation into a
/// wrong-bytes read (on a 64-bit target the guard is free redundancy).
fn crypto_range<'a>(
    payload: BuiltinPayload,
    regs: &[u64; REGISTER_COUNT],
    slab: &'a [u8],
) -> Result<&'a [u8], BuiltinError> {
    let ptr = regs[payload.a as usize];
    let len = regs[payload.b as usize];
    let end = ptr
        .checked_add(len)
        .ok_or(BuiltinError::InvalidRange { ptr, len })?;
    if ptr > slab.len() as u64 || end > slab.len() as u64 {
        return Err(BuiltinError::InvalidRange { ptr, len });
    }
    slab.get(ptr as usize..end as usize)
        .ok_or(BuiltinError::InvalidRange { ptr, len })
}

/// Land a crypto digest: full 32 bytes into the context, first 7 bytes
/// little-endian into `R[dst]`; the reduced word is the result.
fn land_digest(
    dst: usize,
    digest: [u8; 32],
    regs: &mut [u64; REGISTER_COUNT],
    context: &mut BuiltinContext,
) -> u64 {
    context.last_digest = digest;
    let word = digest_word56(&digest);
    regs[dst] = word;
    word
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sigil::MAX_PAYLOAD;
    use core::mem::{align_of, offset_of, size_of};

    /// Fresh register file with `init` overrides.
    fn regs_with(init: &[(usize, u64)]) -> [u64; REGISTER_COUNT] {
        let mut regs = [0u64; REGISTER_COUNT];
        for &(idx, value) in init {
            regs[idx] = value;
        }
        regs
    }

    /// Run dispatch with a fresh context (counter starts at 0).
    fn run(
        payload: BuiltinPayload,
        regs: &mut [u64; REGISTER_COUNT],
        slab: &[u8],
    ) -> Result<u64, BuiltinError> {
        let mut ctx = BuiltinContext::default();
        dispatch(payload, regs, slab, &mut ctx)
    }

    // ===================== Wave 1 ABI courts (regression) =====================

    /// Opcode court: every constant equals its spec literal (spec 3.1-3.3).
    /// Anti-vacuity mutation (spec 5.2 Mutation 1, const-assert arm):
    /// renumber `OP_MATH_SUM` 0x11 -> 0x10 — the const asserts above fail
    /// the BUILD, not just this test.
    #[test]
    fn opcode_court_constants_match_spec() {
        assert_eq!(OP_LOG_TRACE, 0x01);
        assert_eq!(OP_LOG_ASSERT, 0x02);
        assert_eq!(OP_LOG_COUNTERS, 0x03);
        assert_eq!(OP_MATH_GREATER_THAN, 0x10);
        assert_eq!(OP_MATH_SUM, 0x11);
        assert_eq!(OP_MATH_MUL10, 0x12);
        assert_eq!(OP_MATH_MIN, 0x13);
        assert_eq!(OP_CRYPTO_BLAKE3, 0x20);
        assert_eq!(OP_CRYPTO_BLAKE3_WIDE, 0x21);
    }

    /// Registry court: `ALL_OPCODES` has no duplicates, is strictly
    /// ascending, dense within each namespace, and namespace-gapped (spec
    /// 3, 5.2). Anti-vacuity mutation: delete any entry — the length
    /// assert fails; swap two entries — the ordering asserts fail.
    #[test]
    fn registry_court_no_duplicates_dense_gapped() {
        assert_eq!(ALL_OPCODES.len(), OPCODE_COUNT);
        assert_eq!(ALL_OPCODES.len(), 9);
        // No duplicates: every pairwise pair differs.
        for i in 0..ALL_OPCODES.len() {
            for j in (i + 1)..ALL_OPCODES.len() {
                assert_ne!(ALL_OPCODES[i], ALL_OPCODES[j], "i={i} j={j}");
            }
        }
        // Strictly ascending == dense within a namespace and gapped across.
        for w in ALL_OPCODES.windows(2) {
            assert!(w[0] < w[1], "not ascending: {:?} at {:?}", w, ALL_OPCODES);
        }
        assert_eq!(ALL_OPCODES[0], LOG_NS_FIRST);
        assert_eq!(ALL_OPCODES[2], LOG_NS_LAST);
        assert_eq!(ALL_OPCODES[3], MATH_NS_FIRST);
        assert_eq!(ALL_OPCODES[6], MATH_NS_LAST);
        assert_eq!(ALL_OPCODES[7], CRYPTO_NS_FIRST);
        assert_eq!(ALL_OPCODES[8], CRYPTO_NS_LAST);
        // Namespace windows hold every member and exclude every neighbor.
        assert!(ALL_OPCODES[..3].iter().all(|&op| (LOG_NS_FIRST..=LOG_NS_LAST).contains(&op)));
        assert!(ALL_OPCODES[3..7].iter().all(|&op| (MATH_NS_FIRST..=MATH_NS_LAST).contains(&op)));
        assert!(ALL_OPCODES[7..].iter().all(|&op| (CRYPTO_NS_FIRST..=CRYPTO_NS_LAST).contains(&op)));
    }

    /// Payload-layout court: 4 bytes, align 1, pinned byte offsets, and the
    /// layout still fits A11's 56-bit term payload with 24 reserved bits
    /// (spec Section 1, Section 7). Anti-vacuity mutation: widen `b: u8` to
    /// `u32` or reorder fields — the const asserts fail the BUILD.
    #[test]
    fn payload_court_layout_is_4_bytes() {
        assert_eq!(size_of::<BuiltinPayload>(), 4);
        assert_eq!(align_of::<BuiltinPayload>(), 1);
        assert_eq!(offset_of!(BuiltinPayload, opcode), 0);
        assert_eq!(offset_of!(BuiltinPayload, dst), 1);
        assert_eq!(offset_of!(BuiltinPayload, a), 2);
        assert_eq!(offset_of!(BuiltinPayload, b), 3);
        assert_eq!(BUILTIN_PAYLOAD_SIZE, size_of::<BuiltinPayload>());
        // 32 consumed bits + 24 reserved == 56 payload bits.
        assert_eq!(RESERVED_PAYLOAD_BITS, 24);
        assert_eq!(
            (BUILTIN_PAYLOAD_SIZE as u32) * 8 + RESERVED_PAYLOAD_BITS,
            PAYLOAD_BITS
        );
    }

    /// Register-addressing court: `R0..R255` fills a `u8` exactly, so a
    /// payload register byte is a total id with no truncation law (spec
    /// Section 1). Anti-vacuity mutation: set `REGISTER_COUNT` to 512 —
    /// the const assert `REGISTER_COUNT == 256` fails the BUILD.
    #[test]
    fn register_court_r0_to_r255_fits_u8() {
        assert_eq!(REGISTER_COUNT, 256);
        assert_eq!(MAX_REGISTER_ID, 255);
        assert_eq!(MAX_REGISTER_ID, u8::MAX);
        assert_eq!(REGISTER_COUNT, MAX_REGISTER_ID as usize + 1);
    }

    /// Sigil-coupling court: this module's one-way dependency on A11's
    /// authority (spec Section 7). Anti-vacuity mutation: renumber A11's
    /// `BuiltinOp` off 0x04 — the const assert fails the BUILD for both
    /// lanes, surfacing the cross-lane break at compile time.
    #[test]
    fn sigil_court_builtinop_is_0x04() {
        assert_eq!(Sigil::BuiltinOp as u8, 0x04);
        // The registry is payload-resident: it rides under the sigil, so
        // every opcode value is far below A11's term-word payload ceiling.
        assert!(ALL_OPCODES.iter().all(|&op| (op as u64) <= MAX_PAYLOAD));
    }

    // ===================== Wave 2: dispatch courts (spec 5.1) =====================
    // Known-answer tests dispatch RAW spec opcode literals, not the consts:
    // the frozen ABI is pinned at the wire level. Mutation 3's flip mode is
    // therefore the BUILD, not a reroute: a lone const renumber is refused by
    // the ABI-freeze const-assert lattice (wave-7 receipt: E0080), and even a
    // const+literal-assert-consistent swap dies on the namespace
    // equality/gapping asserts (0x10 and 0x20 sit in different asserted
    // namespaces — no buildable consistent form). The KATs are the wire-level
    // second fence for corruptions that reroute dispatch without touching the
    // lattice — observed: M2's arm deletion reroutes 0x13 to `UnknownOpcode`
    // and `known_answer_math_min` flips.

    /// `math:greater_than` known answers (spec 5.1): (3,2)->1, (2,3)->0,
    /// (3,3)->0, bound (2^56-1, 0)->1. Mutation 3 must-kill: the 0x10/0x20
    /// const swap is refused at the BUILD (const-assert lattice, wave-7
    /// receipt); this court's reroute-class kill is observed under M2-class
    /// arm deletion (0x13 -> `UnknownOpcode` -> KAT flip).
    #[test]
    fn known_answer_math_greater_than() {
        let cases: [(u64, u64, u64); 4] = [(3, 2, 1), (2, 3, 0), (3, 3, 0), (MAX_PAYLOAD, 0, 1)];
        for (x, y, expected) in cases {
            let mut regs = regs_with(&[(1, x), (2, y)]);
            let payload = BuiltinPayload::new(0x10, 9, 1, 2);
            let r = run(payload, &mut regs, &[]).expect("greater_than is total");
            assert_eq!(r, expected, "greater_than({x},{y})");
            assert_eq!(regs[9], expected);
        }
    }

    /// `math:sum` known answers (spec 5.1): (1,2)->3; overflow pair
    /// (2^56-1, 1) -> Err(Overflow); (2^56-1, 0) -> 2^56-1 (no wrap, no
    /// silent clamp — checked, typed).
    #[test]
    fn known_answer_math_sum_checked() {
        // Plain sum.
        let mut regs = regs_with(&[(1, 1), (2, 2)]);
        let payload = BuiltinPayload::new(0x11, 9, 1, 2);
        assert_eq!(run(payload, &mut regs, &[]).unwrap(), 3);
        assert_eq!(regs[9], 3);
        // Overflow pair: checked_add alone never fails here (sum < 2^57
        // fits u64) — the ceiling check is the load-bearing arm (spec 7).
        let mut regs = regs_with(&[(1, MAX_PAYLOAD), (2, 1)]);
        let payload = BuiltinPayload::new(0x11, 9, 1, 2);
        assert_eq!(
            run(payload, &mut regs, &[]),
            Err(BuiltinError::Overflow {
                a: MAX_PAYLOAD,
                b: 1
            })
        );
        assert_eq!(regs[9], 0, "refusal must not write dst");
        // Boundary: exactly MAX_PAYLOAD, no wrap, no clamp.
        let mut regs = regs_with(&[(1, MAX_PAYLOAD), (2, 0)]);
        let payload = BuiltinPayload::new(0x11, 9, 1, 2);
        assert_eq!(run(payload, &mut regs, &[]).unwrap(), MAX_PAYLOAD);
        assert_eq!(regs[9], MAX_PAYLOAD);
        // Out-of-domain operand (> 2^56-1) is also an Overflow refusal.
        let mut regs = regs_with(&[(1, MAX_PAYLOAD + 1), (2, 0)]);
        let payload = BuiltinPayload::new(0x11, 9, 1, 2);
        assert_eq!(
            run(payload, &mut regs, &[]),
            Err(BuiltinError::Overflow {
                a: MAX_PAYLOAD + 1,
                b: 0
            })
        );
    }

    /// `math:mul10` known answers (spec 5.1): floor(Max/10) -> Ok;
    /// floor(Max/10)+1 -> Err(Overflow); b register must be 0.
    #[test]
    fn known_answer_math_mul10() {
        let max = MAX_PAYLOAD;
        let floor_div = max / 10;
        let mut regs = regs_with(&[(1, floor_div)]);
        let payload = BuiltinPayload::new(0x12, 9, 1, 0);
        assert_eq!(run(payload, &mut regs, &[]).unwrap(), floor_div * 10);
        assert_eq!(regs[9], floor_div * 10);
        // floor(Max/10)+1 overflows the 56-bit ceiling.
        let mut regs = regs_with(&[(1, floor_div + 1)]);
        let payload = BuiltinPayload::new(0x12, 9, 1, 0);
        assert_eq!(
            run(payload, &mut regs, &[]),
            Err(BuiltinError::Overflow {
                a: floor_div + 1,
                b: 10
            })
        );
        // Reserved operand: the b register (id and value) must be zero.
        let mut regs = regs_with(&[(1, 1), (3, 7)]);
        let payload = BuiltinPayload::new(0x12, 9, 1, 3);
        assert_eq!(
            run(payload, &mut regs, &[]),
            Err(BuiltinError::ReservedOperand {
                opcode: OP_MATH_MUL10,
                field: "b"
            })
        );
        let mut regs = regs_with(&[(1, 1)]);
        let payload = BuiltinPayload::new(0x12, 9, 1, 0);
        assert!(run(payload, &mut regs, &[]).is_ok(), "b=0 is the legal form");
    }

    /// `math:min` known answers (spec 5.1): total, bounds 0 and Max.
    /// Mutation 2 must-kill: deleting the min arm routes 0x13 to
    /// UnknownOpcode and flips this court.
    #[test]
    fn known_answer_math_min() {
        let cases: [(u64, u64, u64); 5] = [
            (0, MAX_PAYLOAD, 0),
            (MAX_PAYLOAD, 0, 0),
            (5, 9, 5),
            (9, 5, 5),
            (MAX_PAYLOAD, MAX_PAYLOAD, MAX_PAYLOAD),
        ];
        for (x, y, expected) in cases {
            let mut regs = regs_with(&[(1, x), (2, y)]);
            let payload = BuiltinPayload::new(0x13, 9, 1, 2);
            let r = run(payload, &mut regs, &[]).expect("min is total");
            assert_eq!(r, expected, "min({x},{y})");
            assert_eq!(regs[9], expected);
        }
    }

    /// `crypto:blake3` known answer (spec 5.1): b"abc" -> the exact
    /// BLAKE3-256 digest, asserted both against the reference literal and
    /// against a direct `cgap_blake3::hash` call, byte-for-byte. This test
    /// doubles as the dependency integration fence (spec Section 7).
    ///
    /// DIVERGENCE (doc yields, 2026-11-01): the lane doc's literal
    /// `ba7816bf8f01cfea...` is SHA-256("abc"), not BLAKE3("abc") — a doc
    /// error this fence caught on first run. The literal below is
    /// reference BLAKE3("abc"), verified against `b3sum` (official BLAKE3
    /// tool) this session; `cgap_blake3::hash` matches it byte-for-byte.
    #[test]
    fn known_answer_crypto_blake3_abc() {
        const SLAB: &[u8] = b"abc";
        const EXPECTED: [u8; 32] = [
            0x64, 0x37, 0xb3, 0xac, 0x38, 0x46, 0x51, 0x33, 0xff, 0xb6, 0x3b, 0x75, 0x27, 0x3a,
            0x8d, 0xb5, 0x48, 0xc5, 0x58, 0x46, 0x5d, 0x79, 0xdb, 0x03, 0xfd, 0x35, 0x9c, 0x6c,
            0xd5, 0xbd, 0x9d, 0x85,
        ];
        // The substrate itself must produce the literal — the fence proves
        // the dependency is the real BLAKE3, not a look-alike.
        assert_eq!(cgap_blake3::hash(SLAB), EXPECTED);

        let mut regs = regs_with(&[(1, 0), (2, 3)]); // ptr=0, len=3
        let payload = BuiltinPayload::new(0x20, 9, 1, 2);
        let mut ctx = BuiltinContext::default();
        let word = dispatch(payload, &mut regs, SLAB, &mut ctx).expect("in-bounds range");
        // Byte-for-byte: full digest in the context.
        assert_eq!(ctx.last_digest, EXPECTED, "context digest vs spec literal");
        assert_eq!(ctx.last_digest, cgap_blake3::hash(SLAB), "context digest vs direct call");
        // Register landing: first 7 bytes little-endian, 56-bit-safe.
        assert_eq!(word, digest_word56(&EXPECTED));
        assert_eq!(regs[9], word);
        assert!(word <= MAX_PAYLOAD, "landing must respect the 56-bit domain");
    }

    /// `crypto:blake3_wide` known answer (spec 5.1): 4 KiB input through
    /// dispatch vs direct `hash_many_simd` — byte-identical, and equal to
    /// `hash` of the same bytes (the "= 0x20" table semantic, spec 3.3).
    #[test]
    fn known_answer_crypto_blake3_wide_4kib() {
        let slab: Vec<u8> = (0..4096u32).map(|i| (i % 251) as u8).collect();
        let mut regs = regs_with(&[(1, 0), (2, slab.len() as u64)]);
        let payload = BuiltinPayload::new(0x21, 9, 1, 2);
        let mut ctx = BuiltinContext::default();
        let word = dispatch(payload, &mut regs, &slab, &mut ctx).expect("in-bounds range");
        let direct_simd = cgap_blake3::hash_many_simd(&slab);
        assert_eq!(ctx.last_digest, direct_simd, "dispatch vs direct hash_many_simd");
        assert_eq!(ctx.last_digest, cgap_blake3::hash(&slab), "= 0x20 semantic");
        assert_eq!(word, digest_word56(&direct_simd));
        assert_eq!(regs[9], word);
    }

    /// Crypto range refusals (spec 3.3/7): out-of-bounds and wrapping
    /// `(ptr, len)` are `InvalidRange`, never a wild read. A zero-length
    /// range at the slab end is legal and hashes the empty slice.
    #[test]
    fn crypto_invalid_range_refusals() {
        let slab: Vec<u8> = (0..16u32).map(|i| i as u8).collect();
        let empty_digest = cgap_blake3::hash(&[]);
        // ptr beyond the slab end.
        let mut regs = regs_with(&[(1, 17), (2, 1)]);
        let payload = BuiltinPayload::new(0x20, 9, 1, 2);
        assert_eq!(
            run(payload, &mut regs, &slab),
            Err(BuiltinError::InvalidRange { ptr: 17, len: 1 })
        );
        // ptr + len wraps u64.
        let mut regs = regs_with(&[(1, 1), (2, u64::MAX)]);
        let payload = BuiltinPayload::new(0x20, 9, 1, 2);
        assert_eq!(
            run(payload, &mut regs, &slab),
            Err(BuiltinError::InvalidRange {
                ptr: 1,
                len: u64::MAX
            })
        );
        // Zero-length range at the exact slab end: legal, empty digest.
        let mut regs = regs_with(&[(1, 16), (2, 0)]);
        let payload = BuiltinPayload::new(0x20, 9, 1, 2);
        let mut ctx = BuiltinContext::default();
        assert!(dispatch(payload, &mut regs, &slab, &mut ctx).is_ok());
        assert_eq!(ctx.last_digest, empty_digest);
        // Zero-length range past the end: still refused.
        let mut regs = regs_with(&[(1, 17), (2, 0)]);
        let payload = BuiltinPayload::new(0x20, 9, 1, 2);
        assert_eq!(
            run(payload, &mut regs, &slab),
            Err(BuiltinError::InvalidRange { ptr: 17, len: 0 })
        );
    }

    /// `log:assert` known answers (spec 3.1, 5.1): success writes 1 to dst;
    /// failure returns `AssertFailed` carrying the actual register value.
    #[test]
    fn known_answer_log_assert() {
        // Success: R[dst]==R[exp] -> dst=1, result 1.
        let mut regs = regs_with(&[(5, 42), (6, 42)]);
        let payload = BuiltinPayload::new(0x02, 5, 6, 0);
        assert_eq!(run(payload, &mut regs, &[]).unwrap(), 1);
        assert_eq!(regs[5], 1);
        // Failure: carries the actual value; dst untouched.
        let mut regs = regs_with(&[(5, 42), (6, 43)]);
        let payload = BuiltinPayload::new(0x02, 5, 6, 0);
        assert_eq!(
            run(payload, &mut regs, &[]),
            Err(BuiltinError::AssertFailed { actual: 42 })
        );
        assert_eq!(regs[5], 42, "refusal must not write dst");
    }

    /// `log:trace` identity court (spec 3.1, 5.1): result = input, dst
    /// unchanged, the value observed in the real trace sink.
    #[test]
    fn log_trace_identity_court() {
        let mut regs = regs_with(&[(7, 123)]);
        let payload = BuiltinPayload::new(0x01, 7, 0, 0);
        let mut ctx = BuiltinContext::default();
        assert_eq!(dispatch(payload, &mut regs, &[], &mut ctx).unwrap(), 123);
        assert_eq!(regs[7], 123, "identity: dst unchanged");
        assert_eq!(ctx.trace, vec![123]);
        dispatch(payload, &mut regs, &[], &mut ctx).unwrap();
        assert_eq!(ctx.trace, vec![123, 123], "sink appends per dispatch");
    }

    /// `log:counters` latch court (spec 3.1, 5.1): the real eval-loop
    /// counter is latched into dst, inclusive of the latching dispatch.
    #[test]
    fn log_counters_latch_court() {
        let mut regs = [0u64; REGISTER_COUNT];
        let mut ctx = BuiltinContext::default();
        // First dispatch: the counters dispatch itself -> latches 1.
        let payload = BuiltinPayload::new(0x03, 9, 0, 0);
        assert_eq!(dispatch(payload, &mut regs, &[], &mut ctx).unwrap(), 1);
        assert_eq!(regs[9], 1);
        assert_eq!(ctx.eval_count, 1);
        // Two more dispatches (any arms), then latch again -> 4 total.
        let trace = BuiltinPayload::new(0x01, 7, 0, 0);
        dispatch(trace, &mut regs, &[], &mut ctx).unwrap();
        dispatch(trace, &mut regs, &[], &mut ctx).unwrap();
        let payload = BuiltinPayload::new(0x03, 9, 0, 0);
        assert_eq!(dispatch(payload, &mut regs, &[], &mut ctx).unwrap(), 4);
        assert_eq!(regs[9], 4);
    }

    /// Closed-registry refusal court (spec 5.2 Mutation 2): unassigned
    /// opcodes — including the namespace gaps 0x00 and 0x04 and the
    /// unassigned ceiling 0xFF — are typed `UnknownOpcode` refusals, never
    /// a fallback-to-identity.
    #[test]
    fn unknown_opcode_typed_refusal() {
        for op in [0x00u8, 0x04, 0x0F, 0x14, 0x1F, 0x22, 0xFF] {
            let mut regs = regs_with(&[(1, 1), (2, 2)]);
            let payload = BuiltinPayload::new(op, 9, 1, 2);
            assert_eq!(
                run(payload, &mut regs, &[]),
                Err(BuiltinError::UnknownOpcode(op)),
                "opcode 0x{op:02X} must refuse typed"
            );
        }
    }

    /// Every registered opcode is dispatchable (spec 5.2 Mutation 2
    /// generalization): deleting ANY arm — not just math:min — flips this
    /// court, because the deleted opcode falls through to UnknownOpcode.
    #[test]
    fn all_registered_opcodes_dispatch() {
        let slab: [u8; 4] = [1, 2, 3, 4];
        for &op in ALL_OPCODES.iter() {
            let mut regs = regs_with(&[(1, 0), (2, 4)]);
            let (dst, a, b) = match op {
                OP_MATH_MUL10 => (9u8, 1u8, 0u8),
                OP_LOG_ASSERT => (1u8, 1u8, 0u8),
                _ => (9u8, 1u8, 2u8),
            };
            let payload = BuiltinPayload::new(op, dst, a, b);
            let mut ctx = BuiltinContext::default();
            let result = dispatch(payload, &mut regs, &slab, &mut ctx);
            assert!(
                result.is_ok(),
                "registered opcode 0x{op:02X} must dispatch, got {result:?}"
            );
        }
    }

    /// Payload encode/decode round-trip court: all 9 opcodes survive
    /// `encode` -> `decode` exactly; dirty reserved bits (32..56) and
    /// sigil-position bits refuse as `ReservedOperand`.
    #[test]
    fn payload_encode_decode_roundtrip() {
        for &op in ALL_OPCODES.iter() {
            let payload = BuiltinPayload::new(op, 200, 100, 50);
            let word = payload.encode();
            assert!(word <= MAX_PAYLOAD, "encoded word must fit 56 bits");
            assert_eq!(BuiltinPayload::decode(word), Ok(payload), "opcode 0x{op:02X}");
        }
        // Byte-level layout check against the spec: [opcode | dst | a | b].
        let word = BuiltinPayload::new(0x11, 0x22, 0x33, 0x44).encode();
        assert_eq!(word, 0x0000_0000_4433_2211);
        // Dirty reserved bits 32..56: typed refusal, never a mask-off.
        assert_eq!(
            BuiltinPayload::decode(0x0001_4433_2211),
            Err(BuiltinError::ReservedOperand {
                opcode: 0x11,
                field: "reserved_payload_bits"
            })
        );
        // Sigil-position bits (56..64) passed by mistake: also refused.
        assert_eq!(
            BuiltinPayload::decode(0x0400_0000_0000_0001),
            Err(BuiltinError::ReservedOperand {
                opcode: 0x01,
                field: "reserved_payload_bits"
            })
        );
    }

    /// Error-surface court: every DoD-7 variant constructs, compares, and
    /// renders; no panic path exists in dispatch (structural: no
    /// unwrap/expect/panic in the module outside tests).
    #[test]
    fn builtin_error_surface_court() {
        let errors = [
            BuiltinError::UnknownOpcode(0xAB),
            BuiltinError::Overflow {
                a: 1,
                b: 2,
            },
            BuiltinError::AssertFailed { actual: 42 },
            BuiltinError::InvalidRange { ptr: 9, len: 1 },
            BuiltinError::ReservedOperand {
                opcode: 0x12,
                field: "b",
            },
        ];
        for e in errors {
            let rendered = format!("{e}");
            assert!(!rendered.is_empty());
            assert_eq!(e, e, "PartialEq derive intact");
        }
        assert_eq!(
            format!("{}", BuiltinError::UnknownOpcode(0x13)),
            "unknown builtin opcode 0x13"
        );
    }

    // ===================== Wave 4: hardening courts =====================

    /// Exhaustive closed-registry sweep (DoD 2, hardening): EVERY `u8`
    /// opcode is either a registered dispatch (`Ok`) or a typed
    /// `UnknownOpcode` refusal — there is no third outcome, no fallback,
    /// no panic. Generalizes both `unknown_opcode_typed_refusal` (7
    /// samples) and `all_registered_opcodes_dispatch` (registered codes
    /// only) to the full 256-value space.
    ///
    /// Mutation 2 must-kill generalization: deleting ANY dispatch arm
    /// flips this court — the deleted opcode is still in `ALL_OPCODES`, so
    /// its `is_ok` assertion fails; adding a tenth arm would flip the
    /// build (`OPCODE_COUNT` const asserts).
    #[test]
    fn registry_exhaustively_closed_sweep() {
        let slab: [u8; 4] = [1, 2, 3, 4];
        for op in 0u8..=u8::MAX {
            let mut regs = regs_with(&[(1, 0), (2, 4)]);
            let (dst, a, b) = match op {
                OP_MATH_MUL10 => (9u8, 1u8, 0u8),
                OP_LOG_ASSERT => (1u8, 1u8, 0u8),
                _ => (9u8, 1u8, 2u8),
            };
            let payload = BuiltinPayload::new(op, dst, a, b);
            let mut ctx = BuiltinContext::default();
            let result = dispatch(payload, &mut regs, &slab, &mut ctx);
            if ALL_OPCODES.contains(&op) {
                assert!(
                    result.is_ok(),
                    "registered opcode 0x{op:02X} must dispatch, got {result:?}"
                );
            } else {
                assert_eq!(
                    result,
                    Err(BuiltinError::UnknownOpcode(op)),
                    "unregistered opcode 0x{op:02X} must refuse typed"
                );
            }
        }
    }

    /// Refusal write-isolation court (DoD 7 hardening): a typed refusal is
    /// an observation, not a partial write. For every refusal path —
    /// `UnknownOpcode`, all three `Overflow` arms, `ReservedOperand` (both
    /// mul10 and decode), `InvalidRange` (both bounds arms), and
    /// `AssertFailed` — the register file is bit-identical after the
    /// refused dispatch, while `eval_count` still counted it (refusals are
    /// evaluations). Anti-vacuity: the SHIM court below proves this test
    /// kills an arm that writes before refusing.
    #[test]
    fn refusal_write_isolation_court() {
        let slab: Vec<u8> = (0..16u32).map(|i| i as u8).collect();
        // (name, init, payload, slab_len_used) — all refusal outcomes.
        let cases: [(&str, [(usize, u64); 2], BuiltinPayload, usize); 8] = [
            ("unknown", [(1, 1), (2, 2)], BuiltinPayload::new(0x00, 9, 1, 2), 16),
            ("sum-overflow", [(1, MAX_PAYLOAD), (2, 1)], BuiltinPayload::new(0x11, 9, 1, 2), 16),
            ("sum-oob-operand", [(1, MAX_PAYLOAD + 1), (2, 0)], BuiltinPayload::new(0x11, 9, 1, 2), 16),
            ("mul10-overflow", [(1, MAX_PAYLOAD), (2, 0)], BuiltinPayload::new(0x12, 9, 1, 0), 16),
            ("mul10-reserved-b", [(1, 1), (3, 7)], BuiltinPayload::new(0x12, 9, 1, 3), 16),
            ("crypto-oob", [(1, 17), (2, 1)], BuiltinPayload::new(0x20, 9, 1, 2), 16),
            ("crypto-wrap", [(1, 1), (2, u64::MAX)], BuiltinPayload::new(0x20, 9, 1, 2), 16),
            ("assert-failed", [(5, 42), (6, 43)], BuiltinPayload::new(0x02, 5, 6, 0), 16),
        ];
        for (name, init, payload, slab_len) in cases {
            let mut regs = regs_with(&init);
            let before = regs;
            let mut ctx = BuiltinContext::default();
            let result = dispatch(payload, &mut regs, &slab[..slab_len], &mut ctx);
            assert!(result.is_err(), "{name}: expected a typed refusal");
            assert_eq!(regs, before, "{name}: refusal must write no register");
            assert_eq!(
                ctx.eval_count, 1,
                "{name}: a refusal is still one evaluation"
            );
        }
        // decode-side ReservedOperand: the encode/decode path refuses dirty
        // reserved bits without any register file in reach — the isolation
        // law there is "no partial payload": decode returns Err, not a
        // half-decoded Some.
        assert_eq!(
            BuiltinPayload::decode(0x0001_4433_2211),
            Err(BuiltinError::ReservedOperand {
                opcode: 0x11,
                field: "reserved_payload_bits"
            })
        );
    }

    /// Eval-counter wrap court (DoD 7 hardening): the `eval_count`
    /// increment is wrapping, so the u64::MAX dispatch does not panic in
    /// the debug profile and the counter wraps to 0 — a named law of the
    /// counter, pinned here so a future `+=` regression flips this court.
    #[test]
    fn eval_count_wraps_at_u64_max_court() {
        let mut regs = [0u64; REGISTER_COUNT];
        let mut ctx = BuiltinContext::default();
        ctx.eval_count = u64::MAX;
        let payload = BuiltinPayload::new(0x03, 9, 0, 0); // log:counters
        let n = dispatch(payload, &mut regs, &[], &mut ctx)
            .expect("wrapping increment has no panic path at u64::MAX");
        assert_eq!(n, 0, "u64::MAX wraps to 0");
        assert_eq!(regs[9], 0);
        assert_eq!(ctx.eval_count, 0);
    }

    // ===================== Wave 5: module-hygiene courts =====================
    // The a11-wave4 audit receipt observed "A11-owned files contain zero
    // `std::` paths (core/alloc only)" and left "add `#![no_std]` to
    // lib.rs" as a coordinator open item; this module's body carried the
    // crate's only non-test `std::` path (the `Error` impl, now
    // `core::error::Error`). These courts pin module hygiene from source
    // bytes — the same file the compiler reads — instead of prose.

    /// Common substrate of the hygiene courts: the module body, comment
    /// lines stripped. `//`-comments (`//!`, `///`, `//`) are cut at the
    /// first `//` of the line; no string literal in this module contains
    /// `//`, so the cut is total over code content. Reading the real file
    /// (the same bytes the compiler reads) is what makes a mutation
    /// sandbox flip these courts on the mutated bytes.
    fn module_body_stripped() -> String {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/builtins.rs"));
        let body = src
            .split("#[cfg(test)]")
            .next()
            .expect("module body precedes the test module");
        body.lines()
            .map(|line| match line.find("//") {
                Some(i) => &line[..i],
                None => line,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// no-std hygiene court: the module body — comments stripped —
    /// carries no `std::` path. Anti-vacuity mutation (must-kill):
    /// restore `impl std::error::Error for BuiltinError` in the body —
    /// this court MUST flip.
    #[test]
    fn nostd_hygiene_court_module_body_has_no_std_paths() {
        let body = module_body_stripped();
        assert!(
            !body.contains("std::"),
            "module body carries a `std::` path; crate direction is no_std (core/alloc only)"
        );
    }

    /// Panic-hygiene court (DoD 7): the module body — comments stripped —
    /// has no runtime panic site: no `unwrap()`, `.expect(`, `panic!`,
    /// `unimplemented!`, `todo!`, or `unreachable!` outside the test
    /// module. The const-eval `assert!`s of the ABI-freeze block are
    /// build-time law, exempt; an indexing panic would have no lexeme, so
    /// this is the lexeme fence alongside (not a replacement for) the
    /// total-index argument in the module docs. Anti-vacuity mutation
    /// (must-kill): insert `.unwrap()` on any body expression — this
    /// court MUST flip.
    #[test]
    fn panic_hygiene_court_module_body_has_no_panic_sites() {
        let body = module_body_stripped();
        for lexeme in [
            "unwrap()",
            ".expect(",
            "panic!",
            "unimplemented!",
            "todo!",
            "unreachable!",
        ] {
            assert!(
                !body.contains(lexeme),
                "module body carries a runtime panic site: {lexeme:?}"
            );
        }
    }

    // ===================== Wave 6: hostile-domain court =====================
    // The wave-5 court receipts' one code-level defect class is a03-wave5
    // F-1: "debug-profile arithmetic overflow panic on hostile bytes in a
    // public parser of untrusted input violates the typed-refusal
    // contract." `dispatch` is exactly such a parser (untrusted payload
    // word + register file + slab range), so the class is pinned HERE by
    // execution: the sweep runs in the debug profile, where arithmetic
    // overflow actually panics.

    /// Wave 6 hostile-domain sweep court (receipt driver: a03-wave5 F-1
    /// class). Over the full hostile domain — every `u8` opcode x hostile
    /// register values x hostile `(ptr, len)` slab ranges — dispatch must
    /// never panic in the debug profile and must land every refused case
    /// on one of the five typed `BuiltinError` variants; each refused
    /// dispatch is exactly one evaluation and writes no register (the
    /// refusal-write-isolation law re-proven over the hostile domain).
    ///
    /// Anti-vacuity mutation (must-kill, executed in the sandbox this
    /// session): replace the `math:sum` arm's `a.checked_add(b)` with
    /// `a + b` — the hostile operand pair (R1=u64::MAX, R2=1) now feeds
    /// the arm through payload-addressed registers and overflow-panics
    /// in this debug run, flipping the suite, while the 56-bit-domain
    /// known-answer courts stay green (the sweep adds kill-power the
    /// spec courts lack; their operands never overflow u64 because
    /// 2^56 + 2^56 < 2^64).
    #[test]
    fn hostile_domain_sweep_court() {
        // Hostile register domain: wrap boundaries and ceiling crosses.
        let values: [u64; 12] = [
            0,
            1,
            MAX_PAYLOAD,
            MAX_PAYLOAD + 1,
            u32::MAX as u64,
            u32::MAX as u64 + 1,
            1 << 32,
            1 << 63,
            (1 << 63) - 1,
            u64::MAX,
            u64::MAX - 1,
            u64::MAX / 2 + 1,
        ];
        let slab: Vec<u8> = (0..16u32).map(|i| i as u8).collect();
        let mut swept = 0u64;
        let mut refused = 0u64;
        for op in 0u8..=u8::MAX {
            // Operand registers (a=1, b=2) are addressed by the payload
            // fields themselves, so both hostile values feed every math
            // arm (the mul10 reserved-b law refuses when vb != 0 — that
            // refusal is part of the swept surface).
            for &va in values.iter() {
                for &vb in values.iter() {
                    let mut regs = regs_with(&[(1, va), (2, vb)]);
                    let before = regs;
                    let payload = BuiltinPayload::new(op, 9, 1, 2);
                    let mut ctx = BuiltinContext::default();
                    let result = dispatch(payload, &mut regs, &slab, &mut ctx);
                    swept += 1;
                    if result.is_err() {
                        refused += 1;
                        assert_eq!(
                            regs, before,
                            "op=0x{op:02X} va={va:#x} vb={vb:#x}: refusal must write no register"
                        );
                    }
                    assert_eq!(
                        ctx.eval_count, 1,
                        "op=0x{op:02X}: a hostile dispatch is still one evaluation"
                    );
                }
            }
        }
        assert_eq!(swept, 256 * 12 * 12);
        assert!(
            refused > 0,
            "the hostile domain must actually reach refusals (a sweep that never refuses is vacuous)"
        );
    }

    /// Wave 6 decode hostile-word court (same F-1 class): every single-bit
    /// word over all 64 bit positions, plus all-ones and both dirty-bit
    /// classes, decodes to `Ok` or the typed `ReservedOperand` refusal —
    /// never a panic, never a mask-off. Anti-vacuity: masking the dirty
    /// bits off (`word & 0xFFFF_FFFF` inside decode) flips the dirty-bit
    /// assertions of `payload_encode_decode_roundtrip` AND this court's
    /// high-word cases.
    #[test]
    fn hostile_decode_word_court() {
        let mut swept = 0u64;
        let mut refused = 0u64;
        for bit in 0u32..64 {
            let word = 1u64 << bit;
            match BuiltinPayload::decode(word) {
                Ok(_) => {}
                Err(BuiltinError::ReservedOperand { .. }) => refused += 1,
                other => panic!("bit {bit}: unexpected decode outcome {other:?}"),
            }
            swept += 1;
        }
        for word in [
            u64::MAX,
            0x0000_0000_FFFF_FFFF,
            0xFFFF_FFFF_0000_0000,
            0x0001_0000_0000_0000 | 0x21,
        ] {
            match BuiltinPayload::decode(word) {
                Ok(_) => {}
                Err(BuiltinError::ReservedOperand { .. }) => refused += 1,
                other => panic!("word {word:#x}: unexpected decode outcome {other:?}"),
            }
            swept += 1;
        }
        assert_eq!(swept, 68);
        assert!(refused >= 30, "dirty-bit words must refuse, got {refused}");
    }
}
