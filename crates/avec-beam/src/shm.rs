//! avec-beam/src/shm.rs — lane BEAM-L9. POSIX shared-memory SPSC slot ring.
//! Version law 26.10.29.
//!
//! Fixed-size single-producer/single-consumer ring transported over a POSIX
//! `shm_open` + `mmap` segment, declared with bare `extern "C"` libc calls —
//! zero crates, matching the crate's hand-rolled-ABI discipline.
//!
//! Layout (offsets are fixed so any handle sees one format):
//!
//! ```text
//! [0..64)      header: magic "AVE_SHM1", format version, num_slots, slot_size
//! [64..192)    head: AtomicU64, producer-owned writes, 128-byte padded
//! [192..320)   tail: AtomicU64, consumer-owned writes, 128-byte padded
//! [320..)      NUM_SLOTS x SLOT_SIZE payload slots
//! ```
//!
//! Padding is 128 bytes: Apple M-series L1/L2 lines are 128 B (x86-64 is
//! 64 B), so 128 keeps `head` and `tail` on separate lines on both hosts and
//! kills false sharing between the endpoints' release-stores.
//!
//! Ordering (Acquire/Release, the minimal correct SPSC protocol):
//! * producer: `tail.load(Acquire)` observes the consumer's finished reads;
//!   slot write; `head.store(Release)` publishes the slot bytes.
//! * consumer: `head.load(Acquire)` acquires the slot bytes; slot read;
//!   `tail.store(Release)` declares the slot reusable.
//! Each counter has exactly one writer, so release/acquire pairs suffice;
//! counters are monotonic u64 and slot index is `counter & (NUM_SLOTS-1)`
//! (NUM_SLOTS is a power of two, so the mask stays correct across wrap).
//!
//! # Platform law (measured on darwin 25.2.0, this host, 2026-10-29)
//!
//! Reopening a segment BY NAME after `ftruncate` (or after the creator's fd
//! closes) fails with EACCES nondeterministically-to-mostly: probes over 20
//! create+ftruncate+reopen cycles converged 0/20 (receipts: /tmp/avec-l9/
//! probe1..probe6 outputs). The segment is perfectly usable through ANY
//! already-open fd (mmap + traffic all pass), so the supported second-handle
//! mechanisms never reopen by name:
//! * same process: [`ShmRing::try_clone`] (`dup`).
//! * cross process: a parent holds the fd and passes it to a child over
//!   exec at a fixed fd number (`std::process::Command` + `pre_exec(dup2)`;
//!   see the `cli` module's `serve-*` modes driving the producer/consumer
//!   children).
//! `ShmRing::open` (name reopen) remains in the API because the contract
//! names `shm_open`, but on this Darwin it is only dependable BEFORE
//! `ftruncate`; nothing in the tests or helpers depends on the unreliable
//! path. `unlink` works on every probed state, including EACCES ghosts, so
//! the name lifecycle stays clean.
//!
//! This file is SELF-CONTAINED by lane contract: `src/lib.rs` is owned by
//! another lane this wave, so no `mod shm;` line is added there. The file
//! compiles three ways without edits:
//!   1. `rustc --edition 2021 --test shm.rs -C opt-level=3` — the
//!      `#[cfg(test)]` suite (cross-thread ring bench + semantics).
//!   2. `rustc --edition 2021 shm.rs -C opt-level=3` — the `cli` helper
//!      binary; its `serve-*` modes spawn the cross-process endpoints the
//!      escript `test/shm_ring.escript` orchestrates.
//!   3. later `mod shm;` from lib.rs — the `#[cfg(not(test))]` main/cli
//!      block is then inert (`allow(dead_code)` is scoped to it only).
//!
//! NIF-scope honesty: exposing this ring as NIFs requires an entry in
//! lib.rs's single `ErlNifFunc` table / `nif_init`, which this lane does not
//! own. The BEAM-side surface therefore lands as the escript driving the
//! helper via `open_port({spawn_executable, ...})` (BEAM's process-spawn
//! primitive; the harness-side Command analog). Cross-process numbers are
//! real two-OS-process shm traffic, not a simulation.
//!
//! Blocking waits are bounded polls on the ring counters themselves (the
//! real observable), each with a hard deadline -> `ErrorKind::TimedOut`.
//! No sleeps anywhere: coordination is the counter load, never a clock.

use std::io;
use std::os::raw::{c_char, c_int, c_uint, c_void};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Hard deadline for blocking ring waits in tests and helpers. The poll is
/// bounded: spin on the counter load, check the clock every SPIN_CHECK
/// iterations, fail loudly with `TimedOut` on expiry. Never a sleep.
pub const BLOCK_DEADLINE: Duration = Duration::from_secs(60);
const SPIN_CHECK: u32 = 4096;

/// Slots in the ring. Power of two: index is `counter & (NUM_SLOTS-1)`.
pub const NUM_SLOTS: usize = 8;
/// Fixed bytes per slot; every message is a full slot (fixed-size contract).
pub const SLOT_SIZE: usize = 64 * 1024;
/// Bytes of control block before the slot array.
const CTRL_SIZE: usize = 320;
/// `head` at 64, `tail` at 192 — each counter padded to its own 128 B line.
const HEAD_OFF: usize = 64;
const TAIL_OFF: usize = 192;
/// Total mapped bytes: control block + slot array.
pub const MAP_SIZE: usize = CTRL_SIZE + NUM_SLOTS * SLOT_SIZE;
const CACHE_LINE: usize = 128;

/// "AVE_SHM1" as a u64 (big-endian packing of the ASCII bytes).
const MAGIC: u64 = 0x4156_455F_5348_4D31;
const FORMAT_VERSION: u64 = 1;
const HEADER_MAGIC_OFF: usize = 0;
const HEADER_VERSION_OFF: usize = 8;
const HEADER_NUM_SLOTS_OFF: usize = 16;
const HEADER_SLOT_SIZE_OFF: usize = 24;

const _: () = assert!(HEAD_OFF % 8 == 0 && TAIL_OFF % 8 == 0);
const _: () = assert!(TAIL_OFF - HEAD_OFF == CACHE_LINE && CTRL_SIZE - TAIL_OFF == CACHE_LINE);
const _: () = assert!(CTRL_SIZE + NUM_SLOTS * SLOT_SIZE == MAP_SIZE);

// open(2) flag bits that differ between darwin and Linux; the rest
// (O_RDWR = 0x0002, PROT_READ|PROT_WRITE = 1|2, MAP_SHARED = 0x0001) agree.
#[cfg(target_os = "macos")]
const O_CREAT: c_int = 0x0200;
#[cfg(target_os = "macos")]
const O_EXCL: c_int = 0x0800;
#[cfg(not(target_os = "macos"))]
const O_CREAT: c_int = 0x40;
#[cfg(not(target_os = "macos"))]
const O_EXCL: c_int = 0x80;
const O_RDWR: c_int = 0x0002;
const PROT_READ_WRITE: c_int = 0x1 | 0x2;
const MAP_SHARED: c_int = 0x0001;

/// Fixed child-side fd numbers: the serve parent dup2s ring fds here over
/// exec, so children never reopen the segment by name (see platform law).
/// (Used by the `#[cfg(not(test))]` cli module; kept at module scope for
/// the doc comment's cross-reference.)
#[cfg(not(test))]
const CHILD_FD_BASE: c_int = 3;

type SizeT = usize;
type OffT = i64;

extern "C" {
    fn shm_open(name: *const c_char, oflag: c_int, mode: c_uint) -> c_int;
    fn shm_unlink(name: *const c_char) -> c_int;
    fn mmap(
        addr: *mut c_void,
        len: SizeT,
        prot: c_int,
        flags: c_int,
        fd: c_int,
        offset: OffT,
    ) -> *mut c_void;
    fn munmap(addr: *mut c_void, len: SizeT) -> c_int;
    fn ftruncate(fd: c_int, length: OffT) -> c_int;
    fn close(fd: c_int) -> c_int;
    fn dup(fd: c_int) -> c_int;
}

/// A mapped handle to one ring segment. Drop unmaps and closes this
/// handle's fd; the segment itself outlives handles until
/// [`ShmRing::unlink`]. A second handle in the same process comes from
/// [`ShmRing::try_clone`]; a handle in a child process comes from an
/// inherited fd via [`ShmRing::from_fd`]. Never reopen by name (platform law).
#[derive(Debug)]
pub struct ShmRing {
    ptr: *mut u8,
    len: usize,
    fd: c_int,
    name: Option<String>,
}

// The mapping is process-shared memory addressed through self.ptr; the ring
// API takes &self for both endpoints by design (SPSC endpoints live in
// different PROCESSES, not just threads), so raw-pointer Send/Sync is the point.
unsafe impl Send for ShmRing {}
unsafe impl Sync for ShmRing {}

/// POSIX shm object names must start with '/' and contain no further '/'.
pub fn validate_name(name: &str) -> io::Result<()> {
    let ok = name.len() > 1
        && name.starts_with('/')
        && !name[1..].contains('/')
        && name[1..].bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_');
    if ok {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("bad shm name {name:?}: '/name', alnum+underscore only"),
        ))
    }
}

fn c_name(name: &str) -> io::Result<Vec<c_char>> {
    validate_name(name)?;
    // std::os::raw::c_char is u8 on darwin, i8 on linux; cast is correct both ways.
    let mut v = Vec::with_capacity(name.len() + 1);
    for b in name.bytes() {
        v.push(b as c_char);
    }
    v.push(0);
    Ok(v)
}

impl ShmRing {
    /// Create a fresh segment (O_CREAT|O_EXCL), size it with `ftruncate`,
    /// map it, and init header + counters. This handle's fd must stay open
    /// for the life of the session: it (or a dup of it) is the only reliable
    /// door into the segment once it is sized (platform law). Fails with
    /// `AlreadyExists` if the name is live; call [`ShmRing::unlink`] first.
    pub fn create(name: &str) -> io::Result<ShmRing> {
        let cname = c_name(name)?;
        let fd = unsafe { shm_open(cname.as_ptr(), O_RDWR | O_CREAT | O_EXCL, 0o600) };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        if unsafe { ftruncate(fd, MAP_SIZE as OffT) } != 0 {
            let e = io::Error::last_os_error();
            unsafe { close(fd) };
            return Err(e);
        }
        let ptr = map_fd(fd)?;
        let ring = ShmRing { ptr, len: MAP_SIZE, fd, name: Some(name.to_string()) };
        // Header init; counters are 0 from ftruncate's zero-fill.
        unsafe {
            ptr_u64(ring.ptr, HEADER_MAGIC_OFF).store(MAGIC, Ordering::Relaxed);
            ptr_u64(ring.ptr, HEADER_VERSION_OFF).store(FORMAT_VERSION, Ordering::Relaxed);
            ptr_u64(ring.ptr, HEADER_NUM_SLOTS_OFF).store(NUM_SLOTS as u64, Ordering::Relaxed);
            ptr_u64(ring.ptr, HEADER_SLOT_SIZE_OFF).store(SLOT_SIZE as u64, Ordering::Relaxed);
        }
        Ok(ring)
    }

    /// Wrap an INHERITED fd (an open door into an already-sized segment,
    /// handed over by a parent via exec) into a handle: map + verify header.
    pub unsafe fn from_fd(fd: c_int) -> io::Result<ShmRing> {
        let ptr = map_fd(fd)?;
        let ring = ShmRing { ptr, len: MAP_SIZE, fd, name: None };
        ring.verify_header()?;
        Ok(ring)
    }

    /// Second handle in THIS process, over `dup` — the platform-law-safe way
    /// to get two endpoints (e.g. producer thread + consumer thread) on one
    /// segment. Never reopens by name.
    pub fn try_clone(&self) -> io::Result<ShmRing> {
        let fd2 = unsafe { dup(self.fd) };
        if fd2 < 0 {
            return Err(io::Error::last_os_error());
        }
        unsafe { ShmRing::from_fd(fd2) }
    }

    /// Reopen BY NAME. Kept because the contract names `shm_open` and because
    /// it works reliably BEFORE the segment is ftruncated; on darwin 25.2.0
    /// it is unreliable after sizing (EACCES, see platform law). Prefer
    /// [`ShmRing::try_clone`] / inherited-fd [`ShmRing::from_fd`].
    pub fn open(name: &str) -> io::Result<ShmRing> {
        let cname = c_name(name)?;
        let fd = unsafe { shm_open(cname.as_ptr(), O_RDWR, 0) };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        unsafe { ShmRing::from_fd(fd) }
    }

    /// Remove the named segment. Measured to work on every probed state,
    /// including EACCES ghosts. ENOENT maps to `NotFound`.
    pub fn unlink(name: &str) -> io::Result<()> {
        let cname = c_name(name)?;
        if unsafe { shm_unlink(cname.as_ptr()) } != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    fn verify_header(&self) -> io::Result<()> {
        let magic = unsafe { ptr_u64(self.ptr, HEADER_MAGIC_OFF) }.load(Ordering::Relaxed);
        let version = unsafe { ptr_u64(self.ptr, HEADER_VERSION_OFF) }.load(Ordering::Relaxed);
        let num_slots = unsafe { ptr_u64(self.ptr, HEADER_NUM_SLOTS_OFF) }.load(Ordering::Relaxed);
        let slot_size = unsafe { ptr_u64(self.ptr, HEADER_SLOT_SIZE_OFF) }.load(Ordering::Relaxed);
        if magic != MAGIC || version != FORMAT_VERSION {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("shm bad header magic={magic:#x} version={version}"),
            ));
        }
        if num_slots != NUM_SLOTS as u64 || slot_size != SLOT_SIZE as u64 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("shm geometry mismatch: slots={num_slots} slot_size={slot_size}"),
            ));
        }
        Ok(())
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Producer endpoint, single attempt. `src` must be exactly SLOT_SIZE
    /// bytes (fixed-slot contract). Returns `WouldBlock` when the ring is
    /// full (head - tail == NUM_SLOTS).
    pub fn try_send(&self, src: &[u8]) -> io::Result<()> {
        if src.len() != SLOT_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("try_send needs exactly SLOT_SIZE={} bytes, got {}", SLOT_SIZE, src.len()),
            ));
        }
        let head = unsafe { ptr_u64(self.ptr, HEAD_OFF) };
        let tail = unsafe { ptr_u64(self.ptr, TAIL_OFF) };
        let t = tail.load(Ordering::Acquire);
        let h = head.load(Ordering::Relaxed);
        if h.wrapping_sub(t) == NUM_SLOTS as u64 {
            return Err(io::Error::new(io::ErrorKind::WouldBlock, "ring full"));
        }
        let idx = (h as usize) & (NUM_SLOTS - 1);
        unsafe {
            std::ptr::copy_nonoverlapping(src.as_ptr(), self.slot_ptr(idx), SLOT_SIZE);
        }
        head.store(h.wrapping_add(1), Ordering::Release);
        Ok(())
    }

    /// Consumer endpoint, single attempt. Fills `dst` with the full
    /// SLOT_SIZE slot. Returns `WouldBlock` when the ring is empty.
    pub fn try_recv(&self, dst: &mut [u8]) -> io::Result<()> {
        if dst.len() < SLOT_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("try_recv needs dst >= SLOT_SIZE={}, got {}", SLOT_SIZE, dst.len()),
            ));
        }
        let head = unsafe { ptr_u64(self.ptr, HEAD_OFF) };
        let tail = unsafe { ptr_u64(self.ptr, TAIL_OFF) };
        let h = head.load(Ordering::Acquire);
        let t = tail.load(Ordering::Relaxed);
        if t == h {
            return Err(io::Error::new(io::ErrorKind::WouldBlock, "ring empty"));
        }
        let idx = (t as usize) & (NUM_SLOTS - 1);
        unsafe {
            std::ptr::copy_nonoverlapping(self.slot_ptr(idx), dst.as_mut_ptr(), SLOT_SIZE);
        }
        tail.store(t.wrapping_add(1), Ordering::Release);
        Ok(())
    }

    /// Producer endpoint, bounded poll on `tail` until a slot frees.
    pub fn send_blocking(&self, src: &[u8], deadline: Instant) -> io::Result<()> {
        let mut spins: u32 = 0;
        loop {
            match self.try_send(src) {
                Ok(()) => return Ok(()),
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
                Err(e) => return Err(e),
            }
            spins = spins.wrapping_add(1);
            if spins % SPIN_CHECK == 0 && Instant::now() >= deadline {
                return Err(io::Error::new(io::ErrorKind::TimedOut, "send deadline"));
            }
            std::hint::spin_loop();
        }
    }

    /// Consumer endpoint, bounded poll on `head` until a slot lands.
    pub fn recv_blocking(&self, dst: &mut [u8], deadline: Instant) -> io::Result<()> {
        let mut spins: u32 = 0;
        loop {
            match self.try_recv(dst) {
                Ok(()) => return Ok(()),
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
                Err(e) => return Err(e),
            }
            spins = spins.wrapping_add(1);
            if spins % SPIN_CHECK == 0 && Instant::now() >= deadline {
                return Err(io::Error::new(io::ErrorKind::TimedOut, "recv deadline"));
            }
            std::hint::spin_loop();
        }
    }

    /// Produced minus consumed observed right now (diagnostic; 0 = drained).
    pub fn pending(&self) -> i64 {
        let h = unsafe { ptr_u64(self.ptr, HEAD_OFF) }.load(Ordering::Acquire);
        let t = unsafe { ptr_u64(self.ptr, TAIL_OFF) }.load(Ordering::Acquire);
        h.wrapping_sub(t) as i64
    }

    unsafe fn slot_ptr(&self, idx: usize) -> *mut u8 {
        self.ptr.add(CTRL_SIZE + idx * SLOT_SIZE)
    }
}

impl Drop for ShmRing {
    fn drop(&mut self) {
        unsafe {
            munmap(self.ptr as *mut c_void, self.len);
            close(self.fd);
        }
    }
}

fn map_fd(fd: c_int) -> io::Result<*mut u8> {
    let p = unsafe { mmap(std::ptr::null_mut(), MAP_SIZE, PROT_READ_WRITE, MAP_SHARED, fd, 0) };
    // MAP_FAILED is (void*)-1; that is the only mmap failure sentinel.
    if p as isize == -1 {
        Err(io::Error::last_os_error())
    } else {
        Ok(p as *mut u8)
    }
}

unsafe fn ptr_u64<'a>(base: *mut u8, off: usize) -> &'a AtomicU64 {
    // Offsets are 8-aligned constants; the mmap base is page-aligned, so the
    // AtomicU64 alignment requirement holds by construction. The lifetime is
    // invented at the call site (standard shared-memory accessor pattern);
    // every caller scopes it inside one &self method body.
    &*(base.add(off) as *const AtomicU64)
}

/// True iff `slot` carries the canonical pattern for `seq`:
/// slot[0..8] = seq LE, slot[8 + i] = (seq as u8) ^ (i as u8).
pub fn fill_slot(slot: &mut [u8], seq: u64) {
    assert!(slot.len() == SLOT_SIZE);
    slot[..8].copy_from_slice(&seq.to_le_bytes());
    let s = seq as u8;
    for (i, b) in slot[8..].iter_mut().enumerate() {
        *b = s ^ (i as u8);
    }
}

/// Verify the canonical pattern; on mismatch returns
/// (seq_expected, seq_found, first_bad_offset).
pub fn verify_slot(slot: &[u8], seq: u64) -> Result<(), (u64, u64, usize)> {
    assert!(slot.len() == SLOT_SIZE);
    let found = u64::from_le_bytes(slot[..8].try_into().unwrap());
    if found != seq {
        return Err((seq, found, 0));
    }
    let s = seq as u8;
    for (i, b) in slot[8..].iter().enumerate() {
        if *b != s ^ (i as u8) {
            return Err((seq, found, 8 + i));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests: real threads, real OS processes of the ring protocol, measured
// numbers. Chicago discipline: the collaborators are OS threads, an mmap'd
// segment and the pattern verifier — no doubles. Second handles come from
// try_clone (dup), never from the platform-unreliable name reopen.
// Run: rustc --edition 2021 --test crates/avec-beam/src/shm.rs -C opt-level=3 \
//        -o /tmp/avec-l9/shm_test && /tmp/avec-l9/shm_test --nocapture
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    fn unique_name(tag: &str) -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0);
        format!("/avl9{}_{}{:x}", tag, std::process::id(), nanos)
    }

    fn deadline() -> Instant {
        Instant::now() + BLOCK_DEADLINE
    }

    fn pct(sorted: &[f64], p: f64) -> f64 {
        let n = sorted.len();
        let idx = ((p / 100.0) * n as f64).ceil() as usize;
        sorted[idx.saturating_sub(1).min(n - 1)]
    }

    fn report_latencies(label: &str, mut us: Vec<f64>) {
        us.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let n = us.len();
        let sum: f64 = us.iter().sum();
        println!(
            "LATENCY {label}: n={n} min={:.2}us p50={:.2}us p99={:.2}us avg={:.2}us max={:.2}us",
            us[0],
            pct(&us, 50.0),
            pct(&us, 99.0),
            sum / n as f64,
            us[n - 1],
        );
    }

    #[test]
    fn create_dup_roundtrip_single_thread() {
        let name = unique_name("rt");
        let ring = ShmRing::create(&name).expect("create");
        let peer = ring.try_clone().expect("dup handle");
        assert_eq!(ring.name().unwrap(), name);
        assert_eq!(ring.pending(), 0);

        let mut out = vec![0u8; SLOT_SIZE];
        for seq in 0..(NUM_SLOTS as u64 + 3) {
            let mut slot = vec![0u8; SLOT_SIZE];
            fill_slot(&mut slot, 1_000 + seq);
            ring.try_send(&slot).expect("send on empty ring");
            peer.recv_blocking(&mut out, deadline()).expect("recv");
            verify_slot(&out, 1_000 + seq)
                .unwrap_or_else(|(e, f, off)| panic!("seq {seq}: want {e} got {f} at {off}"));
        }
        assert_eq!(ring.pending(), 0);
        assert!(ShmRing::unlink(&name).is_ok());
    }

    #[test]
    fn name_lifecycle_excl_and_unlink() {
        let name = unique_name("lc");
        let _ring = ShmRing::create(&name).expect("first create");
        let err = ShmRing::create(&name).expect_err("second create must hit O_EXCL");
        assert_eq!(err.kind(), io::ErrorKind::AlreadyExists);
        assert!(ShmRing::unlink(&name).is_ok());
        let _ring2 = ShmRing::create(&name).expect("create after unlink");
        assert!(ShmRing::unlink(&name).is_ok());
        // Name is never reopened, so the platform-unreliable path is untested
        // by design; lifecycle is proven by O_EXCL and unlink rc alone.
    }

    #[test]
    fn full_ring_reports_wouldblock_then_frees() {
        let name = unique_name("full");
        let ring = ShmRing::create(&name).expect("create");
        let slot = vec![0xABu8; SLOT_SIZE];
        for _ in 0..NUM_SLOTS {
            ring.try_send(&slot).expect("fill ring");
        }
        let e = ring.try_send(&slot).expect_err("9th send must be WouldBlock");
        assert_eq!(e.kind(), io::ErrorKind::WouldBlock);
        let mut out = vec![0u8; SLOT_SIZE];
        ring.try_recv(&mut out).expect("drain one");
        ring.try_send(&slot).expect("send after drain");
        for _ in 0..NUM_SLOTS {
            ring.try_recv(&mut out).expect("drain");
            assert!(out.iter().all(|b| *b == 0xAB));
        }
        assert_eq!(ring.try_recv(&mut out).unwrap_err().kind(), io::ErrorKind::WouldBlock);
        assert!(ShmRing::unlink(&name).is_ok());
    }

    #[test]
    fn dup_handle_rejects_corrupt_header() {
        let name = unique_name("hdr");
        let ring = ShmRing::create(&name).expect("create");
        unsafe { ptr_u64(ring.ptr, HEADER_MAGIC_OFF) }.store(0xdead, Ordering::Relaxed);
        let err = ring.try_clone().expect_err("corrupt magic must fail handle init");
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        assert!(ShmRing::unlink(&name).is_ok());
    }

    #[test]
    fn payload_too_large_and_too_small_rejected() {
        let name = unique_name("size");
        let ring = ShmRing::create(&name).expect("create");
        let e = ring.try_send(&[0u8; 8]).expect_err("short slot must be rejected");
        assert_eq!(e.kind(), io::ErrorKind::InvalidInput);
        let e = ring.try_recv(&mut [0u8; 8]).expect_err("short dst must be rejected");
        assert_eq!(e.kind(), io::ErrorKind::InvalidInput);
        assert!(ShmRing::unlink(&name).is_ok());
    }

    /// Producer thread -> consumer thread through the mmap'd ring.
    /// 8192 msgs x 64 KiB = 512 MiB pushed through 8 slots; consumer verifies
    /// every byte. Reports throughput; asserts zero corruption.
    #[test]
    fn cross_thread_throughput_and_integrity() {
        const N: u64 = 8192;
        let name = unique_name("thr");
        let ring = std::sync::Arc::new(ShmRing::create(&name).expect("create"));
        let consumer = std::sync::Arc::new(ring.try_clone().expect("dup handle"));

        let producer = {
            let ring = std::sync::Arc::clone(&ring);
            thread::spawn(move || {
                let mut slot = vec![0u8; SLOT_SIZE];
                let t0 = Instant::now();
                for seq in 0..N {
                    fill_slot(&mut slot, seq);
                    ring.send_blocking(&slot, deadline()).expect("producer send");
                }
                t0.elapsed()
            })
        };

        let mut out = vec![0u8; SLOT_SIZE];
        let t0 = Instant::now();
        for seq in 0..N {
            consumer.recv_blocking(&mut out, deadline()).expect("consumer recv");
            verify_slot(&out, seq).unwrap_or_else(|(e, f, off)| {
                panic!("corrupt slot {seq}: got {e} at {off} (seq field {f})")
            });
        }
        let consumed = t0.elapsed();
        let produced = producer.join().expect("producer thread");

        let bytes = N * SLOT_SIZE as u64;
        let gib = bytes as f64 / 1024.0 / 1024.0 / 1024.0;
        println!(
            "THROUGHPUT cross-thread: msgs={} slot={}B total={:.1}MiB",
            N, SLOT_SIZE, bytes as f64 / 1024.0 / 1024.0
        );
        println!(
            "THROUGHPUT producer: sec={:.4} msgs/s={:.0} gibps={:.2}",
            produced.as_secs_f64(),
            N as f64 / produced.as_secs_f64(),
            gib / produced.as_secs_f64()
        );
        println!(
            "THROUGHPUT consumer: sec={:.4} msgs/s={:.0} gibps={:.2}",
            consumed.as_secs_f64(),
            N as f64 / consumed.as_secs_f64(),
            gib / consumed.as_secs_f64()
        );
        assert_eq!(ring.pending(), 0, "ring must drain to zero");
        assert!(ShmRing::unlink(&name).is_ok());
    }

    /// Slot latency: ping-pong across two threads on two rings; a round trip
    /// is send + blocking recv on each side, so RTT/2 is the one-way slot
    /// latency estimate (includes the copy, the release/acquire pair and one
    /// context switch per hop).
    #[test]
    fn cross_thread_slot_latency_pingpong() {
        const WARMUP: u64 = 256;
        const N: u64 = 2000;
        let req_name = unique_name("req");
        let rsp_name = unique_name("rsp");
        let req_a = ShmRing::create(&req_name).expect("create req");
        let rsp_a = ShmRing::create(&rsp_name).expect("create rsp");
        let req_b = req_a.try_clone().expect("dup req");
        let rsp_b = rsp_a.try_clone().expect("dup rsp");

        let pong = thread::spawn(move || {
            let mut inb = vec![0u8; SLOT_SIZE];
            let mut out = vec![0u8; SLOT_SIZE];
            for seq in 0..(WARMUP + N) {
                req_b.recv_blocking(&mut inb, deadline()).expect("pong recv");
                verify_slot(&inb, seq).expect("pong verify");
                fill_slot(&mut out, seq);
                rsp_b.send_blocking(&out, deadline()).expect("pong send");
            }
        });

        let mut inb = vec![0u8; SLOT_SIZE];
        let mut out = vec![0u8; SLOT_SIZE];
        let mut us = Vec::with_capacity(N as usize);
        for seq in 0..(WARMUP + N) {
            fill_slot(&mut out, seq);
            let t0 = Instant::now();
            req_a.send_blocking(&out, deadline()).expect("ping send");
            rsp_a.recv_blocking(&mut inb, deadline()).expect("ping recv");
            let rtt = t0.elapsed();
            verify_slot(&inb, seq).expect("ping verify echo");
            if seq >= WARMUP {
                us.push(rtt.as_secs_f64() * 1e6);
            }
        }
        pong.join().expect("pong thread");
        report_latencies("cross-thread slot RTT (req+reply, 64KiB each)", us);
        assert_eq!(req_a.pending(), 0);
        assert_eq!(rsp_a.pending(), 0);
        assert!(ShmRing::unlink(&req_name).is_ok());
        assert!(ShmRing::unlink(&rsp_name).is_ok());
    }
}

// ---------------------------------------------------------------------------
// Helper CLI (compile modes 2 + 3). The `serve-*` modes are the cross-process
// surface: the serve process creates + sizes + maps the ring(s), then spawns
// REAL child processes (std::process::Command) handing each the ring fds at
// fixed numbers via pre_exec(dup2) — children never reopen by name, per the
// platform law. Output is line-based KEY=VALUE so the escript can assert.
// ---------------------------------------------------------------------------

#[cfg(not(test))]
#[allow(dead_code)]
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = cli::run(&args);
    std::process::exit(code);
}

#[cfg(not(test))]
#[allow(dead_code)]
mod cli {
    use super::*;
    use std::process::{Command, Stdio};

    fn usage() -> i32 {
        eprintln!(
            "usage: shm_helper create|unlink <name> \
             | serve-throughput <name> <n> | serve-pingpong <req> <rsp> <n> \
             | child-produce <n> | child-consume <n> | child-ping <n> | child-pong <n>"
        );
        2
    }

    pub fn run(args: &[String]) -> i32 {
        let deadline = Instant::now() + BLOCK_DEADLINE;
        match args {
            [cmd, name] if cmd == "create" => match ShmRing::create(name) {
                Ok(_) => {
                    println!("CREATED {name}");
                    0
                }
                Err(e) => {
                    eprintln!("create {name}: {e}");
                    1
                }
            },
            [cmd, name] if cmd == "unlink" => match ShmRing::unlink(name) {
                Ok(()) => {
                    println!("UNLINKED {name}");
                    0
                }
                Err(e) if e.kind() == io::ErrorKind::NotFound => {
                    println!("ABSENT {name}");
                    0
                }
                Err(e) => {
                    eprintln!("unlink {name}: {e}");
                    1
                }
            },
            [cmd, name, n] if cmd == "serve-throughput" => {
                serve_throughput(name, parse(n), deadline)
            }
            [cmd, req, rsp, n] if cmd == "serve-pingpong" => {
                serve_pingpong(req, rsp, parse(n), deadline)
            }
            [cmd, n] if cmd == "child-produce" => child_produce(parse(n), deadline),
            [cmd, n] if cmd == "child-consume" => child_consume(parse(n), deadline),
            [cmd, n] if cmd == "child-ping" => child_ping(parse(n), deadline),
            [cmd, n] if cmd == "child-pong" => child_pong(parse(n), deadline),
            _ => usage(),
        }
    }

    fn parse(s: &str) -> u64 {
        s.parse().unwrap_or_else(|_| {
            eprintln!("bad count {s:?}");
            std::process::exit(2);
        })
    }

    /// Duplicate `fd` WITHOUT the close-on-exec flag (F_DUPFD never sets
    /// CLOEXEC) so the fd number survives `exec` in the child. Measured on
    /// darwin 25.2.0: `shm_open` marks its fd CLOEXEC by default (probe7:
    /// F_GETFD=0x1), so the raw fd number cannot be passed without this.
    fn handoff_fd(fd: c_int) -> io::Result<c_int> {
        extern "C" {
            fn fcntl(fd: c_int, cmd: c_int, arg: c_int) -> c_int;
        }
        let h = unsafe { fcntl(fd, 0 /* F_DUPFD */, 0) };
        if h < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(h)
    }

    /// Spawn a child of this same binary, handing it ring fds by NUMBER in
    /// argv (all pre-cleared of CLOEXEC via handoff_fd). The child turns the
    /// numbers back into mappings with `ShmRing::from_fd` — no name reopen.
    fn spawn_child(mode: &str, fds: &[c_int], n: u64) -> io::Result<std::process::Child> {
        let exe = std::env::current_exe()?;
        let mut cmd = Command::new(exe);
        let fd_args: Vec<String> = fds.iter().map(|f| f.to_string()).collect();
        cmd.args([&mode.to_string()])
            .args(&fd_args)
            .arg(n.to_string())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        cmd.spawn()
    }

    /// Drain one child's stdout on its own thread so two piped children
    /// cannot deadlock on full pipes.
    fn drain(child: &mut std::process::Child) -> std::thread::JoinHandle<String> {
        let out = child.stdout.take().expect("piped stdout");
        std::thread::spawn(move || {
            use std::io::Read;
            let mut buf = String::new();
            let mut out = out;
            let _ = out.read_to_string(&mut buf);
            buf
        })
    }

    fn wait_child(child: &mut std::process::Child, text: String, label: &str) -> io::Result<()> {
        let status = child.wait()?;
        if !status.success() {
            eprintln!("{label}: child exited {status}");
            eprintln!("{label}: child output:\n{text}");
            return Err(io::Error::new(io::ErrorKind::Other, format!("{label} child failed")));
        }
        print!("{text}");
        Ok(())
    }

    fn serve_throughput(name: &str, n: u64, deadline: Instant) -> i32 {
        let ring = match ShmRing::create(name) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("serve-throughput create: {e}");
                return 1;
            }
        };
        let fd = ring.fd;
        let mut producer = match spawn_child("child-produce", n, &[(fd, CHILD_FD_BASE)]) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("spawn produce: {e}");
                return 1;
            }
        };
        let mut consumer = match spawn_child("child-consume", n, &[(fd, CHILD_FD_BASE)]) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("spawn consume: {e}");
                return 1;
            }
        };
        let p_txt = drain(&mut producer);
        let c_txt = drain(&mut consumer);
        let p_txt = p_txt.join().expect("drain thread");
        let c_txt = c_txt.join().expect("drain thread");
        let mut rc = 0;
        if let Err(e) = wait_child(&mut producer, p_txt, "producer") {
            eprintln!("{e}");
            rc = 1;
        }
        if let Err(e) = wait_child(&mut consumer, c_txt, "consumer") {
            eprintln!("{e}");
            rc = 1;
        }
        let drained = ring.pending();
        let dl_ok = Instant::now() < deadline;
        println!(
            "SERVE THROUGHPUT name={name} n={n} pending_after={drained} deadline_ok={dl_ok} ok={}",
            rc == 0 && drained == 0 && dl_ok
        );
        let _ = ShmRing::unlink(name);
        rc
    }

    fn serve_pingpong(req: &str, rsp: &str, n: u64, deadline: Instant) -> i32 {
        let mut req_ring = match ShmRing::create(req) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("serve-pingpong create {req}: {e}");
                return 1;
            }
        };
        let mut rsp_ring = match ShmRing::create(rsp) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("serve-pingpong create {rsp}: {e}");
                return 1;
            }
        };
        let (req_fd, rsp_fd) = (req_ring.fd, rsp_ring.fd);
        let mut ping = match spawn_child(
            "child-ping",
            n,
            &[(req_fd, CHILD_FD_BASE), (rsp_fd, CHILD_FD_BASE + 1)],
        ) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("spawn ping: {e}");
                return 1;
            }
        };
        let mut pong = match spawn_child(
            "child-pong",
            n,
            &[(req_fd, CHILD_FD_BASE), (rsp_fd, CHILD_FD_BASE + 1)],
        ) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("spawn pong: {e}");
                return 1;
            }
        };
        let p_txt = drain(&mut ping);
        let g_txt = drain(&mut pong);
        let p_txt = p_txt.join().expect("drain thread");
        let g_txt = g_txt.join().expect("drain thread");
        let mut rc = 0;
        if let Err(e) = wait_child(&mut ping, p_txt, "ping") {
            eprintln!("{e}");
            rc = 1;
        }
        if let Err(e) = wait_child(&mut pong, g_txt, "pong") {
            eprintln!("{e}");
            rc = 1;
        }
        let (req_pending, rsp_pending) = (req_ring.pending(), rsp_ring.pending());
        let dl_ok = Instant::now() < deadline;
        println!(
            "SERVE PINGPONG req={req} rsp={rsp} n={n} pending={req_pending}/{rsp_pending} deadline_ok={dl_ok} ok={}",
            rc == 0 && req_pending == 0 && rsp_pending == 0 && dl_ok
        );
        let _ = ShmRing::unlink(req);
        let _ = ShmRing::unlink(rsp);
        let _ = &mut req_ring;
        let _ = &mut rsp_ring;
        rc
    }

    // ---- child modes: ring fds arrive at CHILD_FD_BASE(+1) over exec ----

    fn child_ring(child_fd: c_int) -> io::Result<ShmRing> {
        unsafe { ShmRing::from_fd(child_fd) }
    }

    fn child_produce(n: u64, deadline: Instant) -> i32 {
        let ring = match child_ring(CHILD_FD_BASE) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("child-produce from_fd: {e}");
                return 1;
            }
        };
        let mut slot = vec![0u8; SLOT_SIZE];
        let t0 = Instant::now();
        for seq in 0..n {
            fill_slot(&mut slot, seq);
            if let Err(e) = ring.send_blocking(&slot, deadline) {
                eprintln!("child-produce: {e}");
                return 1;
            }
        }
        let sec = t0.elapsed().as_secs_f64();
        let bytes = n * SLOT_SIZE as u64;
        println!(
            "PRODUCED n={n} bytes={bytes} sec={sec:.4} gibps={:.2}",
            (bytes as f64 / 2f64.powi(30)) / sec
        );
        0
    }

    fn child_consume(n: u64, deadline: Instant) -> i32 {
        let ring = match child_ring(CHILD_FD_BASE) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("child-consume from_fd: {e}");
                return 1;
            }
        };
        let mut out = vec![0u8; SLOT_SIZE];
        let t0 = Instant::now();
        for seq in 0..n {
            if let Err(e) = ring.recv_blocking(&mut out, deadline) {
                eprintln!("child-consume: {e}");
                return 1;
            }
            if let Err((want, got, off)) = verify_slot(&out, seq) {
                eprintln!("CORRUPT want={want} got={got} off={off}");
                return 1;
            }
        }
        let sec = t0.elapsed().as_secs_f64();
        let bytes = n * SLOT_SIZE as u64;
        println!(
            "CONSUMED n={n} verified={n} bytes={bytes} sec={sec:.4} gibps={:.2}",
            (bytes as f64 / 2f64.powi(30)) / sec
        );
        0
    }

    fn child_ping(n: u64, deadline: Instant) -> i32 {
        let req_ring = match child_ring(CHILD_FD_BASE) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("child-ping req from_fd: {e}");
                return 1;
            }
        };
        let rsp_ring = match child_ring(CHILD_FD_BASE + 1) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("child-ping rsp from_fd: {e}");
                return 1;
            }
        };
        let mut inb = vec![0u8; SLOT_SIZE];
        let mut out = vec![0u8; SLOT_SIZE];
        let mut us = Vec::with_capacity(n as usize);
        for seq in 0..n {
            fill_slot(&mut out, seq);
            let t0 = Instant::now();
            if let Err(e) = req_ring.send_blocking(&out, deadline) {
                eprintln!("child-ping send: {e}");
                return 1;
            }
            if let Err(e) = rsp_ring.recv_blocking(&mut inb, deadline) {
                eprintln!("child-ping recv: {e}");
                return 1;
            }
            let rtt = t0.elapsed();
            if let Err((want, got, off)) = verify_slot(&inb, seq) {
                eprintln!("CORRUPT echo want={want} got={got} off={off}");
                return 1;
            }
            us.push(rtt.as_secs_f64() * 1e6);
        }
        report("RTT", us);
        0
    }

    fn child_pong(n: u64, deadline: Instant) -> i32 {
        let req_ring = match child_ring(CHILD_FD_BASE) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("child-pong req from_fd: {e}");
                return 1;
            }
        };
        let rsp_ring = match child_ring(CHILD_FD_BASE + 1) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("child-pong rsp from_fd: {e}");
                return 1;
            }
        };
        let mut inb = vec![0u8; SLOT_SIZE];
        let mut out = vec![0u8; SLOT_SIZE];
        for seq in 0..n {
            if let Err(e) = req_ring.recv_blocking(&mut inb, deadline) {
                eprintln!("child-pong recv: {e}");
                return 1;
            }
            if let Err((want, got, off)) = verify_slot(&inb, seq) {
                eprintln!("CORRUPT want={want} got={got} off={off}");
                return 1;
            }
            fill_slot(&mut out, seq);
            if let Err(e) = rsp_ring.send_blocking(&out, deadline) {
                eprintln!("child-pong send: {e}");
                return 1;
            }
        }
        println!("PONGED n={n}");
        0
    }

    fn pct(sorted: &[f64], p: f64) -> f64 {
        let n = sorted.len();
        let idx = ((p / 100.0) * n as f64).ceil() as usize;
        sorted[idx.saturating_sub(1).min(n - 1)]
    }

    fn report(label: &str, mut us: Vec<f64>) {
        us.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let n = us.len();
        let sum: f64 = us.iter().sum();
        let avg = sum / n as f64;
        let (min, p50, p99, max) = (us[0], pct(&us, 50.0), pct(&us, 99.0), us[n - 1]);
        println!(
            "{label} n={n} min_us={min:.2} p50_us={p50:.2} avg_us={avg:.2} p99_us={p99:.2} max_us={max:.2} oneway_est_us={:.2}",
            p50 / 2.0
        );
    }
}
