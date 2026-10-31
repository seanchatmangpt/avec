//! `status` — wave-10 lane 11: the v26.10.31 milestone dashboard in one
//! command.
//!
//! Sources (each degrades to a loud SKIP section, never a silent pass):
//! - CTQ gates: executes `target/release/ctq_report` (the tool's own job,
//!   ~7 s wall) and parses its JSON verdict. Default: run it if present,
//!   else SKIP with a `just bench` hint. `--run` additionally release-builds
//!   the bench crate first when the binary is missing.
//! - Gate ladder: parses the 10-row Gate Summary table in
//!   docs/jira/v26.10.31/verify-latest.md.
//! - Receipts: per-file tally of `| Standing |` rows across
//!   docs/jira/v26.10.31/receipts/*.md (template.md/README.md excluded;
//!   files without a Standing row are counted and named loudly).
//! - Decisions: `LSL_RATIFIED` in ctq_report.rs (CTQ-06 ratification), the
//!   Amendment `Status:` line in 20-prd.md, and the dirty-entry count from
//!   `git status --porcelain` (read-only git only).
//!
//! Exit 0 always — a dashboard reports, it does not gate. Gate authority
//! stays with ctq_report (exit 1 on FAIL) and the verify ladder. `--json`
//! emits the same data as the aligned table render, machine-readable.

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

type Res<T> = Result<T, String>;

// ================================================================ tiny JSON

/// Minimal JSON value covering exactly what the collected sources emit:
/// objects, arrays, strings, numbers, bools, null.
#[derive(Debug, Clone)]
enum J {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<J>),
    Obj(Vec<(String, J)>),
}

impl J {
    fn get(&self, key: &str) -> Option<&J> {
        match self {
            J::Obj(kvs) => kvs.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }
    fn as_str(&self) -> Option<&str> {
        match self {
            J::Str(s) => Some(s),
            _ => None,
        }
    }
    fn as_arr(&self) -> Option<&[J]> {
        match self {
            J::Arr(a) => Some(a),
            _ => None,
        }
    }
    /// Integers render without a `.0` tail; everything else via `{}`.
    fn num_str(&self) -> Option<String> {
        match self {
            J::Num(n) => Some(if n.fract() == 0.0 && n.abs() < 1e15 {
                format!("{}", *n as i64)
            } else {
                format!("{n}")
            }),
            _ => None,
        }
    }
}

struct P<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> P<'a> {
    fn ws(&mut self) {
        while self.i < self.b.len() && self.b[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }
    fn value(&mut self) -> Res<J> {
        self.ws();
        match self.b.get(self.i) {
            None => Err("unexpected end of JSON".into()),
            Some(b'{') => self.object(),
            Some(b'[') => self.array(),
            Some(b'"') => Ok(J::Str(self.string()?)),
            Some(b't') => self.lit("true", J::Bool(true)),
            Some(b'f') => self.lit("false", J::Bool(false)),
            Some(b'n') => self.lit("null", J::Null),
            Some(c) if *c == b'-' || c.is_ascii_digit() => self.number(),
            Some(c) => Err(format!("unexpected byte 0x{c:02x} at {}", self.i)),
        }
    }
    fn lit(&mut self, word: &str, v: J) -> Res<J> {
        if self.b[self.i..].starts_with(word.as_bytes()) {
            self.i += word.len();
            Ok(v)
        } else {
            Err(format!("bad literal at byte {}", self.i))
        }
    }
    fn object(&mut self) -> Res<J> {
        self.i += 1; // consume '{'
        let mut kvs: Vec<(String, J)> = Vec::new();
        self.ws();
        if self.b.get(self.i) == Some(&b'}') {
            self.i += 1;
            return Ok(J::Obj(kvs));
        }
        loop {
            self.ws();
            if self.b.get(self.i) != Some(&b'"') {
                return Err(format!("expected object key at byte {}", self.i));
            }
            let k = self.string()?;
            self.ws();
            if self.b.get(self.i) != Some(&b':') {
                return Err(format!("expected ':' at byte {}", self.i));
            }
            self.i += 1;
            let v = self.value()?;
            kvs.push((k, v));
            self.ws();
            match self.b.get(self.i) {
                Some(&b',') => self.i += 1,
                Some(&b'}') => {
                    self.i += 1;
                    return Ok(J::Obj(kvs));
                }
                _ => return Err(format!("expected ',' or '}}' at byte {}", self.i)),
            }
        }
    }
    fn array(&mut self) -> Res<J> {
        self.i += 1; // consume '['
        let mut out: Vec<J> = Vec::new();
        loop {
            self.ws();
            if self.b.get(self.i) == Some(&b']') {
                self.i += 1;
                return Ok(J::Arr(out));
            }
            out.push(self.value()?);
            self.ws();
            match self.b.get(self.i) {
                Some(&b',') => self.i += 1,
                Some(&b']') => {
                    self.i += 1;
                    return Ok(J::Arr(out));
                }
                _ => return Err(format!("expected ',' or ']' at byte {}", self.i)),
            }
        }
    }
    fn number(&mut self) -> Res<J> {
        let start = self.i;
        if self.b.get(self.i) == Some(&b'-') {
            self.i += 1;
        }
        while self
            .b
            .get(self.i)
            .map(|c| c.is_ascii_digit() || matches!(c, b'.' | b'e' | b'E' | b'+' | b'-'))
            .unwrap_or(false)
        {
            self.i += 1;
        }
        let txt = std::str::from_utf8(&self.b[start..self.i])
            .map_err(|_| "utf8 in number".to_string())?;
        txt.parse::<f64>()
            .map(J::Num)
            .map_err(|e| format!("bad number '{txt}': {e}"))
    }
    fn string(&mut self) -> Res<String> {
        self.i += 1; // consume opening quote
        let mut out = String::new();
        loop {
            match self.b.get(self.i) {
                None => return Err("unterminated string".into()),
                Some(&b'"') => {
                    self.i += 1;
                    return Ok(out);
                }
                Some(&b'\\') => {
                    self.i += 1;
                    match self.b.get(self.i) {
                        Some(&b'"') => out.push('"'),
                        Some(&b'\\') => out.push('\\'),
                        Some(&b'/') => out.push('/'),
                        Some(&b'b') => out.push('\u{0008}'),
                        Some(&b'f') => out.push('\u{000C}'),
                        Some(&b'n') => out.push('\n'),
                        Some(&b'r') => out.push('\r'),
                        Some(&b't') => out.push('\t'),
                        Some(&b'u') => {
                            let hex4 = std::str::from_utf8(
                                self.b
                                    .get(self.i + 1..self.i + 5)
                                    .ok_or_else(|| "truncated \\u escape".to_string())?,
                            )
                            .map_err(|_| "utf8 in \\u escape".to_string())?;
                            let cp = u32::from_str_radix(hex4, 16)
                                .map_err(|e| format!("bad \\u escape: {e}"))?;
                            out.push(char::from_u32(cp).unwrap_or('\u{FFFD}'));
                            self.i += 4;
                        }
                        _ => return Err(format!("bad escape at byte {}", self.i)),
                    }
                    self.i += 1;
                }
                Some(&c) => {
                    // Consume one UTF-8 scalar; sources are our own UTF-8 tools.
                    let s = std::str::from_utf8(&self.b[self.i..])
                        .map_err(|_| "utf8 in string body".to_string())?;
                    let ch = s.chars().next().ok_or_else(|| "empty body".to_string())?;
                    if c >= 0x80 && ch.len_utf8() == 1 {
                        return Err(format!("invalid utf8 continuation at byte {}", self.i));
                    }
                    out.push(ch);
                    self.i += ch.len_utf8();
                }
            }
        }
    }
}

/// Parse a complete JSON document.
fn parse_json(txt: &str) -> Res<J> {
    let mut p = P { b: txt.as_bytes(), i: 0 };
    let v = p.value()?;
    p.ws();
    Ok(v)
}

/// Escape a string for JSON output.
fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// Serialize a J value back to JSON text.
fn dump(j: &J) -> String {
    match j {
        J::Null => "null".into(),
        J::Bool(b) => b.to_string(),
        J::Num(n) => {
            if n.fract() == 0.0 && n.abs() < 1e15 {
                format!("{}", *n as i64)
            } else {
                format!("{n}")
            }
        }
        J::Str(s) => format!("\"{}\"", esc(s)),
        J::Arr(a) => format!("[{}]", a.iter().map(dump).collect::<Vec<_>>().join(",")),
        J::Obj(kvs) => format!(
            "{{{}}}",
            kvs.iter()
                .map(|(k, v)| format!("\"{}\":{}", esc(k), dump(v)))
                .collect::<Vec<_>>()
                .join(",")
        ),
    }
}

// ============================================================= collections

fn log(msg: &str) {
    let _ = writeln!(std::io::stderr(), "[status] {msg}");
}

/// Repo root: $AVEC_REPO_ROOT, else walk up from cwd hunting for `.git`.
fn repo_root() -> Res<PathBuf> {
    if let Ok(r) = std::env::var("AVEC_REPO_ROOT") {
        let p = PathBuf::from(&r);
        if p.join(".git").exists() {
            return Ok(p);
        }
        return Err(format!("AVEC_REPO_ROOT={r} has no .git"));
    }
    let mut cur = std::env::current_dir().map_err(|e| format!("cwd: {e}"))?;
    loop {
        if cur.join(".git").exists() {
            return Ok(cur);
        }
        if !cur.pop() || cur == Path::new("/") {
            return Err("no repo root found (no .git upward from cwd)".into());
        }
    }
}

/// Unix seconds -> ISO-8601 UTC (civil-from-days, Howard Hinnant's algorithm).
fn iso8601_now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let days = secs.div_euclid(86_400);
    let sod = secs.rem_euclid(86_400);
    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_097) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        y,
        m,
        d,
        sod / 3600,
        (sod % 3600) / 60,
        sod % 60
    )
}

// ---------------------------------------------------------------- FR rows

struct FrRow {
    id: String,
    item: String,
    status: String,
    detail: String,
}

/// Parse the FR requirement table out of 20-prd.md: lines beginning `| FR-`.
fn parse_fr_table(prd: &str) -> Vec<FrRow> {
    prd.lines()
        .filter(|l| l.starts_with("| FR-"))
        .filter_map(|l| {
            let cells: Vec<&str> = l.split('|').collect();
            if cells.len() < 6 {
                return None;
            }
            // ["", " FR-01 ", " Requirement ", " Statement ", " Status ", " Evidence ", ""]
            Some(FrRow {
                id: cells[1].trim().to_string(),
                item: cells[2].trim().to_string(),
                status: cells[4].trim().to_string(),
                detail: cells[5].trim().to_string(),
            })
        })
        .collect()
}

// -------------------------------------------------------------- gate rows

struct GateRow {
    gate: String,
    status: String,
    exit: String,
}

/// Parse the Gate Summary table in verify-latest.md: rows shaped
/// `| \`gate-name\` | PASS | 0 |`.
fn parse_gate_table(md: &str) -> Vec<GateRow> {
    md.lines()
        .filter(|l| l.starts_with("| `"))
        .filter_map(|l| {
            let cells: Vec<&str> = l.split('|').collect();
            if cells.len() < 4 {
                return None;
            }
            Some(GateRow {
                gate: cells[1].trim().trim_matches('`').to_string(),
                status: cells[2].trim().to_string(),
                exit: cells[3].trim().to_string(),
            })
        })
        .collect()
}

// --------------------------------------------------------------- receipts

struct ReceiptSummary {
    dir: String,
    files: usize,
    standing_rows: usize,
    no_standing: Vec<String>,
    tally: Vec<(String, usize)>,
}

impl ReceiptSummary {
    fn empty(dir: &str) -> Self {
        ReceiptSummary {
            dir: dir.to_string(),
            files: 0,
            standing_rows: 0,
            no_standing: Vec::new(),
            tally: Vec::new(),
        }
    }
}

/// Per-file Standing scan over receipts/*.md (template.md/README.md excluded
/// by name). The `{{STANDING}}` placeholder value is excluded from the tally;
/// receipt files with no Standing row are counted and named loudly.
fn scan_receipts(dir: &Path) -> Res<ReceiptSummary> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n.ends_with(".md") && n != "template.md" && n != "README.md")
        .collect();
    names.sort();

    let mut sum = ReceiptSummary::empty(&dir.display().to_string());
    sum.files = names.len();
    for name in &names {
        let txt = fs::read_to_string(dir.join(name)).map_err(|e| format!("{name}: {e}"))?;
        let value = txt
            .lines()
            .find(|l| l.contains("| Standing |"))
            .and_then(|l| l.split('|').nth(2))
            .map(|s| s.trim().to_string());
        match value {
            Some(v) if v.contains("STANDING") => {} // placeholder, not a standing
            Some(v) => {
                sum.standing_rows += 1;
                match sum.tally.iter_mut().find(|(s, _)| *s == v) {
                    Some((_, c)) => *c += 1,
                    None => sum.tally.push((v, 1)),
                }
            }
            None => sum.no_standing.push(name.clone()),
        }
    }
    sum.tally.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    Ok(sum)
}

// -------------------------------------------------------------- decisions

struct Decision {
    state: &'static str, // OPEN / RATIFIED / DIRTY / CLEAN / INFO
    topic: String,
    detail: String,
    source: String,
}

/// ctq_report.rs `const LSL_RATIFIED: Option<f64>` — the CTQ-06 ratification
/// flip, operator-owned.
fn decision_lsl(repo: &Path) -> Res<Decision> {
    let rel = "crates/bench/src/bin/ctq_report.rs";
    let txt = fs::read_to_string(repo.join(rel)).map_err(|e| format!("{rel}: {e}"))?;
    let hit = txt
        .lines()
        .enumerate()
        .find(|(_, l)| l.contains("const LSL_RATIFIED: Option<f64>"));
    let (idx, line) = hit.ok_or_else(|| "LSL_RATIFIED const not found".to_string())?;
    let lineno = idx + 1; // 1-based, matching editor line numbers
    if line.contains("None") {
        Ok(Decision {
            state: "OPEN",
            topic: "CTQ-06 LSL ratification".into(),
            detail: "LSL_RATIFIED = None; CTQ-06 gate stays REAL at the original 8.5 GiB/s \
                     (operator-owned flip; do not flip from a lane)."
                .into(),
            source: format!("{rel}:{lineno}"),
        })
    } else if let Some(v) = line.split("Some(").nth(1).and_then(|r| r.split(')').next()) {
        Ok(Decision {
            state: "RATIFIED",
            topic: "CTQ-06 LSL ratification".into(),
            detail: format!("LSL_RATIFIED = Some({v}); CTQ-06 gate runs at the ratified LSL."),
            source: format!("{rel}:{lineno}"),
        })
    } else {
        Ok(Decision {
            state: "INFO",
            topic: "CTQ-06 LSL ratification".into(),
            detail: format!("unrecognized LSL_RATIFIED line: {}", line.trim()),
            source: format!("{rel}:{lineno}"),
        })
    }
}

/// 20-prd.md Amendment `Status:` line (wave-7 CTQ-06 LSL amendment).
fn decision_amendment(repo: &Path) -> Res<Decision> {
    let rel = "docs/jira/v26.10.31/20-prd.md";
    let txt = fs::read_to_string(repo.join(rel)).map_err(|e| format!("{rel}: {e}"))?;
    let mut section = false;
    let mut hit: Option<(usize, String)> = None;
    for (idx, l) in txt.lines().enumerate() {
        if l.starts_with("## Amendment") {
            section = true;
            continue;
        }
        if section {
            if l.starts_with("## ") {
                break; // next section began; no Status line under Amendment
            }
            if l.starts_with("Status:") && hit.is_none() {
                hit = Some((idx + 1, l.trim_start_matches("Status:").trim().to_string()));
            }
        }
    }
    let (lineno, status_full) = hit
        .ok_or_else(|| "no 'Status:' line under '## Amendment' in 20-prd.md".to_string())?;
    // First sentence only — the Status line wraps; the rest restates normativity.
    let status: String = match status_full.find(". ") {
        Some(p) => status_full[..p + 1].to_string(),
        None => status_full,
    };
    let state = if status.starts_with("PROPOSED") {
        "OPEN"
    } else if status.starts_with("RATIFIED") {
        "RATIFIED"
    } else {
        "INFO"
    };
    Ok(Decision {
        state,
        topic: "CTQ-06 LSL 20-PRD amendment".into(),
        detail: format!("Amendment status: {status}"),
        source: format!("{rel}:{lineno}"),
    })
}

/// Read-only git: dirty-entry count + HEAD sha.
fn decision_tree(repo: &Path) -> Res<Decision> {
    let out = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(repo)
        .output()
        .map_err(|e| format!("git status spawn: {e}"))?;
    if !out.status.success() {
        return Err(format!("git status --porcelain exit {:?}", out.status.code()));
    }
    let n = String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|l| !l.trim().is_empty())
        .count();
    let head = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(repo)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "UNKNOWN".into());
    Ok(Decision {
        state: if n > 0 { "DIRTY" } else { "CLEAN" },
        topic: "working tree".into(),
        detail: format!("{n} uncommitted entries; HEAD {head}"),
        source: "git status --porcelain".into(),
    })
}

// --------------------------------------------------------------- CTQ run

struct CtqRow {
    id: String,
    item: String,
    status: String,
    detail: String,
}

struct CtqSection {
    kind: String,        // OK | SKIP
    detail: String,      // how the section was produced
    verdict: String,     // PASS | FAIL | SKIP
    exit_code: Option<i32>,
    elapsed_s: Option<f64>,
    failed_gates: Vec<String>,
    rows: Vec<CtqRow>,
}

/// Execute target/release/ctq_report and parse its JSON verdict.
/// `force_build`: when the binary is missing, release-build the bench crate
/// first (the `just bench` path) instead of SKIPping.
fn run_ctq(repo: &Path, force_build: bool) -> CtqSection {
    let bin = repo.join("target/release/ctq_report");
    if !bin.exists() {
        if force_build {
            log("target/release/ctq_report missing; building bench crate (release)...");
            let b = Command::new("cargo")
                .args(["build", "--release", "-p", "bench"])
                .current_dir(repo)
                .status();
            match b {
                Ok(s) if s.success() => {}
                Ok(s) => {
                    return skip_ctq(format!(
                        "cargo build --release -p bench failed (exit {:?})",
                        s.code()
                    ))
                }
                Err(e) => return skip_ctq(format!("cargo spawn failed: {e}")),
            }
        } else {
            return skip_ctq(
                "target/release/ctq_report missing; run `just bench` (or `status --run`)".into(),
            );
        }
    }
    let t0 = Instant::now();
    let out = match Command::new(&bin).stderr(Stdio::inherit()).output() {
        Ok(o) => o,
        Err(e) => return skip_ctq(format!("ctq_report spawn failed: {e}")),
    };
    let elapsed = t0.elapsed().as_secs_f64();
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let parsed = match parse_json(stdout.trim()) {
        Ok(v) => v,
        Err(e) => return skip_ctq(format!("ctq_report stdout not parseable JSON: {e}")),
    };
    let verdict = parsed
        .get("verdict")
        .and_then(J::as_str)
        .unwrap_or("UNKNOWN")
        .to_string();
    let failed_gates = parsed
        .get("failed_gates")
        .and_then(J::as_arr)
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    CtqSection {
        kind: "OK".into(),
        detail: format!(
            "executed target/release/ctq_report (exit {:?}, {elapsed:.1}s)",
            out.status.code()
        ),
        verdict,
        exit_code: out.status.code(),
        elapsed_s: Some(elapsed),
        failed_gates,
        rows: flatten_ctq(&parsed),
    }
}

fn skip_ctq(reason: String) -> CtqSection {
    CtqSection {
        kind: "SKIP".into(),
        detail: reason.clone(),
        verdict: "SKIP".into(),
        exit_code: None,
        elapsed_s: None,
        failed_gates: Vec::new(),
        rows: vec![CtqRow {
            id: "CTQ-*".into(),
            item: "ctq_report".into(),
            status: "SKIP".into(),
            detail: reason,
        }],
    }
}

/// Flatten the ctq_report JSON into display rows (CTQ order 02..06).
fn flatten_ctq(top: &J) -> Vec<CtqRow> {
    let mut rows = Vec::new();
    let sections: &[(&str, &str)] = &[
        ("ctq02_zero_alloc", "CTQ-02 zero-alloc"),
        ("ctq03_parquet_level_free_roundtrip", "CTQ-03 parquet roundtrip"),
        ("ctq04_transpose_variance", "CTQ-04 transpose variance"),
        ("ctq05_e2e_admission", "CTQ-05 e2e admission"),
        ("ctq06_blake3", "CTQ-06 blake3"),
    ];
    for (key, label) in sections {
        let obj = match top.get(key) {
            Some(o) => o,
            None => {
                rows.push(CtqRow {
                    id: (*label).split(' ').next().unwrap_or(label).to_string(),
                    item: (*label).to_string(),
                    status: "UNKNOWN".into(),
                    detail: "absent from ctq_report JSON".into(),
                });
                continue;
            }
        };
        let verdict = obj.get("verdict").and_then(J::as_str).unwrap_or("UNKNOWN");
        let gate = obj.get("gate").and_then(J::as_str).unwrap_or("");
        let metric = obj.get("metric").and_then(J::as_str).unwrap_or("");
        let value = obj
            .get("value")
            .and_then(J::num_str)
            .unwrap_or_else(|| "n/a".into());
        if *key == "ctq06_blake3" {
            let lsl = obj
                .get("lsl_gib_s")
                .and_then(J::num_str)
                .unwrap_or_else(|| "?".into());
            for r in obj.get("rows").and_then(J::as_arr).unwrap_or(&[]) {
                let path = r.get("path").and_then(J::as_str).unwrap_or("?");
                let rv = r.get("verdict").and_then(J::as_str).unwrap_or("UNKNOWN");
                let val = r.get("value").and_then(J::num_str).unwrap_or_default();
                rows.push(CtqRow {
                    id: format!("CTQ-06[{path}]"),
                    item: format!("blake3 {path}"),
                    status: rv.to_string(),
                    detail: format!("gib_per_s={val} vs LSL {lsl} (gate REAL)"),
                });
            }
        } else {
            rows.push(CtqRow {
                id: (*label).split(' ').next().unwrap_or(label).to_string(),
                item: (*label).to_string(),
                status: verdict.to_string(),
                detail: format!(
                    "{metric}={value}{}{gate}",
                    if gate.is_empty() { "" } else { " gate=" }
                ),
            });
        }
    }
    rows
}

// =============================================================== rendering

/// Render an aligned table; last column is not padded.
fn render_table(headers: &[&str], rows: &[Vec<String>]) -> String {
    let mut widths: Vec<usize> = headers.iter().map(|h| h.chars().count()).collect();
    for r in rows {
        for (i, cell) in r.iter().enumerate().take(widths.len()) {
            widths[i] = widths[i].max(cell.chars().count());
        }
    }
    let sep: Vec<String> = widths.iter().map(|w| "-".repeat(*w)).collect();
    let head_cells: Vec<String> = headers
        .iter()
        .enumerate()
        .map(|(i, h)| {
            if i + 1 == widths.len() {
                h.to_string()
            } else {
                format!("{:<w$}", h, w = widths[i])
            }
        })
        .collect();
    let mut out = String::new();
    out.push_str(&format!("  {}\n", head_cells.join("  ")));
    out.push_str(&format!("  {}\n", sep.join("  ")));
    for r in rows {
        let padded: Vec<String> = r
            .iter()
            .enumerate()
            .map(|(i, c)| {
                if i + 1 == widths.len() {
                    c.clone()
                } else {
                    format!("{:<w$}", c, w = widths[i])
                }
            })
            .collect();
        out.push_str(&format!("  {}\n", padded.join("  ")));
    }
    out
}

fn ctq_table_rows(ctq: &CtqSection) -> Vec<Vec<String>> {
    ctq.rows
        .iter()
        .map(|r| vec![r.id.clone(), r.item.clone(), r.status.clone(), r.detail.clone()])
        .collect()
}

// ==================================================================== main

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let json = args.iter().any(|a| a == "--json");
    let run = args.iter().any(|a| a == "--run");
    for a in &args {
        if !matches!(a.as_str(), "--json" | "--run") {
            eprintln!("status: unknown flag {a} (known: --json, --run)");
            std::process::exit(64);
        }
    }

    let repo = match repo_root() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("status: {e}");
            std::process::exit(66);
        }
    };
    log(&format!("repo root: {}", repo.display()));

    // ---- FR table ------------------------------------------------------
    let fr_rows: Vec<FrRow> = match fs::read_to_string(repo.join("docs/jira/v26.10.31/20-prd.md"))
    {
        Ok(t) => parse_fr_table(&t),
        Err(e) => vec![FrRow {
            id: "FR-*".into(),
            item: "20-prd.md".into(),
            status: "SKIP".into(),
            detail: format!("unreadable: {e}"),
        }],
    };

    // ---- gate ladder ----------------------------------------------------
    let gates_md = fs::read_to_string(repo.join("docs/jira/v26.10.31/verify-latest.md"));
    let gate_rows: Vec<GateRow> = gates_md
        .as_deref()
        .map(parse_gate_table)
        .unwrap_or_default();
    let gate_generated = gates_md
        .ok()
        .and_then(|md| {
            md.lines()
                .find(|l| l.starts_with("Generated:"))
                .map(|l| {
                    l.trim_start_matches("Generated:")
                        .trim()
                        .split("·")
                        .next()
                        .unwrap_or("")
                        .trim()
                        .to_string()
                })
        })
        .unwrap_or_default();

    // ---- receipts --------------------------------------------------------
    let receipts = match scan_receipts(&repo.join("docs/jira/v26.10.31/receipts")) {
        Ok(r) => r,
        Err(e) => {
            log(&format!("receipts: {e}"));
            ReceiptSummary::empty("docs/jira/v26.10.31/receipts")
        }
    };

    // ---- CTQ + decisions --------------------------------------------------
    let ctq = run_ctq(&repo, run);
    let decisions: Vec<Res<Decision>> = vec![
        decision_lsl(&repo),
        decision_amendment(&repo),
        decision_tree(&repo),
    ];

    // ---- render -----------------------------------------------------------
    if json {
        let doc = build_json(
            &fr_rows,
            &gate_rows,
            &gate_generated,
            &receipts,
            &ctq,
            &decisions,
        );
        println!("{}", dump(&doc));
    } else {
        print!(
            "{}",
            render(
                &gate_generated, &fr_rows, &gate_rows, &receipts, &ctq, &decisions
            )
        );
    }
}

/// Build the machine-JSON view of everything collected.
fn build_json(
    fr_rows: &[FrRow],
    gate_rows: &[GateRow],
    gate_generated: &str,
    receipts: &ReceiptSummary,
    ctq: &CtqSection,
    decisions: &[Res<Decision>],
) -> J {
    let fr = J::Arr(
        fr_rows
            .iter()
            .map(|r| {
                J::Obj(vec![
                    ("id".into(), J::Str(r.id.clone())),
                    ("item".into(), J::Str(r.item.clone())),
                    ("status".into(), J::Str(r.status.clone())),
                    ("evidence".into(), J::Str(r.detail.clone())),
                ])
            })
            .collect(),
    );

    let gates = J::Obj(vec![
        (
            "source".into(),
            J::Str(format!(
                "docs/jira/v26.10.31/verify-latest.md generated {gate_generated}"
            )),
        ),
        (
            "rows".into(),
            J::Arr(
                gate_rows
                    .iter()
                    .map(|g| {
                        J::Obj(vec![
                            ("gate".into(), J::Str(g.gate.clone())),
                            ("status".into(), J::Str(g.status.clone())),
                            ("exit".into(), J::Str(g.exit.clone())),
                        ])
                    })
                    .collect(),
            ),
        ),
        ("pass".into(), J::Num(gate_rows.iter().filter(|g| g.status == "PASS").count() as f64)),
        ("total".into(), J::Num(gate_rows.len() as f64)),
    ]);

    let receipts_json = J::Obj(vec![
        ("dir".into(), J::Str(receipts.dir.clone())),
        ("files".into(), J::Num(receipts.files as f64)),
        ("standing_rows".into(), J::Num(receipts.standing_rows as f64)),
        (
            "no_standing_row".into(),
            J::Arr(
                receipts
                    .no_standing
                    .iter()
                    .map(|n| J::Str(n.clone()))
                    .collect(),
            ),
        ),
        (
            "tally".into(),
            J::Arr(
                receipts
                    .tally
                    .iter()
                    .map(|(s, c)| {
                        J::Obj(vec![("standing".into(), J::Str(s.clone())), ("count".into(), J::Num(*c as f64))])
                    })
                    .collect(),
            ),
        ),
    ]);

    let ctq_json = J::Obj(vec![
        ("kind".into(), J::Str(ctq.kind.clone())),
        ("detail".into(), J::Str(ctq.detail.clone())),
        ("verdict".into(), J::Str(ctq.verdict.clone())),
        (
            "exit_code".into(),
            match ctq.exit_code {
                Some(c) => J::Num(c as f64),
                None => J::Null,
            },
        ),
        (
            "elapsed_s".into(),
            match ctq.elapsed_s {
                Some(e) => J::Num(e),
                None => J::Null,
            },
        ),
        (
            "failed_gates".into(),
            J::Arr(ctq.failed_gates.iter().map(|g| J::Str(g.clone())).collect()),
        ),
        (
            "rows".into(),
            J::Arr(
                ctq.rows
                    .iter()
                    .map(|r| {
                        J::Obj(vec![
                            ("id".into(), J::Str(r.id.clone())),
                            ("item".into(), J::Str(r.item.clone())),
                            ("status".into(), J::Str(r.status.clone())),
                            ("detail".into(), J::Str(r.detail.clone())),
                        ])
                    })
                    .collect(),
            ),
        ),
    ]);

    let decisions_json = J::Arr(
        decisions
            .iter()
            .map(|d| match d {
                Ok(d) => J::Obj(vec![
                    ("state".into(), J::Str(d.state.to_string())),
                    ("topic".into(), J::Str(d.topic.clone())),
                    ("detail".into(), J::Str(d.detail.clone())),
                    ("source".into(), J::Str(d.source.clone())),
                ]),
                Err(e) => J::Obj(vec![
                    ("state".into(), J::Str("UNAVAILABLE".into())),
                    ("topic".into(), J::Str("decision source unreadable".into())),
                    ("detail".into(), J::Str(e.clone())),
                    ("source".into(), J::Str("-".into())),
                ]),
            })
            .collect(),
    );

    J::Obj(vec![
        ("tool".into(), J::Str("status".into())),
        ("milestone".into(), J::Str("v26.10.31".into())),
        ("generated_at".into(), J::Str(iso8601_now())),
        ("fr".into(), fr),
        ("gates".into(), gates),
        ("receipts".into(), receipts_json),
        ("ctq".into(), ctq_json),
        ("decisions".into(), decisions_json),
    ])
}

/// Build the aligned human-table view.
fn render(
    gate_generated: &str,
    fr_rows: &[FrRow],
    gate_rows: &[GateRow],
    receipts: &ReceiptSummary,
    ctq: &CtqSection,
    decisions: &[Res<Decision>],
) -> String {
    let mut out = String::new();
    out.push_str("avec status — v26.10.31 milestone dashboard\n");

    // == FR / CTQ table ====================================================
    out.push_str("\n== FR / CTQ ==\n");
    let mut rows: Vec<Vec<String>> = fr_rows
        .iter()
        .map(|r| vec![r.id.clone(), r.item.clone(), r.status.clone(), r.detail.clone()])
        .collect();
    rows.extend(ctq_table_rows(ctq));
    out.push_str(&render_table(
        &["ID", "ITEM", "STATUS", "DETAIL"],
        &rows,
    ));
    out.push_str(&format!(
        "  ctq_report: {} (verdict {})\n",
        ctq.detail, ctq.verdict
    ));

    // == gates =============================================================
    out.push_str("\n== GATES (verify-latest.md) ==\n");
    if gate_rows.is_empty() {
        out.push_str("  SKIP: verify-latest.md gate table unreadable\n");
    } else {
        let g: Vec<Vec<String>> = gate_rows
            .iter()
            .map(|g| vec![g.gate.clone(), g.status.clone(), g.exit.clone()])
            .collect();
        out.push_str(&render_table(&["GATE", "STATUS", "EXIT"], &g));
        let pass = gate_rows.iter().filter(|g| g.status == "PASS").count();
        out.push_str(&format!(
            "  summary: {pass}/{} PASS (source generated {})\n",
            gate_rows.len(),
            gate_generated
        ));
    }

    // == receipts ==========================================================
    out.push_str("\n== RECEIPTS ==\n");
    if receipts.files == 0 {
        out.push_str("  SKIP: receipts dir unreadable/empty\n");
    } else {
        out.push_str(&format!(
            "  dir={} files={} standing_rows={}\n",
            receipts.dir, receipts.files, receipts.standing_rows
        ));
        for (s, c) in &receipts.tally {
            out.push_str(&format!("  {:>4}  {}\n", c, s));
        }
        for n in &receipts.no_standing {
            out.push_str(&format!("  ----  (no Standing row) {}\n", n));
        }
    }

    // == decisions ==========================================================
    out.push_str("\n== DECISIONS ==\n");
    for d in decisions {
        match d {
            Ok(d) => out.push_str(&format!(
                "  [{}] {} — {} ({})\n",
                d.state, d.topic, d.detail, d.source
            )),
            Err(e) => out.push_str(&format!("  [UNAVAILABLE] decision source unreadable — {e}\n")),
        }
    }

    out
}
