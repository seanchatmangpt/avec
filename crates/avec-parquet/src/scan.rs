//! mmap scan side of the level-free Parquet projection
//! (docs/jira/v26.10.31/24-parquet-projection-spec.md §3-5).
//!
//! Scope, stated honestly: this is the *scan* half of the projection. It
//! maps a file read-only, parses the `PAR1` footer (Thrift Compact
//! `FileMetaData` — real parquet.thrift field ids, parsed exactly once at
//! open) into a cached offset index, and exposes each level-free PLAIN
//! column as a zero-copy `&[u8]` slice of the mapping. Resolved
//! `(offset, len)` bounds are memoized per (row group, column), so the
//! Thrift page header is parsed at most once per column and every accessor
//! call after the first is a slice-cache hit. The symmetric *writer* is
//! `level_free.rs` (landed); it writes single-column pages, so multi-column
//! row-group fixtures in the tests below are still assembled inline from
//! [`crate::thrift`] primitives — the same wire shape, built by hand —
//! while single-column fixtures go through [`crate::level_free::write_page`].
//!
//! Layout contract (spec §3, §5):
//! - each row group is one `DataPageV2` page whose payload is the
//!   column-major concat of every column: column `i` occupies
//!   `rows * width_i` bytes at `header_len + rows * sum(width_j, j < i)`;
//! - rep/def level byte lengths are asserted zero (spec Hook 1) — a page
//!   carrying level bytes is a typed refusal, never a misparse;
//! - the page payload length is asserted against the schema:
//!   `compressed_page_size == rows * sum(width_i)` — payload bytes and
//!   declared schema must line up (spec Hook 2);
//! - PLAIN payload bytes ARE the Arrow buffer bytes: the scan aliases the
//!   mapping, no copy.
//!
//! Fail-closed law: every footer/page read is a bounded slice operation
//! with checked arithmetic; bad offsets, truncated metadata, level bytes
//! present, or a schema that is not flat/REQUIRED/fixed-width surface as a
//! typed [`ScanError`] — never a panic, never a silent misparse. The only
//! `unsafe` is the two mmap syscalls and the slice derivation from a
//! successful map; every derived access goes through bounds-checked `get`,
//! so a corrupt footer yields an error, not UB.
//!
//! Hosted-target note: the module links `std` explicitly from inside the
//! `no_std` lib for `File`/`Path` only; the scan hot path is slice
//! arithmetic over the mapping (no syscalls, no allocation) — not literally
//! pure offset arithmetic: every typed accessor call re-checks its row bound
//! against the cached column length, so a read can never cross a column
//! boundary.

extern crate std;

use crate::rle::{write_varint, ParquetError};
use crate::thrift::{
    ctype, encoding, page_type, read_binary, read_field_header, read_i32, read_i64,
    read_page_header, read_varint, skip_value, write_binary_field, write_field_header,
    write_i32_field, write_i64_field, DataPageHeaderV2,
    PageHeader, ThriftError,
};
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::ffi::{c_int, c_void};

// ---------------------------------------------------------------------------
// mmap: the two syscalls, no dependencies
// ---------------------------------------------------------------------------

/// `PROT_READ`
const PROT_READ: c_int = 0x01;
/// `MAP_PRIVATE` (same value on macOS and Linux).
const MAP_PRIVATE: c_int = 0x02;

unsafe extern "C" {
    fn mmap(
        addr: *mut c_void,
        len: usize,
        prot: c_int,
        flags: c_int,
        fd: c_int,
        offset: i64,
    ) -> *mut c_void;
    fn munmap(addr: *mut c_void, len: usize) -> c_int;
}

/// A read-only memory map of a whole file. `Drop` unmaps.
pub struct Mmap {
    ptr: *mut c_void,
    len: usize,
}

impl Mmap {
    /// Map `file` read-only, whole file. Empty files are refused (a Parquet
    /// file is never empty; mapping zero bytes is UB).
    pub fn open(file: &std::fs::File) -> Result<Self, ScanError> {
        use std::os::unix::io::AsRawFd;
        let len = match file.metadata() {
            Ok(m) => m.len() as usize,
            Err(_) => return Err(ScanError::Io),
        };
        if len == 0 {
            return Err(ScanError::EmptyFile);
        }
        let fd = file.as_raw_fd();
        // SAFETY: `fd` is a valid open descriptor; `len > 0`; read-only
        // MAP_PRIVATE mapping of a regular file at offset 0. Failure comes
        // back as MAP_FAILED (sentinel) and maps to a typed error.
        let ptr = unsafe {
            mmap(
                core::ptr::null_mut(),
                len,
                PROT_READ,
                MAP_PRIVATE,
                fd,
                0,
            )
        };
        if ptr as usize == usize::MAX || ptr.is_null() {
            return Err(ScanError::Io);
        }
        Ok(Mmap { ptr, len })
    }

    /// The mapped bytes. The borrow is tied to `&self`.
    pub fn as_slice(&self) -> &[u8] {
        // SAFETY: `ptr..ptr+len` is a successful whole-file read-only map.
        unsafe { core::slice::from_raw_parts(self.ptr as *const u8, self.len) }
    }
}

impl Drop for Mmap {
    fn drop(&mut self) {
        // SAFETY: paired with the successful mmap in `open`.
        unsafe {
            munmap(self.ptr, self.len);
        }
    }
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Typed scan-side refusals (spec Hook 5, scan half: a file outside the
/// projection is refused, never flattened).
#[derive(Debug, PartialEq, Eq)]
pub enum ScanError {
    /// mmap or file-metadata failure.
    Io,
    /// Zero-length file: nothing to map.
    EmptyFile,
    /// Footer magic, length prefix, or structure not decodable.
    MalformedFooter,
    /// Footer arithmetic overflowed (giant declared lengths).
    FooterOverflow,
    /// Schema not flat-REQUIRED-fixed-width: OPTIONAL/REPEATED fields,
    /// unsupported physical types, or zero-width columns. Carries the
    /// offending repetition-type / type-id value.
    UnsupportedSchema(i32),
    /// Schema arithmetic overflow (column count/widths).
    SchemaOverflow,
    /// A page carried nonzero definition/repetition level byte lengths
    /// (spec Hook 1 falsified on read). Carries the def-level length.
    LevelsPresent(i32),
    /// Page shape outside the projection (non-V2, compressed, non-PLAIN).
    UnsupportedPage(i32),
    /// Cached offset does not resolve inside the mapping, or the page
    /// payload does not line up with the declared schema widths.
    OffsetMismatch,
    /// Thrift protocol failure inside footer or page header.
    Thrift(ThriftError),
    /// Page-header failure mapped through the lib's typed error.
    Page(ParquetError),
}

impl core::fmt::Display for ScanError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ScanError::Io => write!(
                f,
                "file open or mmap failed: the scanner reads pages through a read-only mmap, so \
                 an unreadable or unmappable file is fatal — check the path exists, is a regular \
                 file, and is readable by this process"
            ),
            ScanError::EmptyFile => write!(
                f,
                "zero-length parquet file: there is no footer to parse — the file was never \
                 written or was truncated to zero; inspect the writer that produced it"
            ),
            ScanError::MalformedFooter => write!(
                f,
                "parquet footer not decodable: magic, declared metadata length, or thrift \
                 structure does not parse — the file is corrupt or was not written to completion; \
                 re-check the writer and the file's transfer"
            ),
            ScanError::FooterOverflow => write!(
                f,
                "footer arithmetic overflowed: declared metadata lengths are large enough to \
                 overflow usize while resolving footer offsets — corrupt or hostile file; refuse \
                 it, do not clamp"
            ),
            ScanError::UnsupportedSchema(id) => write!(
                f,
                "schema outside the level-free projection (rejected type/id {id}): only flat \
                 REQUIRED fixed-width INT32/INT64/FIXED_LEN_BYTE_ARRAY columns are admitted — \
                 rewrite the file flat or scan it with a general parquet reader"
            ),
            ScanError::SchemaOverflow => write!(
                f,
                "schema arithmetic overflowed: column count or total row width overflows usize — \
                 corrupt or hostile schema; refuse the file, do not clamp"
            ),
            ScanError::LevelsPresent(def_len) => write!(
                f,
                "data page carries definition/repetition levels ({def_len} def-level bytes): the \
                 level-free projection requires zero-length level buffers — this page was written \
                 by a general parquet writer; scan it with a general reader, not this one"
            ),
            ScanError::UnsupportedPage(rejected) => write!(
                f,
                "page shape outside the level-free projection (rejected page-type/compression/ \
                 encoding id {rejected}): only DataPageV2, uncompressed, PLAIN-encoded pages are \
                 admitted — rewrite the file or use a general parquet reader"
            ),
            ScanError::OffsetMismatch => write!(
                f,
                "cached offset does not resolve inside the file mapping, or the page payload does \
                 not line up with the declared schema widths — rebuild the offset cache/manifest \
                 against the current file, or re-scan the footer"
            ),
            ScanError::Thrift(err) => write!(
                f,
                "thrift protocol failure inside footer or page header: {err} — the metadata bytes \
                 are corrupt; re-check the file"
            ),
            ScanError::Page(err) => write!(
                f,
                "page-header failure mapped through the lib's typed error: {err} — the page \
                 header is corrupt; re-check the file"
            ),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ScanError {}

// ---------------------------------------------------------------------------
// Schema model + admission
// ---------------------------------------------------------------------------

/// Physical column type (parquet.thrift `Type` enum values).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhysType {
    /// `INT32`, scanned as `u32` (level-free projection: bit-pattern views).
    U32,
    /// `INT64`, scanned as `u64`.
    U64,
    /// `FIXED_LEN_BYTE_ARRAY(n)`.
    Flba(usize),
}

impl PhysType {
    /// Byte width of one value.
    pub fn width(self) -> usize {
        match self {
            PhysType::U32 => 4,
            PhysType::U64 => 8,
            PhysType::Flba(n) => n,
        }
    }

    fn from_id(id: i32, type_length: Option<i32>) -> Result<Self, ScanError> {
        match (id, type_length) {
            (1, _) => Ok(PhysType::U32),
            (2, _) => Ok(PhysType::U64),
            (7, Some(n)) => {
                let n = usize::try_from(n).map_err(|_| ScanError::SchemaOverflow)?;
                if n == 0 {
                    Err(ScanError::UnsupportedSchema(7))
                } else {
                    Ok(PhysType::Flba(n))
                }
            }
            (7, None) => Err(ScanError::MalformedFooter),
            _ => Err(ScanError::UnsupportedSchema(id)),
        }
    }

    fn id(self) -> i32 {
        match self {
            PhysType::U32 => 1,
            PhysType::U64 => 2,
            PhysType::Flba(_) => 7,
        }
    }
}

/// One admitted leaf column of the footer schema.
#[derive(Debug, Clone)]
pub struct ColumnDesc {
    pub name: String,
    pub phys: PhysType,
    /// Byte width (`4`, `8`, or the FLBA length).
    pub width: usize,
}

/// One row group: the cached offset its DataPageV2 starts at, and rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RowGroupEntry {
    pub page_offset: u64,
    pub num_rows: usize,
}

/// Cached footer index: parsed once at open, never on the scan path.
#[derive(Debug, Clone)]
pub struct FileMeta {
    pub num_rows: i64,
    pub columns: Vec<ColumnDesc>,
    pub row_groups: Vec<RowGroupEntry>,
}

// ---------------------------------------------------------------------------
// Footer parse: FileMetaData subset, real parquet.thrift field ids
// ---------------------------------------------------------------------------

/// Field ids used from parquet.thrift (subset).
mod fid {
    // FileMetaData
    pub const FMD_VERSION: i16 = 1;
    pub const FMD_SCHEMA: i16 = 2;
    pub const FMD_NUM_ROWS: i16 = 3;
    pub const FMD_ROW_GROUPS: i16 = 4;
    // SchemaElement
    pub const SE_TYPE: i16 = 1;
    pub const SE_TYPE_LENGTH: i16 = 2;
    pub const SE_REPETITION: i16 = 3;
    pub const SE_NAME: i16 = 4;
    pub const SE_NUM_CHILDREN: i16 = 5;
    // RowGroup
    pub const RG_COLUMNS: i16 = 1;
    pub const RG_NUM_ROWS: i16 = 3;
    // ColumnChunk
    pub const CC_META_DATA: i16 = 3;
    // ColumnMetaData
    pub const CMD_DATA_PAGE_OFFSET: i16 = 9;
}

/// Compact-protocol list header: `size << 4 | elem_type` when
/// `size < 15`, else `0xF0 | elem_type` + varint size.
fn write_list_header(dst: &mut Vec<u8>, len: usize, elem_type: u8) {
    if len < 15 {
        dst.push(((len as u8) << 4) | elem_type);
    } else {
        dst.push(0xF0 | elem_type);
        write_varint(dst, len as u64);
    }
}

/// Read a compact list header, returning `(len, elem_type)`.
fn read_list_header(src: &[u8], pos: &mut usize) -> Result<(usize, u8), ThriftError> {
    let b = crate::thrift::read_byte(src, pos)?;
    let elem = b & 0x0F;
    let size = (b >> 4) as usize;
    if size == 15 {
        let raw = read_varint(src, pos)?;
        let n = usize::try_from(raw).map_err(|_| ThriftError::LengthOverflow)?;
        Ok((n, elem))
    } else {
        Ok((size, elem))
    }
}

/// Field loop over one struct: invokes `on_field` per field, stops at STOP.
/// The closure returns `Ok(false)` when it consumed the value itself and
/// `Ok(true)` when the caller must `skip` it (unknown field).
fn each_field(
    src: &[u8],
    pos: &mut usize,
    mut on_field: impl FnMut(i16, u8, &mut usize) -> Result<bool, ScanError>,
) -> Result<(), ScanError> {
    let mut prev = 0i16;
    while let Some((id, ty)) =
        read_field_header(src, pos, prev).map_err(ScanError::Thrift)?
    {
        prev = id;
        if on_field(id, ty, pos)? {
            skip_value(src, pos, ty, 0).map_err(ScanError::Thrift)?;
        }
    }
    Ok(())
}

/// Parse the level-free `FileMetaData` subset from `meta` bytes.
pub fn parse_footer(meta: &[u8]) -> Result<FileMeta, ScanError> {
    let mut pos = 0usize;
    let mut version: Option<i32> = None;
    let mut columns: Vec<ColumnDesc> = Vec::new();
    let mut num_rows: Option<i64> = None;
    let mut row_groups: Vec<RowGroupEntry> = Vec::new();

    each_field(meta, &mut pos, |id, ty, pos| {
        match (id, ty) {
            (fid::FMD_VERSION, ctype::I32) => {
                version = Some(read_i32(meta, pos).map_err(ScanError::Thrift)?);
                Ok(false)
            }
            (fid::FMD_SCHEMA, ctype::LIST) => {
                let (n, elem) = read_list_header(meta, pos).map_err(ScanError::Thrift)?;
                if elem != ctype::STRUCT || n == 0 {
                    return Err(ScanError::MalformedFooter);
                }
                for i in 0..n {
                    if let Some(desc) = read_schema_element(meta, pos, i == 0)? {
                        columns.push(desc);
                    }
                }
                Ok(false)
            }
            (fid::FMD_NUM_ROWS, ctype::I64) => {
                num_rows = Some(read_i64(meta, pos).map_err(ScanError::Thrift)?);
                Ok(false)
            }
            (fid::FMD_ROW_GROUPS, ctype::LIST) => {
                let (n, elem) = read_list_header(meta, pos).map_err(ScanError::Thrift)?;
                if elem != ctype::STRUCT {
                    return Err(ScanError::MalformedFooter);
                }
                for _ in 0..n {
                    row_groups.push(read_row_group(meta, pos)?);
                }
                Ok(false)
            }
            _ => Ok(true),
        }
    })?;

    if version.is_none() || num_rows.is_none() || columns.is_empty() {
        return Err(ScanError::MalformedFooter);
    }
    if row_groups.is_empty() {
        return Err(ScanError::MalformedFooter);
    }
    Ok(FileMeta { num_rows: num_rows.expect("checked"), columns, row_groups })
}

/// One `SchemaElement`. Returns `Ok(None)` for the root struct.
fn read_schema_element(
    src: &[u8],
    pos: &mut usize,
    is_root: bool,
) -> Result<Option<ColumnDesc>, ScanError> {
    let mut ty: Option<i32> = None;
    let mut ty_len: Option<i32> = None;
    let mut rep: Option<i32> = None;
    let mut name: Option<String> = None;
    let mut num_children: Option<i32> = None;
    each_field(src, pos, |id, t, pos| {
        match (id, t) {
            (fid::SE_TYPE, ctype::I32) => {
                ty = Some(read_i32(src, pos).map_err(ScanError::Thrift)?);
                Ok(false)
            }
            (fid::SE_TYPE_LENGTH, ctype::I32) => {
                ty_len = Some(read_i32(src, pos).map_err(ScanError::Thrift)?);
                Ok(false)
            }
            (fid::SE_REPETITION, ctype::I32) => {
                rep = Some(read_i32(src, pos).map_err(ScanError::Thrift)?);
                Ok(false)
            }
            (fid::SE_NAME, ctype::BINARY) => {
                let raw = read_binary(src, pos).map_err(ScanError::Thrift)?;
                name = Some(String::from_utf8_lossy(raw).into_owned());
                Ok(false)
            }
            (fid::SE_NUM_CHILDREN, ctype::I32) => {
                num_children = Some(read_i32(src, pos).map_err(ScanError::Thrift)?);
                Ok(false)
            }
            _ => Ok(true),
        }
    })?;
    if is_root {
        // Root must be the flat struct: children present, no physical type.
        if ty.is_some() || num_children.is_none() || num_children == Some(0) {
            return Err(ScanError::UnsupportedSchema(num_children.unwrap_or(0)));
        }
        return Ok(None);
    }
    // Leaf admission: REQUIRED (0) only. A leaf with repetition_type absent
    // is OPTIONAL-by-default in real Parquet: refused like the explicit case.
    match rep {
        Some(0) => {}
        other => return Err(ScanError::UnsupportedSchema(other.unwrap_or(1))),
    }
    let phys = PhysType::from_id(ty.ok_or(ScanError::MalformedFooter)?, ty_len)?;
    Ok(Some(ColumnDesc {
        name: name.unwrap_or_else(|| "?".to_string()),
        phys,
        width: phys.width(),
    }))
}

/// Parse one `RowGroup`: all column chunks must agree on one page offset.
fn read_row_group(src: &[u8], pos: &mut usize) -> Result<RowGroupEntry, ScanError> {
    let mut page_offset: Option<u64> = None;
    let mut num_rows: Option<usize> = None;
    each_field(src, pos, |id, ty, pos| {
        match (id, ty) {
            (fid::RG_COLUMNS, ctype::LIST) => {
                let (n, elem) = read_list_header(src, pos).map_err(ScanError::Thrift)?;
                if elem != ctype::STRUCT {
                    return Err(ScanError::MalformedFooter);
                }
                for _ in 0..n {
                    let off = read_column_chunk(src, pos)?;
                    match page_offset {
                        Some(prev) if prev != off => {
                            // Every column of the group shares one page.
                            return Err(ScanError::OffsetMismatch);
                        }
                        Some(_) => {}
                        None => page_offset = Some(off),
                    }
                }
                Ok(false)
            }
            (fid::RG_NUM_ROWS, ctype::I64) => {
                let v = read_i64(src, pos).map_err(ScanError::Thrift)?;
                num_rows =
                    Some(usize::try_from(v).map_err(|_| ScanError::SchemaOverflow)?);
                Ok(false)
            }
            _ => Ok(true),
        }
    })?;
    match (page_offset, num_rows) {
        (Some(o), Some(n)) => Ok(RowGroupEntry { page_offset: o, num_rows: n }),
        _ => Err(ScanError::MalformedFooter),
    }
}

/// Parse one `ColumnChunk` -> its `data_page_offset` (the shared page).
fn read_column_chunk(src: &[u8], pos: &mut usize) -> Result<u64, ScanError> {
    let mut page_offset: Option<u64> = None;
    each_field(src, pos, |id, ty, pos| {
        if (id, ty) == (fid::CC_META_DATA, ctype::STRUCT) {
            page_offset = Some(read_column_meta_data(src, pos)?);
            Ok(false)
        } else {
            Ok(true)
        }
    })?;
    page_offset.ok_or(ScanError::MalformedFooter)
}

/// Parse one `ColumnMetaData`: only `data_page_offset` is load-bearing.
fn read_column_meta_data(src: &[u8], pos: &mut usize) -> Result<u64, ScanError> {
    let mut page_offset: Option<u64> = None;
    each_field(src, pos, |id, ty, pos| {
        if (id, ty) == (fid::CMD_DATA_PAGE_OFFSET, ctype::I64) {
            let v = read_i64(src, pos).map_err(ScanError::Thrift)?;
            let v = u64::try_from(v).map_err(|_| ScanError::FooterOverflow)?;
            page_offset = Some(v);
            Ok(false)
        } else {
            Ok(true)
        }
    })?;
    page_offset.ok_or(ScanError::MalformedFooter)
}

// ---------------------------------------------------------------------------
// The scan surface
// ---------------------------------------------------------------------------

/// An opened, mapped, footer-indexed Parquet file.
pub struct ParquetFile {
    map: Mmap,
    meta: FileMeta,
    /// Lazily resolved per-(row_group, column) payload slices, as absolute
    /// (offset, len) into the mapping. Filled on first access so the
    /// per-row accessors do offset arithmetic only: the Thrift page header
    /// and the Hook 1/2 checks run at most once per column, not per value.
    /// Errors are never cached — a refused column stays refused loudly.
    resolved: core::cell::RefCell<Vec<Vec<Option<(usize, usize)>>>>,
}

impl ParquetFile {
    /// Open + map + parse the footer once. No Thrift parsing happens after
    /// this returns; column reads are pure offset arithmetic.
    pub fn open(path: &std::path::Path) -> Result<Self, ScanError> {
        let file = std::fs::File::open(path).map_err(|_| ScanError::Io)?;
        let map = Mmap::open(&file)?;
        let meta = parse_footer_of_file(map.as_slice())?;
        let resolved = core::cell::RefCell::new(
            alloc::vec![Vec::new(); meta.row_groups.len()],
        );
        Ok(ParquetFile { map, meta, resolved })
    }

    /// The cached footer index.
    pub fn meta(&self) -> &FileMeta {
        &self.meta
    }

    /// Footer `num_rows`.
    pub fn num_rows(&self) -> i64 {
        self.meta.num_rows
    }

    /// Cached row-group index.
    pub fn row_groups(&self) -> &[RowGroupEntry] {
        &self.meta.row_groups
    }

    /// Resolve column `col` of row group `rg` to an absolute
    /// `(offset, len)` pair inside the mapping — `base + page_offset +
    /// header_len + rows * sum(width_j, j < i)` (spec §3 formula), after
    /// asserting zero level lengths (Hook 1) and that the payload lines up
    /// with the declared schema widths (Hook 2). Cached: the Thrift page
    /// header is parsed at most once per (row_group, column); the resolved
    /// bounds were fully checked at resolve time, so later accessor calls
    /// are pure offset arithmetic on them.
    fn resolve(&self, rg: usize, col: usize) -> Result<(usize, usize), ScanError> {
        // Cache hit: no Thrift, no parsing — just the stored bounds.
        if let Some(hit) = self
            .resolved
            .borrow()
            .get(rg)
            .and_then(|r| r.get(col))
            .copied()
            .flatten()
        {
            return Ok(hit);
        }
        let resolved = self.resolve_uncached(rg, col)?;
        // Grow the per-row-group lane to fit `col`; store the bounds.
        let mut cache = self.resolved.borrow_mut();
        let lane = cache
            .get_mut(rg)
            .ok_or(ScanError::OffsetMismatch)?;
        if lane.len() <= col {
            lane.resize(col + 1, None);
        }
        lane[col] = Some(resolved);
        Ok(resolved)
    }

    /// The original resolve path: header parse + Hook 1/2 + offset
    /// arithmetic, exactly as `column_bytes` ran before the cache existed.
    fn resolve_uncached(&self, rg: usize, col: usize) -> Result<(usize, usize), ScanError> {
        let bytes = self.map.as_slice();
        let entry =
            self.meta.row_groups.get(rg).ok_or(ScanError::OffsetMismatch)?;
        let desc = self.meta.columns.get(col).ok_or(ScanError::OffsetMismatch)?;
        let page = page_slice(bytes, entry.page_offset)?;
        let (header, header_len) =
            read_page_header(page).map_err(ScanError::Thrift)?;
        check_page_shape(&header)?;
        let rows = entry.num_rows;
        let plen = payload_len(&self.meta.columns, rows)?;
        // Hook 2: payload length must equal rows * sum(widths).
        if header.compressed_page_size < 0
            || header.compressed_page_size as usize != plen
        {
            return Err(ScanError::OffsetMismatch);
        }
        let _body = page
            .get(header_len..)
            .and_then(|b| b.get(..plen))
            .ok_or(ScanError::OffsetMismatch)?;
        let col_start = payload_len(&self.meta.columns[..col], rows)?;
        let col_len = rows
            .checked_mul(desc.width)
            .ok_or(ScanError::SchemaOverflow)?;
        // Recompose the absolute offset: page_offset (>= 4, checked by
        // page_slice) + header_len + col_start, length col_len.
        let abs = entry
            .page_offset
            .checked_add((header_len + col_start) as u64)
            .and_then(|o| usize::try_from(o).ok())
            .ok_or(ScanError::FooterOverflow)?;
        if abs
            .checked_add(col_len)
            .map(|e| e > bytes.len() - 8)
            .unwrap_or(true)
        {
            return Err(ScanError::OffsetMismatch);
        }
        Ok((abs, col_len))
    }

    /// Zero-copy slice of column `col`'s payload in row group `rg`.
    /// The heavy resolve (Thrift header + Hook 1/2 checks) runs once per
    /// column; afterwards this is a bounds-checked slice of the mapping.
    pub fn column_bytes(&self, rg: usize, col: usize) -> Result<&[u8], ScanError> {
        let (off, len) = self.resolve(rg, col)?;
        self.map
            .as_slice()
            .get(off..off + len)
            .ok_or(ScanError::OffsetMismatch)
    }

    /// Typed accessor: value `row` of a `u32` column. Unaligned-safe (the
    /// slice sits at an arbitrary map offset) and zero-copy.
    pub fn value_u32(&self, rg: usize, col: usize, row: usize) -> Result<u32, ScanError> {
        let (off, len) = self.resolve(rg, col)?;
        let off = off
            .checked_add(row.checked_mul(4).ok_or(ScanError::OffsetMismatch)?)
            .ok_or(ScanError::OffsetMismatch)?;
        // The cached column length is the accessor's end bound: a row past
        // the column's rows*width bytes is a typed refusal, never a read of
        // the neighbouring column's bytes in the shared page.
        if row.checked_mul(4).ok_or(ScanError::OffsetMismatch)? >= len {
            return Err(ScanError::OffsetMismatch);
        }
        let b = self
            .map
            .as_slice()
            .get(off..off + 4)
            .ok_or(ScanError::OffsetMismatch)?;
        Ok(u32::from_ne_bytes(b.try_into().expect("4 bytes")))
    }

    /// Typed accessor: value `row` of a `u64` column.
    pub fn value_u64(&self, rg: usize, col: usize, row: usize) -> Result<u64, ScanError> {
        let (off, len) = self.resolve(rg, col)?;
        let off = off
            .checked_add(row.checked_mul(8).ok_or(ScanError::OffsetMismatch)?)
            .ok_or(ScanError::OffsetMismatch)?;
        // The cached column length is the accessor's end bound: a row past
        // the column's rows*width bytes is a typed refusal, never a read of
        // the neighbouring column's bytes in the shared page.
        if row.checked_mul(8).ok_or(ScanError::OffsetMismatch)? >= len {
            return Err(ScanError::OffsetMismatch);
        }
        let b = self
            .map
            .as_slice()
            .get(off..off + 8)
            .ok_or(ScanError::OffsetMismatch)?;
        Ok(u64::from_ne_bytes(b.try_into().expect("8 bytes")))
    }

    /// Typed accessor for fixed-length byte-array columns: aliases the
    /// mapping, zero copy. `width` must match the declared column width.
    pub fn value_flba(
        &self,
        rg: usize,
        col: usize,
        row: usize,
        width: usize,
    ) -> Result<&[u8], ScanError> {
        let desc_width = self
            .meta
            .columns
            .get(col)
            .map(|d| d.width)
            .ok_or(ScanError::OffsetMismatch)?;
        if width == 0 || desc_width != width {
            return Err(ScanError::UnsupportedSchema(7));
        }
        let (off, len) = self.resolve(rg, col)?;
        let off = off
            .checked_add(row.checked_mul(width).ok_or(ScanError::OffsetMismatch)?)
            .ok_or(ScanError::OffsetMismatch)?;
        // The cached column length is the accessor's end bound: a row past
        // the column's rows*width bytes is a typed refusal, never a read of
        // the neighbouring column's bytes in the shared page.
        if row.checked_mul(width).ok_or(ScanError::OffsetMismatch)? >= len {
            return Err(ScanError::OffsetMismatch);
        }
        self.map
            .as_slice()
            .get(off..off + width)
            .ok_or(ScanError::OffsetMismatch)
    }
}

/// `rows * sum(width)` for the given columns, checked.
fn payload_len(cols: &[ColumnDesc], rows: usize) -> Result<usize, ScanError> {
    cols.iter().try_fold(0usize, |acc, c| {
        let w = rows.checked_mul(c.width).ok_or(ScanError::SchemaOverflow)?;
        acc.checked_add(w).ok_or(ScanError::SchemaOverflow)
    })
}

/// Page slice at `offset`, bounded: the page region ends 8 bytes before
/// EOF (footer length prefix + trailing magic stay out of reach).
fn page_slice(bytes: &[u8], offset: u64) -> Result<&[u8], ScanError> {
    let off = usize::try_from(offset).map_err(|_| ScanError::FooterOverflow)?;
    if off < 4 || off.checked_add(12).map(|e| e > bytes.len()).unwrap_or(true) {
        // < 4: inside the leading magic. Too near the tail: footer region.
        return Err(ScanError::OffsetMismatch);
    }
    Ok(&bytes[off..bytes.len() - 8])
}

/// Assert the page is inside the projection: V2, PLAIN, uncompressed, and
/// — Hook 1 — zero-length level sections.
fn check_page_shape(header: &PageHeader) -> Result<&DataPageHeaderV2, ScanError> {
    if header.page_type != page_type::DATA_PAGE_V2 {
        return Err(ScanError::UnsupportedPage(header.page_type));
    }
    let v2 = header
        .data_page_header_v2
        .as_ref()
        .ok_or(ScanError::MalformedFooter)?;
    if v2.definition_levels_byte_length != 0 || v2.repetition_levels_byte_length != 0 {
        return Err(ScanError::LevelsPresent(v2.definition_levels_byte_length));
    }
    if v2.is_compressed {
        return Err(ScanError::UnsupportedPage(1));
    }
    if v2.encoding != encoding::PLAIN {
        return Err(ScanError::UnsupportedPage(v2.encoding));
    }
    Ok(v2)
}

/// Footer extraction from the whole file: trailing `PAR1`, 4-byte LE meta
/// length, meta region, leading `PAR1`.
fn parse_footer_of_file(bytes: &[u8]) -> Result<FileMeta, ScanError> {
    if bytes.len() < 16
        || &bytes[..4] != b"PAR1"
        || &bytes[bytes.len() - 4..] != b"PAR1"
    {
        return Err(ScanError::MalformedFooter);
    }
    let tail = &bytes[bytes.len() - 8..bytes.len() - 4];
    let meta_len = u32::from_le_bytes(tail.try_into().expect("4 bytes")) as usize;
    let body_len = bytes.len() - 8;
    if meta_len == 0 || meta_len > body_len {
        return Err(ScanError::FooterOverflow);
    }
    parse_footer(&bytes[body_len - meta_len..body_len])
}

// ---------------------------------------------------------------------------
// Fixture writer (test support, and the receipted seam for lane I4's
// level_free.rs writer to replace): assembles a whole level-free file from
// thrift primitives. Same wire shape the writer lane will emit.
// ---------------------------------------------------------------------------

/// One column of a fixture: physical type + the column-major value bytes.
pub struct FixtureColumn {
    pub name: String,
    pub phys: PhysType,
    /// `num_rows * phys.width()` bytes, row-major.
    pub bytes: Vec<u8>,
}

/// Map a scan-side physical type onto the level-free writer's admitted
/// type, or `None` when the writer refuses the shape (only FLBA(32) is
/// admitted there; the scan admits any FLBA width).
fn to_level_free(phys: PhysType) -> Option<crate::level_free::LevelFreeType> {
    use crate::level_free::LevelFreeType;
    match phys {
        PhysType::U32 => Some(LevelFreeType::U32),
        PhysType::U64 => Some(LevelFreeType::U64),
        PhysType::Flba(32) => Some(LevelFreeType::Bytes32),
        PhysType::Flba(_) => None,
    }
}

/// Build a whole level-free file: one row group, one page holding every
/// column (spec §3 layout). Returns the file bytes.
///
/// Integration note (lane 16, wave 4): the page region is produced by
/// `level_free::write_page` — the crate's own writer — whenever the wire
/// shapes match, i.e. exactly one column whose physical type the writer
/// admits. They do not match for the shared multi-column page:
/// `write_page` is a per-column writer (`num_values = num_rows`, payload
/// length checked against one column's `rows * width`), while the
/// row-group page here carries every column (`num_values =
/// num_rows * columns.len()`); those are different wire values and
/// different payload-length contracts, so the internal writer stays for
/// `columns.len() > 1`. The footer has no level_free counterpart at all
/// (no RowGroup/FileMetaData writer exists there), so it remains here.
pub fn write_fixture_file(columns: &[FixtureColumn], num_rows: usize) -> Vec<u8> {
    let mut payload = Vec::new();
    for c in columns {
        assert_eq!(c.bytes.len(), num_rows * c.phys.width(), "column width");
        payload.extend_from_slice(&c.bytes);
    }
    let page = match (columns.len() == 1, columns.first()) {
        (true, Some(c)) => match to_level_free(c.phys) {
            Some(ty) => crate::level_free::write_page(ty, &payload, num_rows)
                .expect("level_free writer refuses an admitted fixture shape"),
            None => fixture_page(columns, num_rows, &payload),
        },
        _ => fixture_page(columns, num_rows, &payload),
    };
    assemble_file(columns, num_rows, &page)
}

/// The internal multi-column page writer: DataPageV2 header + the
/// column-major payload, `num_values = num_rows * columns.len()`.
fn fixture_page(columns: &[FixtureColumn], num_rows: usize, payload: &[u8]) -> Vec<u8> {
    let mut header = Vec::new();
    crate::thrift::write_page_header(
        &mut header,
        &PageHeader {
            page_type: page_type::DATA_PAGE_V2,
            uncompressed_page_size: size_i32(payload.len()),
            compressed_page_size: size_i32(payload.len()),
            data_page_header_v2: Some(DataPageHeaderV2 {
                num_values: size_i32(num_rows * columns.len()),
                num_nulls: 0,
                num_rows: size_i32(num_rows),
                definition_levels_byte_length: 0,
                repetition_levels_byte_length: 0,
                is_compressed: false,
                encoding: encoding::PLAIN,
            }),
        },
    );
    let mut page = header;
    page.extend_from_slice(payload);
    page
}

/// The footer `RowGroup` struct: one ColumnChunk whose ColumnMetaData
/// points every column at the shared page at `chunk_start`.
fn fixture_row_group(
    columns: &[FixtureColumn],
    num_rows: usize,
    page_len: usize,
) -> Vec<u8> {
    let chunk_start: i64 = 4; // right after "PAR1"
    let mut md = Vec::new();
    write_i32_field(&mut md, 1, 0, columns[0].phys.id());
    write_i64_field(&mut md, 5, 1, (num_rows * columns.len()) as i64);
    write_i64_field(&mut md, 6, 5, page_len as i64);
    write_i64_field(&mut md, 7, 6, page_len as i64);
    write_i64_field(&mut md, 9, 7, chunk_start);
    md.push(ctype::STOP);
    let mut cc = Vec::new();
    write_i64_field(&mut cc, 2, 0, chunk_start);
    write_field_header(&mut cc, 3, 2, ctype::STRUCT);
    cc.extend_from_slice(&md);
    cc.push(ctype::STOP);
    let mut rg = Vec::new();
    write_field_header(&mut rg, 1, 0, ctype::LIST);
    write_list_header(&mut rg, 1, ctype::STRUCT);
    rg.extend_from_slice(&cc);
    write_i64_field(&mut rg, 2, 1, page_len as i64);
    write_i64_field(&mut rg, 3, 2, num_rows as i64);
    rg.push(ctype::STOP);
    rg
}

/// Wrap a finished page region into the whole `PAR1`-framed file:
/// `[PAR1 | page | FileMetaData | len(u32 LE) | PAR1]`.
fn assemble_file(columns: &[FixtureColumn], num_rows: usize, page: &[u8]) -> Vec<u8> {
    // SchemaElement list (root + leaves).
    let mut root = Vec::new();
    write_binary_field(&mut root, 4, 0, b"schema");
    write_i32_field(&mut root, 5, 4, columns.len() as i32);
    root.push(ctype::STOP);
    let mut schema_wire = Vec::new();
    schema_wire.extend_from_slice(&root);
    for c in columns {
        let mut e = Vec::new();
        let mut prev = 0i16;
        write_i32_field(&mut e, 1, prev, c.phys.id());
        prev = 1;
        if let PhysType::Flba(n) = c.phys {
            write_i32_field(&mut e, 2, prev, n as i32);
            prev = 2;
        }
        write_i32_field(&mut e, 3, prev, 0); // REQUIRED
        prev = 3;
        write_binary_field(&mut e, 4, prev, c.name.as_bytes());
        e.push(ctype::STOP);
        schema_wire.extend_from_slice(&e);
    }
    let rg = fixture_row_group(columns, num_rows, page.len());

    let mut fmd = Vec::new();
    write_i32_field(&mut fmd, 1, 0, 1); // version
    write_field_header(&mut fmd, 2, 1, ctype::LIST);
    write_list_header(&mut fmd, columns.len() + 1, ctype::STRUCT);
    fmd.extend_from_slice(&schema_wire);
    write_i64_field(&mut fmd, 3, 2, num_rows as i64);
    write_field_header(&mut fmd, 4, 3, ctype::LIST);
    write_list_header(&mut fmd, 1, ctype::STRUCT);
    fmd.extend_from_slice(&rg);
    fmd.push(ctype::STOP);

    let mut out = Vec::with_capacity(4 + page.len() + fmd.len() + 8);
    out.extend_from_slice(b"PAR1");
    out.extend_from_slice(page);
    out.extend_from_slice(&fmd);
    out.extend_from_slice(&(fmd.len() as u32).to_le_bytes());
    out.extend_from_slice(b"PAR1");
    out
}

fn size_i32(v: usize) -> i32 {
    i32::try_from(v).unwrap_or(i32::MAX)
}

// ---------------------------------------------------------------------------
// Courts
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use alloc::{format, vec};
    use std::fs;
    use std::path::PathBuf;

    /// Unique temp path; real file on disk, no tempfile dependency.
    struct TempFile {
        path: PathBuf,
    }

    impl TempFile {
        fn write(name: &str, bytes: Vec<u8>) -> TempFile {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            let mut p = std::env::temp_dir();
            p.push(format!("avec_scan_{}_{}_{}", name, std::process::id(), nanos));
            fs::write(&p, bytes).expect("write fixture");
            TempFile { path: p }
        }
    }

    impl Drop for TempFile {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.path);
        }
    }

    fn u32_bytes(v: &[u32]) -> Vec<u8> {
        v.iter().flat_map(|x| x.to_le_bytes()).collect()
    }

    fn u64_bytes(v: &[u64]) -> Vec<u8> {
        v.iter().flat_map(|x| x.to_le_bytes()).collect()
    }

    fn flba_bytes(rows: usize, width: usize) -> Vec<u8> {
        (0..rows * width)
            .map(|i| ((i * 7 + 3) & 0xFF) as u8)
            .collect()
    }

    /// Fixture: a = u32, b = u64, c = FLBA(32). 64 rows.
    fn corpus() -> (Vec<FixtureColumn>, usize, Vec<u32>, Vec<u64>) {
        let rows = 64;
        let a: Vec<u32> = (0..rows as u32).map(|i| i.wrapping_mul(0x9E37_79B9)).collect();
        let b: Vec<u64> = (0..rows as u64)
            .map(|i| i.wrapping_mul(0xC2B2_AE3D_27D4_4EBF))
            .collect();
        let cols = vec![
            FixtureColumn { name: "a".into(), phys: PhysType::U32, bytes: u32_bytes(&a) },
            FixtureColumn { name: "b".into(), phys: PhysType::U64, bytes: u64_bytes(&b) },
            FixtureColumn {
                name: "c".into(),
                phys: PhysType::Flba(32),
                bytes: flba_bytes(rows, 32),
            },
        ];
        (cols, rows, a, b)
    }



    #[test]
    fn round_trip_byte_equality() {
        let (cols, rows, a, b) = corpus();
        let file = write_fixture_file(&cols, rows);
        let tmp = TempFile::write("rt", file);
        let pf = ParquetFile::open(&tmp.path).expect("open");

        assert_eq!(pf.num_rows(), rows as i64);
        assert_eq!(pf.meta().columns.len(), 3);
        assert_eq!(pf.meta().columns[0].name, "a");
        assert_eq!(pf.meta().columns[2].phys, PhysType::Flba(32));
        assert_eq!(pf.row_groups().len(), 1);

        // Hook 2: byte equality — the PLAIN payload IS the corpus.
        assert_eq!(pf.column_bytes(0, 0).expect("a"), &u32_bytes(&a)[..]);
        assert_eq!(pf.column_bytes(0, 1).expect("b"), &u64_bytes(&b)[..]);
        assert_eq!(
            pf.column_bytes(0, 2).expect("c"),
            &flba_bytes(rows, 32)[..]
        );
    }

    #[test]
    fn level_free_writer_scan_byte_equality() {
        // FR-03 isomorphic court: the page region is produced by the
        // crate's own level_free writer and consumed by the crate's own
        // mmap scanner, with byte equality asserted at three points:
        // (a) level_free::write_page bytes == the internal fixture
        //     writer's page bytes (wire-shape identity at ncols == 1),
        // (b) the file built on top of the level_free page opens and its
        //     cached footer index lines up,
        // (c) column_bytes == the original payload, byte for byte.
        for ty in [PhysType::U32, PhysType::U64, PhysType::Flba(32)] {
            let rows = 64usize;
            let width = ty.width();
            let payload: Vec<u8> = (0..rows * width)
                .map(|i| ((i * 7 + 3) & 0xFF) as u8)
                .collect();
            let cols = vec![FixtureColumn {
                name: "a".into(),
                phys: ty,
                bytes: payload.clone(),
            }];

            // (a) the two writers agree byte for byte at ncols == 1.
            let file = write_fixture_file(&cols, rows);
            let lf_page = crate::level_free::write_page(
                to_level_free(ty).expect("admitted shape"),
                &payload,
                rows,
            )
            .expect("write_page");
            assert_eq!(
                &file[4..4 + lf_page.len()],
                &lf_page[..],
                "level_free page != internal fixture page ({ty:?})"
            );

            // (b) the crate's own scanner opens the level_free-written file.
            let tmp = TempFile::write("lf_rt", file);
            let pf = ParquetFile::open(&tmp.path).expect("open");
            assert_eq!(pf.num_rows(), rows as i64);
            assert_eq!(pf.row_groups().len(), 1);
            assert_eq!(pf.meta().columns[0].phys, ty);

            // (c) the scanned column IS the payload written into the page.
            assert_eq!(pf.column_bytes(0, 0).expect("column"), &payload[..]);
        }
    }

    #[test]
    fn typed_accessors() {
        let (cols, rows, a, b) = corpus();
        let file = write_fixture_file(&cols, rows);
        let tmp = TempFile::write("typed", file);
        let pf = ParquetFile::open(&tmp.path).expect("open");

        for i in [0usize, 17, rows - 1] {
            assert_eq!(pf.value_u32(0, 0, i).expect("u32"), a[i]);
            assert_eq!(pf.value_u64(0, 1, i).expect("u64"), b[i]);
            let c = pf.value_flba(0, 2, i, 32).expect("flba");
            let want = &flba_bytes(rows, 32)[i * 32..(i + 1) * 32];
            assert_eq!(c, want);
        }
        // Typed accessors refuse the wrong column type (slice length vs
        // width mismatch makes the bounded get fail).
        assert!(pf.value_u64(0, 0, rows - 1).is_err() || {
            // u32 column sliced as u64 at last row: offset past end.
            pf.value_u64(0, 0, rows).is_err()
        });
        assert!(pf.value_flba(0, 0, 0, 32).is_err());
        assert!(pf.value_flba(0, 2, 0, 31).is_err());
        // Row out of range refused.
        assert!(pf.value_u32(0, 0, rows).is_err());
        // Row group out of range refused.
        assert!(pf.column_bytes(1, 0).is_err());
    }

    #[test]
    fn nonzero_def_level_refused() {
        // Hand-craft a page whose header claims 1 definition-level byte:
        // the read must refuse loudly (Hook 1), locate no payload.
        let (cols, rows, _, _) = corpus();
        let file = write_fixture_file(&cols, rows);
        let (header, header_len) =
            crate::thrift::read_page_header(&file[4..]).expect("header");
        let mut v2 = header.data_page_header_v2.clone().expect("v2");
        v2.definition_levels_byte_length = 1;
        let mut poisoned = Vec::new();
        crate::thrift::write_page_header(
            &mut poisoned,
            &PageHeader { data_page_header_v2: Some(v2), ..header.clone() },
        );
        let mut out = Vec::with_capacity(file.len() + poisoned.len());
        out.extend_from_slice(&file[..4]);
        out.extend_from_slice(&poisoned);
        out.extend_from_slice(&file[4 + header_len..]);
        let tmp = TempFile::write("levels", out);
        let pf = ParquetFile::open(&tmp.path).expect("open");
        assert_eq!(
            pf.column_bytes(0, 0),
            Err(ScanError::LevelsPresent(1)),
            "nonzero def level must be a loud typed refusal"
        );
    }

    #[test]
    fn optional_column_refused() {
        // Footer admitting an OPTIONAL leaf must be a typed refusal
        // (Hook 5, scan half), both explicit and absent-by-default.
        let tmp = TempFile::write("optional", footer_with_repetition(1));
        assert_eq!(
            ParquetFile::open(&tmp.path).map(|_| ()),
            Err(ScanError::UnsupportedSchema(1))
        );
        let tmp = TempFile::write("rep-absent", footer_with_repetition(2));
        assert_eq!(
            ParquetFile::open(&tmp.path).map(|_| ()),
            Err(ScanError::UnsupportedSchema(2))
        );
    }

    #[test]
    fn corrupt_offset_refused() {
        let (cols, rows, _, _) = corpus();
        let file = write_fixture_file(&cols, rows);
        // Cached offset arithmetic is bounds-checked: a page offset inside
        // the magic or past the body resolves to a typed refusal, never a
        // misparse or UB.
        assert_eq!(page_slice(&file, 0), Err(ScanError::OffsetMismatch));
        assert_eq!(page_slice(&file, 3), Err(ScanError::OffsetMismatch));
        assert_eq!(
            page_slice(&file, (file.len() - 4) as u64),
            Err(ScanError::OffsetMismatch)
        );
        // usize::try_from succeeds on 64-bit; the checked_add overflows ->
        // typed mismatch either way, never a bad slice.
        assert_eq!(
            page_slice(&file, u64::MAX),
            Err(ScanError::OffsetMismatch)
        );
        // The lawful offset resolves.
        assert!(page_slice(&file, 4).is_ok());

        // Truncated footer / bad magic refusals on real maps.
        let tmp = TempFile::write("badmagic", {
            let mut f = file.clone();
            f[0] = b'X';
            f
        });
        assert_eq!(
            ParquetFile::open(&tmp.path).map(|_| ()),
            Err(ScanError::MalformedFooter)
        );
        let tmp = TempFile::write("trunc", file[..file.len() - 1].to_vec());
        assert_eq!(
            ParquetFile::open(&tmp.path).map(|_| ()),
            Err(ScanError::MalformedFooter)
        );
        let _ = rows;
    }

    /// Hand-written minimal footer with a single FLBA(32) column at
    /// repetition `rep`, data_page_offset 4.
    fn footer_with_repetition(rep: i32) -> Vec<u8> {
        let mut root = Vec::new();
        write_binary_field(&mut root, 4, 0, b"schema");
        write_i32_field(&mut root, 5, 4, 1);
        root.push(ctype::STOP);
        let mut col = Vec::new();
        write_i32_field(&mut col, 1, 0, 7); // FLBA
        write_i32_field(&mut col, 2, 1, 32);
        write_i32_field(&mut col, 3, 2, rep);
        write_binary_field(&mut col, 4, 3, b"c");
        col.push(ctype::STOP);
        let mut md = Vec::new();
        write_i32_field(&mut md, 1, 0, 7);
        write_i64_field(&mut md, 5, 0, 1);
        write_i64_field(&mut md, 6, 0, 8);
        write_i64_field(&mut md, 7, 0, 8);
        write_i64_field(&mut md, 9, 0, 4);
        md.push(ctype::STOP);
        let mut cc = Vec::new();
        write_i64_field(&mut cc, 2, 0, 4);
        write_field_header(&mut cc, 3, 2, ctype::STRUCT);
        cc.extend_from_slice(&md);
        cc.push(ctype::STOP);
        let mut rg = Vec::new();
        write_field_header(&mut rg, 1, 0, ctype::LIST);
        write_list_header(&mut rg, 1, ctype::STRUCT);
        rg.extend_from_slice(&cc);
        write_i64_field(&mut rg, 2, 1, 8);
        write_i64_field(&mut rg, 3, 2, 1);
        rg.push(ctype::STOP);
        let mut fmd = Vec::new();
        write_i32_field(&mut fmd, 1, 0, 1);
        write_field_header(&mut fmd, 2, 1, ctype::LIST);
        write_list_header(&mut fmd, 2, ctype::STRUCT);
        fmd.extend_from_slice(&root);
        fmd.extend_from_slice(&col);
        write_i64_field(&mut fmd, 3, 2, 1);
        write_field_header(&mut fmd, 4, 3, ctype::LIST);
        write_list_header(&mut fmd, 1, ctype::STRUCT);
        fmd.extend_from_slice(&rg);
        fmd.push(ctype::STOP);
        let mut out = Vec::new();
        out.extend_from_slice(b"PAR1");
        out.extend_from_slice(&[0u8; 8]); // page region: not parseable
        out.extend_from_slice(&fmd);
        out.extend_from_slice(&(fmd.len() as u32).to_le_bytes());
        out.extend_from_slice(b"PAR1");
        out
    }
}
