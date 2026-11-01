//! Memory tier: DMA ring (lane A04, Wave 2 core engines).
//!
//! Spec: `docs/jira/v26.11.1/lanes/A04-triumvirate-isomorphism.md` §7
//! (shape + cache-line discipline) and §7.3 (API sketch). Wave 1 froze the
//! type-level ABI; Wave 2 lands the §7.3 method surface (`new`/`claim`/
//! `publish`/`read`/`release`) on exactly those types, with the §8.3 typed
//! refusals (`Overflow`; unpublished read → `None`) and the §10 fail-closed
//! region policy (64-aligned region, whole-slot division, 64-byte slot
//! stride). Unit courts: `tests/unit_dma_ring.rs` (heap-backed regions are
//! test-only; the crate source stays alloc-free, enforced by
//! `tests/source_scan_gate.rs`).

#![allow(dead_code)] // Kernel ABI for consumer lanes (A14/A17/A19); unused here by design.

use core::sync::atomic::{AtomicUsize, Ordering};

use crate::frame::CgapFrameHeader;

/// Cache-line width used for all padding decisions (§7.2).
pub const CACHE_LINE_BYTES: usize = 64;

/// One ring slot holds one frame: the 128-byte `CgapFrameHeader` plus its
/// payload region (§7.1 "each slot = one frame: 128 B header + payload
/// region").
pub const SLOT_HEADER_LEN: usize = crate::frame::FRAME_HEADER_LEN;

/// Typed ring refusals (§7.3 `Result<_, RingError>`, §8.3, §10).
///
/// `Overflow` and the two region refusals are doc-named; the remaining
/// variants cover ring-protocol refusal paths the sketch leaves open
/// (publish without claim, malformed frame at publish, release on empty) —
/// disclosed additions, never silent no-ops.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RingError {
    /// §8.3: publish payload len > slot capacity.
    Overflow,
    /// §10: `DmaRing::new` requires a 64-byte-aligned region; refuses
    /// otherwise, fail-closed.
    MisalignedRegion,
    /// §7.1: the caller-supplied region cannot hold `2^slot_pow2` slots of
    /// >= 64 B at 64-byte stride.
    RegionTooSmall,
    /// Publish called with no live claim, or twice for one claim.
    PublishWithoutClaim,
    /// The producer's frame failed structural validation at publish
    /// (§7.3: "publish a claimed slot (frame must validate)").
    BadFrame,
    /// Consumer released with no published-but-unreleased frame pending.
    ReleaseOnEmpty,
}

/// §7.2 false-sharing discipline: one producer/consumer index per cache
/// line, `#[repr(align(64))]`-wrapped.
#[repr(align(64))]
struct CachePaddedIndex(AtomicUsize);

impl CachePaddedIndex {
    const fn new(v: usize) -> Self {
        CachePaddedIndex(AtomicUsize::new(v))
    }
    fn load(&self, ord: Ordering) -> usize {
        self.0.load(ord)
    }
    fn store(&self, v: usize, ord: Ordering) {
        self.0.store(v, ord)
    }
}

/// SPSC ring over caller-supplied memory (§7.3). One producer, one consumer;
/// with a single handle, `&mut self` methods enforce the single-accessor
/// discipline by construction, while `head`/`tail` remain `AtomicUsize`
/// (§7.1) so a DMA engine or peer core can observe publish/release progress.
///
/// Slot state machine (§7.3 note): free → claimed → published → released,
/// tracked implicitly by head/tail distance plus the one in-flight claim.
pub struct DmaRing {
    /// Caller-supplied byte region, partitioned into slots at init (§7.1).
    region: &'static mut [u8],
    /// Stride between slots in bytes; a multiple of `CACHE_LINE_BYTES` so
    /// every slot is 64-byte aligned when the region is (§7.2).
    slot_size: usize,
    /// Slot count, `2^slot_pow2` (§7.1 power-of-two, mask-indexed).
    capacity: usize,
    /// Producer publish index, cache-line isolated (§7.2).
    head: CachePaddedIndex,
    /// Consumer release index, cache-line isolated (§7.2).
    tail: CachePaddedIndex,
    /// The one in-flight producer claim (§7.3 claim→publish sequencing).
    /// A claim whose slice was dropped without `publish` is abandoned; the
    /// next `claim` reuses the slot (which cannot alias: the borrow checker
    /// guarantees the previous claim slice is gone).
    claimed: Option<usize>,
}

impl DmaRing {
    /// Partition `region` into `2^slot_pow2` equal slots (§7.3 sketch).
    ///
    /// Fail-closed refusals (§10): `MisalignedRegion` unless `region` starts
    /// on a 64-byte boundary; `RegionTooSmall` unless the region divides
    /// exactly into `2^slot_pow2` slots of a 64-byte multiple, each holding
    /// the 128 B header plus at least one payload byte.
    pub fn new(region: &'static mut [u8], slot_pow2: u8) -> Result<Self, RingError> {
        let region_addr = region.as_ptr() as usize;
        if region_addr % CACHE_LINE_BYTES != 0 {
            return Err(RingError::MisalignedRegion);
        }
        if slot_pow2 >= usize::BITS as u8 {
            return Err(RingError::RegionTooSmall);
        }
        let capacity = 1usize << slot_pow2;
        let slot_size = region.len() / capacity;
        if slot_size == 0
            || region.len() % capacity != 0
            || slot_size % CACHE_LINE_BYTES != 0
            || slot_size < SLOT_HEADER_LEN + 1
        {
            return Err(RingError::RegionTooSmall);
        }
        Ok(DmaRing {
            region,
            slot_size,
            capacity,
            head: CachePaddedIndex::new(0),
            tail: CachePaddedIndex::new(0),
            claimed: None,
        })
    }

    /// Slot stride in bytes (128 B header + per-slot payload capacity).
    pub fn slot_size(&self) -> usize {
        self.slot_size
    }

    /// Slot count, `2^slot_pow2` (§7.1).
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Frames published and not yet released (head/tail distance).
    pub fn pending(&self) -> usize {
        self.head
            .load(Ordering::Acquire)
            .wrapping_sub(self.tail.load(Ordering::Acquire))
    }

    /// §7.3 producer: claim a slot for write, returns `&mut [u8]` of slot
    /// size. `None` when the ring is full (all slots published-unreleased).
    /// The full slot is zero-filled at claim (§10: "claim() zero-fills the
    /// full slot before publish"), so an abandoned previous claim leaves no
    /// stale bytes.
    pub fn claim(&mut self) -> Option<&mut [u8]> {
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Acquire);
        if head.wrapping_sub(tail) == self.capacity {
            return None; // ring full
        }
        let idx = head & (self.capacity - 1); // §7.1 mask-indexed
        let start = idx * self.slot_size;
        let slot = &mut self.region[start..start + self.slot_size];
        slot.fill(0); // §10 padding law: no stale bytes observed at publish
        self.claimed = Some(idx);
        Some(slot)
    }

    /// §7.3 producer: publish the claimed slot. `len` is the total frame
    /// length (header + payload). The frame must validate (§7.3): structural
    /// header validation plus `payload_byte_len == len - 128`, so a published
    /// slot is safe for the consumer's `read` bounds by construction.
    ///
    /// Refusals leave the ring untouched (typed, fail-closed): `Overflow`
    /// when `len > slot_size` (§8.3), `BadFrame` when the header fails
    /// validation or the length disagrees, `PublishWithoutClaim` when no
    /// claim is live (including an abandoned one).
    pub fn publish(&mut self, len: usize) -> Result<(), RingError> {
        let idx = self
            .claimed
            .take()
            .ok_or(RingError::PublishWithoutClaim)?;
        if len > self.slot_size {
            return Err(RingError::Overflow);
        }
        if len < SLOT_HEADER_LEN {
            return Err(RingError::BadFrame);
        }
        let start = idx * self.slot_size;
        let slot = &self.region[start..start + self.slot_size];
        let header = crate::frame::CgapFrameHeader::decode(slot)
            .map_err(|_| RingError::BadFrame)?;
        if header.validate().is_err() {
            return Err(RingError::BadFrame);
        }
        // Wave-5 repair (A04 defect fix): widening compare — `as usize` on
        // the u64 header field truncated on 32-bit targets, so a header
        // declaring `2^32 + k` passed this §7.3 gate against a `128 + k`
        // publish. Widening the usize side is exact on every target.
        if header.payload_byte_len != (len - SLOT_HEADER_LEN) as u64 {
            return Err(RingError::BadFrame);
        }
        let head = self.head.load(Ordering::Acquire);
        // §7.1: "AcqRel head-publish" — the slot bytes are published before
        // the index advance becomes visible (Release); observers read the
        // index with Acquire.
        self.head.store(head.wrapping_add(1), Ordering::Release);
        Ok(())
    }

    /// §7.3 consumer: read the next published slot, returning the payload
    /// slice and a header view into the region. `None` when no frame is
    /// pending (§8.3: "read an unpublished slot → `None` (typed: no frame),
    /// never garbage").
    ///
    /// The header pointer cast is sound because: the region starts 64-aligned
    /// (`new` refuses otherwise) and every slot starts at a multiple of the
    /// 64-byte slot stride, so `CgapFrameHeader`'s `align(64)` is satisfied;
    /// the region bytes are initialized; both structs are gap-free (pinned by
    /// const asserts in `frame.rs`) so no uninitialized padding is ever read;
    /// and every field type (`[u8; N]`, `u8`, `u64`) is valid for all bit
    /// patterns. `publish` is the only code path that advances `head`, and it
    /// guarantees the embedded `payload_byte_len` fits the slot; a region
    /// written around the ring API is refused below (no frame, tail
    /// unchanged), never handed out as garbage.
    pub fn read(&mut self) -> Option<(&[u8], &CgapFrameHeader)> {
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Acquire);
        if head == tail {
            return None;
        }
        let idx = tail & (self.capacity - 1);
        let start = idx * self.slot_size;
        let slot = &self.region[start..start + self.slot_size];
        // SAFETY: alignment + initialization + gap-free layout proven above.
        let header = unsafe { &*(slot.as_ptr() as *const CgapFrameHeader) };
        // Fail-closed bound: refuse (no frame, tail unchanged) rather than
        // hand out an out-of-slot payload if the region was written around
        // the ring API. Wave-5 repair (A04 defect fix): the compare is on
        // the widened u64, never the narrowed `as usize` — on 32-bit targets
        // a lying header declaring `2^32` truncated to 0, slipped the bound,
        // and was handed out with its slot bytes. After the u64 gate proves
        // the declared length fits the slot (hence any `usize`), the cast
        // below is exact on every target.
        if header.payload_byte_len > (self.slot_size - SLOT_HEADER_LEN) as u64 {
            return None;
        }
        let payload_len = header.payload_byte_len as usize;
        Some((
            &slot[SLOT_HEADER_LEN..SLOT_HEADER_LEN + payload_len],
            header,
        ))
    }

    /// §7.3 consumer: release the previously read slot. Typed refusal
    /// (`ReleaseOnEmpty`) instead of a silent no-op when nothing is pending.
    pub fn release(&mut self) -> Result<(), RingError> {
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Acquire);
        if head == tail {
            return Err(RingError::ReleaseOnEmpty);
        }
        self.tail.store(tail.wrapping_add(1), Ordering::Release);
        Ok(())
    }
}

// §7.2 cache-line pins — a padding regression to 32 B must fail to compile.
const _: () = assert!(core::mem::size_of::<CachePaddedIndex>() == CACHE_LINE_BYTES);
const _: () = assert!(core::mem::align_of::<CachePaddedIndex>() == CACHE_LINE_BYTES);
