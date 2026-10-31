//! `doctor` — one-shot health check for the avec v26.10.31 workspace
//! (wave-10 lane 2).
//!
//! Ten checks, each GREEN / YELLOW / RED with a one-line detail and an
//! optional remediation hint. Exit 0 only if every check is GREEN;
//! exit 1 if any YELLOW (no RED), exit 2 if any RED.
//!
//! Checks:
//!   c01 toolchain        cargo + rustc respond with versions
//!   c02 wasm-targets     wasm32-unknown-unknown + wasm32-wasip1 installed
//!   c03 git              git responds
//!   c04 b3sum            b3sum on PATH
//!   c05 pyshacl          `python3 -c "import pyshacl"` succeeds
//!   c06 workspace-check  `cargo check --workspace` exit status
//!   c07 scripts          gate scripts exist and are executable
//!   c08 receipts         receipt count + staleness vs newest crates/ mtime
//!   c09 lsl-ratified     CTQ-06 LSL ratification state in ctq_report.rs
//!   c10 dirty-tree       uncommitted-entry count (informational, always GREEN)
//!
//! Usage: doctor [--json]
//! Table + summary to stdout; `--json` emits one machine-readable object.

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::UNIX_EPOCH;

// ---------------------------------------------------------------------------
// status model
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum St {
    Green,
    Yellow,
    Red,
}

impl St {
    fn label(self) -> &'static str {
        match self {
            St::Green => "GREEN",
            St::Yellow => "YELLOW",
            St::Red => "RED",
        }
    }
    fn rank(self) -> u8 {
        match self {
            St::Green => 0,
            St::Yellow => 1,
            St::Red => 2,
        }
    }
}

struct Check {
    id: &'static str,
    name: &'static str,
    st: St,
    detail: String,
    hint: Option<String>,
}

impl Check {
    fn new(id: &'static str, name: &'static str, st: St, detail: impl Into<String>) -> Self {
        Check {
            id,
            name,
            st,
            detail: detail.into(),
            hint: None,
        }
    }
    fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }
}

// ---------------------------------------------------------------------------
// process helpers
// ---------------------------------------------------------------------------

/// Run a program, capturing exit code + stdout + stderr. `None` if the
/// binary could not be spawned (not found / not executable).
fn capture(program: &str, args: &[&str]) -> Option<(i32, String, String)> {
    let out = Command::new(program).args(args).output().ok()?;
    let code = out.status.code().unwrap_or(-1);
    Some((
        code,
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    ))
}

/// First line of a captured stdout, trimmed.
fn first_line(s: &str) -> String {
    s.lines().next().unwrap_or("").trim().to_string()
}

fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' | '\r' | '\t' => out.push(' '),
            c if (c as u32) < 0x20 => {}
            c => out.push(c),
        }
    }
    out
}

// ---------------------------------------------------------------------------
// filesystem helpers
// ---------------------------------------------------------------------------

/// Repo root: walk up from cwd for a directory holding Cargo.toml + crates/
/// + scripts/; fall back to the compile-time workspace root.
fn repo_root() -> PathBuf {
    let mut dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    loop {
        if dir.join("Cargo.toml").exists()
            && dir.join("crates").is_dir()
            && dir.join("scripts").is_dir()
        {
            return dir;
        }
        match dir.parent() {
            Some(p) => dir = p.to_path_buf(),
            None => break,
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from("."))
}

/// Unix mtime (secs since epoch) of a path, 0 if unavailable.
fn mtime_secs(p: &Path) -> u64 {
    fs::metadata(p)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Newest file mtime anywhere under `root` (recursive walk).
fn newest_mtime_under(root: &Path) -> Option<u64> {
    let mut best: Option<u64> = None;
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let rd = match fs::read_dir(&dir) {
            Ok(rd) => rd,
            Err(_) => continue,
        };
        for entry in rd.flatten() {
            let path = entry.path();
            match fs::symlink_metadata(&path) {
                Ok(md) if md.is_dir() => stack.push(path),
                Ok(md) => {
                    if let Ok(modt) = md.modified() {
                        if let Ok(d) = modt.duration_since(UNIX_EPOCH) {
                            let s = d.as_secs();
                            if best.map_or(true, |b| s > b) {
                                best = Some(s);
                            }
                        }
                    }
                }
                Err(_) => {}
            }
        }
    }
    best
}

// ---------------------------------------------------------------------------
// checks
// ---------------------------------------------------------------------------

/// c01: cargo + rustc respond with a version line.
fn check_toolchain() -> Check {
    let cargo = capture("cargo", &["--version"]);
    let rustc = capture("rustc", &["--version"]);
    match (cargo, rustc) {
        (Some((0, co, _)), Some((0, ro, _))) => Check::new(
            "c01",
            "toolchain",
            St::Green,
            format!("{} / {}", first_line(&co), first_line(&ro)),
        ),
        _ => Check::new("c01", "toolchain", St::Red, "cargo or rustc missing/broken")
            .with_hint("install rustup: https://rustup.rs"),
    }
}

/// c02: wasm targets needed by crates/wasm-footprint + scripts/footprint.sh.
fn check_wasm_targets() -> Check {
    match capture("rustup", &["target", "list", "--installed"]) {
        Some((0, out, _)) => {
            let installed: Vec<String> = out.lines().map(|l| l.trim().to_string()).collect();
            let need = ["wasm32-unknown-unknown", "wasm32-wasip1"];
            let missing: Vec<String> = need
                .iter()
                .filter(|t| !installed.iter().any(|i| i.eq_ignore_ascii_case(t)))
                .map(|t| t.to_string())
                .collect();
            if missing.is_empty() {
                let have: Vec<String> = installed
                    .into_iter()
                    .filter(|t| t.starts_with("wasm32"))
                    .collect();
                let shown = if have.is_empty() {
                    "-".to_string()
                } else {
                    have.join(", ")
                };
                Check::new("c02", "wasm-targets", St::Green, format!("installed: {shown}"))
            } else {
                Check::new(
                    "c02",
                    "wasm-targets",
                    St::Yellow,
                    format!("missing: {}", missing.join(", ")),
                )
                .with_hint(format!("rustup target add {}", missing.join(" ")))
            }
        }
        _ => Check::new("c02", "wasm-targets", St::Red, "rustup not available")
            .with_hint("install rustup: https://rustup.rs"),
    }
}

/// c03: git responds.
fn check_git() -> Check {
    match capture("git", &["--version"]) {
        Some((0, out, _)) => Check::new("c03", "git", St::Green, first_line(&out)),
        _ => Check::new("c03", "git", St::Red, "git not available"),
    }
}

/// c04: b3sum on PATH (cross-check oracle for BLAKE3 KATs).
fn check_b3sum() -> Check {
    match capture("b3sum", &["--version"]) {
        Some((0, out, _)) => Check::new("c04", "b3sum", St::Green, first_line(&out)),
        _ => Check::new("c04", "b3sum", St::Yellow, "b3sum not on PATH").with_hint("brew install b3sum"),
    }
}

/// c05: pyshacl importable via python3.
fn check_pyshacl() -> Check {
    match capture("python3", &["-c", "import pyshacl"]) {
        Some((0, _, _)) => Check::new("c05", "pyshacl", St::Green, "import pyshacl ok"),
        Some((code, _, err)) => {
            let tail = err.lines().last().unwrap_or("").trim().to_string();
            let extra = if tail.is_empty() {
                String::new()
            } else {
                format!(": {tail}")
            };
            Check::new(
                "c05",
                "pyshacl",
                St::Yellow,
                format!("import failed (exit {code}){extra}"),
            )
            .with_hint("python3 -m pip install pyshacl")
        }
        None => Check::new("c05", "pyshacl", St::Yellow, "python3 not available")
            .with_hint("install python3; python3 -m pip install pyshacl"),
    }
}

/// c06: `cargo check --workspace` exit status (the compile gate).
fn check_workspace() -> Check {
    match capture("cargo", &["check", "--workspace"]) {
        Some((0, _out, err)) => {
            let warn = err
                .lines()
                .filter(|l| l.trim_start().starts_with("warning:"))
                .count();
            if warn == 0 {
                Check::new(
                    "c06",
                    "workspace-check",
                    St::Green,
                    "cargo check --workspace exit 0, 0 warnings",
                )
            } else {
                Check::new(
                    "c06",
                    "workspace-check",
                    St::Yellow,
                    format!("exit 0 with {warn} warning(s)"),
                )
                .with_hint("cargo check --workspace 2>&1 | grep -A3 warning")
            }
        }
        Some((code, _out, err)) => {
            let tail: Vec<&str> = err
                .lines()
                .rev()
                .filter(|l| !l.trim().is_empty())
                .take(3)
                .collect();
            let mut parts: Vec<String> = tail.into_iter().rev().map(|l| l.trim().to_string()).collect();
            parts.dedup();
            Check::new("c06", "workspace-check", St::Red, format!("exit {code}: {}", parts.join(" | ")))
                .with_hint("cargo check --workspace 2>&1 | head -50")
        }
        None => Check::new("c06", "workspace-check", St::Red, "cargo not spawnable"),
    }
}

/// c07: gate scripts present and executable. Missing -> YELLOW (dod.sh is
/// expected to land via wave-10 lane 1); exists-but-not-executable -> RED.
fn check_scripts(root: &Path) -> Check {
    const SCRIPTS: [&str; 7] = [
        "verify_v26.10.31.sh",
        "audit_branchless.sh",
        "cfg_homology.sh",
        "validate_ontology.sh",
        "mint_receipt.sh",
        "perf_guard.sh",
        "dod.sh",
    ];
    let mut missing: Vec<&str> = vec![];
    let mut notx: Vec<&str> = vec![];
    for s in SCRIPTS {
        let p = root.join("scripts").join(s);
        match fs::metadata(&p) {
            Ok(md) if md.is_file() => {
                use std::os::unix::fs::PermissionsExt;
                if md.permissions().mode() & 0o111 == 0 {
                    notx.push(s);
                }
            }
            _ => missing.push(s),
        }
    }
    if missing.is_empty() && notx.is_empty() {
        return Check::new("c07", "scripts", St::Green, "7/7 gate scripts executable");
    }
    let mut detail = String::new();
    if !missing.is_empty() {
        detail.push_str(&format!("missing: {}", missing.join(", ")));
    }
    if !notx.is_empty() {
        if !detail.is_empty() {
            detail.push_str("; ");
        }
        detail.push_str(&format!("not executable: {}", notx.join(", ")));
    }
    let st = if !notx.is_empty() { St::Red } else { St::Yellow };
    let hint = if !notx.is_empty() {
        format!("chmod +x scripts/{}", notx.join(" scripts/"))
    } else {
        format!(
            "expected at scripts/{} (lane-1 owner may not have landed yet)",
            missing.join(", scripts/")
        )
    };
    Check::new("c07", "scripts", st, detail).with_hint(hint)
}

/// c08: receipts — count + coarse staleness (newest crates/ mtime vs newest
/// receipt mtime). Code newer than the newest receipt -> YELLOW.
fn check_receipts(root: &Path) -> Check {
    let dir = root.join("docs/jira/v26.10.31/receipts");
    let rd = match fs::read_dir(&dir) {
        Ok(rd) => rd,
        Err(e) => {
            return Check::new("c08", "receipts", St::Red, format!("receipts dir unreadable: {e}"))
                .with_hint("expected docs/jira/v26.10.31/receipts/")
        }
    };
    let mut count = 0usize;
    let mut newest_receipt: Option<u64> = None;
    for e in rd.flatten() {
        let p = e.path();
        if p.extension().map_or(false, |x| x == "md") {
            count += 1;
            let m = mtime_secs(&p);
            if newest_receipt.map_or(true, |b| m > b) {
                newest_receipt = Some(m);
            }
        }
    }
    let crates_newest = newest_mtime_under(&root.join("crates"));
    let stale = matches!((crates_newest, newest_receipt), (Some(c), Some(r)) if c > r);
    let detail = format!(
        "{count} receipts; {}",
        match (crates_newest, newest_receipt) {
            (Some(c), Some(r)) => {
                if c > r {
                    format!("newest crates/ mtime newer than newest receipt by {}s", c - r)
                } else {
                    format!("receipts not stale (newest receipt newer by {}s)", r - c)
                }
            }
            _ => "staleness incomparable".to_string(),
        }
    );
    if stale {
        Check::new("c08", "receipts", St::Yellow, detail)
            .with_hint("mint a fresh receipt for the newest code change (scripts/mint_receipt.sh)")
    } else {
        Check::new("c08", "receipts", St::Green, detail)
    }
}

/// c09: CTQ-06 LSL ratification — parse ctq_report.rs for
/// `const LSL_RATIFIED: Option<f64> = ...`.
fn check_lsl(root: &Path) -> Check {
    let p = root.join("crates/bench/src/bin/ctq_report.rs");
    let src = match fs::read_to_string(&p) {
        Ok(s) => s,
        Err(e) => {
            return Check::new("c09", "lsl-ratified", St::Yellow, format!("ctq_report.rs unreadable: {e}"))
                .with_hint("expected crates/bench/src/bin/ctq_report.rs")
        }
    };
    let line = src
        .lines()
        .find(|l| l.contains("const LSL_RATIFIED: Option<f64> ="));
    match line {
        Some(l) => {
            let rhs = l
                .split('=')
                .last()
                .unwrap_or("")
                .trim()
                .trim_end_matches(';')
                .trim();
            if rhs.starts_with("Some") {
                Check::new("c09", "lsl-ratified", St::Green, format!("RATIFIED ({rhs})"))
            } else {
                Check::new(
                    "c09",
                    "lsl-ratified",
                    St::Yellow,
                    "CTQ-06 ratification pending (59-ratification-package.md)",
                )
            }
        }
        None => Check::new(
            "c09",
            "lsl-ratified",
            St::Yellow,
            "LSL_RATIFIED const not found in ctq_report.rs",
        )
        .with_hint("upstream rename? inspect crates/bench/src/bin/ctq_report.rs"),
    }
}

/// c10: dirty-tree entry count (informational; always GREEN).
fn check_dirty_tree() -> Check {
    match capture("git", &["status", "--porcelain"]) {
        Some((0, out, _)) => {
            let n = out.lines().filter(|l| !l.trim().is_empty()).count();
            Check::new(
                "c10",
                "dirty-tree",
                St::Green,
                format!("{n} uncommitted entries (informational)"),
            )
        }
        _ => Check::new(
            "c10",
            "dirty-tree",
            St::Green,
            "git status unavailable (informational)",
        ),
    }
}

// ---------------------------------------------------------------------------
// rendering
// ---------------------------------------------------------------------------

fn render_table(checks: &[Check], root: &Path) -> String {
    let id_w = checks.iter().map(|c| c.id.len()).max().unwrap_or(5).max(5);
    let name_w = checks.iter().map(|c| c.name.len()).max().unwrap_or(4).max(4);
    let st_w = 6; // "YELLOW"
    let mut out = String::new();
    out.push_str("avec doctor — v26.10.31 workspace health check\n");
    out.push_str(&format!("repo: {}\n\n", root.display()));
    out.push_str(&format!(
        "{:<id_w$}  {:<name_w$}  {:<st_w$}  DETAIL\n",
        "CHECK", "NAME", "STATUS"
    ));
    let rule = format!(
        "{}  {}  {}  {}\n",
        "-".repeat(id_w),
        "-".repeat(name_w),
        "-".repeat(st_w),
        "-".repeat(40)
    );
    out.push_str(&rule);
    for c in checks {
        out.push_str(&format!(
            "{:<id_w$}  {:<name_w$}  {:<st_w$}  {}\n",
            c.id,
            c.name,
            c.st.label(),
            c.detail
        ));
    }
    out
}

fn render_summary(checks: &[Check]) -> (String, St) {
    let green = checks.iter().filter(|c| c.st == St::Green).count();
    let yellow = checks.iter().filter(|c| c.st == St::Yellow).count();
    let red = checks.iter().filter(|c| c.st == St::Red).count();
    let overall = checks
        .iter()
        .map(|c| c.st)
        .max_by_key(|s| s.rank())
        .unwrap_or(St::Green);
    let mut s = String::new();
    s.push_str(&format!(
        "\nsummary: {green} GREEN, {yellow} YELLOW, {red} RED of {} checks\n",
        checks.len()
    ));
    let exit = match overall {
        St::Green => 0,
        St::Yellow => 1,
        St::Red => 2,
    };
    s.push_str(&format!("overall: {} (exit {exit})\n", overall.label()));
    let hints: Vec<&Check> = checks
        .iter()
        .filter(|c| c.st != St::Green && c.hint.is_some())
        .collect();
    if !hints.is_empty() {
        s.push_str("hints:\n");
        for h in hints {
            s.push_str(&format!(
                "  - {} ({}): {}\n",
                h.id,
                h.name,
                h.hint.clone().unwrap_or_default()
            ));
        }
    }
    (s, overall)
}

fn render_json(checks: &[Check], overall: St, root: &Path) -> String {
    let mut rows: Vec<String> = vec![];
    for c in checks {
        let hint = match (&c.hint, c.st) {
            (Some(h), St::Yellow) | (Some(h), St::Red) => format!("\"{}\"", esc(h)),
            _ => "null".to_string(),
        };
        rows.push(format!(
            "{{\"id\":\"{}\",\"status\":\"{}\",\"detail\":\"{}\",\"hint\":{}}}",
            c.id,
            c.st.label(),
            esc(&c.detail),
            hint
        ));
    }
    let green = checks.iter().filter(|c| c.st == St::Green).count();
    let yellow = checks.iter().filter(|c| c.st == St::Yellow).count();
    let red = checks.iter().filter(|c| c.st == St::Red).count();
    format!(
        "{{\"overall\":\"{}\",\"checks\":[{}],\"summary\":{{\"green\":{},\"yellow\":{},\"red\":{},\"total\":{}}},\"repo\":\"{}\"}}\n",
        overall.label(),
        rows.join(","),
        green,
        yellow,
        red,
        checks.len(),
        esc(&root.display().to_string())
    )
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

fn main() {
    let json = std::env::args().any(|a| a == "--json");
    let root = repo_root();
    let checks = vec![
        check_toolchain(),
        check_wasm_targets(),
        check_git(),
        check_b3sum(),
        check_pyshacl(),
        check_workspace(),
        check_scripts(&root),
        check_receipts(&root),
        check_lsl(&root),
        check_dirty_tree(),
    ];
    let (summary, overall) = render_summary(&checks);
    if json {
        print!("{}", render_json(&checks, overall, &root));
    } else {
        print!("{}", render_table(&checks, &root));
        print!("{summary}");
    }
    let _ = std::io::stdout().flush();
    let code = match overall {
        St::Green => 0,
        St::Yellow => 1,
        St::Red => 2,
    };
    std::process::exit(code);
}
