//! FR-06 conformance: official BLAKE3 known-answer tests (external integration test).
//!
//! Corpus generator follows the canonical BLAKE3 test_vectors.json recipe:
//! input byte i = (i % 251), for input lengths spanning chunk/block/tree boundaries.
//!
//! Expected digests derived with `b3sum` (Homebrew BLAKE3 reference implementation,
//! verified byte-identical to the published values in
//! BLAKE3-team/BLAKE3 test_vectors.json for every length where a published value exists:
//! 0, 1, 63, 64, 65, 127, 128, 1023, 1024, 1025, 2048, 2049, 4096, 8192).
//!
//! API coverage note (loud gap, no faked coverage): cgap_blake3 exposes only the
//! plain (keyless, extended-hash-default) 32-byte hash surface — `hash`, `hash_wide_simd`,
//! plus the raw `compress`/`hash_chunk` primitives. There is NO keyed_hash, NO
//! key_derivation (context mode), and NO extendable output (XOF) API, so the
//! keyed/derived-key sections of the official corpus are NOT covered here.

use cgap_blake3::{hash, hash_wide_simd};

/// (input_len, expected_hex) — extended hash, first 32 bytes of the official output.
const CORPUS: &[(usize, &str)] = &[
    (0, "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"),
    (1, "2d3adedff11b61f14c886e35afa036736dcd87a74d27b5c1510225d0f592e213"),
    (2, "7b7015bb92cf0b318037702a6cdd81dee41224f734684c2c122cd6359cb1ee63"),
    (3, "e1be4d7a8ab5560aa4199eea339849ba8e293d55ca0a81006726d184519e647f"),
    (7, "3f8770f387faad08faa9d8414e9f449ac68e6ff0417f673f602a646a891419fe"),
    (8, "2351207d04fc16ade43ccab08600939c7c1fa70a5c0aaca76063d04c3228eaeb"),
    (63, "e9bc37a594daad83be9470df7f7b3798297c3d834ce80ba85d6e207627b7db7b"),
    (64, "4eed7141ea4a5cd4b788606bd23f46e212af9cacebacdc7d1f4c6dc7f2511b98"),
    (65, "de1e5fa0be70df6d2be8fffd0e99ceaa8eb6e8c93a63f2d8d1c30ecb6b263dee"),
    (127, "d81293fda863f008c09e92fc382a81f5a0b4a1251cba1634016a0f86a6bd640d"),
    (128, "f17e570564b26578c33bb7f44643f539624b05df1a76c81f30acd548c44b45ef"),
    (129, "683aaae9f3c5ba37eaaf072aed0f9e30bac0865137bae68b1fde4ca2aebdcb12"),
    (255, "cb97b80a66306dd2d4f1ab7ff9fd17d3d62d88c974e8daf0ea9fbd0b1ae1b1c1"),
    (256, "f462b63aae56ed9fb899ad8eb93aa35d3dd62773fda9c33bfe20f9dab5d3df5f"),
    (257, "3d41df314e2c7af6919d994b391780a7d8abb9a57b1abf64e04ec5d49428788e"),
    (511, "7469b385b5290a1011288fc80a7fdb8677497dbc1d3b6daf93667725f68708f5"),
    (512, "87aa0321ee04decf72d6fe8d5799d6216db538c0da4a367d6d456643e9ea7994"),
    (1023, "10108970eeda3eb932baac1428c7a2163b0e924c9a9e25b35bba72b28f70bd11"),
    (1024, "42214739f095a406f3fc83deb889744ac00df831c10daa55189b5d121c855af7"),
    (1025, "d00278ae47eb27b34faecf67b4fe263f82d5412916c1ffd97c8cb7fb814b8444"),
    (2048, "e776b6028c7cd22a4d0ba182a8bf62205d2ef576467e838ed6f2529b85fba24a"),
    (3072, "b98cb0ff3623be03326b373de6b9095218513e64f1ee2edd2525c7ad1e5cffd2"),
    (4096, "015094013f57a5277b59d8475c0501042c0b642e531b0a1c8f58d2163229e969"),
    (4097, "9b4052b38f1c5fc8b1f9ff7ac7b27cd242487b3d890d15c96a1c25b8aa0fb995"),
    (8192, "aae792484c8efe4f19e2ca7d371d8c467ffb10748d8a5a1ae579948f718a2a63"),
];

fn corpus_input(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i % 251) as u8).collect()
}

fn hex_to_32(hex: &str) -> [u8; 32] {
    assert_eq!(hex.len(), 64, "expected digest must be 64 hex chars");
    let mut out = [0u8; 32];
    for i in 0..32 {
        out[i] = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).expect("valid hex");
    }
    out
}

#[test]
fn official_vectors_hash_known_answer() {
    for &(len, expected_hex) in CORPUS {
        let input = corpus_input(len);
        let actual = hash(&input);
        assert_eq!(
            actual,
            hex_to_32(expected_hex),
            "official BLAKE3 vector mismatch at input_len={}",
            len
        );
    }
}

#[test]
fn differential_hash_wide_simd_matches_hash_on_full_corpus() {
    // Differential court: the SIMD parent-reduction path must be byte-identical
    // to the scalar stack on every corpus input.
    for &(len, _) in CORPUS {
        let input = corpus_input(len);
        assert_eq!(
            hash_wide_simd(&input),
            hash(&input),
            "hash_wide_simd diverged from hash at input_len={}",
            len
        );
    }
}

#[test]
fn chunk_boundary_1024_single_chunk_root() {
    // Exactly one full chunk (1024 B): ROOT flag set on a single-chunk node.
    let input = corpus_input(1024);
    let expected = hex_to_32("42214739f095a406f3fc83deb889744ac00df831c10daa55189b5d121c855af7");
    assert_eq!(hash(&input), expected);
    assert_eq!(hash_wide_simd(&input), expected);
}

#[test]
fn chunk_boundary_2048_two_chunk_parent_root() {
    // Two full chunks: exactly one PARENT|ROOT merge.
    let input = corpus_input(2048);
    let expected = hex_to_32("e776b6028c7cd22a4d0ba182a8bf62205d2ef576467e838ed6f2529b85fba24a");
    assert_eq!(hash(&input), expected);
    assert_eq!(hash_wide_simd(&input), expected);
}

#[test]
fn chunk_boundary_3072_three_chunk_unbalanced_tree() {
    // Three full chunks: unbalanced tree (chunk 2 merges with parent of 0,1).
    let input = corpus_input(3072);
    let expected = hex_to_32("b98cb0ff3623be03326b373de6b9095218513e64f1ee2edd2525c7ad1e5cffd2");
    assert_eq!(hash(&input), expected);
    assert_eq!(hash_wide_simd(&input), expected);
}

#[test]
fn simd_subtree_boundary_65536_and_66560() {
    // 64-chunk (64 KiB) input exercises the full 64->2 SIMD subtree path with a
    // non-SIMD final root; 65 KiB forces one full SIMD subtree plus one scalar chunk.
    for len in [64 * 1024, 65 * 1024] {
        let input = corpus_input(len);
        assert_eq!(
            hash_wide_simd(&input),
            hash(&input),
            "SIMD subtree boundary divergence at len={}",
            len
        );
    }
}
