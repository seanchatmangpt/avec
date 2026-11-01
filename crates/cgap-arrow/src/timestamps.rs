//! Timestamp element types over the i64 fixed-width layout (layout labels
//! `TimestampSec/Milli/Micro/Nano`, prefixes `tss:/tsm:/tsu:/tsn:`).
//!
//! DIVERGENCE from the layout table: the schema format string carries the
//! timezone suffix; this crate pins it to `"UTC"` (no dynamic timezone
//! support under the no-alloc/no-dep law). A consumer wanting another
//! timezone emits its own schema format string at the wire boundary; the
//! buffer layout is identical.
//!
//! Each newtype is a transparent i64: the buffer layout, alignment, and
//! write path are byte-identical to `Primitive<i64>` ("l" columns), so scan
//! kernels treat timestamps as i64 words. The newtypes exist so a schema's
//! format code and the element type travel together and a foreign schema's
//! `ts*:` format string type-checks against the element before any view is
//! handed out (see `foreign.rs`, ticket T2).

use crate::primitive::Scalar;
use core::fmt;

macro_rules! timestamp_impl {
    ($t:ident, $unit:literal) => {
        /// A timestamp column element: count of the named unit since the
        /// epoch, UTC (layout labels `TimestampSec/Milli/Micro/Nano`,
        /// prefixes `tss:/tsm:/tsu:/tsn:`).
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $t(pub i64);

        impl Scalar for $t {
            // The full schema format string, tz suffix included.
            const FORMAT_CODE: &'static str = concat!($unit, "UTC");
            const WIDTH: usize = 8;
            const ZERO: Self = Self(0);
            #[inline]
            fn write_le(self, dst: &mut [u8]) {
                dst[..8].copy_from_slice(&self.0.to_le_bytes());
            }
        }

        impl fmt::Display for $t {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                // Epoch-unit count (no civil-date rendering: that is a
                // calendar concern, outside the substrate).
                core::fmt::Display::fmt(&self.0, f)
            }
        }
    };
}

timestamp_impl!(TimestampSecond, "tss:");
timestamp_impl!(TimestampMilli, "tsm:");
timestamp_impl!(TimestampMicro, "tsu:");
timestamp_impl!(TimestampNano, "tsn:");

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitive::{Primitive, PrimitiveBuilder};

    #[test]
    fn format_codes_match_layout_prefixes() {
        assert_eq!(TimestampSecond::FORMAT_CODE, "tss:UTC");
        assert_eq!(TimestampMilli::FORMAT_CODE, "tsm:UTC");
        assert_eq!(TimestampMicro::FORMAT_CODE, "tsu:UTC");
        assert_eq!(TimestampNano::FORMAT_CODE, "tsn:UTC");
    }

    #[test]
    fn builder_round_trip_through_i64_layout() {
        let mut b = PrimitiveBuilder::new();
        b.push(Some(TimestampMicro(1_700_000_000_000_001)));
        b.push(None);
        b.push(Some(TimestampMicro(-1)));
        let arr = b.finish();
        assert_eq!(arr.len(), 3);
        assert_eq!(Primitive::<TimestampMicro>::format_code(), "tsu:UTC");
        assert_eq!(arr.values()[0], TimestampMicro(1_700_000_000_000_001));
        assert_eq!(arr.values()[1], TimestampMicro(0));
        assert_eq!(arr.values()[2], TimestampMicro(-1));
        // Null slot is the deterministic zero.
        assert!(!arr.validity().is_none_or(|bm| bm.get(1)));
    }

    #[test]
    fn i64_layout_parity() {
        // The buffer bytes are byte-identical to an i64 column carrying the
        // same counts.
        let mut ts = PrimitiveBuilder::new();
        let mut plain = PrimitiveBuilder::<i64>::new();
        for v in [0i64, 1, -1, i64::MAX, i64::MIN] {
            ts.push(Some(TimestampMicro(v)));
            plain.push(Some(v));
        }
        let (ts_arr, plain_arr) = (ts.finish(), plain.finish());
        assert_eq!(ts_arr.values().len(), plain_arr.values().len());
        assert!(ts_arr
            .values()
            .iter()
            .zip(plain_arr.values())
            .all(|(t, p)| t.0 == *p));
    }
}
