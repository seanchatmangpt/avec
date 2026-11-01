//! Manifest + README court — lane A04 §9 DoD 1 (zero-deps) and DoD 10
//! (P28-SIL license URN cited, no SPDX identifiers) — Wave 4 hardening:
//! the two prose DoD items become executed assertions instead of prose
//! claims. Reads the real manifest/README from disk (real collaborators,
//! no mocks).

const MANIFEST: &str = include_str!("../Cargo.toml");
const README: &str = include_str!("../README.md");

/// FROZEN license URN (docs/jira/v26.11.1/03-rename-receipt.md decision D2;
/// the lane doc §6.2's `urn:cgap:...` draft spelling yields to D2).
const URN: &str = "urn:avec:license:p28-sil:v2.1.0-sovereign";

#[test]
fn dod1_manifest_has_zero_runtime_dependencies() {
    // §3: "Any external crate dependency" is out of scope; the ONLY named
    // exception is the #[cfg(test)]-gated dev-dependency (cgap-blake3).
    assert!(
        !MANIFEST.contains("[dependencies]"),
        "DoD 1: cgap-core must carry zero runtime dependencies (E=0 externals); \
         found a [dependencies] section"
    );
    assert!(
        MANIFEST.contains("[dev-dependencies]"),
        "court sanity: the named dev-dependency exception is expected to be present"
    );
    assert!(
        MANIFEST.contains("cgap-blake3"),
        "court sanity: the dev-dependency must be the doc-named digest substrate"
    );
}

#[test]
fn dod10_p28_urn_cited_in_manifest_and_readme() {
    assert!(
        MANIFEST.contains(URN),
        "DoD 10: the P28-SIL v2.1.0 license URN must be cited in Cargo.toml"
    );
    assert!(
        README.contains(URN),
        "DoD 10: the P28-SIL v2.1.0 license URN must be cited in README.md"
    );
}

#[test]
fn dod10_no_spdx_license_identifiers_anywhere() {
    // DoD 10's letter: "no SPDX identifiers" — no Cargo `license` /
    // `license-file` keys (the SPDX expression slots), no LicenseRef-* or
    // SPDX token shapes, in either file, code or prose. The URN is the only
    // lawful license instrument reference.
    for (name, text) in [("Cargo.toml", MANIFEST), ("README.md", README)] {
        for (i, line) in text.lines().enumerate() {
            let spdx_shape = line.contains("LicenseRef-")
                || line.contains("license =")
                || line.contains("license-file");
            assert!(
                !spdx_shape,
                "DoD 10: SPDX-shaped license reference in {name}:{}: {line}",
                i + 1
            );
        }
    }
}
