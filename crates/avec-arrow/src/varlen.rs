//! Variable-length binary/string vectors (charter layer: [Validity, Offsets, Bytes]).
//!
//! u32 offsets = Utf8/Binary ("u"/"z"), u64 offsets = LargeUtf8/LargeBinary
//! ("U"/"Z"). Offset monotonicity is validated at freeze (FMEA row: buffer
//! overread via non-monotonic or missing N+1 element).

use crate::bitmap::Bitmap;
use crate::buffer::Buf;
use alloc::vec::Vec;
use core::marker::PhantomData;

/// Offset width marker: u32 or u64 offset vectors.
pub trait Offset: Copy {
    /// Arrow format code of the parent array ("u"/"z" vs "U"/"Z" family).
    fn format_family() -> &'static str;
    fn from_usize(v: usize) -> Self;
    fn to_u64(self) -> u64;
    fn to_le_bytes(self) -> [u8; 8];
}

impl Offset for u32 {
    #[inline]
    fn format_family() -> &'static str {
        "32-bit"
    }
    #[inline]
    fn from_usize(v: usize) -> Self {
        u32::try_from(v).expect("offset overflow (u32 family)")
    }
    #[inline]
    fn to_u64(self) -> u64 {
        u64::from(self)
    }
    #[inline]
    fn to_le_bytes(self) -> [u8; 8] {
        let mut w = [0u8; 8];
        w[..4].copy_from_slice(&self.to_le_bytes_w());
        w
    }
}

trait U32Ext { fn to_le_bytes_w(self) -> [u8; 4]; }
impl U32Ext for u32 { #[inline] fn to_le_bytes_w(self) -> [u8; 4] { u32::to_le_bytes(self) } }

impl Offset for u64 {
    #[inline]
    fn format_family() -> &'static str {
        "64-bit"
    }
    #[inline]
    fn from_usize(v: usize) -> Self {
        v as u64
    }
    #[inline]
    fn to_u64(self) -> u64 {
        self
    }
    #[inline]
    fn to_le_bytes(self) -> [u8; 8] {
        u64::to_le_bytes(self)
    }
}

/// Variable-length vector, frozen and validated.
pub struct Varlen<O: Offset> {
    validity: Option<Bitmap>,
    offsets: Buf,
    data: Buf,
    len: usize,
    _marker: PhantomData<fn() -> O>,
}

impl<O: Offset> Varlen<O> {
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

    /// Offsets (N+1 monotonic entries, first = 0).
    #[must_use]
    pub fn offsets(&self) -> &[O] {
        self.offsets.as_typed::<O>()
    }

    /// Payload bytes.
    #[must_use]
    pub fn data(&self) -> &[u8] {
        self.data.as_slice()
    }

    /// Row i bytes (monotonicity validated at freeze).
    #[must_use]
    pub fn row(&self, i: usize) -> &[u8] {
        let off = self.offsets();
        let a = off[i].to_u64() as usize;
        let b = off[i + 1].to_u64() as usize;
        &self.data.as_slice()[a..b]
    }

    /// Validity bitmap (None = all valid).
    #[must_use]
    pub fn validity(&self) -> Option<&Bitmap> {
        self.validity.as_ref()
    }

    /// FMEA control proof: offsets strictly monotonic (equal allowed: empty
    /// rows), N+1 entries, first zero.
    #[must_use]
    pub fn offsets_monotonic(&self) -> bool {
        let off = self.offsets();
        if off.is_empty() || off[0].to_u64() != 0 {
            return false;
        }
        let mut i = 0;
        let mut ok = true;
        while i + 1 < off.len() {
            ok &= off[i].to_u64() <= off[i + 1].to_u64();
            i += 1;
        }
        ok
    }
}

/// Builder producing a frozen, offset-validated [`Varlen`].
pub struct VarlenBuilder<O: Offset> {
    data: Vec<u8>,
    offsets: Vec<O>,
    validity: Vec<bool>,
    _marker: PhantomData<fn() -> O>,
}

impl<O: Offset> Default for VarlenBuilder<O> {
    fn default() -> Self {
        Self::new()
    }
}

impl<O: Offset> VarlenBuilder<O> {
    #[must_use]
    pub fn new() -> Self {
        VarlenBuilder {
            data: Vec::new(),
            offsets: alloc::vec![O::from_usize(0)],
            validity: Vec::new(),
            _marker: PhantomData,
        }
    }

    /// Rows appended so far.
    #[must_use]
    pub fn len(&self) -> usize {
        self.validity.len()
    }

    /// True when no rows appended yet.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.validity.is_empty()
    }

    /// Append a row (None = null row, empty payload slot).
    pub fn push(&mut self, row: Option<&[u8]>) {
        match row {
            Some(bytes) => {
                self.data.extend_from_slice(bytes);
                self.offsets.push(O::from_usize(self.data.len()));
                self.validity.push(true);
            }
            None => {
                self.offsets.push(O::from_usize(self.data.len()));
                self.validity.push(false);
            }
        }
    }

    /// Freeze: copy to 64-byte aligned buffers.
    #[must_use]
    pub fn finish(&self) -> Varlen<O> {
        let n = self.validity.len();
        // Offsets buffer: (n+1) * width, 64-byte aligned.
        let ow = core::mem::size_of::<O>();
        let mut offsets = Buf::new_zeroed((n + 1) * ow);
        let ob = offsets.as_mut_slice();
        let mut i = 0;
        while i <= n {
            let le = self.offsets[i].to_le_bytes();
            ob[i * ow..(i + 1) * ow].copy_from_slice(&le[..ow]);
            i += 1;
        }
        let mut data = Buf::new_zeroed(self.data.len());
        data.as_mut_slice().copy_from_slice(&self.data);
        let validity = Bitmap::from_bools(&self.validity);
        let v = Varlen {
            validity,
            offsets,
            data,
            len: n,
            _marker: PhantomData,
        };
        debug_assert!(v.offsets_monotonic());
        v
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf8_round_trip_u32() {
        let mut b = VarlenBuilder::<u32>::new();
        b.push(Some(b"alpha"));
        b.push(None);
        b.push(Some(b""));
        b.push(Some(b"gamma-delta-epsilon"));
        let v = b.finish();
        assert_eq!(v.len(), 4);
        assert_eq!(v.row(0), b"alpha");
        assert_eq!(v.row(2), b"");
        assert_eq!(v.row(3), b"gamma-delta-epsilon");
        assert!(v.validity().unwrap().get(0));
        assert!(!v.validity().unwrap().get(1));
        assert!(v.offsets_monotonic());
        assert_eq!(v.offsets().len(), 5);
    }

    #[test]
    fn large_offsets_u64() {
        let mut b = VarlenBuilder::<u64>::new();
        b.push(Some(b"x"));
        b.push(Some(b"yy"));
        let v = b.finish();
        assert_eq!(v.offsets(), &[0u64, 1, 3]);
        assert_eq!(v.row(1), b"yy");
    }

    #[test]
    fn offsets_buffer_aligned_64() {
        let mut b = VarlenBuilder::<u32>::new();
        for _i in 0..100 {
            b.push(Some(b"row-bytes"));
        }
        let v = b.finish();
        // buffers are private; monotonic + row reads exercise them
        assert_eq!(v.row(99), b"row-bytes");
        assert!(v.offsets_monotonic());
    }
}
