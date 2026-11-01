//! Authority tier: the 80-byte `PhiLease` and its monotonic `Tick` (lane A16).
//!
//! Spec: `docs/jira/v26.11.1/lanes/A16-chatman-pqc.md` §3.2 (byte layout) and
//! §5 (lease mechanics).
//!
//! Wave 1 froze the ABI (structs, consts, compile-time layout pins). Wave 2
//! (this file) lands the §5 core engine: the monotonic `tick()` advance, the
//! exhaustion TRAP (§5, lane-A17 trap discipline), and `Phi_lease` snapshot
//! production for seal time. The PQC seal itself stays Wave 3 (§4); the
//! unsealed engine path is the §4 UNSUPPORTED clause.
//!
//! Layout convention matches lane A04 `cgap-core/src/frame.rs` (Wave 1 ABI
//! freeze): `#[repr(C)]`, per-field offset docs, core-only `const _` asserts,
//! no `static_assertions` dependency.

#![allow(dead_code)] // Wave-1 ABI consts not yet read by the Wave-3 PQC path.

/// `lease_id`: 32 bytes, random at lease creation (§3.2).
pub const LEASE_ID_LEN: usize = 32;
/// `tick`: 64-bit unsigned little-endian, monotonic, starts at 0 (§3.2).
pub const TICK_LEN: usize = 8;
/// `tick_ceiling`: 64-bit unsigned little-endian, fixed at creation (§3.2).
pub const TICK_CEILING_LEN: usize = 8;
/// `issued_root`: 32-byte BLAKE3 digest binding the lease to the key epoch
/// (§3.2; key-epoch rotation is the §3.4 UNKNOWN — not decided here).
pub const ISSUED_ROOT_LEN: usize = 32;
/// Total `Phi_lease` wire size: `32 + 8 + 8 + 32` bytes (§3.2).
pub const PHI_LEASE_LEN: usize = 80;

/// Monotonic tick counter newtype (§5).
///
/// The field is private and the ONLY constructor from a raw value is
/// crate-internal (`Tick(0)` inside `Lease::new`). There is no `set_tick`,
/// no `Sub` impl, and no public or crate API that accepts a caller-supplied
/// tick: rewind is unrepresentable in the type, not merely unchecked (§5).
pub struct Tick(u64);

/// A live lease epoch: the authority term `Phi_lease` (§3.2) plus the §5
/// mechanics that make its tick counter monotonic and its exhaustion a trap.
///
/// Fields are private; the 80-byte wire form exists only as the read-only
/// [`PhiLease`] snapshot produced at seal time (`Lease::phi`, §5 "produced
/// only at seal time ... no public path exposes mutable lease bytes").
pub struct Lease {
    /// 32 B, supplied at construction; randomness is the constructor
    /// caller's policy (this zero-dependency crate owns no RNG — recorded
    /// in the lane receipt as divergence D1).
    lease_id: [u8; LEASE_ID_LEN],
    /// Monotonic counter, starts at 0 (§3.2, §5).
    tick: Tick,
    /// Fixed at construction; must be > 0 (§5).
    tick_ceiling: u64,
    /// 32 B BLAKE3 digest binding the lease to its key epoch (§3.2).
    issued_root: [u8; ISSUED_ROOT_LEN],
}

impl Lease {
    /// Creates a lease with a zero tick and a fixed ceiling (§3.2, §5).
    ///
    /// TRAPS (panic) when `tick_ceiling == 0`: §5 fixes the only ceiling
    /// policy as "ceiling > 0", and a zero-ceiling lease is exhausted at
    /// birth — an invariant failure of the abstraction, not a recoverable
    /// condition (same trap discipline as §5 exhaustion).
    pub fn new(
        lease_id: [u8; LEASE_ID_LEN],
        tick_ceiling: u64,
        issued_root: [u8; ISSUED_ROOT_LEN],
    ) -> Self {
        if tick_ceiling == 0 {
            panic!(
                "cgap-crypto: Phi_lease tick_ceiling must be > 0 (§5); got 0 \
                 — refusing a lease that is exhausted at birth"
            );
        }
        Lease {
            lease_id,
            tick: Tick(0),
            tick_ceiling,
            issued_root,
        }
    }

    /// 32 B lease identity (§3.2).
    pub fn lease_id(&self) -> &[u8; LEASE_ID_LEN] {
        &self.lease_id
    }

    /// Fixed tick ceiling (§3.2).
    pub fn tick_ceiling(&self) -> u64 {
        self.tick_ceiling
    }

    /// 32 B key-epoch binding digest (§3.2; rotation story is the §3.4
    /// UNKNOWN — this crate never decides it in prose).
    pub fn issued_root(&self) -> &[u8; ISSUED_ROOT_LEN] {
        &self.issued_root
    }

    /// Current tick value, pure read (observability/courts). Reading is not
    /// rewinding: no API mutates the counter except [`Lease::tick`].
    pub fn current_tick(&self) -> u64 {
        self.tick.0
    }

    /// §5: "returns the current value and advances it." One tick per sealed
    /// receipt; `chatman::seal_unsealed` is the only intended caller.
    ///
    /// TRAPS (panic in hosted builds; `panic = "abort"` makes this a real
    /// trap on wasm32) when the counter is exhausted: `tick ==
    /// tick_ceiling` means authority is spent. §5: an error return "would
    /// invite callers to handle authority exhaustion by minting a new lease
    /// mid-delta, silently splitting one epoch into two" — the trap makes
    /// the quiet continuation impossible; a lease that must continue mints
    /// a NEW lease through [`Lease::new`].
    pub fn tick(&mut self) -> u64 {
        let t = self.tick.0;
        if t >= self.tick_ceiling {
            panic!(
                "cgap-crypto: Phi_lease authority EXHAUSTED: tick {} == \
                 tick_ceiling {} (lease_id {:?}) — §5 exhaustion trap; mint a \
                 new lease instead of continuing this epoch",
                t, self.tick_ceiling, self.lease_id
            );
        }
        self.tick.0 = t + 1;
        t
    }

    /// Produces the 80-byte [`PhiLease`] snapshot for seal time (§5: bytes
    /// come from internal state; nothing mutable is exposed).
    ///
    /// `pub(crate)`: `chatman::seal_unsealed` is the only caller — phi bytes
    /// without a consumed tick must not leak past the crate boundary.
    /// Defense-in-depth: also TRAPS when already exhausted, so no phi with
    /// `tick >= tick_ceiling` can ever be emitted by this crate.
    pub(crate) fn phi(&self) -> PhiLease {
        let t = self.tick.0;
        if t >= self.tick_ceiling {
            panic!(
                "cgap-crypto: refusing to emit Phi_lease with tick {} >= \
                 tick_ceiling {} (lease_id {:?})",
                t, self.tick_ceiling, self.lease_id
            );
        }
        PhiLease {
            lease_id: self.lease_id,
            tick: t,
            tick_ceiling: self.tick_ceiling,
            issued_root: self.issued_root,
        }
    }
}

/// 80-byte lease epoch: `lease_id || tick || tick_ceiling || issued_root`
/// (§3.2).
///
/// `#[repr(C)]`, gap-free by construction: offsets 0, 32, 40, 48; total
/// exactly 80 bytes; multi-byte integers little-endian (§3.2). The struct
/// mirrors the canonical byte string one-to-one, so the §3.2 encoding has no
/// serialization choices to make.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PhiLease {
    /// offset 0, 32 B — random at lease creation (§3.2).
    pub lease_id: [u8; LEASE_ID_LEN],
    /// offset 32, 8 B — u64 LE, monotonic, starts at 0 (§3.2, §5).
    pub tick: u64,
    /// offset 40, 8 B — u64 LE, fixed at creation; ceiling > 0 (§5).
    pub tick_ceiling: u64,
    /// offset 48, 32 B — BLAKE3 digest binding the lease to the key epoch
    /// (§3.2).
    pub issued_root: [u8; ISSUED_ROOT_LEN],
}

// §3.2 layout pins — compile-time; a field reorder that preserves total size
// still trips the offset pins.
const _: () = assert!(core::mem::size_of::<PhiLease>() == PHI_LEASE_LEN);
const _: () = assert!(core::mem::align_of::<PhiLease>() == 8);
const _: () = assert!(core::mem::offset_of!(PhiLease, lease_id) == 0);
const _: () = assert!(core::mem::offset_of!(PhiLease, tick) == 32);
const _: () = assert!(core::mem::offset_of!(PhiLease, tick_ceiling) == 40);
const _: () = assert!(core::mem::offset_of!(PhiLease, issued_root) == 48);
const _: () = assert!(
    PHI_LEASE_LEN == LEASE_ID_LEN + TICK_LEN + TICK_CEILING_LEN + ISSUED_ROOT_LEN
);

impl PhiLease {
    /// The canonical 80-byte encoding: `lease_id || tick(LE) ||
    /// tick_ceiling(LE) || issued_root` (§3.2 — fixed order, little-endian,
    /// no serialization choices).
    pub fn to_bytes(&self) -> [u8; PHI_LEASE_LEN] {
        let mut out = [0u8; PHI_LEASE_LEN];
        out[0..32].copy_from_slice(&self.lease_id);
        out[32..40].copy_from_slice(&self.tick.to_le_bytes());
        out[40..48].copy_from_slice(&self.tick_ceiling.to_le_bytes());
        out[48..80].copy_from_slice(&self.issued_root);
        out
    }

    /// Decodes the canonical 80-byte encoding (courts / wire parsing; the
    /// crate never rebuilds a live `Lease` from bytes — a decoded `PhiLease`
    /// is an inert snapshot, not authority).
    pub fn from_bytes(bytes: &[u8; PHI_LEASE_LEN]) -> Self {
        let mut lease_id = [0u8; LEASE_ID_LEN];
        lease_id.copy_from_slice(&bytes[0..32]);
        let mut tick_bytes = [0u8; TICK_LEN];
        tick_bytes.copy_from_slice(&bytes[32..40]);
        let mut ceiling_bytes = [0u8; TICK_CEILING_LEN];
        ceiling_bytes.copy_from_slice(&bytes[40..48]);
        let mut issued_root = [0u8; ISSUED_ROOT_LEN];
        issued_root.copy_from_slice(&bytes[48..80]);
        PhiLease {
            lease_id,
            tick: u64::from_le_bytes(tick_bytes),
            tick_ceiling: u64::from_le_bytes(ceiling_bytes),
            issued_root,
        }
    }
}
