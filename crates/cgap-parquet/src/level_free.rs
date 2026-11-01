//! Level-free Parquet projection (FR-03, spec
//! `docs/jira/v26.10.31/24-parquet-projection-spec.md`): flat, non-nullable,
//! fixed-width schemas only; the refusal IS a feature.
//!
//! The module admits exactly one schema shape — `u32`, `u64`,
//! `fixed_len_byte_array(32)`, every field REQUIRED, max def level = max rep
//! level = 0 — and returns a typed refusal for anything else (optional,
//! repeated, nested, wrong width, unknown type). Writers never silently
//! flatten (spec Hook 5).
//!
//! Pages are `DataPageV2` with zero-length rep/def level buffers (spec Hook
//! 1), `PLAIN` encoding, `is_compressed = false`; the payload is the
//! contiguous fixed-width slice whose bytes ARE the Arrow buffer bytes
//! (spec Hook 2, PLAIN path). Row-group spans are padded to the 256B block
//! grid via [`align_rows_256b`].

use crate::thrift::{
    ctype, encoding, page_type, read_binary, read_field_header, read_i32, skip_value,
    write_binary_field, write_field_header, write_i32_field, write_i64_field,
    write_page_header, DataPageHeaderV2, PageHeader, ThriftError,
};
use alloc::vec::Vec;
use core::fmt;

/// Typed admission refusals and page-shape errors for the level-free
/// projection. Loud, never silent, never a panic (the `rle.rs` /
/// `thrift.rs` error mold).
#[derive(Debug, PartialEq, Eq)]
pub enum LevelFreeError {
    /// A field carried `repetition_type = OPTIONAL` (1).
    OptionalField { index: usize, name: Vec<u8> },
    /// A field carried `repetition_type = REPEATED` (2).
    RepeatedField { index: usize, name: Vec<u8> },
    /// A field was a group (no physical type / `num_children > 0`).
    NestedField { index: usize, name: Vec<u8> },
    /// A physical type outside the admitted set {INT32, INT64,
    /// FIXED_LEN_BYTE_ARRAY(32)}.
    UnsupportedType {
        index: usize,
        name: Vec<u8>,
        code: i32,
    },
    /// `FIXED_LEN_BYTE_ARRAY` with a type length other than 32.
    UnsupportedWidth {
        index: usize,
        name: Vec<u8>,
        length: i32,
    },
    /// A repetition code outside {REQUIRED, OPTIONAL, REPEATED}.
    UnknownRepetition {
        index: usize,
        name: Vec<u8>,
        code: i32,
    },
    /// A written page could not be parsed back (corrupt header or a
    /// truncated payload).
    UndecodablePage,
    /// The parsed payload length does not equal `rows * width`.
    PayloadMismatch { expected: usize, actual: usize },
}

impl fmt::Display for LevelFreeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LevelFreeError::OptionalField { index, name } => write!(
                f,
                "field {index} ('{}'): repetition_type OPTIONAL — a nullable column cannot map \
                 onto a fixed-width level-free record batch; make the field REQUIRED in the \
                 schema or scan the file with a general parquet reader",
                alloc::string::String::from_utf8_lossy(name)
            ),
            LevelFreeError::RepeatedField { index, name } => write!(
                f,
                "field {index} ('{}'): repetition_type REPEATED — a repeated column cannot map \
                 onto a fixed-width level-free record batch; flatten the repetition away in the \
                 schema or scan the file with a general parquet reader",
                alloc::string::String::from_utf8_lossy(name)
            ),
            LevelFreeError::NestedField { index, name } => write!(
                f,
                "field {index} ('{}'): a group with no physical type (num_children > 0) — flatten \
                 the schema; nested groups are outside the level-free projection",
                alloc::string::String::from_utf8_lossy(name)
            ),
            LevelFreeError::UnsupportedType { index, name, code } => write!(
                f,
                "field {index} ('{}'): physical type id {code} is outside the admitted set \
                 {{INT32(1), INT64(2), FIXED_LEN_BYTE_ARRAY(7)}} — rewrite the column to an \
                 admitted type or extend the projection",
                alloc::string::String::from_utf8_lossy(name)
            ),
            LevelFreeError::UnsupportedWidth { index, name, length } => write!(
                f,
                "field {index} ('{}'): FIXED_LEN_BYTE_ARRAY length {length} is not the admitted \
                 width 32 — change the schema width to 32 or extend the projection",
                alloc::string::String::from_utf8_lossy(name)
            ),
            LevelFreeError::UnknownRepetition { index, name, code } => write!(
                f,
                "field {index} ('{}'): repetition code {code} is outside the parquet.thrift \
                 FieldRepetitionType range {{0=REQUIRED, 1=OPTIONAL, 2=REPEATED}} — the schema \
                 metadata is corrupt; re-check the writer",
                alloc::string::String::from_utf8_lossy(name)
            ),
            LevelFreeError::UndecodablePage => write!(
                f,
                "written page could not be parsed back: page header corrupt or payload truncated — \
                 re-check the page writer (write_page_header) and the buffer construction; a page \
                 this crate wrote must round-trip"
            ),
            LevelFreeError::PayloadMismatch { expected, actual } => write!(
                f,
                "page payload is {actual} bytes but rows*width declares {expected}: a PLAIN V2 \
                 payload must be exactly rows*sum(width) bytes — re-check the writer's row-count \
                 and column-width arithmetic"
            ),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for LevelFreeError {}

/// parquet.thrift `Type` codes.
pub mod ptype {
    pub const BOOLEAN: i32 = 0;
    pub const INT32: i32 = 1;
    pub const INT64: i32 = 2;
    pub const INT96: i32 = 3;
    pub const FLOAT: i32 = 4;
    pub const DOUBLE: i32 = 5;
    pub const BYTE_ARRAY: i32 = 6;
    pub const FIXED_LEN_BYTE_ARRAY: i32 = 7;
}

/// parquet.thrift `FieldRepetitionType` codes.
pub mod repetition {
    pub const REQUIRED: i32 = 0;
    pub const OPTIONAL: i32 = 1;
    pub const REPEATED: i32 = 2;
}

/// Admitted physical types with their fixed byte widths.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LevelFreeType {
    /// `INT32` interpreted as unsigned 32-bit (4 bytes).
    U32,
    /// `INT64` interpreted as unsigned 64-bit (8 bytes).
    U64,
    /// `FIXED_LEN_BYTE_ARRAY(32)` (256-bit value).
    Bytes32,
}

impl LevelFreeType {
    /// Fixed byte width of one value.
    pub fn width(self) -> usize {
        match self {
            LevelFreeType::U32 => 4,
            LevelFreeType::U64 => 8,
            LevelFreeType::Bytes32 => 32,
        }
    }

    /// parquet.thrift `Type` code.
    pub fn code(self) -> i32 {
        match self {
            LevelFreeType::U32 => ptype::INT32,
            LevelFreeType::U64 => ptype::INT64,
            LevelFreeType::Bytes32 => ptype::FIXED_LEN_BYTE_ARRAY,
        }
    }
}

/// One admitted column: a REQUIRED flat field of a fixed-width type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LevelFreeColumn {
    pub name: Vec<u8>,
    pub ty: LevelFreeType,
}

/// A schema element as it appears on the wire (parquet.thrift
/// `SchemaElement`, required field ids 1-5 represented). `name` is owned
/// because admission outlives the input slice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WireSchemaElement {
    /// Field 1: `Type` code, absent for groups.
    pub field_type: Option<i32>,
    /// Field 2: `type_length`, meaningful only for FIXED_LEN_BYTE_ARRAY.
    pub type_length: Option<i32>,
    /// Field 3: `FieldRepetitionType`; parquet.thrift semantics treat an
    /// absent repetition as REQUIRED.
    pub repetition_type: Option<i32>,
    /// Field 4 (required): field name.
    pub name: Vec<u8>,
    /// Field 5: child count, nonzero for groups.
    pub num_children: Option<i32>,
}

/// Write a compact-protocol list header for `size` elements of
/// `elem_type` (delta form below 15, explicit varint form at 15+).
fn write_list_header(dst: &mut Vec<u8>, size: usize, elem_type: u8) {
    if size < 15 {
        dst.push(((size as u8) << 4) | elem_type);
    } else {
        dst.push(0xF0 | elem_type);
        crate::thrift::write_zigzag_i32(dst, size as i32);
    }
}

/// Emit the `schema` list contents of a `FileMetaData`: each element as one
/// compact-protocol struct, back to back. The caller wraps with the list
/// header (field 2 of FileMetaData) and the enclosing struct STOP. The
/// root element (flat schema root) must be included by the caller first.
pub fn write_schema_elements(dst: &mut Vec<u8>, cols: &[LevelFreeColumn]) {
    for c in cols {
        let mut prev: i16 = 0;
        write_i32_field(dst, 1, prev, c.ty.code());
        prev = 1;
        if let LevelFreeType::Bytes32 = c.ty {
            write_i32_field(dst, 2, prev, 32);
            prev = 2;
        }
        write_i32_field(dst, 3, prev, repetition::REQUIRED);
        prev = 3;
        write_binary_field(dst, 4, prev, &c.name);
        // Field 5 (num_children) omitted: leaf fields have no children.
        dst.push(ctype::STOP);
    }
}

/// Root element of the flat schema (`schema` list entry 0): no type, no
/// repetition, `num_children = N`.
pub fn write_root_element(dst: &mut Vec<u8>, num_children: usize) {
    write_i32_field(dst, 5, 0, i32::try_from(num_children).unwrap_or(i32::MAX));
    // Field 4 follows field 5 on the wire: the previous-id argument must be
    // 5 or the compact-protocol delta header implies a different field id.
    write_binary_field(dst, 4, 5, b"flat_schema");
    dst.push(ctype::STOP);
}

/// Emit a complete `FileMetaData` field-2 list (header + root + columns).
pub fn write_schema_list(dst: &mut Vec<u8>, cols: &[LevelFreeColumn]) {
    write_list_header(dst, cols.len() + 1, ctype::STRUCT);
    write_root_element(dst, cols.len());
    write_schema_elements(dst, cols);
}

/// Parse a sequence of compact-protocol `SchemaElement` structs from `src`
/// (the contents of a `schema` list). Stops at end of input; a malformed
/// element surfaces the underlying [`ThriftError`].
pub fn parse_schema_elements(src: &[u8]) -> Result<Vec<WireSchemaElement>, ThriftError> {
    let mut out = Vec::new();
    let mut pos = 0usize;
    while pos < src.len() {
        let mut field_type: Option<i32> = None;
        let mut type_length: Option<i32> = None;
        let mut repetition_type: Option<i32> = None;
        let mut name: Option<Vec<u8>> = None;
        let mut num_children: Option<i32> = None;
        let mut prev: i16 = 0;
        loop {
            let Some((field_id, type_id)) = read_field_header(src, &mut pos, prev)? else {
                break;
            };
            prev = field_id;
            match (field_id, type_id) {
                (1, ctype::I32) => field_type = Some(read_i32(src, &mut pos)?),
                (2, ctype::I32) => type_length = Some(read_i32(src, &mut pos)?),
                (3, ctype::I32) => repetition_type = Some(read_i32(src, &mut pos)?),
                (4, ctype::BINARY) => name = Some(read_binary(src, &mut pos)?.to_vec()),
                (5, ctype::I32) => num_children = Some(read_i32(src, &mut pos)?),
                (_, other) => skip_value(src, &mut pos, other, 0)?,
            }
        }
        let name = match name {
            Some(n) => n,
            None => return Err(ThriftError::MissingRequiredField(4)),
        };
        out.push(WireSchemaElement {
            field_type,
            type_length,
            repetition_type,
            name,
            num_children,
        });
    }
    Ok(out)
}

/// Admission check (spec Hook 5): the schema is either exactly the admitted
/// flat fixed-width shape — every leaf REQUIRED, physical type in the
/// admitted set — or a typed refusal naming the offending field.
///
/// `elements` includes the root at index 0 (see [`write_schema_list`]); it
/// may also be the leaf-only slice returned by [`parse_schema_elements`]
/// over leaf structs. A root-shaped element (no type, `num_children > 0`)
/// is consumed as the root and not admitted as a column.
pub fn admit_schema(elements: &[WireSchemaElement]) -> Result<Vec<LevelFreeColumn>, LevelFreeError> {
    let mut cols = Vec::new();
    for (i, e) in elements.iter().enumerate() {
        // Root/group shape: no physical type with children.
        // The schema root is entry 0 with neither a physical type nor a
        // repetition; a group carrying a repetition is a nested field and
        // is refused below, never treated as the root.
        let is_root = i == 0 && e.field_type.is_none() && e.repetition_type.is_none();
        if is_root {
            continue;
        }
        let is_group = e.field_type.is_none() || e.num_children.map(|n| n > 0).unwrap_or(false);
        let name = e.name.clone();
        if is_group {
            return Err(LevelFreeError::NestedField { index: i, name });
        }
        match e.repetition_type {
            None | Some(repetition::REQUIRED) => {}
            Some(repetition::OPTIONAL) => {
                return Err(LevelFreeError::OptionalField { index: i, name })
            }
            Some(repetition::REPEATED) => {
                return Err(LevelFreeError::RepeatedField { index: i, name })
            }
            Some(code) => return Err(LevelFreeError::UnknownRepetition { index: i, name, code }),
        }
        let ty = match e.field_type {
            Some(ptype::INT32) => LevelFreeType::U32,
            Some(ptype::INT64) => LevelFreeType::U64,
            Some(ptype::FIXED_LEN_BYTE_ARRAY) => {
                if e.type_length == Some(32) {
                    LevelFreeType::Bytes32
                } else {
                    return Err(LevelFreeError::UnsupportedWidth {
                        index: i,
                        name,
                        length: e.type_length.unwrap_or(-1),
                    });
                }
            }
            Some(code) => {
                return Err(LevelFreeError::UnsupportedType {
                    index: i,
                    name,
                    code,
                })
            }
            None => return Err(LevelFreeError::NestedField { index: i, name }),
        };
        cols.push(LevelFreeColumn { name, ty });
    }
    Ok(cols)
}

/// Round `rows` up so `rows * width` is a multiple of 256 (the cgap
/// storage-plane block grid; spec "Row-group sizing"). `width` must be
/// nonzero (all admitted widths are).
pub fn align_rows_256b(rows: usize, width: usize) -> usize {
    let bytes = rows * width;
    let padded = bytes.div_ceil(256) * 256;
    padded / width
}

/// Write one level-free `DataPageV2`: compact-protocol `PageHeader` (V2
/// subheader with `num_nulls = 0`, zero-length rep/def level buffers,
/// `PLAIN`, uncompressed) followed by the contiguous fixed-width payload.
///
/// `payload.len()` must equal `num_rows * width` — the 1:1 Arrow-buffer
/// mapping (spec Hook 2); a mismatch is a typed error, never a silent
/// truncate.
pub fn write_page(
    ty: LevelFreeType,
    payload: &[u8],
    num_rows: usize,
) -> Result<Vec<u8>, LevelFreeError> {
    let width = ty.width();
    let expected = num_rows * width;
    if payload.len() != expected {
        return Err(LevelFreeError::PayloadMismatch { expected, actual: payload.len() });
    }
    let header = PageHeader {
        page_type: page_type::DATA_PAGE_V2,
        uncompressed_page_size: size_i32(payload.len()),
        compressed_page_size: size_i32(payload.len()),
        data_page_header_v2: Some(DataPageHeaderV2 {
            num_values: size_i32(num_rows),
            num_nulls: 0,
            num_rows: size_i32(num_rows),
            definition_levels_byte_length: 0,
            repetition_levels_byte_length: 0,
            is_compressed: false,
            encoding: encoding::PLAIN,
        }),
    };
    let mut out = Vec::with_capacity(payload.len() + 64);
    write_page_header(&mut out, &header);
    out.extend_from_slice(payload);
    Ok(out)
}

// ---------------------------------------------------------------------------
// Multi-column (row-group) writer + FileMetaData footer writer
// ---------------------------------------------------------------------------
//
// Wave-4 lane 16 documented the ncols > 1 mismatch: [`write_page`] is a
// per-column writer (`num_values = num_rows`, payload checked against ONE
// column's `rows * width`), while a row-group page carries every column of
// the schema (`num_values = num_rows * ncols`, payload `rows * Σwidth`).
// [`write_row_group_page`] closes the writer half of that gap; the scan side
// (`crate::scan`) already parses it. The footer writer ([`write_footer`])
// emits the `FileMetaData` struct — schema via [`write_schema_list`], one
// `RowGroup` per entry, each row group carrying one `ColumnChunk` per column
// whose `ColumnMetaData` points at the shared page — completing the
// writer/reader isomorphism for the whole file, not just the page.

fn size_i32(v: usize) -> i32 {
    i32::try_from(v).unwrap_or(i32::MAX)
}

/// One row group's footer facts: the file offset its shared DataPageV2
/// starts at, and the page's byte length (header + full column-major
/// payload). Every column chunk of the group points at the same page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FooterRowGroup {
    /// File offset of the group's page (e.g. `4`, just after `PAR1`).
    pub page_offset: i64,
    /// Total page byte length (`header_len + rows * Σwidth`).
    pub page_len: i64,
}

/// Write one row-group page: the column-major concat of every column's
/// payload under a single `DataPageV2` header whose `num_values` is
/// `num_rows * columns.len()` and whose `num_rows` is `num_rows`.
///
/// Each `payloads[i]` must equal `num_rows * columns[i].ty.width()` bytes —
/// the per-column 1:1 Arrow-buffer mapping (spec Hook 2) — with the two
/// slice lengths taken in the same column order. A mismatch is a typed
/// [`LevelFreeError::PayloadMismatch`] naming the expected/actual byte
/// counts, never a silent truncate or reordering.
pub fn write_row_group_page(
    cols: &[LevelFreeColumn],
    payloads: &[&[u8]],
    num_rows: usize,
) -> Result<Vec<u8>, LevelFreeError> {
    if cols.len() != payloads.len() {
        return Err(LevelFreeError::PayloadMismatch {
            expected: cols.len(),
            actual: payloads.len(),
        });
    }
    let mut body = Vec::new();
    for (c, p) in cols.iter().zip(payloads.iter()) {
        let expected = num_rows.saturating_mul(c.ty.width());
        if p.len() != expected {
            return Err(LevelFreeError::PayloadMismatch {
                expected,
                actual: p.len(),
            });
        }
        body.extend_from_slice(p);
    }
    let ncols = cols.len();
    let header = PageHeader {
        page_type: page_type::DATA_PAGE_V2,
        uncompressed_page_size: size_i32(body.len()),
        compressed_page_size: size_i32(body.len()),
        data_page_header_v2: Some(DataPageHeaderV2 {
            num_values: size_i32(num_rows.saturating_mul(ncols)),
            num_nulls: 0,
            num_rows: size_i32(num_rows),
            definition_levels_byte_length: 0,
            repetition_levels_byte_length: 0,
            is_compressed: false,
            encoding: encoding::PLAIN,
        }),
    };
    let mut out = Vec::with_capacity(body.len() + 64);
    write_page_header(&mut out, &header);
    out.extend_from_slice(&body);
    Ok(out)
}

/// Emit one `ColumnMetaData` struct (parquet.thrift field ids: 1 `type`,
/// 5 `num_values`, 6 `total_uncompressed_size`, 7 `total_compressed_size`,
/// 9 `data_page_offset`) for column `col` of a row group whose shared page
/// sits at `page_offset` with byte length `page_len`. The scan side
/// (`crate::scan::read_column_meta_data`) loads exactly field 9.
fn write_column_meta_data(dst: &mut Vec<u8>, col: &LevelFreeColumn, num_rows: usize, page_offset: i64, page_len: i64) {
    let mut prev: i16 = 0;
    write_i32_field(dst, 1, prev, col.ty.code());
    prev = 1;
    write_i64_field(dst, 5, prev, size_i64(num_rows));
    prev = 5;
    write_i64_field(dst, 6, prev, page_len);
    prev = 6;
    write_i64_field(dst, 7, prev, page_len);
    prev = 7;
    write_i64_field(dst, 9, prev, page_offset);
    dst.push(ctype::STOP);
}

fn size_i64(v: usize) -> i64 {
    i64::try_from(v).unwrap_or(i64::MAX)
}

/// Emit one `RowGroup` struct: a `columns` list (field 1) holding one
/// `ColumnChunk` per column (each pointing its `ColumnMetaData` at the
/// shared page), `total_byte_size` (field 2), `num_rows` (field 3).
fn write_row_group(dst: &mut Vec<u8>, cols: &[LevelFreeColumn], num_rows: usize, rg: &FooterRowGroup) {
    write_field_header(dst, 1, 0, ctype::LIST);
    write_list_header(dst, cols.len(), ctype::STRUCT);
    for c in cols {
        // ColumnChunk: file_offset (field 2), meta_data (field 3).
        write_i64_field(dst, 2, 0, rg.page_offset);
        write_field_header(dst, 3, 2, ctype::STRUCT);
        write_column_meta_data(dst, c, num_rows, rg.page_offset, rg.page_len);
        dst.push(ctype::STOP); // end ColumnChunk
    }
    write_i64_field(dst, 2, 1, rg.page_len);
    write_i64_field(dst, 3, 2, size_i64(num_rows));
    dst.push(ctype::STOP); // end RowGroup
}

/// Emit a complete `FileMetaData` struct (parquet.thrift field ids:
/// 1 `version`, 2 `schema`, 3 `num_rows`, 4 `row_groups`). The schema list
/// is [`write_schema_list`] (root + REQUIRED leaves); every row group's
/// per-column chunks point at their shared pages. The caller frames the
/// result into the file (`PAR1 | pages | meta | len(u32 LE) | PAR1`), or
/// uses [`write_file`] which does it.
pub fn write_footer(
    cols: &[LevelFreeColumn],
    num_rows: usize,
    row_groups: &[FooterRowGroup],
) -> Vec<u8> {
    let mut out = Vec::new();
    write_i32_field(&mut out, 1, 0, 1); // version
    write_field_header(&mut out, 2, 1, ctype::LIST);
    write_schema_list(&mut out, cols);
    write_i64_field(&mut out, 3, 2, size_i64(num_rows));
    write_field_header(&mut out, 4, 3, ctype::LIST);
    write_list_header(&mut out, row_groups.len(), ctype::STRUCT);
    for rg in row_groups {
        write_row_group(&mut out, cols, num_rows, rg);
    }
    out.push(ctype::STOP); // end FileMetaData
    out
}

/// Assemble a whole single-row-group level-free Parquet file:
/// `PAR1 | row-group page | FileMetaData | len(u32 LE) | PAR1`. The page is
/// [`write_row_group_page`]'s output; the footer's single row group points
/// at offset 4 (immediately after the leading magic). The byte-exact
/// counterpart check lives in the scan side's `ParquetFile::open`.
pub fn write_file(
    cols: &[LevelFreeColumn],
    payloads: &[&[u8]],
    num_rows: usize,
) -> Result<Vec<u8>, LevelFreeError> {
    let page = write_row_group_page(cols, payloads, num_rows)?;
    let footer = write_footer(
        cols,
        num_rows,
        &[FooterRowGroup { page_offset: 4, page_len: page.len() as i64 }],
    );
    let mut out = Vec::with_capacity(4 + page.len() + footer.len() + 8);
    out.extend_from_slice(b"PAR1");
    out.extend_from_slice(&page);
    out.extend_from_slice(&footer);
    out.extend_from_slice(&(footer.len() as u32).to_le_bytes());
    out.extend_from_slice(b"PAR1");
    Ok(out)
}

/// Parse a written page back: returns `(payload_slice, num_rows)`. The
/// payload borrows from `page` (zero-copy, PLAIN path). Every page-shape
/// expectation of the projection is re-asserted on the way in: V2 page
/// type, zero level byte lengths, uncompressed, PLAIN.
pub fn read_page_payload(page: &[u8], ty: LevelFreeType) -> Result<(&[u8], usize), LevelFreeError> {
    let (header, header_len) = crate::thrift::read_page_header(page)
        .map_err(|_| LevelFreeError::UndecodablePage)?;
    let width = ty.width();
    let v2 = header.data_page_header_v2.ok_or(LevelFreeError::UndecodablePage)?;
    if header.page_type != page_type::DATA_PAGE_V2
        || v2.definition_levels_byte_length != 0
        || v2.repetition_levels_byte_length != 0
        || v2.is_compressed
        || v2.encoding != encoding::PLAIN
    {
        return Err(LevelFreeError::UndecodablePage);
    }
    if v2.num_nulls != 0 || v2.num_values != v2.num_rows {
        return Err(LevelFreeError::UndecodablePage);
    }
    let rows = usize::try_from(v2.num_rows).map_err(|_| LevelFreeError::UndecodablePage)?;
    let payload = page.get(header_len..).ok_or(LevelFreeError::UndecodablePage)?;
    if payload.len() != rows * width {
        return Err(LevelFreeError::PayloadMismatch {
            expected: rows * width,
            actual: payload.len(),
        });
    }
    Ok((payload, rows))
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn elem(name: &[u8], ty: Option<i32>, rep: Option<i32>, tlen: Option<i32>) -> WireSchemaElement {
        WireSchemaElement { field_type: ty, type_length: tlen, repetition_type: rep, name: name.to_vec(), num_children: None }
    }

    #[test]
    fn schema_wire_round_trip_and_admission() {
        let cols = vec![
            LevelFreeColumn { name: b"a_u32".to_vec(), ty: LevelFreeType::U32 },
            LevelFreeColumn { name: b"b_u64".to_vec(), ty: LevelFreeType::U64 },
            LevelFreeColumn { name: b"h_256".to_vec(), ty: LevelFreeType::Bytes32 },
        ];
        let mut wire = Vec::new();
        write_schema_list(&mut wire, &cols);
        // Drop the list header byte; parse root + leaves together. The
        // admission step must consume the root and admit the 3 leaves.
        let parsed = parse_schema_elements(&wire[1..]).expect("parse");
        assert_eq!(parsed.len(), 4);
        let admitted = admit_schema(&parsed).expect("admit");
        assert_eq!(admitted, cols);
    }

    #[test]
    fn refuses_optional_field() {
        let els = vec![elem(b"opt", Some(ptype::INT32), Some(repetition::OPTIONAL), None)];
        assert_eq!(
            admit_schema(&els),
            Err(LevelFreeError::OptionalField { index: 0, name: b"opt".to_vec() })
        );
    }

    #[test]
    fn refuses_repeated_field() {
        let els = vec![elem(b"rep", Some(ptype::INT64), Some(repetition::REPEATED), None)];
        assert_eq!(
            admit_schema(&els),
            Err(LevelFreeError::RepeatedField { index: 0, name: b"rep".to_vec() })
        );
    }

    #[test]
    fn refuses_nested_field() {
        let els = vec![WireSchemaElement {
            field_type: None,
            type_length: None,
            repetition_type: Some(repetition::REQUIRED),
            name: b"grp".to_vec(),
            num_children: Some(2),
        }];
        assert_eq!(
            admit_schema(&els),
            Err(LevelFreeError::NestedField { index: 0, name: b"grp".to_vec() })
        );
    }

    #[test]
    fn refuses_unsupported_type_and_width() {
        let els = vec![elem(b"f", Some(ptype::DOUBLE), Some(repetition::REQUIRED), None)];
        assert!(matches!(
            admit_schema(&els),
            Err(LevelFreeError::UnsupportedType { code: ptype::DOUBLE, .. })
        ));
        let els = vec![elem(b"g", Some(ptype::FIXED_LEN_BYTE_ARRAY), Some(repetition::REQUIRED), Some(16))];
        assert!(matches!(
            admit_schema(&els),
            Err(LevelFreeError::UnsupportedWidth { length: 16, .. })
        ));
    }

    #[test]
    fn page_round_trip_u32_u64_bytes32_zero_levels() {
        for (ty, gen) in [
            (LevelFreeType::U32, 4usize),
            (LevelFreeType::U64, 8),
            (LevelFreeType::Bytes32, 32),
        ] {
            let rows = 10usize;
            let payload: Vec<u8> = (0..rows * gen).map(|i| (i * 7 + 3) as u8).collect();
            let page = write_page(ty, &payload, rows).expect("write");
            let (header, header_len) = crate::thrift::read_page_header(&page).expect("hdr");
            let v2 = header.data_page_header_v2.expect("v2");
            // Hook 1: zero levels, no compression, PLAIN.
            assert_eq!(header.page_type, page_type::DATA_PAGE_V2);
            assert_eq!(v2.definition_levels_byte_length, 0);
            assert_eq!(v2.repetition_levels_byte_length, 0);
            assert!(!v2.is_compressed);
            assert_eq!(v2.encoding, encoding::PLAIN);
            assert_eq!(v2.num_nulls, 0);
            assert_eq!(v2.num_values as usize, rows);
            assert_eq!(v2.num_rows as usize, rows);
            // Hook 2 (PLAIN): payload bytes ARE the input bytes.
            let (got, got_rows) = read_page_payload(&page, ty).expect("payload");
            assert_eq!(got_rows, rows);
            assert_eq!(got, &payload[..]);
            assert_eq!(page.len(), header_len + rows * gen);
        }
    }

    #[test]
    fn payload_length_mismatch_is_typed() {
        let payload = vec![0u8; 40];
        assert_eq!(
            write_page(LevelFreeType::U32, &payload, 9),
            Err(LevelFreeError::PayloadMismatch { expected: 36, actual: 40 })
        );
    }

    #[test]
    fn row_group_256b_alignment_smoke() {
        // widths: u32=4, u64=8, bytes32=32 — all divide 256, so alignment
        // pads rows only when rows*width misses the grid.
        for width in [4usize, 8, 32] {
            let aligned = align_rows_256b(100, width);
            assert_eq!(aligned * width % 256, 0, "width {width}");
            assert!(aligned >= 100);
            let one_row = align_rows_256b(1, width);
            assert_eq!(one_row * width, 256, "width {width}");
        }
        // 100 u32 rows = 400 bytes -> padded to 512 -> 128 rows.
        assert_eq!(align_rows_256b(100, 4), 128);
    }
}
