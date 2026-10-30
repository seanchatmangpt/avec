//! Typed zero-copy ingest of foreign C Data Interface pairs (ticket T2).
//!
//! The C Data Interface's importer duty: the importer NEVER owns the
//! buffers and must not run the foreign `release` while any view is alive.
//! These helpers produce lifetime-free typed views over the foreign
//! buffers; the caller (who received the pair) owns that timing contract.
//! Buffer byte lengths are NOT carried by the interface — the caller
//! supplies them from out-of-band knowledge (the spec's documented gap),
//! and every view is bounds- and alignment-checked against that supply.
//!
//! Provenance note: new module (ticket T2), sitting beside `c_abi.rs`'s
//! raw [`ForeignArray`] and reusing [`crate::bitmap::get_bit`] for the
//! validity walk.

use crate::c_abi::{ArrowArray, ArrowSchema};
use crate::primitive::Scalar;
use alloc::string::String;
use core::ffi::{c_char, CStr};
use core::fmt;
use core::slice;

/// Why a foreign pair refuses a typed view.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ForeignError {
    /// The schema's format string is not what the view type demands.
    FormatMismatch {
        /// The element type's format code.
        expected: &'static str,
        /// The schema's actual format string.
        found: String,
    },
    /// The schema format string is not valid UTF-8 or is null.
    InvalidFormat,
    /// The array's buffer slot is absent (n_buffers too small) or null.
    MissingBuffer {
        /// The buffer slot index.
        index: usize,
    },
    /// The caller-supplied buffer byte length cannot hold the view.
    BufferTooShort {
        /// Bytes required (`len * WIDTH` or the bitmap length).
        required: usize,
        /// Bytes the caller supplied.
        got: usize,
    },
    /// The foreign buffer does not satisfy `<T as Scalar>`'s alignment.
    Misaligned {
        /// Required alignment.
        required: usize,
        /// Observed pointer value.
        got: usize,
    },
    /// The array's length is negative.
    NegativeLength {
        /// The observed length.
        got: i64,
    },
}

impl fmt::Display for ForeignError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ForeignError::FormatMismatch { expected, found } => {
                write!(f, "format {found:?} does not match {expected:?}")
            }
            ForeignError::InvalidFormat => f.write_str("schema format is null or not UTF-8"),
            ForeignError::MissingBuffer { index } => write!(f, "buffer slot {index} missing"),
            ForeignError::BufferTooShort { required, got } => {
                write!(f, "buffer holds {got} bytes; view needs {required}")
            }
            ForeignError::Misaligned { required, got } => {
                write!(f, "buffer at {got:#x} is not {required}-aligned")
            }
            ForeignError::NegativeLength { got } => write!(f, "array length is negative: {got}"),
        }
    }
}

/// The schema's format string, borrowed for the schema's lifetime.
///
/// # Safety
/// `schema` must point to a live, initialized `ArrowSchema` whose `format`
/// is either null or a valid NUL-terminated string, untouched by release
/// for the duration of `'a`.
pub unsafe fn format_of<'a>(schema: *const ArrowSchema) -> Result<&'a str, ForeignError> {
    let raw = unsafe { (*schema).format };
    if raw.is_null() {
        return Err(ForeignError::InvalidFormat);
    }
    let bytes = unsafe { CStr::from_ptr(raw) }.to_bytes();
    core::str::from_utf8(bytes).map_err(|_| ForeignError::InvalidFormat)
}

/// Typed view over a foreign array's value buffer (buffer slot 1 for the
/// two-buffer primitive layout).
///
/// # Safety
/// Same contract as [`format_of`], plus: `array` points to a live
/// `ArrowArray` (not yet released, `length >= 0`), the caller's
/// `values_buffer_byte_len` is the true byte length of buffer slot 1, and
/// no release runs while the returned slice is alive.
pub unsafe fn primitive_view<'a, T: Scalar>(
    schema: *const ArrowSchema,
    array: *const ArrowArray,
    values_buffer_byte_len: usize,
) -> Result<&'a [T], ForeignError> {
    let found = unsafe { format_of(schema) }?;
    if found != T::FORMAT_CODE {
        return Err(ForeignError::FormatMismatch {
            expected: T::FORMAT_CODE,
            found: String::from(found),
        });
    }
    let array = unsafe { &*array };
    if (array.n_buffers as usize) < 2 {
        return Err(ForeignError::MissingBuffer { index: 1 });
    }
    if array.length < 0 {
        return Err(ForeignError::NegativeLength { got: array.length });
    }
    let len = array.length as usize;
    let required = len.saturating_mul(T::WIDTH);
    if values_buffer_byte_len < required {
        return Err(ForeignError::BufferTooShort {
            required,
            got: values_buffer_byte_len,
        });
    }
    let slot = unsafe { *array.buffers.add(1) };
    if slot.is_null() {
        return Err(ForeignError::MissingBuffer { index: 1 });
    }
    let ptr = slot as *const T;
    let align = core::mem::align_of::<T>();
    if (ptr as usize) % align != 0 {
        return Err(ForeignError::Misaligned {
            required: align,
            got: ptr as usize,
        });
    }
    // SAFETY (within this unsafe fn): the format matched T, the buffer is
    // non-null, bounds-checked against the caller's supplied length, and
    // alignment-checked; the caller guarantees liveness for 'a.
    Ok(unsafe { slice::from_raw_parts(ptr, len) })
}

/// Whether one row is valid, over a foreign validity bitmap (buffer slot
/// 0, absent or null when the producer emitted no bitmap = all valid).
///
/// # Safety
/// `array` points to a live `ArrowArray`; `bitmap_byte_len` is the true
/// byte length of buffer slot 0 when present.
pub unsafe fn is_valid(
    array: *const ArrowArray,
    bitmap_byte_len: usize,
    row: usize,
) -> Result<bool, ForeignError> {
    let array = unsafe { &*array };
    if (array.n_buffers as usize) < 1 {
        return Err(ForeignError::MissingBuffer { index: 0 });
    }
    let slot = unsafe { *array.buffers };
    if slot.is_null() {
        // Arrow law: absent validity buffer = all valid.
        return Ok(true);
    }
    let needed = row / 8 + 1;
    if bitmap_byte_len < needed {
        return Err(ForeignError::BufferTooShort {
            required: needed,
            got: bitmap_byte_len,
        });
    }
    if row >= usize::try_from(array.length.max(0)).unwrap_or(usize::MAX) {
        return Ok(false);
    }
    let bytes = unsafe { slice::from_raw_parts(slot as *const u8, bitmap_byte_len) };
    Ok(crate::bitmap::get_bit(bytes, row) == 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::c_abi::export;
    use crate::primitive::PrimitiveBuilder;
    use alloc::vec::Vec;

    /// Build a nullable i32 array, export it through the C Data Interface,
    /// and read it back through the typed foreign view: values and
    /// validity agree with the source. Exercises format match, bounds, and
    /// the validity walk on a real pair.
    #[test]
    fn export_view_round_trip() {
        let mut b = PrimitiveBuilder::<i32>::new();
        b.push(Some(7));
        b.push(None);
        b.push(Some(-9));
        let arr = b.finish();
        let format = b"i\0".as_ptr() as *const c_char;
        let (array, schema) = export(arr, format);
        let validity_bytes = crate::bitmap::bitmap_bytes(3);

        let found = unsafe { format_of(&schema) }.unwrap();
        assert_eq!(found, "i");

        let view = unsafe {
            primitive_view::<i32>(
                &schema,
                &array,
                3 * core::mem::size_of::<i32>(),
            )
        }
        .unwrap();
        assert_eq!(view, &[7, 0, -9]);

        let validity: Vec<bool> = (0..3)
            .map(|row| unsafe { is_valid(&array, validity_bytes, row) }.unwrap())
            .collect();
        assert_eq!(validity, [true, false, true]);

        // Release duty stays with the pair's owner; the views were read
        // before any release call (none here — the arena leaks in this
        // test, as in c_abi's own tests).
    }

    #[test]
    fn format_mismatch_is_refused() {
        let arr = PrimitiveBuilder::<i64>::new().finish();
        let format = b"l\0".as_ptr() as *const c_char;
        let (array, schema) = export(arr, format);
        let error = unsafe {
            primitive_view::<i32>(&schema, &array, 8 * core::mem::size_of::<i32>())
        }
        .unwrap_err();
        assert_eq!(
            error,
            ForeignError::FormatMismatch {
                expected: "i",
                found: String::from("l"),
            }
        );
    }

    #[test]
    fn short_buffer_is_refused() {
        let mut b = PrimitiveBuilder::<i32>::new();
        b.push(Some(5));
        let arr = b.finish();
        let format = b"i\0".as_ptr() as *const c_char;
        let (array, schema) = export(arr, format);
        let error =
            unsafe { primitive_view::<i32>(&schema, &array, 0) }.unwrap_err();
        assert_eq!(
            error,
            ForeignError::BufferTooShort {
                required: 4,
                got: 0,
            }
        );
    }
}
