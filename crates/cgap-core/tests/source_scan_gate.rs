//! §8.3 source-scan gate + §9 DoD 5: zero-allocation / std-free / no-sleep
//! source scan over `cgap-core/src`, with a poison positive control — a gate
//! that cannot fail is a VACUOUS GATE and must exit non-zero (here: fail the
//! test run loudly).
//!
//! Scope: every `.rs` file directly under `crates/cgap-core/src/` (the lib
//! target's actual source — `#[cfg(test)]` code included, so inline tests
//! stay under the same discipline; heap-backed courts belong in `tests/`).
//!
//! Banned classes (§3 E=0 externals + E=0 allocation; no-sleep doctrine):
//!   - heap allocation paths: `alloc::`, `vec!`, `Box`
//!   - std paths: `std::` (the kernel compiles against `core` only)
//!   - timing assumptions: `sleep` (no-sleep doctrine)
//!   - loud-unimplemented law: `todo!`, `unimplemented!` (typed refusals
//!     only — a `todo!()` in the kernel would be a silent time-bomb)

const BANNED: [(&str, &str); 7] = [
    ("alloc::", "heap allocation path"),
    ("vec!", "heap vector macro"),
    ("Box", "heap pointer type"),
    ("std::", "std path (kernel is core-only)"),
    ("sleep", "no-sleep doctrine: time is not a proxy for state"),
    ("todo!()", "unimplemented hole: use a typed refusal"),
    ("unimplemented!()", "unimplemented hole: use a typed refusal"),
];

/// Remove line/block comments (incl. `///`/`//!` doc comments) while keeping
/// string and char literals intact, so a `//` inside a string literal cannot
/// mask the code after it.
fn strip_comments(src: &str) -> String {
    let b = src.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    let mut block_depth = 0usize;
    while i < b.len() {
        if block_depth > 0 {
            if b[i] == b'/' && i + 1 < b.len() && b[i + 1] == b'*' {
                block_depth += 1;
                i += 2;
            } else if b[i] == b'*' && i + 1 < b.len() && b[i + 1] == b'/' {
                block_depth -= 1;
                i += 2;
            } else {
                if b[i] == b'\n' {
                    out.push(b'\n');
                }
                i += 1;
            }
            continue;
        }
        if b[i] == b'/' && i + 1 < b.len() && b[i + 1] == b'/' {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if b[i] == b'/' && i + 1 < b.len() && b[i + 1] == b'*' {
            block_depth = 1;
            i += 2;
            continue;
        }
        if b[i] == b'"' {
            // string literal: copy verbatim
            out.push(b'"');
            i += 1;
            while i < b.len() {
                if b[i] == b'\\' && i + 1 < b.len() {
                    out.push(b[i]);
                    out.push(b[i + 1]);
                    i += 2;
                    continue;
                }
                out.push(b[i]);
                let close = b[i] == b'"';
                i += 1;
                if close {
                    break;
                }
            }
            continue;
        }
        if b[i] == b'\'' && i + 1 < b.len() {
            // char literal ('a', '\n', '\u{1F}') vs lifetime tick ('a).
            if i + 1 < b.len() && b[i + 1] == b'\\' {
                // escaped char literal: copy through the closing quote
                out.push(b'\'');
                i += 1;
                while i < b.len() {
                    out.push(b[i]);
                    let c = b[i];
                    i += 1;
                    if c == b'\'' {
                        break;
                    }
                }
                continue;
            }
            if i + 2 < b.len() && b[i + 2] == b'\'' {
                // simple char literal 'x'
                out.push(b[i]);
                out.push(b[i + 1]);
                out.push(b[i + 2]);
                i += 3;
                continue;
            }
            // lifetime: emit as-is
            out.push(b'\'');
            i += 1;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Boundary-aware token search: a word-shaped token must not be glued into a
/// longer identifier (`Boxed`, `asleep`); punctuation-shaped tokens (`std::`,
/// `vec!`, `alloc::`) are matched wherever they occur.
fn contains_token(haystack: &str, token: &str) -> bool {
    let bytes = haystack.as_bytes();
    let t = token.as_bytes();
    let word_start = t.first().is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'_');
    let word_end = t.last().is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'_');
    let mut from = 0;
    while let Some(pos) = haystack[from..].find(token) {
        let at = from + pos;
        let before_ok = !word_start
            || at == 0
            || (!bytes[at - 1].is_ascii_alphanumeric() && bytes[at - 1] != b'_');
        let end = at + t.len();
        let after_ok = !word_end
            || end >= bytes.len()
            || (!bytes[end].is_ascii_alphanumeric() && bytes[end] != b'_');
        if before_ok && after_ok {
            return true;
        }
        from = at + 1;
    }
    false
}

fn scan_text(text: &str) -> Vec<&'static str> {
    let stripped = strip_comments(text);
    BANNED
        .iter()
        .filter(|(token, _)| contains_token(&stripped, token))
        .map(|(token, _)| *token)
        .collect()
}

/// Directory-level scan (DoD 5's gate surface): every `.rs` file directly
/// under `dir`, one violation line per (file, banned token) with the file
/// NAMED — the fail-loud contract.
fn scan_dir(dir: &std::path::Path) -> (usize, Vec<String>) {
    let mut scanned = 0usize;
    let mut violations = Vec::new();
    for entry in std::fs::read_dir(dir).expect("scan dir") {
        let path = entry.expect("dirent").path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        scanned += 1;
        let text = std::fs::read_to_string(&path).expect("read src file");
        for token in scan_text(&text) {
            violations.push(format!("{}: banned token {token:?}", path.display()));
        }
    }
    (scanned, violations)
}

#[test]
fn gate_exits_clean_on_real_cgap_core_src() {
    let manifest = env!("CARGO_MANIFEST_DIR");
    let src_dir = std::path::Path::new(manifest).join("src");
    let (scanned, violations) = scan_dir(&src_dir);
    assert!(scanned >= 4, "expected lib.rs + 3 modules under src/, found {scanned}");
    assert!(
        violations.is_empty(),
        "SOURCE_SCAN_GATE_FAIL (DoD 5):\n{}",
        violations.join("\n")
    );
}

#[test]
fn gate_fails_on_an_injected_vec_file_dod5_letter() {
    // DoD 5's letter: the gate "FAILS on an injected vec! (positive control,
    // non-vacuous)" — witnessed at FILE level: a poisoned .rs file dropped
    // into a scanned directory must be found AND named by path.
    let dir = std::env::temp_dir().join(format!("cgap-a04-scan-inject-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let clean = dir.join("clean.rs");
    std::fs::write(&clean, "pub fn f() -> u8 { 5 }\n").expect("write clean file");
    let injected = dir.join("injected_vec.rs");
    std::fs::write(
        &injected,
        "pub fn g() -> Vec<u8> {\n    let v = vec![0u8; 8];\n    v\n}\n",
    )
    .expect("write injected file");
    let (scanned, violations) = scan_dir(&dir);
    let _ = std::fs::remove_dir_all(&dir); // temp lease deleted in-file

    assert_eq!(scanned, 2, "both files must be scanned");
    assert!(
        violations.iter().any(|v| v.contains("injected_vec.rs") && v.contains("vec!")),
        "VACUOUS GATE: directory scan did not flag the injected vec! file: {violations:?}"
    );
}

#[test]
fn gate_clean_directory_yields_zero_violations() {
    // Negative control: a directory of clean source must scan clean, so the
    // positive control above is attributable to the violation, not the scan.
    let dir = std::env::temp_dir().join(format!("cgap-a04-scan-clean-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let a = dir.join("a.rs");
    std::fs::write(&a, "pub fn f() -> u8 { 5 }\n").expect("write a");
    let b = dir.join("b.rs");
    // NB: the scanner matches punctuation tokens anywhere (string literals
    // are kept intact by strip_comments by design), so "clean" here means
    // zero banned tokens even inside strings.
    std::fs::write(&b, "pub const K: &str = \"clean\";\npub fn g(x: u8) -> u8 { x + 1 }\n").expect("write b");
    let (scanned, violations) = scan_dir(&dir);
    let _ = std::fs::remove_dir_all(&dir);

    assert_eq!(scanned, 2);
    assert!(
        violations.is_empty(),
        "clean files must not trip the gate: {violations:?}"
    );
}

#[test]
fn gate_positive_control_poison_must_flip() {
    // Anti-vacuity (DoD 5): the gate must FAIL on an injected violation.
    // If the scanner accepts this poison, the gate is vacuous.
    let poison = concat!(
        "use std::vec; ",
        "let v = vec![0u8; 4]; ",
        "let b = Box::new(0u8); ",
        "extern crate alloc; ",
        "let p = std::alloc::alloc(layout); ",
        "thread::sleep(dur); ",
        "todo!(); ",
        "let z = unimplemented!(); "
    );
    let hits = scan_text(poison);
    let missing: Vec<&str> = BANNED
        .iter()
        .filter(|(token, _)| !hits.contains(token))
        .map(|(token, _)| *token)
        .collect();
    assert_eq!(
        missing,
        Vec::<&str>::new(),
        "VACUOUS GATE: scanner accepted poisoned source; missed {missing:?}"
    );
    assert_eq!(hits.len(), 7, "expected all 7 banned classes to flip, got {hits:?}");
}

#[test]
fn gate_comment_stripping_is_not_a_hole() {
    // The banned tokens live in these comments; stripping must remove them,
    // while the SAME tokens in code still trip. A gate fooled by comments is
    // a hole; a gate fooled by stripped code is vacuous — both flip here.
    let commented = "// let v = vec![0u8]; Box::new(1); std::alloc::alloc(l); sleep(1);\nlet x = 5;";
    assert!(scan_text(commented).is_empty(), "comment tokens must not trip the gate");
    let in_code = "let y = 5; Box::new(1);";
    assert_eq!(scan_text(in_code), vec!["Box"]);
}
