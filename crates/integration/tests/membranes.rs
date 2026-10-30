//! Membrane coverage for the avec-arrow integrator surface (lane REL-L13).
//!
//! Extends `cross_crate.rs` (which owns the i32 C-ABI walk, the alignment
//! law, the format-code table, the u32 varlen and the views roundtrip) with
//! the remaining membranes a downstream integrator crosses:
//!
//! 1. C-ABI roundtrips for the 64-bit / float scalar family
//!    (i64/u64/f32/f64): export -> foreign heap -> `ForeignArray::take` ->
//!    bitwise buffer walk -> release / double-release.
//! 2. Varlen u64 offsets family (LargeBinary/LargeUtf8, Arrow "U"/"Z").
//! 3. NEON-vs-portable bitwise parity of the mask kernels over identical
//!    deterministic inputs across boundary sizes 0/1/63/64/65/8192 and one
//!    1M-word pass.
//! 4. Nested struct export: raw-pointer ABI walk over a two-child
//!    `export_struct` pair (f64 + u32 children), parent double-release.
//!
//! Scope note (lane decision, per contract): `rle_encode` /
//! `rle_encode_fast` byte-identity and any parquet page roundtrip live in
//! `avec-parquet`, which this crate does not depend on by law (deps are
//! avec-arrow only) — skipped, reported to the coordinator.
//!
//! Chicago discipline: real builders, real C Data Interface structs heaped
//! exactly as a foreign importer holds them, expectations derived from the
//! builder *input* (never re-read from the array), assertions on final
//! memory state through the raw buffer pointers. No mocks, no call counts.

use core::ffi::CStr;
use core::slice;

use avec_arrow::bitmap::mask_words;
use avec_arrow::c_abi::ForeignArray;
use avec_arrow::c_abi_nested::export_struct;
use avec_arrow::primitive::Scalar;
use avec_arrow::silicon::mask_words_neon;
use avec_arrow::varlen::Offset;
use avec_arrow::{export, ArrowSchema, PrimitiveBuilder, VarlenBuilder, AVEC_FLAG_NULLABLE};

/// Importer duty for the schema half of a pair: `export` boxes the schema
/// shell and aliases `private_data` to it, so the house release frees exactly
/// that allocation. (Same duty as `cross_crate.rs`; test binaries cannot
/// share code without a common module, so the duty is restated here.)
fn release_schema_once(schema: &ArrowSchema) {
    let shell = schema.private_data as *mut ArrowSchema;
    assert!(!shell.is_null(), "export must allocate a schema shell");
    let release = schema.release.expect("export must set a schema release");
    // SAFETY: shell is the live Box::into_raw allocation made by `export`;
    // release_schema CAS-swaps the slot then frees exactly this box.
    unsafe { release(shell) };
}

#[test]
fn c_abi_roundtrip_i64_u64_f32_f64_bitwise_and_double_release() {
    // Inputs sized to land on validity-bitmap boundaries: 7 (first word),
    // 70 (spills 6 bits into the second byte), 65 (1 bit in the second
    // byte), 63 (last bit of the first 8-byte word).
    let i64_rows: [Option<i64>; 7] = [
        Some(i64::MIN),
        None,
        Some(-1),
        Some(i64::MAX),
        Some(0),
        None,
        Some(0x0123_4567_89AB_CDEF),
    ];
    let u64_rows: Vec<Option<u64>> = (0..70)
        .map(|i| {
            if i % 7 == 3 {
                None // 10 nulls: rows 3,10,17,...,66
            } else {
                Some((i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15))
            }
        })
        .collect();
    let f32_rows: Vec<Option<f32>> = (0..65)
        .map(|i| {
            if i % 13 == 5 {
                None
            } else {
                Some((i as f32) + 0.5)
            }
        })
        .collect();
    let f64_rows: Vec<Option<f64>> = (0..63)
        .map(|i| {
            if i % 9 == 4 {
                None
            } else {
                Some((i as f64) * core::f64::consts::PI)
            }
        })
        .collect();

    roundtrip_bitwise::<i64>(&i64_rows, c"l");
    roundtrip_bitwise::<u64>(&u64_rows, c"L");
    roundtrip_bitwise::<f32>(&f32_rows, c"f");
    roundtrip_bitwise::<f64>(&f64_rows, c"g");
}

/// One full C-ABI membrane walk for one scalar type: real builder with null
/// rows -> `export` -> heap the `ArrowArray` (foreign importer stance) ->
/// `ForeignArray::take` -> bitwise validity/value buffer walk against
/// input-derived expectations -> typed re-read -> release + double-release +
/// shell drop + schema release.
fn roundtrip_bitwise<T: Scalar>(rows: &[Option<T>], fmt: &CStr)
where
    T: Scalar + PartialEq + core::fmt::Debug,
{
    let n = rows.len();
    let nulls = rows.iter().filter(|r| r.is_none()).count();
    assert!(nulls >= 1, "this walk pins the null-slot laws; feed >= 1 null");
    assert!(n > 0);

    // -- arrange: real builder, format code cross-checked against the type's
    // own table entry so a table rotation cannot pass silently ---------------
    assert_eq!(T::FORMAT_CODE, fmt.to_str().expect("ascii format code"));
    let mut b = PrimitiveBuilder::<T>::new();
    for &r in rows {
        b.push(r);
    }
    let arr = b.finish();
    assert_eq!(arr.len(), n);
    assert_eq!(
        arr.validity().map_or(0, |bm| bm.null_count()),
        nulls,
        "builder-side null count"
    );

    // Expectations derived from the INPUT, not from the array.
    let expected_validity: Vec<u8> = {
        let mut bytes = vec![0u8; n.div_ceil(8)];
        for (i, &r) in rows.iter().enumerate() {
            if r.is_some() {
                bytes[i / 8] |= 1 << (i % 8);
            }
        }
        bytes
    };
    let expected_values: Vec<u8> = {
        let mut bytes = Vec::with_capacity(n * T::WIDTH);
        for &r in rows {
            let mut slot = [0u8; 16]; // widest Scalar is 8 bytes
            match r {
                Some(v) => v.write_le(&mut slot),
                None => T::ZERO.write_le(&mut slot), // null slots carry ZERO
            }
            bytes.extend_from_slice(&slot[..T::WIDTH]);
        }
        bytes
    };

    // -- act: real C ABI export -> heap -> foreign import ---------------------
    let (array, schema) = export(arr, fmt.as_ptr());

    assert_eq!(array.length, n as i64);
    assert_eq!(array.null_count, nulls as i64);
    assert_eq!(array.n_buffers, 2);
    assert_eq!(array.offset, 0);
    assert_eq!(array.n_children, 0);
    assert_eq!(schema.flags, AVEC_FLAG_NULLABLE);
    assert_eq!(schema.n_children, 0);
    assert!(schema.release.is_some());
    // SAFETY: format is a 'static C string; the shell is alive pre-release.
    assert_eq!(
        unsafe { CStr::from_ptr(schema.format) }.to_bytes(),
        fmt.to_bytes(),
        "schema format string"
    );

    // Heap the ArrowArray exactly as a foreign FFI importer would hold it.
    let ptr = Box::into_raw(Box::new(array));
    // SAFETY: live spec-shaped ArrowArray; this test owns the release.
    let foreign = unsafe { ForeignArray::take(ptr) };

    // -- assert: metadata + bitwise buffer contents ---------------------------
    assert_eq!(foreign.len(), n as i64);
    // SAFETY: n_buffers == 2 asserted above; slot 0 validity, slot 1 values.
    let vptr = unsafe { foreign.buffer(0) } as *const u8;
    let dptr = unsafe { foreign.buffer(1) } as *const u8;
    assert!(!vptr.is_null(), "null rows require a validity buffer");
    assert!(!dptr.is_null());

    // SAFETY: both buffers live for the import's lifetime; sized
    // (n+7)/8 validity bytes and n*WIDTH value bytes per the spec.
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

    // Typed re-read of the same buffer: 64-byte alignment guarantees aligned
    // scalar loads; null rows read ZERO.
    // SAFETY: live value buffer of n*T::WIDTH bytes, alignment 64.
    let typed = unsafe { slice::from_raw_parts(dptr as *const T, n) };
    for (i, &r) in rows.iter().enumerate() {
        assert_eq!(typed[i], r.unwrap_or(T::ZERO), "typed re-read row {i}");
    }

    // -- release protocol ------------------------------------------------------
    // SAFETY: live shell; the release slot is Some while the import lives.
    let release_fn = unsafe { (*foreign.array).release }.expect("release must be set");
    // SAFETY: single-owner release; frees the boxed OwnedPrimitive<T>.
    unsafe { release_fn(foreign.array) };
    assert!(
        unsafe { (*foreign.array).release }.is_none(),
        "first release must CAS-swap the slot to NULL"
    );
    // SAFETY: double release — observes prev == NULL, typed no-op.
    unsafe { release_fn(foreign.array) };
    // ForeignArray::drop over an already-released array: sees NULL, skips.
    drop(foreign);
    // SAFETY: recreate the shell box made above (release never frees it).
    unsafe { drop(Box::from_raw(ptr)) };

    release_schema_once(&schema);
}

#[test]
fn varlen_u64_offsets_family_roundtrip() {
    // Format-family cross-pin: the u64 offsets family is Arrow's
    // LargeUtf8/LargeBinary ("U"/"Z"), the u32 family Utf8/Binary.
    assert_eq!(<u64 as Offset>::format_family(), "64-bit");
    assert_eq!(<u32 as Offset>::format_family(), "32-bit");

    let long = vec![b'x'; 300]; // one long row: offsets must carry real distances
    let rows: [Option<&[u8]>; 7] = [
        Some(b"alpha"),
        Some("βήτα".as_bytes()),   // 2-byte code units
        Some(&[]),                 // empty payload, valid row
        Some(long.as_slice()),     // 300-byte row
        Some("日本語".as_bytes()), // 3-byte code units
        None,                      // null row
        Some(b"omega"),
    ];

    let mut b = VarlenBuilder::<u64>::new();
    for &r in &rows {
        b.push(r);
    }
    let v = b.finish();
    assert_eq!(v.len(), rows.len());
    assert_eq!(v.offsets().len(), rows.len() + 1, "N+1 offsets");
    assert!(v.offsets_monotonic(), "offsets must be monotonic");

    // Offsets recomputed from the input: entry 0 is zero; null rows repeat
    // the current offset (empty payload slot, same law the u32 test pins).
    let mut expected_offsets: Vec<u64> = vec![0];
    let mut expected_data: Vec<u8> = Vec::new();
    for &r in rows.iter() {
        if let Some(bytes) = r {
            expected_data.extend_from_slice(bytes);
        }
        expected_offsets.push(expected_data.len() as u64);
    }
    assert_eq!(
        v.offsets(),
        expected_offsets.as_slice(),
        "u64 offsets bitwise vs input-derived expectation"
    );
    assert_eq!(v.offsets()[0], 0);
    assert_eq!(
        *v.offsets().last().expect("N+1 guarantees a tail"),
        expected_data.len() as u64,
        "tail offset equals the data-block length"
    );

    assert_eq!(v.data(), expected_data.as_slice(), "data block contents");

    // Row reads: valid rows exact; the null row reads empty.
    for (i, &r) in rows.iter().enumerate() {
        let expected: &[u8] = r.unwrap_or(&[]);
        assert_eq!(v.row(i), expected, "row {i} mismatch");
    }

    // Validity: exact per-row, one null total.
    let bm = v.validity().expect("null row present -> bitmap exists");
    for (i, &r) in rows.iter().enumerate() {
        assert_eq!(bm.get(i), r.is_some(), "validity row {i}");
    }
    assert_eq!(bm.null_count(), 1);
}

#[test]
fn neon_vs_portable_mask_bitwise_equality() {
    /// Deterministic xorshift64* fill — the same generator the silicon unit
    /// tests use, so both suites see identically-shaped inputs. No time, no
    /// rand, no floats: fully reproducible on every host.
    fn pattern(seed: u64, n: usize) -> Vec<u64> {
        let mut s = seed | 1;
        (0..n)
            .map(|_| {
                s ^= s << 13;
                s ^= s >> 7;
                s ^= s << 17;
                s.wrapping_mul(0x2545_F491_4F6C_DD1D)
            })
            .collect()
    }

    // Non-vacuity: on a non-aarch64 target `mask_words_neon` delegates to the
    // portable kernel and this comparison degenerates into a tautology (a
    // kernel compared against itself). Refuse loudly there instead of passing
    // vacuously (house rule, mirrors silicon::tests::dispatch_paths_by_target).
    assert!(
        cfg!(target_arch = "aarch64"),
        "non-aarch64 target: mask_words_neon dispatches to the portable \
         fallback — this suite would compare a kernel against itself"
    );

    // Boundary sizes from the contract plus one 1M-word pass.
    for n in [0_usize, 1, 63, 64, 65, 8192, 1 << 20] {
        let valid = pattern(0x9E37_79B9_7F4A_7C15, n);
        let pred = pattern(0xBF58_476D_1CE4_E5B9, n);
        // Distinct poison patterns: a skipped write fails loudly against the
        // poison instead of passing against stale zeros.
        let mut out_neon = vec![0xDEAD_BEEF_DEAD_BEEF; n];
        let mut out_portable = vec![0xBAAD_F00D_BAAD_F00D; n];

        mask_words_neon(&valid, &pred, &mut out_neon);
        mask_words(&valid, &pred, &mut out_portable);

        assert_eq!(
            out_neon, out_portable,
            "n = {n}: NEON and portable kernels must agree bitwise"
        );
        // The contract itself: out[i] == valid[i] & pred[i] for every word.
        for i in 0..n {
            assert_eq!(
                out_neon[i],
                valid[i] & pred[i],
                "n = {n}, word {i}: contract violated"
            );
        }
    }
}

#[test]
fn nested_export_struct_abi_walk_two_children() {
    // Two real child exports — f64 + u32, distinct types from the in-crate
    // i32/i64 walk — with null rows in each child.
    let f64_valid = [
        true, false, true, true, true, true, true, true, false,
    ];
    let f64_rows: [Option<f64>; 9] = [
        Some(0.5),
        None,
        Some(2.5),
        Some(3.5),
        Some(4.5),
        Some(5.5),
        Some(6.5),
        Some(7.5),
        None,
    ];
    let u32_valid = [
        true, true, true, false, false, true, true, true, true,
    ];
    let u32_rows: [Option<u32>; 9] = [
        Some(10),
        Some(20),
        Some(30),
        None,
        None,
        Some(60),
        Some(70),
        Some(80),
        Some(90),
    ];

    let mut bf = PrimitiveBuilder::<f64>::new();
    for &r in f64_rows.iter() {
        bf.push(r);
    }
    let mut bu = PrimitiveBuilder::<u32>::new();
    for &r in u32_rows.iter() {
        bu.push(r);
    }
    let child_f64 = export(bf.finish(), c"g".as_ptr());
    let child_u32 = export(bu.finish(), c"I".as_ptr());

    let (array, schema) = export_struct(vec![child_f64, child_u32], 9, c"point".as_ptr());

    // -- parent array header: raw field reads ---------------------------------
    assert_eq!(array.length, 9);
    assert_eq!(array.null_count, 0);
    assert_eq!(array.offset, 0);
    assert_eq!(array.n_buffers, 1, "struct layout: validity slot only");
    assert_eq!(array.n_children, 2);
    assert!(array.release.is_some());
    // SAFETY: one-slot buffers arena from export_struct.
    assert!(unsafe { *array.buffers }.is_null(), "no parent validity");
    assert!(array.dictionary.is_null());

    // -- parent schema ---------------------------------------------------------
    assert_eq!(schema.n_children, 2);
    assert_eq!(schema.flags, AVEC_FLAG_NULLABLE);
    assert!(schema.release.is_some());
    assert!(schema.metadata.is_null());
    assert!(schema.dictionary.is_null());

    // Expectations from the inputs.
    let f64_valid_bits = validity_bytes(&f64_valid);
    let u32_valid_bits = validity_bytes(&u32_valid);
    let expected_f64: Vec<u8> = f64_rows
        .iter()
        .flat_map(|r| r.unwrap_or(0.0).to_le_bytes())
        .collect();
    let expected_u32: Vec<u8> = u32_rows
        .iter()
        .flat_map(|r| r.unwrap_or(0_u32).to_le_bytes())
        .collect();

    // -- raw-pointer ABI walk over both children -------------------------------
    // SAFETY: children arenas are two-entry and every shell is alive
    // pre-release; format/name are 'static C strings from this test.
    unsafe {
        assert_eq!(CStr::from_ptr(schema.format).to_bytes(), b"+s");
        assert_eq!(CStr::from_ptr(schema.name).to_bytes(), b"point");
        let c0 = *array.children.add(0);
        let c1 = *array.children.add(1);
        let s0 = *schema.children.add(0);
        let s1 = *schema.children.add(1);

        // --- child 0: f64 [0.5, NULL, 2.5, .., 7.5, NULL] ---
        assert_eq!((*c0).length, 9);
        assert_eq!((*c0).null_count, 2);
        assert_eq!((*c0).offset, 0);
        assert_eq!((*c0).n_buffers, 2);
        assert_eq!((*c0).n_children, 0);
        let v0 = *(*c0).buffers.add(0) as *const u8;
        let d0 = *(*c0).buffers.add(1) as *const u8;
        assert_eq!(
            slice::from_raw_parts(v0, f64_valid_bits.len()),
            f64_valid_bits.as_slice(),
            "child 0 validity bits"
        );
        assert_eq!(
            slice::from_raw_parts(d0, expected_f64.len()),
            expected_f64.as_slice(),
            "child 0 value bytes bitwise (null slots zero-filled)"
        );
        // Typed re-read through the raw pointer.
        let t0 = d0 as *const f64;
        for (i, r) in f64_rows.iter().enumerate() {
            assert_eq!(*t0.add(i), r.unwrap_or(0.0), "child 0 row {i}");
        }

        // --- child 1: u32 [10, 20, 30, NULL, NULL, 60, .., 90] ---
        assert_eq!((*c1).length, 9);
        assert_eq!((*c1).null_count, 2);
        assert_eq!((*c1).offset, 0);
        assert_eq!((*c1).n_buffers, 2);
        assert_eq!((*c1).n_children, 0);
        let v1 = *(*c1).buffers.add(0) as *const u8;
        let d1 = *(*c1).buffers.add(1) as *const u8;
        assert_eq!(
            slice::from_raw_parts(v1, u32_valid_bits.len()),
            u32_valid_bits.as_slice(),
            "child 1 validity bits"
        );
        assert_eq!(
            slice::from_raw_parts(d1, expected_u32.len()),
            expected_u32.as_slice(),
            "child 1 value bytes bitwise"
        );

        // --- schema children wiring ---
        assert_eq!(CStr::from_ptr((*s0).format).to_bytes(), b"g");
        assert_eq!(CStr::from_ptr((*s1).format).to_bytes(), b"I");
        assert_eq!((*s0).n_children, 0);
        assert_eq!((*s1).n_children, 0);
        assert!((*s0).release.is_some());
        assert!((*s1).release.is_some());

        // -- teardown per the ownership table: the parent array release
        // cascades every child release exactly once and frees the child
        // shells; the parent shell is freed by this box holder. Double
        // release on both parents must be a typed no-op. ------------------
        let pa = Box::into_raw(Box::new(array));
        let f = (*pa).release.expect("parent array release set");
        f(pa);
        assert!((*pa).release.is_none(), "parent array slot swapped to null");
        f(pa); // double: no re-cascade
        drop(Box::from_raw(pa));

        let ps = Box::into_raw(Box::new(schema));
        let g = (*ps).release.expect("parent schema release set");
        g(ps);
        assert!((*ps).release.is_none(), "parent schema slot swapped to null");
        g(ps); // double: typed no-op
        drop(Box::from_raw(ps));
    }
    // Nothing below this line may touch the children: the cascades freed
    // them (spec: children die with the parent).
}

/// LSB-first validity bytes computed from per-row flags — the same encoding
/// law `cross_crate.rs` pins, restated as a helper for input-derived
/// expectations.
fn validity_bytes(valid: &[bool]) -> Vec<u8> {
    let mut bytes = vec![0u8; valid.len().div_ceil(8)];
    for (i, &ok) in valid.iter().enumerate() {
        if ok {
            bytes[i / 8] |= 1 << (i % 8);
        }
    }
    bytes
}
