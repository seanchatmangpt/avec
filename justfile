# cgap — one-command DX recipes (wave-10 lane 3)
#
#   just doctor   - toolchain/env health probe via cgap-cli doctor bin (lands with wave-10 lane 2)
#   just verify   - full v26.10.31 verification ladder (scripts/verify_v26.10.31.sh)
#   just bench    - release-build the bench crate, then run the CTQ report binary
#   just courts   - 3 court suites sequentially with headers, fail-fast
#   just receipts - newest receipts + Standing tally from docs/jira/v26.10.31/receipts/
#   just dod      - Definition-of-Done gate (scripts/dod.sh; lands with wave-10 lane 1)
#   just status   - portfolio status bin via cgap-cli status (lands with wave-10 lane 11)
#
# Every recipe wires an existing script/bin — no logic duplicated here. `doctor`,
# `dod`, and `status` target artifacts owned by other lanes and 404 until those
# lanes land, by design.

default:
    @just --list

# Toolchain/env health probe (bin owned by wave-10 lane 2; fails until landed)
doctor:
    cargo run -q --manifest-path crates/cli/Cargo.toml --bin doctor

# Full v26.10.31 verification ladder
verify:
    bash scripts/verify_v26.10.31.sh

# Release-build the bench crate, then run the CTQ report
bench:
    cargo build --release -p bench
    target/release/ctq_report

# All 3 court suites, sequential, fail-fast, one header each
courts:
    @echo "== court 1/3: validate_ontology =="
    bash scripts/validate_ontology.sh
    @echo "== court 2/3: audit_branchless --selftest =="
    bash scripts/audit_branchless.sh --selftest
    @echo "== court 3/3: cfg_homology --mutant =="
    bash scripts/cfg_homology.sh --mutant

# Newest 20 receipts + Standing tally (STANDING template row excluded by case)
receipts:
    @ls -lt docs/jira/v26.10.31/receipts/ | head -20
    @echo ""
    @echo "Standing tally:"
    @grep -h '| Standing |' docs/jira/v26.10.31/receipts/*.md \
        | grep -v STANDING \
        | awk -F'|' '{gsub(/^[ \t]+|[ \t]+$/, "", $3); c[$3]++} END {for (s in c) printf "  %-24s %d\n", s, c[s]}' \
        | sort

# Definition-of-Done gate (script owned by wave-10 lane 1; fails until landed)
dod:
    @if [ -f scripts/dod.sh ]; then bash scripts/dod.sh; else echo "dod: scripts/dod.sh not present yet (wave-10 lane 1 owns it)" >&2; exit 1; fi

# Portfolio status bin (bin owned by wave-10 lane 11; fails until landed)
status:
    cargo run -q --manifest-path crates/cli/Cargo.toml --bin status
