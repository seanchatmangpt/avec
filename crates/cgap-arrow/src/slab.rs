//! 64-byte cache-aligned arrow record slab (lane A01, Wave 1 ABI-freeze
//! slice; lane doc §4).
//!
//! Slice boundary: Wave 1 landed the struct/const definitions, size/align
//! static asserts and const construction. Wave 2 landed the core-engine
//! slice: `push`/`seal`/`seal_with_mask`/`as_bytes`/`from_bytes` and the
//! §4.1 digest computation (courts C5, C7, C8, C9), reusing the
//! Wave-1-frozen error names plus the typed variants named in the module
//! docs below (recorded Wave-2 negotiation additions). Wave 6 is the
//! defect-repair slice: the wave-5 audit probe demonstrated that
//! `from_bytes` silently accepted digest-valid windows with structurally
//! invalid headers (`len > cap`, `reserved0 != 0`, reserved flag bits);
//! those are now typed refusals with flipping courts (`wave6_tests`,
//! W6-C1..C4). Wave 8 is the second defect-repair slice: the wave-8
//! audit probe (fresh /tmp path-dep crate, this lane) witnessed five
//! more silent accepts of the same class — foreign `layout_version`,
//! wrong `presence_len`, `SEALED` cleared, `PRESENT_INLINE` cleared,
//! nonzero slot padding — all repaired here as typed refusals with
//! flipping courts (`wave8_tests`, W8-C1..C6). No A01 wave-7 court
//! receipt file exists on disk (recorded divergence D-A01-1); the
//! wave-8 findings come from this lane's own audit probe plus a full
//! mutation sweep over the in-code wave-2/6 courts.
//!
//! Wave 9 is the cross-lane integration slice: the `wave9_tests` courts
//! bind the slab to the four authoritative neighbor surfaces at their
//! real, on-disk implementations (A04 [`cgap_core::frame::CgapFrameHeader`],
//! A14 [`cgap_gate::penstock::ingress`], A05
//! [`cgap_storage::atomic_commit`]) — a sealed slab rides a real frame,
//! the dam admits or refuses it, and power-cut sweeps enumerate every
//! byte offset on both the framed-slab plane and the real double-slot
//! journal plane; the A20 1M-row stress pages a deterministic
//! 1,000,000-row corpus through the frozen alias set. The collaborator
//! crates ride test-only dev-dependencies (`Cargo.toml
//! [dev-dependencies]`): runtime externals stay E=0 and the zero-alloc
//! structural law of the lib path is untouched.
//!
//! Zero-alloc is structural: no `alloc` path is reachable from this
//! module (court C2 symbol audit), and construction is `const fn`, so a
//! slab lives in `.bss`/`.data` or on a bare-metal stack (court C3).
//!
//! Layout ground truth is pinned by compile-time asserts (size,
//! alignment, field offsets) exactly like [`crate::layout::Quad64`] — a
//! regression is a build failure, not a runtime surprise.

/// Header magic `"SLB1"` (§4.1, offset 0x00).
pub const MAGIC: u32 = 0x534C_4231;
/// Layout version (§4.1, offset 0x04). Any field change bumps it.
pub const LAYOUT_VERSION: u16 = 1;
/// `flags` bit 0 (§4.1, offset 0x06).
pub const FLAG_PRESENT_INLINE: u16 = 1 << 0;
/// `flags` bit 1 (§4.1, offset 0x06).
pub const FLAG_SEALED: u16 = 1 << 1;
/// Header size in bytes: one cache line (§4.1).
pub const HEADER_SIZE: usize = 64;
/// Slot size in bytes: one cache line (§4.2).
pub const SLOT_SIZE: usize = 64;

/// Slab header: exactly 64 bytes, one cache line (lane doc §4.1).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SlabHeader {
    /// 0x00: layout version discriminator.
    pub magic: u32,
    /// 0x04.
    pub layout_version: u16,
    /// 0x06: bit0 [`FLAG_PRESENT_INLINE`], bit1 [`FLAG_SEALED`],
    /// bits 2-15 reserved-zero.
    pub flags: u16,
    /// 0x08: row capacity (== the `CAP` generic, pinned for ABI).
    pub cap: u32,
    /// 0x0C: rows written; `len <= cap` enforced by typed error (Wave 2).
    pub len: u32,
    /// 0x10: monotonic slab sequence (plain u32; no atomics — §2.3).
    pub seq: u32,
    /// 0x14: bytes used by the presence-mask region.
    pub presence_len: u32,
    /// 0x18: zero-filled; MUST be zero at seal.
    pub reserved0: u32,
    /// 0x1C: FNV-1a over 0x00..0x1C at seal (Wave 2; donor
    /// [`crate::fingerprint`]).
    pub header_digest: u32,
    /// 0x20: digest over the sealed payload at seal (Wave 2; donor
    /// [`crate::blake3`]).
    pub digest: [u8; 32],
}

/// One slab row: exactly 64 bytes (lane doc §4.2).
///
/// The §4.2 table's "unused bytes 0x01..0x03" are an explicit field so
/// `payload` lands at offset 0x04 mechanically under `repr(C)` (a u8
/// followed by `[u8; 60]` alone would be 61 bytes with align 1 and no
/// padding at all).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SlabSlot {
    /// 0x00: shadow of the row's presence bit. The bitmask is
    /// authoritative; agreement is enforced at `seal()` (Wave 2, court
    /// C8).
    pub presence_shadow: u8,
    /// 0x01..0x03: padding, zero-filled at seal.
    pub padding: [u8; 3],
    /// 0x04: opaque row payload, interpreted by the consumer (A02/A04).
    pub payload: [u8; 60],
}

/// Typed slab error surface. C5/C7/C9 names are Wave-1-frozen; the
/// remaining variants are the recorded Wave-2 `from_bytes`/seal
/// negotiation additions (§5.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlabError {
    /// Push past `cap` (court C7).
    SlabFull,
    /// Mutation attempted after `seal()` (court C9).
    Sealed,
    /// Push payload longer than the 60-byte slot payload (§4.2).
    PayloadTooLong,
    /// Presence-mask vs shadow-byte disagreement at
    /// [`ArrowRecordSlab::seal_with_mask`] (§4.2, court C8).
    PresenceMismatch,
    /// Payload digest mismatch at `from_bytes` (court C5).
    DigestMismatch,
    /// Header digest mismatch at `from_bytes` (court C5).
    HeaderDigestMismatch,
    /// Byte window length is not `64 + CAP * 64` (§5.1).
    BadLength {
        /// Window length observed.
        got: usize,
        /// Length the receiving monomorphization requires.
        expected: usize,
    },
    /// Window magic is not `"SLB1"` (§4.1).
    BadMagic {
        /// Magic observed in the window.
        got: u32,
    },
    /// Window `cap` field disagrees with the receiving monomorphization.
    BadCap {
        /// Cap observed in the window.
        got: u32,
    },
    /// Window `len` exceeds `cap` (§4.1: `len <= cap` enforced by typed
    /// error). Wave-6 repair court W6-C1: a digest-valid window with
    /// `len > cap` was silently accepted before this variant existed.
    LenExceedsCap {
        /// Row count observed in the window.
        len: u32,
        /// Row capacity of the receiving monomorphization.
        cap: u32,
    },
    /// Window `reserved0 != 0` (§4.1: zero-filled, MUST be zero at
    /// seal). Wave-6 repair court W6-C2.
    ReservedNonZero {
        /// `reserved0` observed in the window.
        got: u32,
    },
    /// Window `flags` has reserved bits (2-15) set (§4.1: reserved-zero).
    /// Wave-6 repair court W6-C3.
    ReservedFlagsSet {
        /// `flags` observed in the window.
        got: u16,
    },
    /// Window `layout_version != [`LAYOUT_VERSION`]` (§4.1: "any field
    /// change bumps it" — a foreign version is not interpretable by this
    /// receiver). Wave-8 repair court W8-C1.
    LayoutVersionMismatch {
        /// Layout version observed in the window.
        got: u16,
    },
    /// Window `presence_len != mask_bytes(CAP)` (§4.1: the mask region
    /// size the receiving monomorphization mints at [`ArrowRecordSlab::seal`]).
    /// Wave-8 repair court W8-C2.
    PresenceLenMismatch {
        /// `presence_len` observed in the window.
        got: u32,
        /// `mask_bytes(CAP)` for the receiving monomorphization.
        expected: u32,
    },
    /// Window `flags` lacks [`FLAG_SEALED`] although `from_bytes` is
    /// documented to decode a *sealed* slab window (§5.1/§6 C5). Wave-8
    /// repair court W8-C3.
    NotSealed {
        /// `flags` observed in the window.
        got: u16,
    },
    /// Window `flags` lacks [`FLAG_PRESENT_INLINE`] although every slot
    /// carries an inline presence shadow byte (§4.2) — a window that
    /// clears the bit misdescribes its own layout. Wave-8 repair court
    /// W8-C4.
    PresenceNotInline {
        /// `flags` observed in the window.
        got: u16,
    },
    /// A slot's `padding` bytes (0x01..0x03) are nonzero (§4.2: padding
    /// is "zero-filled at seal"). Wave-8 repair court W8-C5.
    PaddingNonZero {
        /// Index of the first offending row.
        row: usize,
    },
}

/// Fixed-capacity inline arrow record slab: `64 + CAP * 64` bytes,
/// 64-byte aligned (lane doc §4.3). One record never straddles a cache
/// line, on any target, and no allocator exists anywhere in the type.
#[repr(C, align(64))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ArrowRecordSlab<const CAP: usize = 64> {
    /// 0x0000..0x0040: [`SlabHeader`].
    pub header: SlabHeader,
    /// 0x0040..: `CAP` rows of [`SlabSlot`], 64 bytes each.
    pub slots: [SlabSlot; CAP],
}

impl SlabSlot {
    /// Zeroed slot (sentinel initialization).
    pub const ZERO: Self = Self {
        presence_shadow: 0,
        padding: [0; 3],
        payload: [0; 60],
    };
}

impl<const CAP: usize> ArrowRecordSlab<CAP> {
    /// Const-constructible empty slab: magic, layout version and the
    /// `PRESENT_INLINE` flag stamped; everything else zero (court C3).
    #[must_use]
    pub const fn new() -> Self {
        Self {
            header: SlabHeader {
                magic: MAGIC,
                layout_version: LAYOUT_VERSION,
                flags: FLAG_PRESENT_INLINE,
                cap: CAP as u32,
                len: 0,
                seq: 0,
                presence_len: 0,
                reserved0: 0,
                header_digest: 0,
                digest: [0; 32],
            },
            slots: [SlabSlot::ZERO; CAP],
        }
    }
}

impl<const CAP: usize> Default for ArrowRecordSlab<CAP> {
    fn default() -> Self {
        Self::new()
    }
}

/// Byte length of a slot's opaque payload region (§4.2, offset 0x04).
pub const SLOT_PAYLOAD_SIZE: usize = 60;

impl<const CAP: usize> ArrowRecordSlab<CAP> {
    /// Rows written so far (§4.1 `len`).
    #[must_use]
    pub const fn len(&self) -> u32 {
        self.header.len
    }

    /// Row capacity (== `CAP`, pinned in the header at construction).
    #[must_use]
    pub const fn capacity(&self) -> u32 {
        CAP as u32
    }

    /// `true` once [`Self::seal`] has stamped [`FLAG_SEALED`] (court C9).
    #[must_use]
    pub const fn is_sealed(&self) -> bool {
        self.header.flags & FLAG_SEALED != 0
    }

    /// FNV-1a (32-bit) over `bytes` — §4.1 header-digest donor pattern
    /// (`fingerprint.rs`), implemented locally because `fingerprint.rs`
    /// is outside this lane's lease and its 128-bit word-folding
    /// `Fingerprint` is not the §4.1 byte-wise FNV-1a.
    #[must_use]
    pub const fn fnv1a32(bytes: &[u8]) -> u32 {
        let mut h: u32 = 0x811C_9DC5;
        let mut i = 0;
        while i < bytes.len() {
            h ^= bytes[i] as u32;
            h = h.wrapping_mul(0x0100_0193);
            i += 1;
        }
        h
    }

    /// §4.1 LE wire encoding of header bytes `0x00..0x1C` — the span the
    /// FNV-1a header digest covers. LE on the wire regardless of host
    /// endianness (§4.1), which is what makes `from_bytes` portable.
    fn header_wire_prefix(&self) -> [u8; 0x1C] {
        let h = &self.header;
        let mut out = [0u8; 0x1C];
        out[0x00..0x04].copy_from_slice(&h.magic.to_le_bytes());
        out[0x04..0x06].copy_from_slice(&h.layout_version.to_le_bytes());
        out[0x06..0x08].copy_from_slice(&h.flags.to_le_bytes());
        out[0x08..0x0C].copy_from_slice(&h.cap.to_le_bytes());
        out[0x0C..0x10].copy_from_slice(&h.len.to_le_bytes());
        out[0x10..0x14].copy_from_slice(&h.seq.to_le_bytes());
        out[0x14..0x18].copy_from_slice(&h.presence_len.to_le_bytes());
        out[0x18..0x1C].copy_from_slice(&h.reserved0.to_le_bytes());
        out
    }

    /// The slots region as a contiguous byte window.
    ///
    /// Soundness: `SlabSlot` is `repr(C)` with all 64 bytes accounted
    /// (1 shadow + 3 padding + 60 payload, const-pinned in this module)
    /// and `self.slots` is a contiguous `[SlabSlot; CAP]`, so every byte
    /// of the window is initialized.
    fn slots_as_bytes(&self) -> &[u8] {
        unsafe {
            core::slice::from_raw_parts(
                self.slots.as_ptr() as *const u8,
                core::mem::size_of::<[SlabSlot; CAP]>(),
            )
        }
    }

    /// Append one row: zero-fills the slot, copies `payload` (at most
    /// [`SLOT_PAYLOAD_SIZE`] bytes), stamps the presence shadow byte.
    /// Sealed refusals first (C9), then capacity (C7), then payload
    /// length — all typed, never panics (§4.4 no-panic law).
    pub fn push(&mut self, payload: &[u8], present: bool) -> Result<(), SlabError> {
        if self.is_sealed() {
            return Err(SlabError::Sealed);
        }
        if self.header.len as usize >= CAP {
            return Err(SlabError::SlabFull);
        }
        if payload.len() > SLOT_PAYLOAD_SIZE {
            return Err(SlabError::PayloadTooLong);
        }
        let i = self.header.len as usize;
        let slot = &mut self.slots[i];
        slot.presence_shadow = u8::from(present);
        slot.payload = [0; SLOT_PAYLOAD_SIZE];
        slot.payload[..payload.len()].copy_from_slice(payload);
        self.header.len += 1;
        Ok(())
    }

    /// Seal the slab: canonicalizes slot padding to zeros (§4.2:
    /// "zero-filled at seal" — Wave-8 repair, court W8-C5; the API's own
    /// image was already zero-padded, so this is a no-op for API-produced
    /// slabs and only bites in-memory-mutated padding), stamps
    /// `presence_len` (`mask_bytes(CAP)`), zeroes `reserved0`, sets
    /// [`FLAG_SEALED`] (inside the digest span, so the sealed state
    /// itself is digest-covered), computes the §4.1 FNV-1a header digest
    /// over wire bytes `0x00..0x1C` and the BLAKE3 payload digest over
    /// the full slots region (total window coverage: any single-byte
    /// mutation maps to a typed error, court C5). Double seal is
    /// [`SlabError::Sealed`] — no silent re-digest.
    pub fn seal(&mut self) -> Result<(), SlabError> {
        if self.is_sealed() {
            return Err(SlabError::Sealed);
        }
        // §4.2 zero-fill law: canonicalize padding BEFORE both digests,
        // so the sealed image carries the zero-filled invariant and the
        // digests cover it (any later padding mutation is a typed error).
        let mut i = 0;
        while i < CAP {
            self.slots[i].padding = [0; 3];
            i += 1;
        }
        self.header.presence_len = crate::bitmask::mask_bytes(CAP) as u32;
        self.header.reserved0 = 0;
        self.header.flags |= FLAG_SEALED;
        let prefix = self.header_wire_prefix();
        self.header.header_digest = Self::fnv1a32(&prefix);
        self.header.digest = crate::blake3::hash(self.slots_as_bytes());
        Ok(())
    }

    /// Whole-slab in-memory byte window (§5.1). Borrows `&self`, so the
    /// A02 discipline (never hold the window across slab mutation) is
    /// compiler-enforced.
    ///
    /// Soundness: `ArrowRecordSlab` is `repr(C, align(64))`; the header
    /// is exactly 64 bytes (const-pinned field ladder, no holes) and
    /// every slot exactly 64 bytes (const-pinned), so the image contains
    /// no unaccounted padding.
    pub fn as_bytes(&self) -> &[u8] {
        unsafe {
            core::slice::from_raw_parts(
                core::ptr::from_ref(self) as *const u8,
                core::mem::size_of::<Self>(),
            )
        }
    }

    /// Decode and fully verify a sealed slab byte window (§5.1): length,
    /// magic, `cap`-vs-monomorphization, FNV-1a header digest over the
    /// LE wire span `0x00..0x1C`, BLAKE3 payload digest over the slots
    /// region. Header scalars decode little-endian (§4.1 wire law), so
    /// an LE-wire window verifies on any host endianness; a BE native
    /// image fails the header digest with a typed error (the §6 C5
    /// cross-endian clause: typed error or identity, never silent
    /// accept).
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, SlabError> {
        let expected = core::mem::size_of::<Self>();
        if bytes.len() != expected {
            return Err(SlabError::BadLength {
                expected,
                got: bytes.len(),
            });
        }
        let magic = u32::from_le_bytes([bytes[0x00], bytes[0x01], bytes[0x02], bytes[0x03]]);
        if magic != MAGIC {
            return Err(SlabError::BadMagic { got: magic });
        }
        // Positions below are pinned by the exact-length gate above.
        let stored_header_digest =
            u32::from_le_bytes([bytes[0x1C], bytes[0x1D], bytes[0x1E], bytes[0x1F]]);
        if Self::fnv1a32(&bytes[0x00..0x1C]) != stored_header_digest {
            return Err(SlabError::HeaderDigestMismatch);
        }
        let mut stored_digest = [0u8; 32];
        stored_digest.copy_from_slice(&bytes[0x20..0x40]);
        if crate::blake3::hash(&bytes[HEADER_SIZE..]) != stored_digest {
            return Err(SlabError::DigestMismatch);
        }
        let layout_version = u16::from_le_bytes([bytes[0x04], bytes[0x05]]);
        let flags = u16::from_le_bytes([bytes[0x06], bytes[0x07]]);
        let cap = u32::from_le_bytes([bytes[0x08], bytes[0x09], bytes[0x0A], bytes[0x0B]]);
        if cap as usize != CAP {
            return Err(SlabError::BadCap { got: cap });
        }
        // Wave-8 structural law (§4.1): a foreign layout version is not
        // interpretable by this receiver — checked after both digest
        // gates, own flipping court W8-C1 (mutation M-W8-1).
        let layout_probe = u16::from_le_bytes([bytes[0x04], bytes[0x05]]);
        if layout_probe != LAYOUT_VERSION {
            return Err(SlabError::LayoutVersionMismatch { got: layout_probe });
        }
        // Wave-6 structural laws (§4.1), checked after both digest gates:
        // a digest-valid window can still carry a header the receiver
        // must refuse (each law has its own flipping court, W6-C1..C3).
        let len_probe = u32::from_le_bytes([bytes[0x0C], bytes[0x0D], bytes[0x0E], bytes[0x0F]]);
        if len_probe > cap {
            return Err(SlabError::LenExceedsCap {
                len: len_probe,
                cap,
            });
        }
        let reserved0_probe =
            u32::from_le_bytes([bytes[0x18], bytes[0x19], bytes[0x1A], bytes[0x1B]]);
        if reserved0_probe != 0 {
            return Err(SlabError::ReservedNonZero {
                got: reserved0_probe,
            });
        }
        let flags_probe = u16::from_le_bytes([bytes[0x06], bytes[0x07]]);
        if flags_probe & !(FLAG_PRESENT_INLINE | FLAG_SEALED) != 0 {
            return Err(SlabError::ReservedFlagsSet { got: flags_probe });
        }
        // Wave-8 structural flag laws (§4.1/§4.2): the window must claim
        // the layout this monomorphization actually mints — inline
        // presence (every slot carries the shadow byte) and a sealed
        // image (`from_bytes` decodes sealed slabs; §5.1). Own flipping
        // courts W8-C3/W8-C4 (mutations M-W8-3/M-W8-4).
        if flags_probe & FLAG_PRESENT_INLINE == 0 {
            return Err(SlabError::PresenceNotInline { got: flags_probe });
        }
        if flags_probe & FLAG_SEALED == 0 {
            return Err(SlabError::NotSealed { got: flags_probe });
        }
        // Wave-8 structural law (§4.1): `presence_len` must equal the
        // mask-region size the receiving monomorphization stamps at
        // seal. Own flipping court W8-C2 (mutation M-W8-2).
        let presence_len_probe =
            u32::from_le_bytes([bytes[0x14], bytes[0x15], bytes[0x16], bytes[0x17]]);
        if presence_len_probe != crate::bitmask::mask_bytes(CAP) as u32 {
            return Err(SlabError::PresenceLenMismatch {
                got: presence_len_probe,
                expected: crate::bitmask::mask_bytes(CAP) as u32,
            });
        }
        let len = u32::from_le_bytes([bytes[0x0C], bytes[0x0D], bytes[0x0E], bytes[0x0F]]);
        let seq = u32::from_le_bytes([bytes[0x10], bytes[0x11], bytes[0x12], bytes[0x13]]);
        let presence_len =
            u32::from_le_bytes([bytes[0x14], bytes[0x15], bytes[0x16], bytes[0x17]]);
        let reserved0 =
            u32::from_le_bytes([bytes[0x18], bytes[0x19], bytes[0x1A], bytes[0x1B]]);
        let slots_window = &bytes[HEADER_SIZE..];
        let mut slots = [SlabSlot::ZERO; CAP];
        let mut i = 0;
        while i < CAP {
            let s = &slots_window[i * SLOT_SIZE..(i + 1) * SLOT_SIZE];
            // Wave-8 structural law (§4.2: padding "zero-filled at
            // seal"): a slot with nonzero padding bytes misdescribes the
            // canonical sealed image. Own flipping court W8-C5 (mutation
            // M-W8-5).
            if s[1] != 0 || s[2] != 0 || s[3] != 0 {
                return Err(SlabError::PaddingNonZero { row: i });
            }
            slots[i].presence_shadow = s[0];
            slots[i].padding = [s[1], s[2], s[3]];
            slots[i].payload.copy_from_slice(&s[4..SLOT_SIZE]);
            i += 1;
        }
        Ok(Self {
            header: SlabHeader {
                magic: MAGIC,
                layout_version,
                flags,
                cap,
                len,
                seq,
                presence_len,
                reserved0,
                header_digest: stored_header_digest,
                digest: stored_digest,
            },
            slots,
        })
    }
}

/// §4.2: the presence mask is authoritative; a shadow-byte vs mask
/// disagreement at seal is a typed error (court C8). Masks are held by
/// the caller (the frozen slab layout has no mask field), so the
/// cross-check lives on the concrete alias set — the mask type derives
/// from `CAP`, and deriving `mask_bytes(CAP)` in a const-generic bound
/// is nightly-only (`generic_const_exprs`, recorded divergence in
/// `bitmask.rs`).
macro_rules! impl_seal_with_mask {
    ($alias:ty, $mask:ty) => {
        impl $alias {
            /// Cross-check shadow bytes against the authoritative mask
            /// over `0..len`, then seal (see [`ArrowRecordSlab::seal`]).
            pub fn seal_with_mask(&mut self, mask: &$mask) -> Result<(), SlabError> {
                let mut i = 0;
                while i < self.len() as usize {
                    let bit = mask.get(i) == Ok(true);
                    if self.slots[i].presence_shadow != u8::from(bit) {
                        return Err(SlabError::PresenceMismatch);
                    }
                    i += 1;
                }
                self.seal()
            }
        }
    };
}
impl_seal_with_mask!(Slab64, crate::bitmask::Mask64);
impl_seal_with_mask!(Slab256, crate::bitmask::Mask256);
impl_seal_with_mask!(Slab1024, crate::bitmask::Mask1024);

/// Concrete monomorphizations for C-ABI pinning — generics do not cross
/// a C ABI (lane doc §2.1). `Slab64` is the embedded-first alias (§8.3).
pub type Slab64 = ArrowRecordSlab<64>;
/// See [`Slab64`].
pub type Slab256 = ArrowRecordSlab<256>;
/// See [`Slab64`].
pub type Slab1024 = ArrowRecordSlab<1024>;

// ---- §4.1 header layout laws (build failures on regression) ----
const _: () = assert!(core::mem::size_of::<SlabHeader>() == 64);
const _: () = assert!(core::mem::offset_of!(SlabHeader, magic) == 0x00);
const _: () = assert!(core::mem::offset_of!(SlabHeader, layout_version) == 0x04);
const _: () = assert!(core::mem::offset_of!(SlabHeader, flags) == 0x06);
const _: () = assert!(core::mem::offset_of!(SlabHeader, cap) == 0x08);
const _: () = assert!(core::mem::offset_of!(SlabHeader, len) == 0x0C);
const _: () = assert!(core::mem::offset_of!(SlabHeader, seq) == 0x10);
const _: () = assert!(core::mem::offset_of!(SlabHeader, presence_len) == 0x14);
const _: () = assert!(core::mem::offset_of!(SlabHeader, reserved0) == 0x18);
const _: () = assert!(core::mem::offset_of!(SlabHeader, header_digest) == 0x1C);
const _: () = assert!(core::mem::offset_of!(SlabHeader, digest) == 0x20);

// ---- §4.2 slot layout laws ----
const _: () = assert!(core::mem::size_of::<SlabSlot>() == 64);
const _: () = assert!(core::mem::offset_of!(SlabSlot, presence_shadow) == 0x00);
const _: () = assert!(core::mem::offset_of!(SlabSlot, padding) == 0x01);
const _: () = assert!(core::mem::offset_of!(SlabSlot, payload) == 0x04);

// ---- §4.3 slab size/alignment laws (concrete monomorphizations) ----
const _: () = assert!(core::mem::align_of::<Slab64>() == 64);
const _: () = assert!(core::mem::size_of::<Slab64>() == 64 + 64 * 64);
const _: () = assert!(core::mem::align_of::<Slab256>() == 64);
const _: () = assert!(core::mem::size_of::<Slab256>() == 64 + 256 * 64);
const _: () = assert!(core::mem::align_of::<Slab1024>() == 64);
const _: () = assert!(core::mem::size_of::<Slab1024>() == 64 + 1024 * 64);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bitmask::PresenceMask;

    // C3 — const-constructibility court. M3: make `new` non-const and
    // this static plus the const-eval probes below refuse to compile
    // (E0015); M3b: corrupt `magic` in `new` and the first-read asserts
    // fail at runtime.
    static SLAB: Slab64 = ArrowRecordSlab::new();
    const NEW_MAGIC: u32 = ArrowRecordSlab::<64>::new().header.magic;
    const NEW_CAP: u32 = ArrowRecordSlab::<64>::new().header.cap;
    const _: () = assert!(NEW_MAGIC == MAGIC);
    const _: () = assert!(NEW_CAP == 64);

    #[test]
    fn c3_static_slab_is_const_constructible_and_well_formed() {
        // First read of the .bss-resident static: header fields stamped,
        // payload zeroed.
        assert_eq!(SLAB.header.magic, MAGIC);
        assert_eq!(SLAB.header.layout_version, LAYOUT_VERSION);
        assert_eq!(SLAB.header.flags, FLAG_PRESENT_INLINE);
        assert_eq!(SLAB.header.cap, 64);
        assert_eq!(SLAB.header.len, 0);
        assert_eq!(SLAB.header.seq, 0);
        assert_eq!(SLAB.header.presence_len, 0);
        assert_eq!(SLAB.header.reserved0, 0);
        assert_eq!(SLAB.header.header_digest, 0);
        assert_eq!(SLAB.header.digest, [0; 32]);
        assert_eq!(SLAB.slots.len(), 64);
        assert_eq!(SLAB.slots[0], SlabSlot::ZERO);
        assert_eq!(SLAB.slots[63], SlabSlot::ZERO);
    }

    // C1 — alignment court (host). M1: change `#[repr(C, align(64))]` to
    // `align(8)`; the const asserts above fail the build.
    #[test]
    fn c1_size_and_alignment_laws() {
        assert_eq!(core::mem::align_of::<Slab64>(), 64);
        assert_eq!(core::mem::align_of::<Slab256>(), 64);
        assert_eq!(core::mem::align_of::<Slab1024>(), 64);
        assert_eq!(core::mem::size_of::<Slab64>(), 64 + 64 * 64);
        assert_eq!(core::mem::size_of::<Slab256>(), 64 + 256 * 64);
        assert_eq!(core::mem::size_of::<Slab1024>(), 64 + 1024 * 64);
        assert_eq!(core::mem::size_of::<Slab64>(), 4160);
        assert_eq!(HEADER_SIZE, 64);
        assert_eq!(SLOT_SIZE, 64);
    }

    #[test]
    fn c1_field_offsets_match_spec_tables() {
        // §4.1 header table.
        assert_eq!(core::mem::offset_of!(SlabHeader, magic), 0x00);
        assert_eq!(core::mem::offset_of!(SlabHeader, layout_version), 0x04);
        assert_eq!(core::mem::offset_of!(SlabHeader, flags), 0x06);
        assert_eq!(core::mem::offset_of!(SlabHeader, cap), 0x08);
        assert_eq!(core::mem::offset_of!(SlabHeader, len), 0x0C);
        assert_eq!(core::mem::offset_of!(SlabHeader, seq), 0x10);
        assert_eq!(core::mem::offset_of!(SlabHeader, presence_len), 0x14);
        assert_eq!(core::mem::offset_of!(SlabHeader, reserved0), 0x18);
        assert_eq!(core::mem::offset_of!(SlabHeader, header_digest), 0x1C);
        assert_eq!(core::mem::offset_of!(SlabHeader, digest), 0x20);
        // §4.2 slot table.
        assert_eq!(core::mem::offset_of!(SlabSlot, presence_shadow), 0x00);
        assert_eq!(core::mem::offset_of!(SlabSlot, padding), 0x01);
        assert_eq!(core::mem::offset_of!(SlabSlot, payload), 0x04);
    }

    #[test]
    fn header_flag_constants_pin_doc_bits() {
        assert_eq!(MAGIC, 0x534C_4231);
        assert_eq!(LAYOUT_VERSION, 1);
        assert_eq!(FLAG_PRESENT_INLINE, 0b0000_0000_0000_0001);
        assert_eq!(FLAG_SEALED, 0b0000_0000_0000_0010);
    }

    #[test]
    fn slab_capacity_header_agrees_with_generic_for_all_aliases() {
        assert_eq!(Slab64::new().header.cap, 64);
        assert_eq!(Slab256::new().header.cap, 256);
        assert_eq!(Slab1024::new().header.cap, 1024);
    }

    // Presence-mask integration: the slab's row masks come from
    // bitmask.rs and share the frozen CAP set.
    #[test]
    fn mask_alias_set_matches_slab_alias_set() {
        assert_eq!(core::mem::size_of::<PresenceMask<64, 8>>(), 8);
        assert_eq!(core::mem::size_of::<PresenceMask<256, 32>>(), 32);
        assert_eq!(core::mem::size_of::<PresenceMask<1024, 128>>(), 128);
    }
}

#[cfg(test)]
mod wave2_tests {
    use super::*;
    use crate::bitmask::Mask64;

    /// Seeded slab + in-sync authoritative mask: `n` rows, alternating
    /// presence, deterministic payloads. Shared by C5/C8/C9 courts.
    fn seeded_slab(n: usize) -> (Slab64, Mask64) {
        let mut slab = Slab64::new();
        let mut mask = Mask64::EMPTY;
        for i in 0..n {
            let mut payload = [0u8; SLOT_PAYLOAD_SIZE];
            for (j, b) in payload.iter_mut().enumerate() {
                *b = ((i * 7 + j * 13) & 0xFF) as u8;
            }
            let present = i % 2 == 0;
            slab.push(&payload, present).unwrap();
            mask.set(i).unwrap();
            if !present {
                mask.clear(i).unwrap();
            }
        }
        (slab, mask)
    }

    // C5 — seal/verify digest court (positive path): a sealed slab
    // round-trips `from_bytes(as_bytes(s)) == s` structurally, with both
    // digests re-verified on decode.
    #[test]
    fn c5_sealed_round_trip_is_structural_identity() {
        let (mut slab, mask) = seeded_slab(64);
        slab.seal_with_mask(&mask).unwrap();
        assert!(slab.is_sealed());
        assert_eq!(slab.header.presence_len, 8);
        let round = Slab64::from_bytes(slab.as_bytes()).unwrap();
        assert_eq!(round, slab);
        // Header state survives: sealed flag, digests, row count.
        assert!(round.is_sealed());
        assert_eq!(round.len(), 64);
        assert_eq!(round.header.header_digest, slab.header.header_digest);
        assert_eq!(round.header.digest, slab.header.digest);
    }

    // C5 negative paths: any single-byte mutation of the sealed window
    // maps to a typed error — payload region → DigestMismatch, header
    // digest span → HeaderDigestMismatch, digest fields themselves
    // included, never silent accept.
    #[test]
    fn c5_single_byte_mutations_are_typed_errors() {
        let (mut slab, mask) = seeded_slab(9);
        slab.seal_with_mask(&mask).unwrap();
        let image = slab.as_bytes().to_vec();
        assert_eq!(image.len(), 64 + 64 * 64);

        // Payload byte (row 3, inside len) → DigestMismatch.
        let mut m = image.clone();
        m[0x40 + 3 * 64 + 10] ^= 0xFF;
        assert_eq!(Slab64::from_bytes(&m), Err(SlabError::DigestMismatch));

        // Tail slot byte (row 40: beyond len=9 but inside CAP —
        // full-region digest coverage, not just written rows).
        m = image.clone();
        m[0x40 + 40 * 64 + 5] ^= 0x01;
        assert_eq!(Slab64::from_bytes(&m), Err(SlabError::DigestMismatch));

        // Header field in the digest span (cap, 0x08) → HeaderDigestMismatch.
        m = image.clone();
        m[0x08] ^= 0x01;
        assert_eq!(
            Slab64::from_bytes(&m),
            Err(SlabError::HeaderDigestMismatch)
        );

        // Stored header_digest field itself → HeaderDigestMismatch.
        m = image.clone();
        m[0x1C] ^= 0x01;
        assert_eq!(
            Slab64::from_bytes(&m),
            Err(SlabError::HeaderDigestMismatch)
        );

        // Stored payload digest field itself → DigestMismatch.
        m = image.clone();
        m[0x20] ^= 0x01;
        assert_eq!(Slab64::from_bytes(&m), Err(SlabError::DigestMismatch));

        // Length law and magic law.
        assert_eq!(
            Slab64::from_bytes(&image[..image.len() - 1]),
            Err(SlabError::BadLength {
                expected: 64 + 64 * 64,
                got: image.len() - 1,
            })
        );
        m = image;
        m[0x00] = b'X';
        // In-memory magic bytes are the LE image of 0x534C_4231, i.e.
        // [0x31, 0x42, 0x4C, 0x53] ("1BLS"); byte 0 → 'X'.
        assert_eq!(
            Slab64::from_bytes(&m),
            Err(SlabError::BadMagic {
                got: u32::from_le_bytes([b'X', b'B', b'L', b'S'])
            })
        );
    }

    // C5 — unsealed window: digest fields are still zero, so
    // verification refuses with a typed error (pinned behavior: the
    // header digest check fires first).
    #[test]
    fn c5_unsealed_window_is_typed_error() {
        let (mut slab, _) = seeded_slab(4);
        slab.push(&[1; 60], false).unwrap();
        assert!(!slab.is_sealed());
        assert_eq!(
            Slab64::from_bytes(slab.as_bytes()),
            Err(SlabError::HeaderDigestMismatch)
        );
    }

    // §6 C5 cross-endian clause, host-emulated: a big-endian wire image
    // (multi-byte header scalars byte-swapped, slots verbatim) must NOT
    // be silently accepted — the header digest recomputed over the
    // swapped span disagrees with the LE-stored digest → typed error.
    // (A BE image of the same slab is accepted iff A04's frozen contract
    // says so; today the §4.1 LE wire law governs.)
    #[test]
    fn c5_cross_endian_swapped_header_is_typed_error() {
        let (mut slab, mask) = seeded_slab(5);
        slab.seal_with_mask(&mask).unwrap();
        let mut be = slab.as_bytes().to_vec();
        for (off, w) in [(0x00, 4usize), (0x08, 4), (0x0C, 4), (0x10, 4), (0x14, 4), (0x18, 4)] {
            be[off..off + w].reverse();
        }
        be[0x04..0x06].reverse();
        be[0x06..0x08].reverse();
        // The swapped magic hits the first gate — exactly the outcome a
        // big-endian native image gets on this LE-wire contract.
        assert_eq!(
            Slab64::from_bytes(&be),
            Err(SlabError::BadMagic {
                got: u32::from_le_bytes([0x53, 0x4C, 0x42, 0x31])
            })
        );
    }

    // C7 — capacity law court: push CAP rows OK with `len()` == CAP
    // throughout; the CAP+1-th push is a typed `SlabFull`; state
    // unchanged after the refusal.
    #[test]
    fn c7_capacity_law_typed_full_error() {
        let mut slab = ArrowRecordSlab::<4>::new();
        for i in 0..4 {
            assert_eq!(slab.push(&[i as u8; 8], true), Ok(()));
            assert_eq!(slab.len(), i as u32 + 1);
        }
        assert_eq!(slab.len(), 4);
        let before = slab;
        assert_eq!(slab.push(&[0xFF; 8], true), Err(SlabError::SlabFull));
        assert_eq!(slab, before, "refused push must not mutate state");
        // Same law on the embedded-first alias.
        let mut s64 = Slab64::new();
        for i in 0..64 {
            assert_eq!(s64.push(&[i as u8], i % 3 == 0), Ok(()));
        }
        assert_eq!(s64.len(), 64);
        assert_eq!(s64.push(&[0; 1], true), Err(SlabError::SlabFull));
    }

    // Push payload law: > 60 bytes is a typed `PayloadTooLong`; a short
    // payload is zero-padded; an empty payload row is all-zero.
    #[test]
    fn push_payload_length_law_and_zero_padding() {
        let mut slab = Slab64::new();
        assert_eq!(
            slab.push(&[0xAA; 61], true),
            Err(SlabError::PayloadTooLong)
        );
        assert_eq!(slab.len(), 0, "refused push must not advance len");
        assert_eq!(slab.push(&[0xBB; 60], true), Ok(()));
        assert_eq!(slab.push(&[0xCC; 3], true), Ok(()));
        assert_eq!(slab.push(&[], false), Ok(()));
        assert_eq!(&slab.slots[1].payload[..3], &[0xCC, 0xCC, 0xCC]);
        assert!(slab.slots[1].payload[3..].iter().all(|&b| b == 0));
        assert!(slab.slots[2].payload.iter().all(|&b| b == 0));
        assert_eq!(slab.slots[2].presence_shadow, 0);
        assert_eq!(slab.slots[2].padding, [0; 3]);
    }

    // C8 — shadow-byte agreement court: a 64-row alternating set/clear
    // sweep keeps slot shadow bytes in exact agreement with the
    // authoritative mask.
    #[test]
    fn c8_shadow_byte_agrees_with_authoritative_mask_sweep() {
        let (slab, mask) = seeded_slab(64);
        for i in 0..64 {
            let expect = matches!(mask.get(i), Ok(true));
            assert_eq!(
                slab.slots[i].presence_shadow,
                u8::from(expect),
                "shadow disagreement at row {i}"
            );
            assert_eq!(slab.slots[i].presence_shadow != 0, i % 2 == 0);
        }
        assert_eq!(mask.count(), 32);
    }

    // C8 negative path: a mask corrupted within the written span makes
    // `seal_with_mask` refuse with `PresenceMismatch` — the §4.2
    // "disagreement is a typed seal error" law.
    #[test]
    fn c8_mask_disagreement_is_typed_seal_error() {
        let (mut slab, mut mask) = seeded_slab(8);
        let good = mask;
        // Flip one authoritative bit inside the written span: row 3 is
        // absent in the seeded pattern, so setting it diverges from the
        // shadow byte (which push stamped 0).
        mask.set(3).unwrap();
        assert_eq!(
            slab.seal_with_mask(&mask),
            Err(SlabError::PresenceMismatch)
        );
        assert!(!slab.is_sealed(), "refused seal must not stamp SEALED");
        // In-sync mask seals cleanly.
        assert_eq!(slab.seal_with_mask(&good), Ok(()));
        assert!(slab.is_sealed());
    }

    // C9 — sealed immutability court: after `seal()`, `push` is a typed
    // refusal and the SEALED flag is observable; state unchanged.
    #[test]
    fn c9_sealed_slab_refuses_push_and_reseal() {
        let (mut slab, mask) = seeded_slab(3);
        slab.seal_with_mask(&mask).unwrap();
        assert!(slab.is_sealed());
        assert_ne!(slab.header.flags & FLAG_SEALED, 0);
        let frozen = slab;
        assert_eq!(slab.push(&[9; 4], true), Err(SlabError::Sealed));
        assert_eq!(slab.seal(), Err(SlabError::Sealed));
        assert_eq!(slab, frozen, "sealed slab must be mutation-frozen");
        // A decoded sealed slab is sealed too: the flag rode the window.
        let mut round = Slab64::from_bytes(slab.as_bytes()).unwrap();
        assert_eq!(round.push(&[1], true), Err(SlabError::Sealed));
    }

    // FNV-1a reference pins: §4.1 donor-pattern constants and the empty
    // / single-byte identities.
    #[test]
    fn fnv1a32_reference_pins() {
        assert_eq!(ArrowRecordSlab::<4>::fnv1a32(&[]), 0x811C_9DC5);
        assert_eq!(ArrowRecordSlab::<4>::fnv1a32(&[0x00]), 0x050C_5D1F);
        assert_eq!(ArrowRecordSlab::<4>::fnv1a32(b"a"), 0xE40C_292C);
    }
}

/// Wave-6 defect-repair courts (lane A01 slice: wave-5 audit probe
/// witnesses). The wave-5 probe crafted digest-valid windows that the
/// receiver accepted silently: `len > cap`, `reserved0 != 0`, reserved
/// flag bits set. Each law below names its anti-vacuity mutation; a
/// court that cannot flip is FAIL.
#[cfg(test)]
mod wave6_tests {
    use super::*;
    use crate::bitmask::Mask64;
    use alloc::vec::Vec;

    /// Craft a digest-valid window from a real sealed slab: patch
    /// `field`'s wire bytes, then recompute the §4.1 FNV-1a header
    /// digest (0x00..0x1C) with the slab's own public `fnv1a32`. The
    /// BLAKE3 payload digest is untouched (it covers only 0x40..).
    fn craft(field: &str, slab: &Slab64) -> Vec<u8> {
        let mut m = slab.as_bytes().to_vec();
        match field {
            "len" => m[0x0C..0x10].copy_from_slice(&65u32.to_le_bytes()),
            "reserved0" => m[0x18..0x1C].copy_from_slice(&7u32.to_le_bytes()),
            "flags" => m[0x06..0x08].copy_from_slice(&0x0013u16.to_le_bytes()),
            _ => unreachable!(),
        }
        let dig = Slab64::fnv1a32(&m[0x00..0x1C]);
        m[0x1C..0x20].copy_from_slice(&dig.to_le_bytes());
        m
    }

    fn sealed_two_row_slab() -> Slab64 {
        let mut s = Slab64::new();
        s.push(&[1u8; 8], true).unwrap();
        s.push(&[2u8; 8], false).unwrap();
        s.seal().unwrap();
        s
    }

    // W6-C1 — §4.1 len law: a digest-valid window with len > cap is a
    // typed refusal, state exact (len, cap reported). Mutation
    // M-W6-1: delete the `len_probe > cap` check in `from_bytes` —
    // the court flips (the crafted window decodes Ok and the assert
    // on the variant fails).
    #[test]
    fn w6_c1_len_over_cap_digest_valid_window_is_typed_error() {
        let slab = sealed_two_row_slab();
        // Positive control: the unmodified window still decodes.
        assert_eq!(
            Slab64::from_bytes(slab.as_bytes()),
            Ok(slab),
            "positive control broke: craft() corrupted the baseline"
        );
        let w = craft("len", &slab);
        assert_eq!(
            Slab64::from_bytes(&w),
            Err(SlabError::LenExceedsCap { len: 65, cap: 64 })
        );
    }

    // W6-C2 — §4.1 reserved0 law ("MUST be zero at seal"): a
    // digest-valid window with reserved0 = 7 is a typed refusal.
    // Mutation M-W6-2: delete the `reserved0_probe != 0` check — the
    // court flips (Ok decode instead of the typed variant).
    #[test]
    fn w6_c2_reserved0_nonzero_digest_valid_window_is_typed_error() {
        let slab = sealed_two_row_slab();
        let w = craft("reserved0", &slab);
        assert_eq!(
            Slab64::from_bytes(&w),
            Err(SlabError::ReservedNonZero { got: 7 })
        );
    }

    // W6-C3 — §4.1 flags law (bits 2-15 reserved-zero): a digest-valid
    // window with reserved bit 4 set is a typed refusal. Mutation
    // M-W6-3: delete the `flags_probe & !(PRESENT_INLINE|SEALED)`
    // check — the court flips.
    #[test]
    fn w6_c3_reserved_flag_bits_digest_valid_window_is_typed_error() {
        let slab = sealed_two_row_slab();
        let w = craft("flags", &slab);
        assert_eq!(
            Slab64::from_bytes(&w),
            Err(SlabError::ReservedFlagsSet { got: 0x0013 })
        );
    }

    // W6-C4 — positive-path regression guard for the repair: every
    // structural law accepts exactly the API-produced surface. A full
    // alternating-presence slab seals and decodes; all three new laws
    // pass on the same window the negative courts attack.
    #[test]
    fn w6_c4_api_produced_windows_still_decode_after_repair() {
        let (mut slab, mask) = {
            let mut slab = Slab64::new();
            let mut mask = Mask64::EMPTY;
            for i in 0..64usize {
                let present = i % 2 == 0;
                slab.push(&[i as u8], present).unwrap();
                if present {
                    mask.set(i).unwrap();
                }
            }
            (slab, mask)
        };
        slab.seal_with_mask(&mask).unwrap();
        let round = Slab64::from_bytes(slab.as_bytes()).unwrap();
        assert_eq!(round, slab);
        assert!(round.is_sealed());
        assert_eq!(round.len(), 64);
        // len == cap is the boundary that W6-C1 guards: exactly-full is
        // legal, over-full is not.
        assert_eq!(round.len(), round.capacity());
    }
}

/// Wave-8 defect-repair courts (lane A01 slice: this lane's audit probe,
/// witnessed live against the pre-repair artifact in a /tmp path-dep
/// crate). Five digest-valid-but-structurally-invalid window classes
/// decoded `Ok` before this repair; each law below names its
/// anti-vacuity mutation — a court that cannot flip is FAIL.
#[cfg(test)]
mod wave8_tests {
    use super::*;
    use crate::bitmask::Mask64;

    /// Re-stamp the §4.1 FNV-1a header digest over the patched span so
    /// the crafted window stays digest-valid (the wave-6 `craft`
    /// pattern, generalized to arbitrary field patches).
    fn reheader(m: &mut [u8]) {
        let dig = Slab64::fnv1a32(&m[0x00..0x1C]);
        m[0x1C..0x20].copy_from_slice(&dig.to_le_bytes());
    }

    /// Re-stamp the BLAKE3 payload digest over the patched slots region.
    fn repayload(m: &mut [u8]) {
        let dig = crate::blake3::hash(&m[HEADER_SIZE..]);
        m[0x20..0x40].copy_from_slice(&dig);
    }

    fn sealed_four_row_slab() -> Slab64 {
        let mut s = Slab64::new();
        for i in 0..4 {
            s.push(&[i as u8 + 1; 8], i % 2 == 0).unwrap();
        }
        s.seal().unwrap();
        s
    }

    // W8-C1 — §4.1 layout-version law: a digest-valid window carrying a
    // foreign layout version is a typed refusal; the pristine window
    // still decodes (positive control proves the craft harness did not
    // corrupt the baseline). Mutation M-W8-1: delete the
    // `layout_probe != LAYOUT_VERSION` check in `from_bytes` — the
    // crafted window decodes Ok and this assert fails.
    #[test]
    fn w8_c1_foreign_layout_version_digest_valid_window_is_typed_error() {
        let slab = sealed_four_row_slab();
        assert_eq!(
            Slab64::from_bytes(slab.as_bytes()),
            Ok(slab),
            "positive control broke"
        );
        let mut w = slab.as_bytes().to_vec();
        w[0x04..0x06].copy_from_slice(&2u16.to_le_bytes());
        reheader(&mut w);
        assert_eq!(
            Slab64::from_bytes(&w),
            Err(SlabError::LayoutVersionMismatch { got: 2 })
        );
    }

    // W8-C2 — §4.1 presence_len law: the window's mask-region size must
    // equal `mask_bytes(CAP)` for the receiving monomorphization.
    // Mutation M-W8-2: delete the `presence_len_probe != mask_bytes`
    // check — the court flips.
    #[test]
    fn w8_c2_wrong_presence_len_digest_valid_window_is_typed_error() {
        let slab = sealed_four_row_slab();
        assert_eq!(slab.header.presence_len, 8);
        assert_eq!(crate::bitmask::mask_bytes(64), 8);
        let mut w = slab.as_bytes().to_vec();
        w[0x14..0x18].copy_from_slice(&999u32.to_le_bytes());
        reheader(&mut w);
        assert_eq!(
            Slab64::from_bytes(&w),
            Err(SlabError::PresenceLenMismatch {
                got: 999,
                expected: 8
            })
        );
    }

    // W8-C3 — sealed-window law: `from_bytes` decodes a *sealed* slab
    // (§5.1); a digest-valid window with FLAG_SEALED cleared is a typed
    // refusal. Mutation M-W8-3: delete the `flags_probe & FLAG_SEALED`
    // check — the court flips.
    #[test]
    fn w8_c3_unsealed_digest_valid_window_is_typed_error() {
        let slab = sealed_four_row_slab();
        let mut w = slab.as_bytes().to_vec();
        let flags = u16::from_le_bytes([w[0x06], w[0x07]]) & !FLAG_SEALED;
        w[0x06..0x08].copy_from_slice(&flags.to_le_bytes());
        reheader(&mut w);
        assert_eq!(
            Slab64::from_bytes(&w),
            Err(SlabError::NotSealed { got: FLAG_PRESENT_INLINE })
        );
    }

    // W8-C4 — inline-presence law: every slot carries an inline shadow
    // byte (§4.2), so a window with FLAG_PRESENT_INLINE cleared
    // misdescribes its own layout and is a typed refusal. Mutation
    // M-W8-4: delete the `flags_probe & FLAG_PRESENT_INLINE` check —
    // the court flips.
    #[test]
    fn w8_c4_presence_not_inline_digest_valid_window_is_typed_error() {
        let slab = sealed_four_row_slab();
        let mut w = slab.as_bytes().to_vec();
        let flags = u16::from_le_bytes([w[0x06], w[0x07]]) & !FLAG_PRESENT_INLINE;
        w[0x06..0x08].copy_from_slice(&flags.to_le_bytes());
        reheader(&mut w);
        assert_eq!(
            Slab64::from_bytes(&w),
            Err(SlabError::PresenceNotInline { got: FLAG_SEALED })
        );
    }

    // W8-C5a — §4.2 padding law (receiver side): a digest-valid window
    // with nonzero slot padding (row 0 bytes 0x01..0x03 patched, payload
    // digest re-stamped) is a typed refusal. Mutation M-W8-5: delete the
    // padding check in the decode loop — the court flips.
    #[test]
    fn w8_c5a_nonzero_padding_digest_valid_window_is_typed_error() {
        let slab = sealed_four_row_slab();
        let mut w = slab.as_bytes().to_vec();
        w[HEADER_SIZE + SLOT_SIZE * 2 + 1] = 0xAA;
        w[HEADER_SIZE + SLOT_SIZE * 2 + 3] = 0xBB;
        repayload(&mut w);
        assert_eq!(
            Slab64::from_bytes(&w),
            Err(SlabError::PaddingNonZero { row: 2 })
        );
    }

    // W8-C5b — §4.2 padding law (seal side): "zero-filled at seal" is
    // enforced — in-memory-mutated padding is canonicalized by `seal`
    // BEFORE both digests, so the sealed image is lawful and decodes.
    // Mutation M-W8-5b: delete the padding zero-fill loop in `seal` —
    // the sealed image keeps the nonzero padding, `from_bytes` refuses
    // it with `PaddingNonZero`, and this assert flips.
    #[test]
    fn w8_c5b_seal_canonicalizes_mutated_padding_before_digest() {
        let mut slab = sealed_four_row_slab_neutral();
        slab.slots[1].padding = [0, 0, 0x7F];
        slab.slots[3].padding = [0x11, 0, 0];
        slab.seal().unwrap();
        assert_eq!(slab.slots[1].padding, [0; 3]);
        assert_eq!(slab.slots[3].padding, [0; 3]);
        let round = Slab64::from_bytes(slab.as_bytes()).unwrap();
        assert_eq!(round, slab);
        assert_eq!(round.slots[1].padding, [0; 3]);
    }

    /// Unsealed two-row slab (separate helper so the C5b mutation below
    /// cannot hide behind the C5a craft path).
    fn sealed_four_row_slab_neutral() -> Slab64 {
        let mut s = Slab64::new();
        s.push(&[0xA5; 4], true).unwrap();
        s.push(&[0x5A; 4], false).unwrap();
        s
    }

    // W8-C6 — positive-path regression guard for the whole repair: an
    // API-produced full slab satisfies every wave-8 law and decodes;
    // header state post-decode is exactly the sealed mint (presence_len,
    // flags, layout_version). This is the court M-W8-x cannot break
    // silently in the positive direction — the repair widens refusal
    // only to windows the API never mints.
    #[test]
    fn w8_c6_api_produced_windows_still_decode_after_wave8_repair() {
        let mut slab = Slab64::new();
        let mut mask = Mask64::EMPTY;
        for i in 0..64usize {
            let present = i % 3 != 0;
            let payload: [u8; SLOT_PAYLOAD_SIZE] =
                core::array::from_fn(|j| ((i * 31 + j * 7) & 0xFF) as u8);
            slab.push(&payload, present).unwrap();
            if present {
                mask.set(i).unwrap();
            }
        }
        slab.seal_with_mask(&mask).unwrap();
        let round = Slab64::from_bytes(slab.as_bytes()).unwrap();
        assert_eq!(round, slab);
        assert!(round.is_sealed());
        assert_eq!(round.len(), 64);
        // Every new law holds on the minted image.
        assert_eq!(round.header.layout_version, LAYOUT_VERSION);
        assert_eq!(round.header.presence_len, crate::bitmask::mask_bytes(64) as u32);
        assert_eq!(
            round.header.flags,
            FLAG_PRESENT_INLINE | FLAG_SEALED
        );
        for slot in &round.slots {
            assert_eq!(slot.padding, [0; 3]);
        }
    }
}

/// Wave-9 cross-lane integration courts (lane A01 slice: A04 identity
/// court, A14 dam over A04 frames, A05 power-cut sweeps, A20 1M-row
/// stress). Every collaborator is the real neighbor-lane implementation
/// pulled in as a test-only dev-dependency — no mirrors, no re-implementations,
/// no stubs. Every court names its anti-vacuity mutation (MW9-*); a court
/// that cannot flip is FAIL. No sleeps anywhere: cuts are deterministic
/// byte truncations, generation is a pure function of the row index.
#[cfg(test)]
mod wave9_tests {
    use super::*;
    use crate::bitmask::Mask64;
    use alloc::vec::Vec;
    use cgap_core::frame::{
        CgapFrameHeader, FrameError, P28LicenseAttestation, FLAG_ARROW, FRAME_HEADER_LEN,
        FRAME_MAGIC, FRAME_VERSION, LICENSE_FLAG_ROOT, LICENSE_MAGIC,
    };
    use cgap_gate::penstock::{
        ingress, CGAP_ERR_INVALID_MAGIC, CGAP_ERR_OABI_MISMATCH, CGAP_ERR_P28_SPOLIATION, CGAP_OK,
        FrameView,
    };
    use cgap_storage::atomic_commit::{
        begin_txn, recover, tear_class_for_offset, write_record_cut, FlashMedium, JournalWriter,
        NewRootDescriptor, RecoveryReport, SlotVerdict, TearClass, FLAG_DUAL_WRITE, FIRST_SEQ,
        OFF_PARQUET_ROOT, PAYLOAD_SIZE, RECORD_SIZE,
    };

    // ------------------------------------------------------------------
    // Deterministic corpus generator (A20 §5.1 shape): SplitMix64 with a
    // fixed seed committed in source. No entropy, no clocks, no fs order.
    // ------------------------------------------------------------------

    /// Fixed corpus seed (committed per A20 §5.1; drift is a court failure).
    const W9_SEED: u64 = 0xA20C_0FFE_E5A7_E000;

    /// Exactly 1,000,000 rows (A20 §5 headline scale).
    const W9_ROWS: usize = 1_000_000;
    /// Rows per `Slab64` -> 15,625 slabs = 65,000,000 sealed bytes.
    const W9_SLABS: usize = W9_ROWS / 64;

    /// One SplitMix64 step (Steele et al. finalizer constants).
    const fn splitmix64(state: &mut u64) -> u64 {
        *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = *state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Row payload: a pure function of the row index (60 bytes, full slot
    /// payload). Eight SplitMix64 draws seeded by the row — identical on
    /// every pass by construction.
    fn row_payload(row: usize) -> [u8; SLOT_PAYLOAD_SIZE] {
        let mut state = W9_SEED ^ (row as u64).wrapping_mul(0xD1B5_4A32_D192_ED03);
        let mut payload = [0u8; SLOT_PAYLOAD_SIZE];
        let mut filled = 0;
        while filled < SLOT_PAYLOAD_SIZE {
            let v = splitmix64(&mut state);
            let bytes = v.to_le_bytes();
            let take = core::cmp::min(8, SLOT_PAYLOAD_SIZE - filled);
            payload[filled..filled + take].copy_from_slice(&bytes[..take]);
            filled += take;
        }
        payload
    }

    /// Presence pattern of the corpus: even rows present (alternating), so
    /// a full slab carries exactly 32 present rows and the whole corpus
    /// carries exactly 500,000.
    const fn row_present(row: usize) -> bool {
        row % 2 == 0
    }

    /// Seal rows `start..start + 64` into one `Slab64` with its
    /// authoritative mask, via the real `push`/`seal_with_mask` path.
    fn sealed_slab_for_range(start: usize) -> (Slab64, Mask64) {
        let mut slab = Slab64::new();
        let mut mask = Mask64::EMPTY;
        for r in 0..64 {
            let row = start + r;
            let present = row_present(row);
            slab.push(&row_payload(row), present).unwrap();
            if present {
                mask.set(r).unwrap();
            }
        }
        assert_eq!(slab.len(), 64);
        slab.seal_with_mask(&mask).unwrap();
        (slab, mask)
    }

    /// Small deterministic fixture for the frame/dam/sweep courts: `n`
    /// rows, alternating presence, index-derived payloads.
    fn seeded_slab64(n: usize) -> Slab64 {
        let mut slab = Slab64::new();
        let mut mask = Mask64::EMPTY;
        for i in 0..n {
            let mut payload = [0u8; SLOT_PAYLOAD_SIZE];
            for (j, b) in payload.iter_mut().enumerate() {
                *b = ((i * 7 + j * 13) & 0xFF) as u8;
            }
            let present = i % 2 == 0;
            slab.push(&payload, present).unwrap();
            if present {
                mask.set(i).unwrap();
            }
        }
        slab.seal_with_mask(&mask).unwrap();
        slab
    }

    // ------------------------------------------------------------------
    // A04 frame fixtures
    // ------------------------------------------------------------------

    /// A17-authoritative ROOT attestation: zero upstream receipt (the §2.2
    /// receipt law: ROOT xor receipt-zero), ceiling inside the safe harbor.
    fn root_attestation() -> P28LicenseAttestation {
        P28LicenseAttestation {
            license_magic: LICENSE_MAGIC,
            version: 1,
            attestation_flags: LICENSE_FLAG_ROOT,
            safe_harbor_ceiling_cents: 1_000_000_000,
            origin_authority_digest: [0xAA; 32],
            upstream_receipt: [0; 32],
        }
    }

    /// Frame a sealed slab as an A04 §6 frame: `FLAG_ARROW`,
    /// `payload_byte_len` = the sealed window length, `oabi_digest` =
    /// real in-crate BLAKE3 over the window (the digest fn injected into
    /// `verify_payload`), ROOT attestation.
    fn frame_slab(slab: &Slab64) -> Vec<u8> {
        let window = slab.as_bytes();
        let header = CgapFrameHeader {
            magic: FRAME_MAGIC,
            version: FRAME_VERSION,
            flags: FLAG_ARROW,
            reserved: [0; 2],
            payload_byte_len: window.len() as u64,
            oabi_digest: crate::blake3::hash(window),
            attestation: root_attestation(),
        };
        let mut frame = alloc::vec![0u8; FRAME_HEADER_LEN + window.len()];
        header.encode_into(&mut frame).unwrap();
        frame[FRAME_HEADER_LEN..].copy_from_slice(window);
        assert_eq!(frame.len(), FRAME_HEADER_LEN + 64 + 64 * 64);
        frame
    }

    // ------------------------------------------------------------------
    // W9-C1 — A04 identity court
    // ------------------------------------------------------------------

    // A sealed slab rides a real A04 §6 frame; the whole identity chain
    // decode -> validate -> payload view -> verify_payload -> from_bytes
    // closes on structural identity, and every corruption class named by
    // A04 §8.3 maps to its typed refusal.
    // Anti-vacuity mutation MW9-1: swap the injected digest fn for a
    // zero-constant fn — the positive-path `verify_payload` assert flips
    // (digest bypass detected by the court itself).
    #[test]
    fn w9_c1_a04_identity_court() {
        let slab = seeded_slab64(64);
        let frame = frame_slab(&slab);
        assert_eq!(slab.header.digest, crate::blake3::hash(&slab.as_bytes()[64..]));

        // Positive control: pristine chain closes on identity.
        let hdr = CgapFrameHeader::decode(&frame).unwrap();
        hdr.validate().unwrap();
        let payload = hdr.payload(&frame).unwrap();
        assert_eq!(payload, slab.as_bytes());
        hdr.verify_payload(payload, crate::blake3::hash).unwrap();
        let round = Slab64::from_bytes(payload).unwrap();
        assert_eq!(round, slab);

        // Slab-plane corruption seen through the frame identity: one
        // payload byte inside the carried slab flips -> the frame digest
        // court refuses (DigestMismatch) AND the slab plane refuses
        // (DigestMismatch) — two independent identity courts, one mutation.
        let mut m = frame.clone();
        m[FRAME_HEADER_LEN + 0x40 + 3 * 64 + 10] ^= 0xFF;
        let hdr = CgapFrameHeader::decode(&m).unwrap();
        let p = hdr.payload(&m).unwrap();
        assert_eq!(
            hdr.verify_payload(p, crate::blake3::hash),
            Err(FrameError::DigestMismatch)
        );
        assert_eq!(Slab64::from_bytes(p), Err(SlabError::DigestMismatch));

        // A04 §8.3 header-plane classes:
        // magic flip -> BadMagic;
        let mut m = frame.clone();
        m[0] = b'X';
        assert_eq!(
            CgapFrameHeader::decode(&m).unwrap().validate(),
            Err(FrameError::BadMagic)
        );
        // attestation corruption (safe-harbor ceiling over max) ->
        // AttestationMismatch (the ceiling u64 is LE: byte 7 is the high
        // byte; setting it plants bit 62, far past the $50M ceiling);
        let mut m = frame.clone();
        m[48 + 8 + 7] = 0x40;
        assert_eq!(
            CgapFrameHeader::decode(&m).unwrap().validate(),
            Err(FrameError::AttestationMismatch)
        );
        // payload_byte_len lie -> PayloadLenMismatch (the u64 length is
        // never narrowed, it is compared);
        let mut m = frame.clone();
        m[8] ^= 0x01;
        let hdr = CgapFrameHeader::decode(&m).unwrap();
        assert_eq!(
            hdr.verify_payload(&m[FRAME_HEADER_LEN..], crate::blake3::hash),
            Err(FrameError::PayloadLenMismatch)
        );
        // oabi_digest field corruption -> DigestMismatch.
        let mut m = frame.clone();
        m[16] ^= 0x01;
        let hdr = CgapFrameHeader::decode(&m).unwrap();
        assert_eq!(
            hdr.verify_payload(hdr.payload(&m).unwrap(), crate::blake3::hash),
            Err(FrameError::DigestMismatch)
        );
    }

    // ------------------------------------------------------------------
    // W9-C2 — A14 dam over A04 frames carrying slab payloads
    // ------------------------------------------------------------------

    // The full ingress taxonomy rows 1-5 over a real framed sealed slab:
    // the lawful frame ADMITs and the out-view carries exactly the slab
    // window; every named fault class maps to its A19 register code.
    // Anti-vacuity mutation MW9-2: change the expected code of the
    // zero-digest row from CGAP_ERR_OABI_MISMATCH to CGAP_OK — the row
    // assert flips.
    #[test]
    fn w9_c2_a14_dam_over_a04_frames() {
        let slab = seeded_slab64(64);
        let frame = frame_slab(&slab);
        let n = frame.len();

        // Positive control: ADMIT, and the out-view IS the slab window.
        let mut view = FrameView { payload: &[] };
        assert_eq!(ingress(&frame, n, Some(&mut view)), CGAP_OK);
        assert_eq!(view.payload, slab.as_bytes());

        // Row 1: truncated header -> INVALID_MAGIC; a lying caller-declared
        // length -> INVALID_MAGIC (the gate reads nothing past the slice).
        assert_eq!(ingress(&frame, 127, None), CGAP_ERR_INVALID_MAGIC);
        assert_eq!(ingress(&frame, n + 1, None), CGAP_ERR_INVALID_MAGIC);

        // Row 2: magic flip -> INVALID_MAGIC; version bump -> INVALID_MAGIC.
        let mut m = frame.clone();
        m[0] = b'X';
        assert_eq!(ingress(&m, n, None), CGAP_ERR_INVALID_MAGIC);
        let mut m = frame.clone();
        m[4] = 2;
        assert_eq!(ingress(&m, n, None), CGAP_ERR_INVALID_MAGIC);

        // Row 3a: all-zero oabi_digest -> OABI_MISMATCH.
        let mut m = frame.clone();
        m[16..48].fill(0);
        assert_eq!(ingress(&m, n, None), CGAP_ERR_OABI_MISMATCH);
        // MW9-2 anchor: this row's code is the mutation target.
        assert_ne!(CGAP_ERR_OABI_MISMATCH, CGAP_OK);

        // Row 3b: declared payload beyond the buffer -> OABI_MISMATCH.
        let mut m = frame.clone();
        m[8] ^= 0x01;
        assert_eq!(ingress(&m, n, None), CGAP_ERR_OABI_MISMATCH);

        // Row 4: attestation magic flip P28S -> P28X -> P28_SPOLIATION.
        let mut m = frame.clone();
        m[48] = b'X';
        assert_eq!(ingress(&m, n, None), CGAP_ERR_P28_SPOLIATION);

        // Row 5a: attestation version 2 -> P28_SPOLIATION.
        let mut m = frame.clone();
        m[48 + 4] = 2;
        assert_eq!(ingress(&m, n, None), CGAP_ERR_P28_SPOLIATION);
        // Row 5b: reserved attestation flag bit 15 -> P28_SPOLIATION.
        let mut m = frame.clone();
        m[48 + 6 + 1] = 0x80;
        assert_eq!(ingress(&m, n, None), CGAP_ERR_P28_SPOLIATION);
        // Row 5c: two lineage bits set -> P28_SPOLIATION.
        let mut m = frame.clone();
        m[48 + 6] = 0x03;
        assert_eq!(ingress(&m, n, None), CGAP_ERR_P28_SPOLIATION);
        // Row 5d: safe-harbor ceiling over max -> P28_SPOLIATION (LE high
        // byte plants bit 62, past the $50M ceiling).
        let mut m = frame.clone();
        m[48 + 8 + 7] = 0x40;
        assert_eq!(ingress(&m, n, None), CGAP_ERR_P28_SPOLIATION);
        // Row 5e: ROOT lineage with a NONZERO upstream receipt -> P28_SPOLIATION
        // (receipt law: ROOT xor receipt-zero; upstream_receipt sits at
        // frame offset 48 + 48 = 96).
        let mut m = frame.clone();
        m[96] = 0x01;
        assert_eq!(ingress(&m, n, None), CGAP_ERR_P28_SPOLIATION);
        // Row 5f: DERIVATIVE lineage (0x02) with a ZERO receipt ->
        // P28_SPOLIATION (the mirror law).
        let mut m = frame.clone();
        m[48 + 6] = 0x02;
        assert_eq!(ingress(&m, n, None), CGAP_ERR_P28_SPOLIATION);
    }

    // ------------------------------------------------------------------
    // W9-C3 — A05 power-cut sweep, framed-slab plane
    // ------------------------------------------------------------------

    // Lane A05 §5 invariant I1 on the slab commit boundary: cut the
    // 4288-byte framed sealed slab at EVERY byte offset k in 0..=N (4289
    // cases enumerated, never sampled). Any partial cut is a typed refusal
    // on all three identity planes (A14 dam, A04 verify/decode, A01
    // from_bytes); only the complete cut (k = N) commits. Never a partial
    // artifact exposed.
    // Anti-vacuity mutation MW9-3: invert the C4 branch (expect refusal at
    // k = N) — the sweep fails at k = N itself.
    #[test]
    fn w9_c3_a05_power_cut_sweep_framed_slab_plane() {
        let slab = seeded_slab64(64);
        let frame = frame_slab(&slab);
        let n = frame.len();
        assert_eq!(n, FRAME_HEADER_LEN + 64 + 64 * 64);

        let mut refusals = 0;
        for k in 0..=n {
            let cut = &frame[..k];
            if k == n {
                // C4: the complete cut is the commit; the full chain closes.
                let mut view = FrameView { payload: &[] };
                assert_eq!(ingress(cut, k, Some(&mut view)), CGAP_OK);
                assert_eq!(view.payload, slab.as_bytes());
                let hdr = CgapFrameHeader::decode(cut).unwrap();
                hdr.validate().unwrap();
                hdr.verify_payload(hdr.payload(cut).unwrap(), crate::blake3::hash)
                    .unwrap();
                assert_eq!(Slab64::from_bytes(view.payload).unwrap(), slab);
            } else if k < FRAME_HEADER_LEN {
                // C0/C1: torn or absent header -> dam INVALID_MAGIC; the
                // A04 decoder refuses the short buffer.
                assert_eq!(ingress(cut, k, None), CGAP_ERR_INVALID_MAGIC);
                assert_eq!(
                    CgapFrameHeader::decode(cut),
                    Err(FrameError::BufferTooSmall)
                );
                refusals += 1;
            } else {
                // C2: complete header, torn payload -> dam OABI_MISMATCH
                // (declared payload does not fit the bytes present), the
                // A04 decoder accepts the layout but the length court
                // refuses, and the slab plane refuses BadLength.
                assert_eq!(ingress(cut, k, None), CGAP_ERR_OABI_MISMATCH);
                let hdr = CgapFrameHeader::decode(cut).unwrap();
                assert_eq!(hdr.payload(cut), Err(FrameError::PayloadLenMismatch));
                assert_eq!(
                    Slab64::from_bytes(&cut[FRAME_HEADER_LEN..]),
                    Err(SlabError::BadLength {
                        expected: 64 + 64 * 64,
                        got: k - FRAME_HEADER_LEN,
                    })
                );
                refusals += 1;
            }
        }
        assert_eq!(refusals, n, "every partial cut must be a typed refusal");
    }

    // ------------------------------------------------------------------
    // W9-C3b — A05 power-cut sweep, real double-slot journal plane
    // ------------------------------------------------------------------

    // The slab checkpoint rides the REAL A05 transaction machinery
    // (FlashMedium + fence + begin_txn/write_record + recover). Cut at
    // every k in 0..=RECORD_SIZE (113 cases, enumerated): recovery returns
    // the prior root (checkpoint lost, nothing partial) or the new root
    // (checkpoint durable with its slab manifest) — never a third value.
    // The descriptor's `parquet_root` u64 carries the sealed-slab manifest
    // value (first 8 bytes of the sealed BLAKE3, LE) — a TEST-PLANE
    // binding disclosed here; the production meaning of the field is A02's
    // per A05 §6 (cgap-storage treats it opaquely).
    // Anti-vacuity mutation MW9-3b: expect the new root at a torn offset —
    // the first torn k flips the sweep.
    #[test]
    fn w9_c3b_a05_journal_plane_slab_checkpoint_sweep() {
        let slab = seeded_slab64(64);
        assert!(slab.is_sealed());
        let digest = slab.header.digest;
        let manifest = u64::from_le_bytes(digest[..8].try_into().unwrap());

        let base = NewRootDescriptor::new([9u8; 20], 0, 9, 0, FLAG_DUAL_WRITE);
        let checkpoint = NewRootDescriptor::new([9u8; 20], manifest, 10, 0, FLAG_DUAL_WRITE);

        // Tear-class oracle: A05's own classifier names the band, the
        // sweep's expectations derive from it (a constants drift trips the
        // court, not the other way around).
        assert_eq!(tear_class_for_offset(0), TearClass::PreWrite);
        assert_eq!(tear_class_for_offset(1), TearClass::TornHeader);
        assert_eq!(tear_class_for_offset(48), TearClass::TornPayload);
        assert_eq!(tear_class_for_offset(96), TearClass::TornTrailer);
        assert_eq!(tear_class_for_offset(RECORD_SIZE), TearClass::CompleteRecord);

        // Positive path: the complete commit is durable; recovery returns
        // the checkpoint; the manifest matches the sealed slab digest; the
        // committed slab still decodes with structural identity.
        let mut medium = FlashMedium::new(base);
        medium.fence_artifacts();
        let mut txn = begin_txn(&mut medium, FIRST_SEQ);
        txn.write_record(&checkpoint).unwrap();
        let root = txn.commit_txn().unwrap();
        assert_eq!(root, checkpoint);
        let out = recover(&medium).unwrap();
        assert_eq!(out.root, checkpoint);
        assert_eq!(out.recovered_seq, Some(FIRST_SEQ));
        assert_eq!(out.report, RecoveryReport::Clean);
        assert_eq!(out.slot_verdicts, [SlotVerdict::Valid, SlotVerdict::Empty]);
        // `NewRootDescriptor` is repr(C, packed): the manifest is read
        // through the public wire codec, never by field reference.
        let wire = out.root.to_bytes();
        assert_eq!(
            u64::from_le_bytes(wire[OFF_PARQUET_ROOT..OFF_PARQUET_ROOT + 8].try_into().unwrap()),
            manifest
        );
        assert_eq!(wire.len(), PAYLOAD_SIZE);
        assert_eq!(Slab64::from_bytes(slab.as_bytes()).unwrap(), slab);

        // The sweep: fresh medium per offset, real byte-granular cut.
        for k in 0..=RECORD_SIZE {
            let mut medium = FlashMedium::new(base);
            medium.fence_artifacts();
            if k == RECORD_SIZE {
                write_record_cut(&mut medium, FIRST_SEQ, &checkpoint, k).unwrap();
                let out = recover(&medium).unwrap();
                assert_eq!(out.root, checkpoint, "k={k} complete cut must commit");
                assert_eq!(out.recovered_seq, Some(FIRST_SEQ));
                assert_eq!(out.report, RecoveryReport::Clean);
                assert_eq!(out.slot_verdicts, [SlotVerdict::Valid, SlotVerdict::Empty]);
            } else {
                write_record_cut(&mut medium, FIRST_SEQ, &checkpoint, k).unwrap();
                let out = recover(&medium).unwrap();
                // I1: rollback to the prior root, never a partial artifact.
                assert_eq!(out.root, base, "k={k} torn cut must roll back");
                assert_eq!(out.recovered_seq, None);
                match tear_class_for_offset(k) {
                    TearClass::PreWrite => {
                        assert_eq!(out.slot_verdicts, [SlotVerdict::Empty, SlotVerdict::Empty]);
                        assert_eq!(out.report, RecoveryReport::Clean);
                    }
                    TearClass::TornHeader | TearClass::TornPayload | TearClass::TornTrailer => {
                        assert_eq!(out.slot_verdicts, [SlotVerdict::Damaged, SlotVerdict::Empty]);
                        assert_eq!(out.report, RecoveryReport::RecoveryDegraded);
                    }
                    TearClass::CompleteRecord => unreachable!("k={k} handled above"),
                }
            }
        }
    }

    // ------------------------------------------------------------------
    // W9-C4 — A20 1M-row stress
    // ------------------------------------------------------------------

    /// Folded corpus digest pinned after first observation (two independent
    /// passes agreed on it; drift in the generator, the slab layout law, or
    /// the seal digest now flips this court).
    const W9_CORPUS_FOLDED: [u8; 32] = [
        0x52, 0x1e, 0x87, 0x73, 0xfc, 0xc6, 0x0e, 0x14, 0x1c, 0x73, 0x9d, 0xfc, 0xe1, 0xe9, 0xd5,
        0xf8, 0xbc, 0x84, 0xc9, 0xee, 0x28, 0x86, 0x52, 0xaa, 0x8f, 0x61, 0x5b, 0x75, 0xc3, 0x85,
        0x9f, 0x35,
    ];

    /// Aggregate state laws of one stress pass.
    struct StressTotals {
        rows: usize,
        slabs: usize,
        present: usize,
        sealed_bytes: usize,
    }

    /// One pass over the whole corpus: 1,000,000 rows paged through 15,625
    /// `Slab64`s; every slab seals (BLAKE3 over its 4096 slot bytes) and
    /// decodes with structural identity; presence counts accumulate; the
    /// sealed digests fold into one running digest.
    fn stress_pass() -> ([u8; 32], StressTotals) {
        let mut folded = [0u8; 32];
        let mut totals = StressTotals {
            rows: 0,
            slabs: 0,
            present: 0,
            sealed_bytes: 0,
        };
        for s in 0..W9_SLABS {
            let (slab, mask) = sealed_slab_for_range(s * 64);
            assert_eq!(slab.len(), 64);
            assert_eq!(slab.capacity(), 64);
            // The mask region law: presence_len names exactly the mask
            // bytes the authoritative mask occupies (A01 §4.1 x A01 §4.4).
            assert_eq!(slab.header.presence_len as usize, mask.bytes().len());
            assert_eq!(mask.count(), 32);
            // Sealed identity: decode closes on structural identity.
            let round = Slab64::from_bytes(slab.as_bytes()).unwrap();
            assert_eq!(round, slab);
            // Cache-line size law holds at scale.
            assert_eq!(slab.as_bytes().len() % 64, 0);
            folded = crate::blake3::hash(&[folded.as_slice(), round.header.digest.as_slice()].concat());
            totals.rows += round.len() as usize;
            totals.slabs += 1;
            totals.present += mask.count();
            totals.sealed_bytes += slab.as_bytes().len();
        }
        (folded, totals)
    }

    // Exactly 1,000,000 rows: two independent deterministic passes agree
    // (determinism court), agree with the pinned folded digest (corpus
    // receipt), carry exactly 500,000 present rows and 65,000,000 sealed
    // bytes, and the deterministic first slab closes the full cross-lane
    // identity chain (A04 frame -> A14 dam -> A04 verify -> A01 decode).
    // Anti-vacuity mutation MW9-4: perturb one byte of W9_CORPUS_FOLDED —
    // the passes still agree (determinism) but the pin assert flips.
    #[test]
    fn w9_c4_a20_1m_row_stress() {
        let (f1, t1) = stress_pass();
        let (f2, t2) = stress_pass();
        assert_eq!(f1, f2, "deterministic regeneration must agree");
        assert_eq!(f1, W9_CORPUS_FOLDED, "pinned corpus digest drift");
        assert_eq!(t1.rows, t2.rows);
        assert_eq!(t1.rows, W9_ROWS);
        assert_eq!(t1.slabs, W9_SLABS);
        assert_eq!(t1.present, 500_000);
        assert_eq!(t1.sealed_bytes, W9_SLABS * (64 + 64 * 64));

        // Cross-lane chain on the deterministic first slab.
        let (first, _) = sealed_slab_for_range(0);
        let frame = frame_slab(&first);
        let mut view = FrameView { payload: &[] };
        assert_eq!(ingress(&frame, frame.len(), Some(&mut view)), CGAP_OK);
        assert_eq!(view.payload, first.as_bytes());
        let hdr = CgapFrameHeader::decode(&frame).unwrap();
        hdr.validate().unwrap();
        hdr.verify_payload(hdr.payload(&frame).unwrap(), crate::blake3::hash)
            .unwrap();
        assert_eq!(Slab64::from_bytes(view.payload).unwrap(), first);
    }
}
