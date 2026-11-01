//! Shared real collaborators for the A05 courts (lane doc §7: "Real
//! collaborators throughout"). `FileMedium` is the hosted backend: a real
//! file, real byte-granular pwrite syscalls, real `sync_all` durability
//! fence — Chicago discipline, no mocks anywhere in this lane.

#![forbid(unsafe_code)]
// Each court binary links the whole shared module; per-binary dead-code
// warnings for helpers another court uses are noise, not signal here.
#![allow(dead_code)]

use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use cgap_storage::atomic_commit::*;

static UNIQUE: AtomicU64 = AtomicU64::new(0);

fn unique_dir() -> PathBuf {
    let n = UNIQUE.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "cgap-storage-court-{}-{n}",
        std::process::id()
    ))
}

/// The immutable superblock root every court starts from: dag_seq 9, so
/// committed records at dag 10/11 satisfy the A03 monotonic contract and any
/// valid record with dag_seq <= 9 is typed `SeqRegression`.
pub fn base_root() -> NewRootDescriptor {
    NewRootDescriptor::new([0xAA; 20], 0x1000, 9, 0, FLAG_DUAL_WRITE)
}

/// Prior-record payload (seq 1): dag_seq 10 > base dag_seq 9.
pub fn prior_payload() -> NewRootDescriptor {
    NewRootDescriptor::new([0xBB; 20], 0x2000, 10, 0xAAAA, FLAG_DUAL_WRITE)
}

/// New-record payload (seq 2): dag_seq 11.
pub fn new_payload() -> NewRootDescriptor {
    NewRootDescriptor::new([0xCC; 20], 0x3000, 11, 0xBBBB, 0)
}

/// Hosted journal medium backed by real files: `journal.bin` (2 x 112 bytes,
/// created erased to 0xFF), `base.bin` (48-byte superblock descriptor),
/// `artifact.bin` (the artifact bytes that must be fenced before the journal
/// record — §9 ordering precondition).
pub struct FileMedium {
    dir: PathBuf,
    journal: PathBuf,
    base: PathBuf,
    fenced: bool,
}

impl FileMedium {
    /// Create a fresh medium on disk: journal region erased (0xFF fill).
    pub fn new(base: NewRootDescriptor) -> Self {
        let dir = unique_dir();
        std::fs::create_dir_all(&dir).expect("create court tempdir");
        let journal = dir.join("journal.bin");
        let base_path = dir.join("base.bin");
        std::fs::write(&journal, [0xFF; 2 * RECORD_SIZE]).expect("erase journal region");
        std::fs::write(&base_path, base.to_bytes()).expect("write superblock");
        std::fs::write(dir.join("artifact.bin"), [0xAB; 64]).expect("write artifacts");
        Self {
            dir,
            journal,
            base: base_path,
            fenced: false,
        }
    }

    /// Directory holding this medium's files (court introspection).
    pub fn dir(&self) -> &Path {
        &self.dir
    }
}

impl Drop for FileMedium {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

impl Medium for FileMedium {
    fn read_base_root(&self) -> NewRootDescriptor {
        let bytes = std::fs::read(&self.base).expect("read superblock");
        let mut b = [0u8; PAYLOAD_SIZE];
        b.copy_from_slice(&bytes);
        NewRootDescriptor::from_bytes(&b)
    }

    fn read_slot(&self, slot: usize) -> [u8; RECORD_SIZE] {
        assert!(slot < SLOT_COUNT, "slot index out of range");
        let mut f = OpenOptions::new()
            .read(true)
            .open(&self.journal)
            .expect("open journal file");
        f.seek(SeekFrom::Start((slot * RECORD_SIZE) as u64))
            .expect("seek journal file");
        let mut buf = [0u8; RECORD_SIZE];
        f.read_exact(&mut buf).expect("read journal slot");
        buf
    }

    fn artifacts_fenced(&self) -> bool {
        self.fenced
    }
}

impl JournalWriter for FileMedium {
    /// Byte-granular pwrite: a plain full-byte store (hosted media have no
    /// flash AND-semantics).
    fn write_journal_byte(
        &mut self,
        slot: usize,
        offset: usize,
        value: u8,
    ) -> Result<(), MediumError> {
        if slot >= SLOT_COUNT {
            return Err(MediumError::SlotIndex);
        }
        if offset >= RECORD_SIZE {
            return Err(MediumError::OffsetRange);
        }
        let mut f = OpenOptions::new()
            .write(true)
            .open(&self.journal)
            .map_err(|_| MediumError::Io)?;
        f.seek(SeekFrom::Start((slot * RECORD_SIZE + offset) as u64))
            .map_err(|_| MediumError::Io)?;
        f.write_all(&[value]).map_err(|_| MediumError::Io)
    }

    /// Real durability fence: fsync the artifact file AND the journal file.
    fn fence_artifacts(&mut self) {
        for name in ["artifact.bin", "journal.bin"] {
            let f = OpenOptions::new()
                .write(true)
                .open(self.dir.join(name))
                .expect("open for fsync");
            f.sync_all().expect("fsync artifact bytes");
        }
        self.fenced = true;
    }
}

/// Age-corrupt one stored byte on the hosted medium (F1 fault injector).
pub fn flip_bit_file(m: &mut FileMedium, slot: usize, offset: usize, mask: u8) {
    let mut slot_bytes = m.read_slot(slot);
    slot_bytes[offset] ^= mask;
    m.write_journal_byte(slot, offset, slot_bytes[offset])
        .expect("hosted byte rewrite");
}

/// Full fenced commit of `seq`/`payload` (the unc-cut write, C4).
pub fn commit<M: JournalWriter>(m: &mut M, seq: u64, payload: &NewRootDescriptor) {
    m.fence_artifacts();
    write_record_cut(m, seq, payload, RECORD_SIZE).expect("full fenced record write");
}
