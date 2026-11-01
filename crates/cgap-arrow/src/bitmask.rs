//! Allocation-free presence bitmask over fixed-capacity slab rows
//! (lane A01, Wave 1 ABI-freeze slice; lane doc §4.4).
//!
//! Sibling of [`crate::bitmap::Bitmap`] (heap-backed over `Buf`): this
//! module is the const-constructible, zero-allocation counterpart.
//! Backing storage is an inline `[u8; BYTES]`; every constructor,
//! accessor and mutator is a `const fn`, and no `alloc` path is reachable
//! from this module (court C2 symbol audit).
//!
//! Bit order is LSB-first, identical to [`crate::bitmap::get_bit`] —
//! `(bits[i >> 3] >> (i & 7)) & 1` — one bit order codebase-wide, no
//! exceptions. Court C4 pins the order with the asymmetric probe set
//! `{0, 6}`.
//!
//! Encoding note (recorded divergence from §4.4): stable Rust cannot
//! express an inline array whose length derives arithmetically from a
//! const parameter (`[u8; mask_bytes(CAP)]` requires nightly
//! `generic_const_exprs`; the trait/where-clause encodings were probed
//! and rejected on this toolchain). The byte length is therefore a
//! second const parameter, and the §4.4 law
//! `BYTES == ceil(CAP/8)`-word-filled is enforced at const construction:
//! every constructor carries a `const` pair-law assert, so a mispaired
//! instantiation fails the build at its first const use (witnessed by
//! mutation M4b).
//!
//! Overflow law (§4.4): `get`/`set`/`clear` for `i >= CAP` return
//! `Err(BitmaskError::OutOfRange)` — never a panic (`no_std` hot paths
//! have no panic handler to spare).

/// Typed bitmask error (lane doc §4.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BitmaskError {
    /// Row index `>= CAP` refused; state unchanged.
    OutOfRange,
}

/// Byte length of the inline storage covering `cap` presence bits,
/// rounded up so it is a multiple of 8 (word-fill for SWAR-friendly host
/// scans; the re-use of [`crate::bitmap::count_valid`]-style popcount
/// over `u64` words is a target, not a claim — lane doc §4.4).
#[must_use]
pub const fn mask_bytes(cap: usize) -> usize {
    // Panic-free by construction (§4.4 no-panic law): after
    // `wrapping_add(63)` the value is <= usize::MAX, so `>> 6 << 3`
    // cannot overflow on any pointer width — this is exactly
    // `ceil(cap / 64) * 8` without the debug-profile overflow checks
    // (which emitted `panic_const_*` refs — observed, then removed).
    cap.wrapping_add(63) >> 6 << 3
}

/// Presence bitmask over `CAP` rows backed by `BYTES` inline bytes:
/// LSB-first, allocation-free, `const`-constructible. The pair law
/// `BYTES == mask_bytes(CAP)` is asserted in every constructor (§4.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PresenceMask<const CAP: usize, const BYTES: usize> {
    bits: [u8; BYTES],
}

impl<const CAP: usize, const BYTES: usize> PresenceMask<CAP, BYTES> {
    /// Inline storage byte length.
    pub const MASK_BYTES: usize = BYTES;

    /// All rows absent. Const surface (§4.4).
    pub const EMPTY: Self = {
        assert!(BYTES == mask_bytes(CAP));
        Self { bits: [0; BYTES] }
    };

    /// Bits `0..CAP` set. Padding bits beyond `CAP` are structurally
    /// kept clear — a mask never sets a bit it does not cover.
    pub const ALL_VALID: Self = {
        assert!(BYTES == mask_bytes(CAP));
        let mut m = Self { bits: [0; BYTES] };
        let mut i = 0;
        while i < CAP {
            m.set_raw(i);
            i += 1;
        }
        m
    };

    /// Row capacity this mask covers.
    #[must_use]
    pub const fn capacity(&self) -> usize {
        CAP
    }

    /// Raw LSB-first set. Private; caller contract `i < CAP`.
    const fn set_raw(&mut self, i: usize) {
        self.bits[i >> 3] |= 1u8 << (i & 7);
    }

    /// Bit `i` present? `Err` when `i >= CAP`.
    #[must_use]
    pub const fn get(&self, i: usize) -> Result<bool, BitmaskError> {
        if i >= CAP {
            return Err(BitmaskError::OutOfRange);
        }
        Ok(self.bits[i >> 3] & (1u8 << (i & 7)) != 0)
    }

    /// Set bit `i`; `Err` when `i >= CAP` (state unchanged).
    #[must_use]
    pub const fn set(&mut self, i: usize) -> Result<(), BitmaskError> {
        if i >= CAP {
            return Err(BitmaskError::OutOfRange);
        }
        self.bits[i >> 3] |= 1u8 << (i & 7);
        Ok(())
    }

    /// Clear bit `i`; `Err` when `i >= CAP` (state unchanged).
    #[must_use]
    pub const fn clear(&mut self, i: usize) -> Result<(), BitmaskError> {
        if i >= CAP {
            return Err(BitmaskError::OutOfRange);
        }
        self.bits[i >> 3] &= !(1u8 << (i & 7));
        Ok(())
    }

    /// Popcount of present rows (never counts padding bits: they are
    /// structurally kept clear by [`Self::ALL_VALID`]/[`Self::EMPTY`]
    /// plus the `i < CAP` guards).
    #[must_use]
    pub const fn count(&self) -> usize {
        let mut n = 0;
        let mut b = 0;
        while b < Self::MASK_BYTES {
            n += self.bits[b].count_ones() as usize;
            b += 1;
        }
        n
    }

    /// Inline backing bytes, LSB-first.
    #[must_use]
    pub const fn bytes(&self) -> &[u8; BYTES] {
        &self.bits
    }
}

/// Frozen alias set mirroring the slab aliases: concrete pinned types
/// for the C-ABI negotiation surface (A06/A17).
pub type Mask64 = PresenceMask<64, 8>;
pub type Mask256 = PresenceMask<256, 32>;
pub type Mask1024 = PresenceMask<1024, 128>;

const _: () = assert!(mask_bytes(1) == 8);
const _: () = assert!(mask_bytes(63) == 8);
const _: () = assert!(mask_bytes(64) == 8);
const _: () = assert!(mask_bytes(65) == 16);
const _: () = assert!(mask_bytes(256) == 32);
const _: () = assert!(mask_bytes(1024) == 128);

const _: () = assert!(core::mem::size_of::<Mask64>() % 8 == 0);
const _: () = assert!(core::mem::size_of::<Mask256>() % 8 == 0);
const _: () = assert!(core::mem::size_of::<Mask1024>() % 8 == 0);
const _: () = assert!(core::mem::size_of::<PresenceMask<7, 8>>() == 8);
const _: () = assert!(core::mem::size_of::<PresenceMask<63, 8>>() == 8);
const _: () = assert!(core::mem::size_of::<PresenceMask<65, 16>>() == 16);

#[cfg(test)]
mod tests {
    use super::*;

    // C4 — presence bitmask law court. Mutation M4: flip `get`/`set` to
    // MSB-first; expected failure: the asymmetric byte-0 probe below
    // (0b0100_0001 expected, 0b1000_0010 observed).
    #[test]
    fn c4_lsb_first_probe_set_and_readback() {
        let mut m = PresenceMask::<64, 8>::EMPTY;
        assert_eq!(m.set(0), Ok(()));
        assert_eq!(m.set(6), Ok(()));
        // Byte 0 is asymmetric under bit-order reversal: LSB-first {0,6}
        // gives 0b0100_0001; MSB-first would give 0b1000_0010.
        assert_eq!(m.bytes()[0], 0b0100_0001);
        assert_eq!(m.set(63), Ok(()));
        assert_eq!(m.bytes()[7], 0b1000_0000);
        assert_eq!(m.get(0), Ok(true));
        assert_eq!(m.get(6), Ok(true));
        assert_eq!(m.get(63), Ok(true));
        assert_eq!(m.get(1), Ok(false));
        assert_eq!(m.count(), 3);
        let mut present = 0;
        for i in 0..64 {
            if m.get(i) == Ok(true) {
                present += 1;
            }
        }
        assert_eq!(present, 3);
    }

    #[test]
    fn c4_all_valid_and_empty_consts() {
        assert_eq!(PresenceMask::<64, 8>::EMPTY.count(), 0);
        assert_eq!(PresenceMask::<64, 8>::ALL_VALID.count(), 64);
        for i in 0..64 {
            assert_eq!(PresenceMask::<64, 8>::ALL_VALID.get(i), Ok(true));
        }
        // Word-fill aliases pin the frozen pair set.
        assert_eq!(Mask64::MASK_BYTES, 8);
        assert_eq!(Mask256::MASK_BYTES, 32);
        assert_eq!(Mask1024::MASK_BYTES, 128);
        assert_eq!(Mask1024::ALL_VALID.count(), 1024);
    }

    #[test]
    fn c4_out_of_range_is_typed_error_state_unchanged() {
        let mut m = PresenceMask::<64, 8>::EMPTY;
        assert_eq!(m.get(64), Err(BitmaskError::OutOfRange));
        assert_eq!(m.set(64), Err(BitmaskError::OutOfRange));
        assert_eq!(m.clear(64), Err(BitmaskError::OutOfRange));
        // Post-state: the refused calls wrote nothing.
        assert_eq!(m.count(), 0);
        assert_eq!(m.bytes(), &[0; 8]);
    }

    #[test]
    fn c4_clear_round_trip_from_all_valid() {
        let mut m = PresenceMask::<64, 8>::ALL_VALID;
        assert_eq!(m.clear(0), Ok(()));
        assert_eq!(m.clear(6), Ok(()));
        assert_eq!(m.bytes()[0], 0b1011_1110);
        assert_eq!(m.count(), 62);
    }

    // Masks are const-constructible in a static (court C3 companion).
    static M64: Mask64 = Mask64::ALL_VALID;

    #[test]
    fn c3_companion_static_mask_reads_back() {
        assert_eq!(M64.count(), 64);
        assert_eq!(M64.get(63), Ok(true));
        assert_eq!(M64.capacity(), 64);
    }

    const _: () = assert!(PresenceMask::<64, 8>::EMPTY.count() == 0);
    const _: () = assert!(PresenceMask::<64, 8>::ALL_VALID.count() == 64);
}
