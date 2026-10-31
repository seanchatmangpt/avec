# Receipt: gate-arrow-tests-v3

| Field | Value |
|---|---|
| Receipt ID | gate-arrow-tests-v3 |
| Date | 2026-10-31T04:11:36Z |
| Subject | minted by scripts/mint_receipt.sh |
| Command | `cargo test -p avec-arrow --lib` |
| Exit | 0 |
| Machine | ProductName:		macOS ProductVersion:		26.2 BuildVersion:		25C56 Mac15,9 |
| Input SHA (git HEAD) | ead40329a699878a91b070378066b5e50541ba33 |
| Tree state | dirty (70 entries) |
| Evidence digest (blake3 of stdout tail) | `75b034dcbc2cf2fc12d9a682b829f051cbe50f4da0eecf6c8ac018cc34c57404` |
| Standing | ALIVE |

## Standing rule

`ALIVE` requires exit 0 **and** non-empty captured evidence. Anything else is
`FAILED` (with exit code) — a receipt is never silently minted from a failing
or evidence-free run.

<!-- auto-generated block; part of the receipt -->

## Evidence (stdout tail, last 40 lines)

```text
test runend::tests::all_distinct_extreme ... ok
test runend::tests::alignment_law_holds_via_buf_api ... ok
test runend::tests::empty_round_trip ... ok
test runend::tests::equal_neighbors_merge_boundaries_at_run_edges ... ok
test runend::tests::format_code_is_plus_r ... ok
test runend::tests::round_trip_encode_decode ... ok
test runend::tests::single_run_extreme ... ok
test silicon::silicon_tests::dispatch_paths_by_target ... ok
test silicon::silicon_tests::neon_matches_scalar_n0 ... ok
test silicon::silicon_tests::neon_matches_scalar_n1 ... ok
test silicon::silicon_tests::neon_matches_scalar_n63 ... ok
test silicon::silicon_tests::neon_matches_scalar_n64 ... ok
test silicon::silicon_tests::neon_matches_scalar_n65 ... ok
test slice::tests::empty_slice_when_offset_eq_len ... ok
test slice::tests::misaligned_window_offset3_len10_reads_parent_cells ... ok
test slice::tests::slice_bounds_errors_are_typed_not_panics ... ok
test timestamps::tests::builder_round_trip_through_i64_layout ... ok
test timestamps::tests::format_codes_match_layout_prefixes ... ok
test timestamps::tests::i64_layout_parity ... ok
test runend::tests::get_beyond_logical_len_panics_loudly - should panic ... ok
test runend::tests::decode_wrong_size_panics_loudly - should panic ... ok
test transpose::tests::odd_length_remainder_is_correct ... ok
test transpose::tests::digest_buffer_len_is_len_times_32 ... ok
test transpose::tests::size_and_align_are_64 ... ok
test transpose::tests::zero_length_yields_empty_columns ... ok
test varlen::tests::large_offsets_u64 ... ok
test blake3::tests::test_official_blake3_vectors_multi_block_and_merkle_tree ... ok
test varlen::tests::offsets_buffer_aligned_64 ... ok
test varlen::tests::utf8_round_trip_u32 ... ok
test views::tests::branchless_inline_classification ... ok
test views::tests::inline_and_foreign_rows ... ok
test silicon::silicon_tests::neon_matches_scalar_n8192 ... ok
test transpose::tests::round_trip_random_corpora ... ok
test predicates::tests::filter_matches_reference_on_random_corpora ... ok
test blake3::tests::test_simd_parent_reduction_parity_64_to_128_chunks ... ok
test slice::tests::validity_and_values_coherence_vs_scalar_recompute ... ok
test slice::tests::zero_copy_1000_slices_of_1m_rows_allocate_zero ... ok

test result: ok. 93 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
```

<!-- Tree state: dirty (70 entries) -->
