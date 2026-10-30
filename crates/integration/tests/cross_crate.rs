//! Cross-crate integration tests for avec-arrow (lane INTEGRATION).
//!
//! Chicago discipline: real collaborators only — the real builders, the real
//! C Data Interface structs heaped exactly as a foreign importer would hold
//! them, and assertions on final memory state read through the raw buffer
//! pointers the importer receives. No mocks, no call-counting.

use core::ffi::{c_char, CStr};
use core::slice;

use avec_arrow::c_abi::ForeignArray;
use avec_arrow::{
    export, ArrowSchema, Primitive, PrimitiveBuilder, VarlenBuilder, ViewVecBuilder,
    AVEC_FLAG_NULLABLE,
};

/// Importer duty for the schema half of the pair: `export` allocates the
/// schema shell on the heap and hands the caller a copy whose `private_data`
/// still addresses the live shell. Releasing that pointer frees the real
/// allocation. (Unlike the array path, the schema shell IS the freed
/// allocation, so this is exercised exactly once.)
fn release_schema_once(schema: &ArrowSchema) {
    let shell = schema.private_data as *mut ArrowSchema;
    assert!(!shell.is_null(), "export must allocate a schema shell");
    let release = schema.release.expect("export must set a schema release");
    // SAFETY: shell is the live Box::into_raw allocation made by `export`;
    // release_schema CAS-swaps the slot then frees exactly this box.
    unsafe { release(shell) };
}

#[test]
fn c_abi_export_import_roundtrip_bitwise_and_double_release() {
    // -- arrange: real builder, null rows present -----------------------
    let input: [Option<i32>; 7] = [
        Some(42),
        None,
        Some(-1),
        Some(i32::MAX),
        None,
        Some(0),
        Some(-7),
    ];
    let mut b = PrimitiveBuilder::<i32>::new();
    for &v in &input {
        b.push(v);
    }
    let arr = b.finish();
    assert_eq!(arr.len(), 7);
    assert_eq!(
        arr.validity().expect("null rows present").null_count(),
        2,
        "builder-side null count"
    );

    // -- act: real C ABI export -> heap -> foreign import ----------------
    let format = b"i\0".as_ptr() as *const c_char;
    let (array, schema) = export(arr, format);

    assert_eq!(array.length, 7);
    assert_eq!(array.null_count, 2);
    assert_eq!(array.n_buffers, 2);
    assert_eq!(array.offset, 0);
    assert_eq!(array.n_children, 0);
    assert_eq!(schema.flags, AVEC_FLAG_NULLABLE);
    assert_eq!(
        unsafe { CStr::from_ptr(schema.format) }.to_bytes(),
        b"i",
        "schema format string"
    );

    // Heap the ArrowArray exactly as a foreign FFI importer would hold it.
    let ptr = Box::into_raw(Box::new(array));
    // SAFETY: ptr is a live, spec-shaped ArrowArray; this test now owns its
    // release (last-reference importer per the C Data Interface spec).
    let foreign = unsafe { ForeignArray::take(ptr) };

    // -- assert: metadata + bitwise buffer contents ----------------------
    assert_eq!(foreign.len(), 7);
    assert_eq!(unsafe { (*foreign.array).null_count }, 2);

    // SAFETY: n_buffers == 2 asserted above; slot 0 is the validity bitmap
    // base pointer, slot 1 the value buffer base pointer of the live import.
    let vptr = unsafe { foreign.buffer(0) } as *const u8;
    let dptr = unsafe { foreign.buffer(1) } as *const u8;
    assert!(!vptr.is_null(), "null rows require a validity buffer");
    assert!(!dptr.is_null());

    // Expected bytes recomputed from the builder input, not the array.
    let expected_validity: Vec<u8> = {
        let mut bytes = vec![0u8; input.len().div_ceil(8)];
        for (i, &row) in input.iter().enumerate() {
            if row.is_some() {
                bytes[i / 8] |= 1 << (i % 8);
            }
        }
        bytes
    };
    let expected_values: Vec<u8> = input
        .iter()
        .flat_map(|row| row.unwrap_or(0).to_le_bytes())
        .collect();

    // SAFETY: both buffers are live for the imported array's lifetime and
    // sized (len+7)/8 validity bytes / len*4 value bytes per the spec.
    let got_validity = unsafe { slice::from_raw_parts(vptr, expected_validity.len()) };
    let got_values = unsafe { slice::from_raw_parts(dptr, expected_values.len()) };
    assert_eq!(
        got_validity,
        expected_validity.as_slice(),
        "validity bitmap must match the input bitwise"
    );
    assert_eq!(
        got_values,
        expected_values.as_slice(),
        "value buffer must match the input bitwise (null slots zero-filled)"
    );

    // -- release protocol -------------------------------------------------
    // SAFETY: live shell; the release slot is Some while the import lives.
    let release_fn = unsafe { (*foreign.array).release }.expect("release must be set");
    // SAFETY: single-owner release; frees the boxed OwnedPrimitive.
    unsafe { release_fn(foreign.array) };
    assert!(
        unsafe { (*foreign.array).release }.is_none(),
        "first release must CAS-swap the slot to NULL"
    );
    // SAFETY: double release — the CAS swap observes prev == NULL and returns
    // without touching the freed private data (c_abi::release_array law).
    unsafe { release_fn(foreign.array) };

    // ForeignArray::drop over an already-released array: sees release ==
    // NULL and must skip without a second free.
    drop(foreign);
    // SAFETY: recreate the shell box made above; the shell outlives the
    // private data (release_array never frees the shell).
    unsafe { drop(Box::from_raw(ptr)) };

    release_schema_once(&schema);
}

#[test]
fn export_all_valid_hands_null_validity_buffer() {
    let mut b = PrimitiveBuilder::<i64>::new();
    b.push(Some(1));
    b.push(Some(-1));
    let (array, schema) = export(b.finish(), b"l\0".as_ptr() as *const c_char);
    assert_eq!(array.null_count, 0);

    let ptr = Box::into_raw(Box::new(array));
    // SAFETY: live array; this test owns the release.
    let foreign = unsafe { ForeignArray::take(ptr) };
    // SAFETY: n_buffers == 2 for a primitive export.
    let vptr = unsafe { foreign.buffer(0) };
    assert!(
        vptr.is_null(),
        "Arrow law: null_count == 0 must export a NULL validity buffer"
    );
    let dptr = unsafe { foreign.buffer(1) } as *const i64;
    // SAFETY: live value buffer holding 2 i64 rows.
    let got = unsafe { slice::from_raw_parts(dptr, 2) };
    assert_eq!(got, &[1_i64, -1]);

    drop(foreign);
    // SAFETY: recreate the shell box made above.
    unsafe { drop(Box::from_raw(ptr)) };
    release_schema_once(&schema);
}

#[test]
fn alignment_law_u64_across_sizes() {
    for n in [0_usize, 1, 63, 64, 65, 1000, 10000] {
        let mut b = PrimitiveBuilder::<u64>::new();
        for i in 0..n {
            b.push(if i % 3 == 0 {
                None
            } else {
                Some((i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15))
            });
        }
        let arr = b.finish();
        assert_eq!(arr.len(), n);
        assert!(
            arr.buffers_aligned_64(),
            "u64 buffers must be 64-byte aligned at n={n}"
        );
    }
}

#[test]
fn format_codes_match_arrow_scalar_table() {
    // `generated_layout` is a private module of avec-arrow (not re-exported
    // from lib.rs), so the LAYOUT_SPECS cross-check is unreachable from an
    // external crate; the contract fallback applies — assert the literal
    // Arrow scalar format-code table in type order.
    macro_rules! code_of {
        ($t:ty) => {
            Primitive::<$t>::format_code()
        };
    }
    let codes = [
        code_of!(i8),
        code_of!(u8),
        code_of!(i16),
        code_of!(u16),
        code_of!(i32),
        code_of!(u32),
        code_of!(i64),
        code_of!(u64),
        code_of!(f32),
        code_of!(f64),
    ];
    assert_eq!(codes, ["c", "C", "s", "S", "i", "I", "l", "L", "f", "g"]);
    // Spot pins so a table rotation cannot pass silently as a permutation.
    assert_eq!(Primitive::<i32>::format_code(), "i");
    assert_eq!(Primitive::<f64>::format_code(), "g");
}

#[test]
fn varlen_u32_unicode_empty_null_roundtrip() {
    let rows: [&str; 6] = ["héllo", "", "日本語テキスト", "", "a", "🎉 spam 🎉"];
    let valid = [true, true, true, false, true, true];

    let mut b = VarlenBuilder::<u32>::new();
    for (i, r) in rows.iter().enumerate() {
        if valid[i] {
            b.push(Some(r.as_bytes()));
        } else {
            b.push(None);
        }
    }
    let v = b.finish();
    assert_eq!(v.len(), rows.len());

    // Offsets recomputed from the input: N+1 entries, first zero, null rows
    // repeat the current offset (empty payload slot).
    let mut expected_offsets: Vec<u32> = vec![0];
    let mut expected_data: Vec<u8> = Vec::new();
    for (i, r) in rows.iter().enumerate() {
        if valid[i] {
            expected_data.extend_from_slice(r.as_bytes());
        }
        expected_offsets.push(u32::try_from(expected_data.len()).expect("offset fits u32"));
    }
    assert_eq!(v.offsets(), expected_offsets.as_slice(), "offsets bitwise");
    assert_eq!(v.offsets()[0], 0);
    assert!(v.offsets_monotonic(), "offsets must be monotonic");

    // Row reads: valid rows exact, null row reads empty payload.
    for (i, r) in rows.iter().enumerate() {
        let expected: &[u8] = if valid[i] { r.as_bytes() } else { b"" };
        assert_eq!(v.row(i), expected, "row {i} mismatch");
    }

    // Validity: exact per-row, one null total.
    let bm = v.validity().expect("null row present -> bitmap exists");
    for (i, &ok) in valid.iter().enumerate() {
        assert_eq!(bm.get(i), ok, "validity row {i}");
    }
    assert_eq!(bm.null_count(), 1);
}

#[test]
fn views_inline_foreign_boundary_roundtrip() {
    let rows: [Option<&[u8]>; 8] = [
        Some(b"short"),                            // 5  -> inline
        Some(b""),                                 // 0  -> inline
        Some(b"abcdefghijkl"),                     // 12 -> inline boundary
        Some(b"abcdefghijklm"),                    // 13 -> foreign boundary
        Some(b"this-one-is-longer-than-twelve"),   // 30 -> foreign
        Some("日本語のちょっと長いテキストです".as_bytes()), // multibyte foreign
        None,                                      // null row
        Some(b"x"),                                // 1  -> inline
    ];

    let mut b = ViewVecBuilder::new();
    for &r in &rows {
        b.push(r);
    }
    let v = b.finish();
    assert_eq!(v.len(), rows.len());
    assert_eq!(
        v.views_bytes().len(),
        rows.len() * 16,
        "one 16-byte descriptor per row"
    );
    assert_eq!(avec_arrow::views::MAX_INLINE, 12);

    // Branchless classification must agree with the 12-byte law everywhere,
    // including both boundaries.
    for (i, r) in rows.iter().enumerate() {
        let len = r.map_or(0, |bytes| bytes.len()) as u32;
        let inline = avec_arrow::views::is_inline(len) == 1;
        assert_eq!(inline, len <= 12, "is_inline({len}) misclassifies row {i}");
    }

    // row() reads back exactly; the null row reads empty.
    for (i, r) in rows.iter().enumerate() {
        let expected: &[u8] = r.unwrap_or(&[]);
        assert_eq!(v.row(i), expected, "row {i} mismatch");
    }

    // Only foreign rows (>12 bytes) land in the variadic data block.
    let mut expected_data: Vec<u8> = Vec::new();
    for r in rows.iter().copied().flatten() {
        if r.len() > 12 {
            expected_data.extend_from_slice(r);
        }
    }
    assert_eq!(v.data(), expected_data.as_slice(), "data block contents");

    // Validity: exact per-row.
    let bm = v.validity().expect("null row present -> bitmap exists");
    for (i, r) in rows.iter().enumerate() {
        assert_eq!(bm.get(i), r.is_some(), "validity row {i}");
    }
}
