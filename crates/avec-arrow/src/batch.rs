//! Named column batches: the record-batch layer over the vector builders
//! (consumer ticket T1).
//!
//! A [`Batch`] is a named, row-count-synchronized column set. Every column
//! is a builder-produced array (the 64-byte alignment invariant holds by
//! construction); [`Batch::try_new`] refuses ragged columns and duplicate
//! names, so a finished batch always denotes a rectangular table.
//!
//! Determinism: column order is insertion order (`Vec`), names are
//! `&'static str`, and the length law is enforced at construction — never
//! repaired.

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use crate::primitive::{Primitive, PrimitiveBuilder, Scalar};
use crate::runend::{RunEndEncoded, RunEndEncodedBuilder};
use crate::varlen::{Varlen, VarlenBuilder};
use crate::views::ViewVec;

/// One column of a [`Batch`]: a typed array. Closed over the substrate's
/// array families; the EAV consumers project one `Batch` per table.
#[derive(Debug)]
pub enum Column {
    /// `Primitive<i8>` ("c").
    Int8(Primitive<i8>),
    /// `Primitive<u8>` ("C").
    UInt8(Primitive<u8>),
    /// `Primitive<i16>` ("s").
    Int16(Primitive<i16>),
    /// `Primitive<u16>` ("S").
    UInt16(Primitive<u16>),
    /// `Primitive<i32>` ("i").
    Int32(Primitive<i32>),
    /// `Primitive<u32>` ("I").
    UInt32(Primitive<u32>),
    /// `Primitive<i64>` ("l").
    Int64(Primitive<i64>),
    /// `Primitive<u64>` ("L").
    UInt64(Primitive<u64>),
    /// `Primitive<f32>` ("f").
    Float32(Primitive<f32>),
    /// `Primitive<f64>` ("g").
    Float64(Primitive<f64>),
    /// `Varlen<i32>` ("u").
    Utf8(Varlen<i32>),
    /// `Varlen<i64>` ("U").
    LargeUtf8(Varlen<i64>),
    /// `ViewVec` ("vu").
    Utf8View(ViewVec),
    /// `RunEndEncoded<i32>` ("+r").
    RunEnd(RunEndEncoded<i32>),
}

impl Column {
    /// The column's row count.
    #[must_use]
    pub fn len(&self) -> usize {
        match self {
            Column::Int8(a) => a.len(),
            Column::UInt8(a) => a.len(),
            Column::Int16(a) => a.len(),
            Column::UInt16(a) => a.len(),
            Column::Int32(a) => a.len(),
            Column::UInt32(a) => a.len(),
            Column::Int64(a) => a.len(),
            Column::UInt64(a) => a.len(),
            Column::Float32(a) => a.len(),
            Column::Float64(a) => a.len(),
            Column::Utf8(a) => a.len(),
            Column::LargeUtf8(a) => a.len(),
            Column::Utf8View(a) => a.len(),
            Column::RunEnd(a) => a.logical_len(),
        }
    }

    /// True when the column has zero rows.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The Arrow format code of the column's element type.
    #[must_use]
    pub fn format_code(&self) -> &'static str {
        match self {
            Column::Int8(_) => i8::FORMAT_CODE,
            Column::UInt8(_) => u8::FORMAT_CODE,
            Column::Int16(_) => i16::FORMAT_CODE,
            Column::UInt16(_) => u16::FORMAT_CODE,
            Column::Int32(_) => i32::FORMAT_CODE,
            Column::UInt32(_) => u32::FORMAT_CODE,
            Column::Int64(_) => i64::FORMAT_CODE,
            Column::UInt64(_) => u64::FORMAT_CODE,
            Column::Float32(_) => f32::FORMAT_CODE,
            Column::Float64(_) => f64::FORMAT_CODE,
            Column::Utf8(_) => "u",
            Column::LargeUtf8(_) => "U",
            Column::Utf8View(_) => "vu",
            Column::RunEnd(_) => "+r",
        }
    }
}

/// Errors from batch construction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BatchError {
    /// Two columns share one name.
    DuplicateColumn {
        /// The duplicated name.
        name: String,
    },
    /// A column's row count differs from the batch's row count.
    LengthMismatch {
        /// The offending column's name.
        column: String,
        /// The batch's row count (the first column's).
        expected: usize,
        /// The offending column's row count.
        got: usize,
    },
}

impl fmt::Display for BatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BatchError::DuplicateColumn { name } => write!(f, "duplicate column {name:?}"),
            BatchError::LengthMismatch { column, expected, got } => write!(
                f,
                "column {column:?} has {got} rows; batch has {expected}"
            ),
        }
    }
}

/// A named, row-count-synchronized column set.
#[derive(Debug)]
pub struct Batch {
    columns: Vec<(&'static str, Column)>,
    row_count: usize,
}

impl Batch {
    /// Construct, refusing duplicate names and ragged lengths.
    ///
    /// # Errors
    /// [`BatchError::DuplicateColumn`] or [`BatchError::LengthMismatch`].
    pub fn try_new(columns: Vec<(&'static str, Column)>) -> Result<Self, BatchError> {
        let row_count = columns.first().map_or(0, |(_, c)| c.len());
        for (index, (name, column)) in columns.iter().enumerate() {
            if columns[..index]
                .iter()
                .any(|(prior, _)| *prior == *name)
            {
                return Err(BatchError::DuplicateColumn {
                    name: String::from(*name),
                });
            }
            if column.len() != row_count {
                return Err(BatchError::LengthMismatch {
                    column: String::from(*name),
                    expected: row_count,
                    got: column.len(),
                });
            }
        }
        Ok(Self {
            columns,
            row_count,
        })
    }

    /// The row count.
    #[must_use]
    pub fn row_count(&self) -> usize {
        self.row_count
    }

    /// The named columns, insertion order.
    #[must_use]
    pub fn columns(&self) -> &[(&'static str, Column)] {
        &self.columns
    }

    /// One column by name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Column> {
        self.columns
            .iter()
            .find(|(column_name, _)| *column_name == name)
            .map(|(_, column)| column)
    }
}

/// A builder mirror of [`Column`]: assemble named columns, then
/// [`BatchBuilder::finish`] freezes them under [`Batch`]'s length law.
#[derive(Default)]
pub struct BatchBuilder {
    columns: Vec<(&'static str, ColumnBuilder)>,
}

/// The per-column builders, mirroring [`Column`]'s families.
pub enum ColumnBuilder {
    /// `PrimitiveBuilder<i8>`.
    Int8(PrimitiveBuilder<i8>),
    /// `PrimitiveBuilder<u8>`.
    UInt8(PrimitiveBuilder<u8>),
    /// `PrimitiveBuilder<i16>`.
    Int16(PrimitiveBuilder<i16>),
    /// `PrimitiveBuilder<u16>`.
    UInt16(PrimitiveBuilder<u16>),
    /// `PrimitiveBuilder<i32>`.
    Int32(PrimitiveBuilder<i32>),
    /// `PrimitiveBuilder<u32>`.
    UInt32(PrimitiveBuilder<u32>),
    /// `PrimitiveBuilder<i64>`.
    Int64(PrimitiveBuilder<i64>),
    /// `PrimitiveBuilder<u64>`.
    UInt64(PrimitiveBuilder<u64>),
    /// `PrimitiveBuilder<f32>`.
    Float32(PrimitiveBuilder<f32>),
    /// `PrimitiveBuilder<f64>`.
    Float64(PrimitiveBuilder<f64>),
    /// `VarlenBuilder<i32>`.
    Utf8(VarlenBuilder<i32>),
    /// `VarlenBuilder<i64>`.
    LargeUtf8(VarlenBuilder<i64>),
    /// `ViewVecBuilder`.
    Utf8View(ViewVecBuilder),
    /// `RunEndEncodedBuilder<i32>`.
    RunEnd(RunEndEncodedBuilder<i32>),
}

impl ColumnBuilder {
    /// Rows appended so far.
    #[must_use]
    pub fn len(&self) -> usize {
        match self {
            ColumnBuilder::Int8(b) => b.len(),
            ColumnBuilder::UInt8(b) => b.len(),
            ColumnBuilder::Int16(b) => b.len(),
            ColumnBuilder::UInt16(b) => b.len(),
            ColumnBuilder::Int32(b) => b.len(),
            ColumnBuilder::UInt32(b) => b.len(),
            ColumnBuilder::Int64(b) => b.len(),
            ColumnBuilder::UInt64(b) => b.len(),
            ColumnBuilder::Float32(b) => b.len(),
            ColumnBuilder::Float64(b) => b.len(),
            ColumnBuilder::Utf8(b) => b.len(),
            ColumnBuilder::LargeUtf8(b) => b.len(),
            ColumnBuilder::Utf8View(b) => b.len(),
            ColumnBuilder::RunEnd(b) => b.len(),
        }
    }

    fn finish(&self) -> Column {
        match self {
            ColumnBuilder::Int8(b) => Column::Int8(b.finish()),
            ColumnBuilder::UInt8(b) => Column::UInt8(b.finish()),
            ColumnBuilder::Int16(b) => Column::Int16(b.finish()),
            ColumnBuilder::UInt16(b) => Column::UInt16(b.finish()),
            ColumnBuilder::Int32(b) => Column::Int32(b.finish()),
            ColumnBuilder::UInt32(b) => Column::UInt32(b.finish()),
            ColumnBuilder::Int64(b) => Column::Int64(b.finish()),
            ColumnBuilder::UInt64(b) => Column::UInt64(b.finish()),
            ColumnBuilder::Float32(b) => Column::Float32(b.finish()),
            ColumnBuilder::Float64(b) => Column::Float64(b.finish()),
            ColumnBuilder::Utf8(b) => Column::Utf8(b.finish()),
            ColumnBuilder::LargeUtf8(b) => Column::LargeUtf8(b.finish()),
            ColumnBuilder::Utf8View(b) => Column::Utf8View(b.finish()),
            ColumnBuilder::RunEnd(b) => Column::RunEnd(b.finish()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Three typed columns built through the batch builder; the frozen
    /// batch preserves names, insertion order, format codes, and values.
    #[test]
    fn named_batch_round_trip() {
        let mut b = BatchBuilder::new();
        b = b
            .with_column("id", ColumnBuilder::Int32(PrimitiveBuilder::new()))
            .with_column("score", ColumnBuilder::Float64(PrimitiveBuilder::new()))
            .with_column("name", ColumnBuilder::Utf8(VarlenBuilder::new()));
        if let Some(ColumnBuilder::Int32(ids)) = b.column_mut("id") {
            ids.push(Some(1));
            ids.push(Some(2));
        }
        if let Some(ColumnBuilder::Float64(scores)) = b.column_mut("score") {
            scores.push(Some(0.5));
            scores.push(None);
        }
        if let Some(ColumnBuilder::Utf8(names)) = b.column_mut("name") {
            names.push(Some(b"ada"));
            names.push(Some(b"grace"));
        }
        let batch = b.finish().unwrap();
        assert_eq!(batch.row_count(), 2);
        let names: Vec<&str> = batch.columns().iter().map(|(n, _)| *n).collect();
        assert_eq!(names, ["id", "score", "name"]);
        match batch.get("id") {
            Some(Column::Int32(v)) => assert_eq!(v.values(), &[1, 2]),
            other => panic!("expected i32 column, got {other:?}"),
        }
        assert!(matches!(batch.get("score"), Some(Column::Float64(_))));
        match batch.get("name") {
            Some(Column::Utf8(v)) => {
                assert_eq!(v.row(0), b"ada");
                assert_eq!(v.row(1), b"grace");
            }
            other => panic!("expected utf8 column, got {other:?}"),
        }
    }

    #[test]
    fn ragged_columns_are_refused() {
        let mut b = BatchBuilder::new();
        b = b
            .with_column("a", ColumnBuilder::Int32(PrimitiveBuilder::new()))
            .with_column("b", ColumnBuilder::Int64(PrimitiveBuilder::new()));
        if let Some(ColumnBuilder::Int32(a)) = b.column_mut("a") {
            a.push(Some(1));
        }
        if let Some(ColumnBuilder::Int64(bs)) = b.column_mut("b") {
            bs.push(Some(1));
            bs.push(Some(2));
        }
        match b.finish() {
            Err(BatchError::LengthMismatch { column, expected, got }) => {
                assert_eq!(column, "b");
                assert_eq!((expected, got), (1, 2));
            }
            other => panic!("expected length mismatch, got {other:?}"),
        }
    }

    #[test]
    fn duplicate_names_are_refused() {
        let batch = Batch::try_new(vec![
            ("x", Column::Int32(PrimitiveBuilder::<i32>::new().finish())),
            ("x", Column::Int64(PrimitiveBuilder::<i64>::new().finish())),
        ]);
        match batch {
            Err(BatchError::DuplicateColumn { name }) => assert_eq!(name, "x"),
            other => panic!("expected duplicate refusal, got {other:?}"),
        }
    }

    #[test]
    fn format_codes_round_trip() {
        let batch = Batch::try_new(vec![
            ("i", Column::Int32(PrimitiveBuilder::<i32>::new().finish())),
            ("u", Column::Utf8(VarlenBuilder::<i32>::new().finish())),
            (
                "vu",
                Column::Utf8View(ViewVecBuilder::new().finish()),
            ),
            (
                "+r",
                Column::RunEnd(RunEndEncodedBuilder::<i32>::new().finish()),
            ),
        ])
        .unwrap();
        let codes: Vec<&'static str> = batch
            .columns()
            .iter()
            .map(|(_, c)| c.format_code())
            .collect();
        assert_eq!(codes, ["i", "u", "vu", "+r"]);
    }
}

impl BatchBuilder {
    /// An empty builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add one named column builder (any subtype).
    #[must_use]
    pub fn with_column(mut self, name: &'static str, builder: ColumnBuilder) -> Self {
        self.columns.push((name, builder));
        self
    }

    /// A named column's builder, for pushes.
    #[must_use]
    pub fn column_mut(&mut self, name: &str) -> Option<&mut ColumnBuilder> {
        self.columns
            .iter_mut()
            .find(|(column_name, _)| *column_name == name)
            .map(|(_, builder)| builder)
    }

    /// Rows appended so far (the first column's count).
    #[must_use]
    pub fn len(&self) -> usize {
        self.columns.first().map_or(0, |(_, b)| b.len())
    }

    /// True when no columns are attached.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.columns.is_empty()
    }

    /// Freeze every column and apply [`Batch`]'s law.
    ///
    /// # Errors
    /// [`BatchError::DuplicateColumn`] or [`BatchError::LengthMismatch`].
    pub fn finish(&self) -> Result<Batch, BatchError> {
        let columns = self
            .columns
            .iter()
            .map(|(name, builder)| (*name, builder.finish()))
            .collect();
        Batch::try_new(columns)
    }
}
