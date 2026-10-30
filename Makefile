# avec — DFLSS gate surface (DMEDI Measure = real commands, real exits)
# Lane-owned file. Each gate runs the REAL command; a nonzero exit fails the
# target (fail-closed). No mocked, skipped, or silenced gates.
#
#   make test    — cargo test --workspace
#   make gate0   — ggen sync run x2, byte-stable replay proof, freestanding C header compile
#   make gate1   — CC=1 machine-code branch audit (builds audit profile first)
#   make gate2   — release bench run
#   make gate3   — C Data Interface wire parity through pyarrow _import_from_c
#   make lint    — clippy, warnings tolerated (tail -5 shown)
#   make publish-check — cargo publish dry-runs
#   make gates   — gate0 gate1 gate2 gate3, in order

.DEFAULT_GOAL := test
.DELETE_ON_ERROR:

# Fail-closed: every recipe line runs under `sh -eu`. Any nonzero exit
# terminates the target with that exit code.
SHELL := /bin/sh
.SHELLFLAGS := -eu -c

.PHONY: test gate0 gate1 gate2 gate3 gates lint publish-check \
        bench-all bench-page bench-gate gate-parquet footprint branch-audit symbol-gate

# --- tool resolution ----------------------------------------------------------
# macOS ships shasum; Linux ships sha256sum. Fail loudly if neither exists.
SHA_CMD := $(shell command -v sha256sum 2>/dev/null || command -v shasum 2>/dev/null)
ifeq ($(strip $(SHA_CMD)),)
$(error no sha256 tool on PATH (need sha256sum or shasum))
endif
# shasum needs -a 256 to select the digest; sha256sum does not.
SHA_FLAGS := $(if $(findstring shasum,$(notdir $(SHA_CMD))),-a 256,)

GATE0_FILES := include/avec_c_data.h \
               include/avec_format_codes.h \
               crates/avec-arrow/src/generated_layout.rs \
               crates/avec-arrow/src/generated_version.rs

test:
	cargo test --workspace

# Gate 0: byte-stable replay proof. Two consecutive `ggen sync run` passes must
# leave the four generated surfaces byte-identical; then the C data header must
# compile freestanding under -std=c99 -Wall -Wextra -pedantic.
gate0:
	@echo "== gate0: ggen sync replay determinism + freestanding C header =="
	ggen sync run
	for f in $(GATE0_FILES); do $(SHA_CMD) $(SHA_FLAGS) $$f; done > /tmp/avec_gate0_run1.sha
	ggen sync run
	for f in $(GATE0_FILES); do $(SHA_CMD) $(SHA_FLAGS) $$f; done > /tmp/avec_gate0_run2.sha
	cmp /tmp/avec_gate0_run1.sha /tmp/avec_gate0_run2.sha
	cat /tmp/avec_gate0_run2.sha
	echo "gate0: byte-stable replay proof PASS"
	cc -std=c99 -ffreestanding -Wall -Wextra -pedantic -c include/avec_c_data.h -o /tmp/avec_h_gate0.o
	echo "gate0: header compile PASS"

# Gate 1: CTQ-3 — scan kernels must compile to machine code with zero
# conditional branches inside each kernel symbol's extent. Builds the audit
# profile cdylib first (branch_audit.py disassembles target/audit/libavec_audit.dylib).
gate1:
	@echo "== gate1: CC=1 machine-code branch audit =="
	cargo build --profile audit -p avec-audit
	python3 scripts/branch_audit.py
	printf 'mutation court: see receipts (cargo mutants run is long; recorded in docs/RECEIPTS.md)\n'

# Gate 2: release bench — real artifact, real run.
gate2:
	@echo "== gate2: release bench =="
	cargo run --release -p bench

# Gate 3: C Data Interface wire parity. The harness CDLL-loads
# target/debug/libavec_ffi.dylib, so the debug cdylib must exist first.
gate3:
	@echo "== gate3: C Data Interface wire parity (pyarrow _import_from_c) =="
	cargo build -p avec-ffi
	/Users/sac/.local/bin/uv run --with pyarrow python3 scripts/gate3_cdata.py

gates: gate0 gate1 gate2 gate3

# Capture the true clippy status without letting `| tail` swallow it, POSIX-sh
# safe (no pipefail). Warnings tolerated per charter; hard errors still fail.
lint:
	@clippy_log=`cargo clippy --workspace --all-targets 2>&1` && st=0 || st=$$?; \
	printf '%s\n' "$$clippy_log" | tail -5; \
	exit $$st

publish-check:
	cargo publish --dry-run --allow-dirty -p avec-arrow
	cargo publish --dry-run --allow-dirty -p avec-parquet

# --- OPT-L9 additions ---------------------------------------------------------

# All three Gate-2 bench binaries, raw runs (no gating; bench-gate adds floors).
bench-all:
	cargo run --release -p bench --bin bench
	cargo run --release -p bench --bin parquet_bench
	cargo run --release -p bench --bin page_bench

# Page kernel bench only (OPT page lane's bins).
bench-page:
	cargo run --release -p bench --bin page_bench

# Floor gate: record vs docs/BENCH_FLOORS.json, gate all three bins' floors.
# --record reseeds; default mode exits 1 on any >25% drop below floor.
bench-gate:
	python3 scripts/bench_gate.py

# Parquet mutation court is long-running (cargo mutants); the receipt of the
# recorded run lives in docs/. Echo-only per charter: points at the receipt,
# never re-runs the court inline.
gate-parquet:
	@test -f docs/mutation-court-backlog.txt || \
		{ echo "gate-parquet FAIL: docs/mutation-court-backlog.txt missing"; exit 1; }
	@echo "gate-parquet: mutation-court receipt: docs/mutation-court-backlog.txt"
	@echo "gate-parquet: live court is 'cargo mutants' (long-running; recorded, not run inline)"

# wasm32-wasip1 footprint gate.
footprint:
	sh scripts/footprint.sh

# CC=1 machine-code branch audit (builds nothing; gate1 wraps it with the
# audit-profile build).
branch-audit:
	python3 scripts/branch_audit.py

# Zero-panic Jidoka gate.
symbol-gate:
	python3 scripts/symbol_gate.py

# --- OPT20-L10 additions ------------------------------------------------------

# Release ladder: every release gate in sequence, fail-closed, naming the first
# failing target. Each step's real sub-make exit is echoed as
# `PASS <target> exit=<n>`; a nonzero exit prints `FAIL <target> exit=<n>`,
# names the target, and terminates release-check with that same exit code.
# `if $(MAKE)` guards the step so `sh -e` cannot abort before the FAIL line
# is printed. Sequential by construction (for-loop, one shell), not -j.
.PHONY: release-check

RELEASE_CHECK_STEPS := gate0 gate1 gate2 gate3 bench-gate footprint branch-audit symbol-gate publish-check

release-check:
	@for t in $(RELEASE_CHECK_STEPS); do \
		echo "== release-check: $$t =="; \
		if $(MAKE) --no-print-directory $$t; then \
			echo "PASS $$t exit=0"; \
		else \
			st=$$?; \
			echo "FAIL $$t exit=$$st"; \
			echo "release-check: stopped at $$t (exit=$$st)"; \
			exit $$st; \
		fi; \
	done; \
	echo "release-check: all $(words $(RELEASE_CHECK_STEPS)) steps PASS"
