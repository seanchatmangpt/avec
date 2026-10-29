//! Fixed-width primitive vectors (charter layer 1: [Validity Bitmap, Value Buffer]).
//!
//! Zero external dependencies. Coverage: i8/i16/i32/i64, u8/u16/u32/u64,
//! f32/f64 — each carrying its Arrow format code from the ontology table.

use crate::bitmap::Bitmap;
use crate::buffer::{Buf, ALIGNMENT};
use alloc::vec::Vec;
use core::marker::PhantomData;

/// Fixed-width element type: Copy + Arrow format code + zero value.
pub trait Scalar: Copy {
    /// Arrow format code (matches `generated_layout.rs` table).
    const FORMAT_CODE: &'static str;
    /// Element width in bytes.
    const WIDTH: usize;
    /// Slot filler for null rows (Arrow leaves arbitrary bytes in null slots;
    /// we write ZERO deterministically).
    const ZERO: Self;
    /// Write self little-endian into the front of `dst` (WIDTH bytes).
    fn write_le(self, dst: &mut [u8]);
}

macro_rules! scalar_impl {
    ($t:ty, $code:literal) => {
        impl Scalar for $t {
            const FORMAT_CODE: &'static str = $code;
            const WIDTH: usize = core::mem::size_of::<$t>();
            const ZERO: Self = 0 as $t;
            #[inline]
            fn write_le(self, dst: &mut [u8]) {
                dst[..Self::WIDTH].copy_from_slice(&<$t>::to_le_bytes(self));
            }
        }
    };
}

scalar_impl!(i8, "c");
scalar_impl!(u8, "C");
scalar_impl!(i16, "s");
scalar_impl!(u16, "S");
scalar_impl!(i32, "i");
scalar_impl!(u32, "I");
scalar_impl!(i64, "l");
scalar_impl!(u64, "L");
scalar_impl!(f32, "f");
scalar_impl!(f64, "g");

/// Fixed-width primitive vector: optional validity bitmap + value buffer.
pub struct Primitive<T: Scalar> {
    validity: Option<Bitmap>,
    values: Buf,
    len: usize,
    _marker: PhantomData<fn() -> T>,
}

impl<T: Scalar> Primitive<T> {
    /// Row count.
    #[must_use]
    pub fn len(&self) -> usize {
        self.len
    }

    /// True when zero rows.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Arrow format code of the element type.
    #[must_use]
    pub fn format_code() -> &'static str {
        T::FORMAT_CODE
    }

    /// Typed value view.
    #[must_use]
    pub fn values(&self) -> &[T] {
        self.values.as_typed::<T>()
    }

    /// Validity bitmap (None = all valid).
    #[must_use]
    pub fn validity(&self) -> Option<&Bitmap> {
        self.validity.as_ref()
    }

    /// Value buffer base pointer (zero-copy export uses this).
    #[must_use]
    pub fn values_ptr(&self) -> *const u8 {
        self.values.as_ptr()
    }

    /// Validity buffer base pointer (null when all-valid).
    #[must_use]
    pub fn validity_ptr(&self) -> *const u8 {
        match &self.validity {
            Some(bm) => bm.as_ptr(),
            None => core::ptr::null(),
        }
    }

    /// Charter M-03 proof: both buffers 64-byte aligned.
    #[must_use]
    pub fn buffers_aligned_64(&self) -> bool {
        let v_ok = self
            .validity
            .as_ref()
            .map_or(true, |bm| bm.as_ptr() as usize % ALIGNMENT == 0);
        v_ok && self.values.as_ptr() as usize % ALIGNMENT == 0
    }

    /// Freeze a builder into the aligned vector.
    pub(crate) fn from_parts(validity: Option<Bitmap>, values: Buf, len: usize) -> Self {
        Primitive {
            validity,
            values,
            len,
            _marker: PhantomData,
        }
    }

    /// Consume into parts (export path).
    pub(crate) fn into_parts(self) -> (Option<Bitmap>, Buf, usize) {
        (self.validity, self.values, self.len)
    }
}

/// Builder producing a frozen [`Primitive`].
pub struct PrimitiveBuilder<T: Scalar> {
    values: Vec<T>,
    validity: Vec<bool>,
    _marker: PhantomData<fn() -> T>,
}

impl<T: Scalar> Default for PrimitiveBuilder<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Scalar> PrimitiveBuilder<T> {
    #[must_use]
    pub fn new() -> Self {
        PrimitiveBuilder {
            values: Vec::new(),
            validity: Vec::new(),
            _marker: PhantomData,
        }
    }

    /// Append one row.
    pub fn push(&mut self, v: Option<T>) {
        self.values.push(match v {
            Some(x) => x,
            None => T::ZERO,
        });
        self.validity.push(v.is_some());
    }

    /// Rows appended so far.
    #[must_use]
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Freeze into the aligned vector. All-valid inputs drop the validity
    /// buffer (Arrow law: null_count == 0 -> no validity buffer).
    #[must_use]
    pub fn finish(&self) -> Primitive<T> {
        let n = self.values.len();
        // Zero-fill first (write-before-read), then copy values in.
        let bytes = n * T::WIDTH;
        let mut values = Buf::new_zeroed(bytes);
        let src = self.values.as_slice();
        let dst_bytes = values.as_mut_slice();
        let mut i = 0;
        while i < n {
            let slot = &mut dst_bytes[i * T::WIDTH..(i + 1) * T::WIDTH];
            src[i].write_le(slot);
            i += 1;
        }
        let validity = Bitmap::from_bools(&self.validity);
        Primitive::from_parts(validity, values, n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn i32_round_trip_with_nulls() {
        let mut b = PrimitiveBuilder::<i32>::new();
        b.push(Some(1));
        b.push(None);
        b.push(Some(i32::MAX));
        b.push(Some(-7));
        let arr = b.finish();
        assert_eq!(arr.len(), 4);
        assert_eq!(Primitive::<i32>::format_code(), "i");
        assert_eq!(arr.values()[0], 1);
        assert_eq!(arr.values()[1], 0);
        assert_eq!(arr.values()[2], i32::MAX);
        assert_eq!(arr.values()[3], -7);
        let v = arr.validity().unwrap();
        assert!(!v.get(1));
        assert!(v.get(0));
        assert_eq!(arr.buffers_aligned_64(), true);
    }

    #[test]
    fn all_valid_drops_bitmap() {
        let mut b = PrimitiveBuilder::<f64>::new();
        b.push(Some(1.5));
        b.push(Some(-2.5));
        let arr = b.finish();
        assert!(arr.validity().is_none());
        assert_eq!(Primitive::<f64>::format_code(), "g");
        assert_eq!(arr.values()[0], 1.5);
        assert_eq!(arr.buffers_aligned_64(), true);
    }

    #[test]
    fn alignment_law_holds() {
        let mut b = PrimitiveBuilder::<u64>::new();
        for i in 0..1000u64 {
            b.push(if i % 17 == 0 { None } else { Some(i) });
        }
        assert!(b.finish().buffers_aligned_64());
    }
}
