#!/usr/bin/env bash
# scripts/validate_ontology.sh — SHACL engine-validation of the CGAP canonical ontology.
# Fail-closed: any engine absence, parse error, or non-conformance exits non-zero.
# Flow: extract TTL -> self-validate -> anti-vacuity perturbation court.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SRC="$ROOT/docs/jira/v26.10.31/19-ontology-ttl.md"
TMP="$(mktemp -d /tmp/cgap-shacl.XXXXXX)"
trap 'rm -rf "$TMP"' EXIT
ONTO="$TMP/ontology.ttl"

# 1. Extract the Turtle block from the markdown source of truth.
awk '/^```turtle$/{f=1;next} /^```$/{f=0} f' "$SRC" > "$ONTO"
[ -s "$ONTO" ] || { echo "FAIL: empty TTL extraction from $SRC"; exit 1; }

# 2. Engine present?
python3 -c "import pyshacl, rdflib" 2>/dev/null || {
  echo "FAIL: pyshacl/rdflib unavailable — install: pip3 install --user pyshacl rdflib"; exit 1; }

# 3. Self-validation (data graph = shapes graph = ontology).
echo "== SHACL self-validation: $ONTO =="
python3 -m pyshacl -e -f human "$ONTO" > "$TMP/base.txt" 2>&1 || true
grep -q "^Conforms:" "$TMP/base.txt" || { echo "FAIL: pyshacl engine error"; cat "$TMP/base.txt"; exit 1; }
cat "$TMP/base.txt"
if grep -q "^Conforms: True" "$TMP/base.txt"; then
  BASE_CONFORMS=true
else
  BASE_CONFORMS=false
fi

# 4. Anti-vacuity court: perturb settlementLatencyNs 14900 -> 50000 in a copy;
#    engine MUST report non-conformant on the mutant, else the gate is VACUOUS.
sed 's/avec:settlementLatencyNs 14900/avec:settlementLatencyNs 50000/' "$ONTO" > "$TMP/mutant.ttl"
cmp -s "$ONTO" "$TMP/mutant.ttl" && { echo "FAIL: perturbation did not mutate the TTL"; exit 1; }
python3 -m pyshacl -e -f human "$TMP/mutant.ttl" > "$TMP/mut.txt" 2>&1 || true
if grep -q "^Conforms: False" "$TMP/mut.txt"; then
  echo "== ANTI-VACUITY: PASS (mutant rejected: REFUSAL-13 violation on settlementLatencyNs) =="
  MUT_VERDICT="PASS"
else
  echo "== ANTI-VACUITY: VACUOUS GATE (mutant accepted — engine does not enforce shapes) =="
  MUT_VERDICT="VACUOUS"
fi

# 5. Verdict.
echo
if [ "$MUT_VERDICT" = "VACUOUS" ]; then
  echo "VERDICT: VACUOUS GATE — exit 1"
  exit 1
fi
if [ "$BASE_CONFORMS" = "true" ]; then
  echo "VERDICT: PASS — ontology conforms to its own shapes; anti-vacuity court live."
  exit 0
else
  echo "VERDICT: NON-CONFORMANT — Genesis fixture violates its own shapes (violations above)."
  exit 1
fi
