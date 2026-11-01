#!/usr/bin/env python3
"""shacl_negative_corpus_b.py — SHACL negative/boundary corpus B (cases N-08..N-14).

Executes the wave-14 half of the corpus pinned in docs/jira/v26.10.31/
30-shacl-validation-suite.md against the canonical ontology artifact
(docs/jira/v26.10.31/19-ontology-ttl.md) using a real engine (pySHACL + rdflib).

Discipline (assert-flip, shim-court style) — every case runs:

  BASELINE  canonical TTL bytes, sha256 pinned.
  BEFORE    fresh parse must self-conform (Conforms: True).
  MUTANT    fresh parse + STRUCTURED mutation (rdflib triple add/remove on a
            corpus-namespaced focus node — never text sed, never shared-graph
            edits) must be rejected: Conforms: False AND the pinned REFUSAL
            code present in the results graph.
  RESTORE   fresh re-parse of the pristine baseline bytes must hash equal
            (isomorphism-stable graph hash == baseline; bnode labels are
            minted per-parse, so raw NT bytes are not parse-stable) — a
            court that cannot prove its restore is FAIL.

Case ownership (corpus B only; N-01..N-07 are the corpus A lane):

  N-08  fanout != 256            -> REFUSAL-08  (sh:hasValue family)
  N-09  variance > 1.5           -> REFUSAL-10  (sh:maxInclusive family)
  N-10  heapAllocs > 0           -> REFUSAL-11  (sh:hasValue family)
  N-11  branchMiss > 0           -> REFUSAL-12  (sh:hasValue family)
  N-12  settlement mutant        -> REFUSAL-13  (+ cross-ref: the existing
         sed-based court in scripts/validate_ontology.sh must also exit 0)
  N-13  Cpk < 1.67               -> REFUSAL-14  (sh:minInclusive family)
  N-14  ODRL $50M boundary tiling (49999999.99 harbor / 50000000.00 duty /
        50000001.01 duty).

NOT-BINDING probes: the suite spec (30-shacl-validation-suite.md, "Note for
implementers") claims sh:hasValue is satisfied vacuously when the property
is absent. Each hasValue case therefore carries an explicit absence probe
(bare typed node, property absent); the engine's actual behavior is recorded
in the flip table either way. First witnessed run: pyshacl 0.31.0 REJECTS
absence (sh:hasValue is presence-binding) — the spec note is refuted, and
mutants are therefore built as CONFORMING INSTANCES of the target class with
exactly one property perturbed, so each flip isolates its single pinned code.

N-14 ODRL layering note: SHACL does not evaluate odrl: operator semantics;
ODRL evaluation is application-layer. This court therefore pins the policy's
monetary tiling with SPARQL ASK queries over the policy graph where the
odrl:lt / odrl:gte operator semantics are expressed as FILTER comparisons
EXECUTED BY THE RDFLIB SPARQL ENGINE (never a Python-side numeric comparison),
plus the duty-presence triple pattern. What is pinned here is constraint-level
tiling (no gap, no overlap, exact boundary routing); full duty-discharge
evaluation remains application-layer.

Exit code: 0 only if baseline conforms, every wrong-value mutant flips with
exactly its pinned code, every restore is byte-exact, the ODRL tiling is exact
on the base policy, the ODRL boundary mutant is rejected, the
validate_ontology.sh cross-ref exits 0, and the positive control detects a
corrupted expected-code pin. Anything else exits 1 fail-closed.
"""

from __future__ import annotations

import hashlib
import json
import re
import subprocess
import sys
from decimal import Decimal
from pathlib import Path

import rdflib
import pyshacl
from rdflib import Graph, Literal, Namespace, URIRef
from rdflib.compare import to_isomorphic
from rdflib.namespace import RDF

REPO = Path(__file__).resolve().parent.parent
ARTIFACT_MD = REPO / "docs" / "jira" / "v26.10.31" / "19-ontology-ttl.md"
VALIDATOR_SH = REPO / "scripts" / "validate_ontology.sh"

CGAP = Namespace("https://avec.chatman.dev/ontology/26.10.31#")
ODRL = Namespace("http://www.w3.org/ns/odrl/2/")
EX = Namespace("https://avec.chatman.dev/corpus-b/")
SH = Namespace("http://www.w3.org/ns/shacl#")

AUM_HARBOR = Decimal("49999999.99")
AUM_BOUNDARY = Decimal("50000000.00")
AUM_DUTY = Decimal("50000000.01")


# --------------------------------------------------------------------------
# baseline: extract + content-address the canonical TTL artifact
# --------------------------------------------------------------------------

def extract_ttl(md_path: Path) -> bytes:
    """Extract the fenced ```turtle block (same flow as validate_ontology.sh)."""
    in_block = False
    lines: list[str] = []
    for line in md_path.read_text(encoding="utf-8").splitlines(keepends=True):
        if not in_block and line.strip() == "```turtle":
            in_block = True
            continue
        if in_block:
            if line.strip() == "```":
                break
            lines.append(line)
    ttl = "".join(lines).encode("utf-8")
    if not ttl.strip():
        raise SystemExit(f"FAIL: empty TTL extraction from {md_path}")
    return ttl


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def canon_hash(g: Graph) -> str:
    """Isomorphism-stable graph hash (bnode-label independent).

    A raw sorted-NT sha256 is NOT parse-stable here: the ontology carries
    blank nodes (odrl:permission [ ... ], sh:property [ ... ]) and rdflib
    mints fresh bnode labels on every parse. The restore proof therefore
    compares isomorphic-graph hashes, not byte dumps; the artifact bytes
    themselves are pinned by the ttl sha256 content address.
    """
    ih = to_isomorphic(g).internal_hash()
    # rdflib >=7 returns an int digest; older returns a hashlib object.
    digest = ih.digest() if hasattr(ih, "digest") else str(ih).encode("utf-8")
    return hashlib.sha256(digest).hexdigest()


def parse(ttl: bytes) -> Graph:
    g = Graph()
    g.parse(data=ttl, format="turtle")
    return g


def validate(g: Graph):
    """Real engine run: data graph == shapes graph (self-validation mode)."""
    conforms, results_graph, _text = pyshacl.validate(
        data_graph=g, shacl_graph=g, inference="none", advanced=False
    )
    codes: set[str] = set()
    messages: list[str] = []
    for r in results_graph.subjects(RDF.type, SH.ValidationResult):
        msg = results_graph.value(r, SH.resultMessage)
        if msg is None:
            continue
        text = str(msg)
        messages.append(text)
        codes.update(re.findall(r"REFUSAL-\d{2}", text))
    return bool(conforms), codes, messages


# --------------------------------------------------------------------------
# case runner: BASELINE / BEFORE / MUTANT / RESTORE
# --------------------------------------------------------------------------

def run_flip_case(name: str, probe: str, ttl: bytes, base_hash: str,
                  mutate, pinned_code: str, rows: list[dict]) -> None:
    """Wrong-value mutant: must be rejected with the pinned code."""
    # BEFORE — fresh parse must conform.
    before_g = parse(ttl)
    conforms, codes, _ = validate(before_g)
    if not conforms:
        rows.append(dict(case=name, probe=probe, verdict="FAIL-BASELINE",
                         pinned=pinned_code, observed=sorted(codes)))
        return

    # MUTANT — structured mutation on an isolated fresh parse.
    mutant_g = parse(ttl)
    mutate(mutant_g)
    m_conforms, m_codes, _ = validate(mutant_g)

    # RESTORE — pristine baseline bytes re-parse to the identical graph.
    restored_hash = canon_hash(parse(ttl))
    restored = restored_hash == base_hash

    if m_conforms:
        verdict = "VACUOUS"  # mutant accepted: shape never had teeth
    elif pinned_code not in m_codes:
        verdict = "CODE-MISS"  # flipped, but not with the pinned code
    elif len(m_codes) > 1:
        verdict = "CODE-EXTRA"  # flipped but mutation is not isolating one code
    else:
        verdict = "FLIP"
    if not restored:
        verdict = "RESTORE-MISMATCH"
    rows.append(dict(case=name, probe=probe, verdict=verdict,
                     pinned=pinned_code, observed=sorted(m_codes),
                     restore="iso-exact" if restored else "MISMATCH"))


def run_absence_probe(name: str, ttl: bytes, base_hash: str, mutate, rows) -> None:
    """Typed node WITHOUT the property: record whether the shape binds.

    Outcome is a recorded finding (NOT-BINDING vs BINDING-ON-ABSENCE), never a
    silent pass — the suite spec requires the hasValue-vacuity to be decided
    and recorded, and the restore proof still applies.
    """
    g = parse(ttl)
    mutate(g)
    conforms, codes, _ = validate(g)
    restored = canon_hash(parse(ttl)) == base_hash
    rows.append(dict(
        case=name, probe="property absent", verdict="NOT-BINDING" if conforms
        else "BINDING-ON-ABSENCE", pinned="-",
        observed=[f"conforms={conforms}"] + sorted(codes),
        restore="iso-exact" if restored else "MISMATCH"))


# --------------------------------------------------------------------------
# structured mutations (corpus-namespaced focus nodes; add-only)
#
# First run (witnessed) showed pyshacl 0.31.0 enforces sh:hasValue as
# presence-binding — a typed node WITHOUT the property is rejected, so a
# "bare typed node + one perturbed property" mutant trips every hasValue
# constraint in the shape at once. To isolate exactly one REFUSAL code per
# case, each mutant starts from a CONFORMING instance of the class (every
# hasValue property present at its pinned value; inequality properties that
# are truly vacuous on absence omitted) and perturbs exactly one property.
# --------------------------------------------------------------------------

def git_conforming(g: Graph, node: URIRef) -> None:
    """Git plane instance conforming to cgap:GitSequencingPlaneShape."""
    g.add((node, RDF.type, CGAP.GitSequencingPlane))
    g.add((node, CGAP.packfileVersion, Literal(2)))
    g.add((node, CGAP.fanoutTableEntries, Literal(256)))
    g.add((node, CGAP.searchLadderStages, Literal(16)))


def turnstile_conforming(g: Graph, node: URIRef) -> None:
    """Turnstile instance conforming to cgap:TurnstileAdmissionShape
    (settlement at the receipted 14900 ns; Cpk omitted — minInclusive is
    vacuous on absence, and its presence-binding is probed by N-13)."""
    g.add((node, RDF.type, CGAP.TurnstileAdmission))
    g.add((node, CGAP.heapAllocationBytes, Literal(0)))
    g.add((node, CGAP.branchMissRate, Literal(Decimal("0.000"))))
    g.add((node, CGAP.settlementLatencyNs, Literal(14900)))


def m_n08_fanout(g: Graph) -> None:
    git_conforming(g, EX.Case_N08)
    g.set((EX.Case_N08, CGAP.fanoutTableEntries, Literal(128)))


def m_n08_absent(g: Graph) -> None:
    g.add((EX.Case_N08_absent, RDF.type, CGAP.GitSequencingPlane))


def m_n09_variance(g: Graph) -> None:
    git_conforming(g, EX.Case_N09)
    g.add((EX.Case_N09, CGAP.searchTimeVarianceNs, Literal(Decimal("4.0"))))


def m_n10_heap(g: Graph) -> None:
    turnstile_conforming(g, EX.Case_N10)
    g.set((EX.Case_N10, CGAP.heapAllocationBytes, Literal(4096)))


def m_n10_absent(g: Graph) -> None:
    g.add((EX.Case_N10_absent, RDF.type, CGAP.TurnstileAdmission))


def m_n11_branch(g: Graph) -> None:
    turnstile_conforming(g, EX.Case_N11)
    g.set((EX.Case_N11, CGAP.branchMissRate, Literal(Decimal("0.003"))))


def m_n11_absent(g: Graph) -> None:
    g.add((EX.Case_N11_absent, RDF.type, CGAP.TurnstileAdmission))


def m_n12_settlement(g: Graph) -> None:
    turnstile_conforming(g, EX.Case_N12)
    g.set((EX.Case_N12, CGAP.settlementLatencyNs, Literal(40000)))


def m_n13_cpk(g: Graph) -> None:
    turnstile_conforming(g, EX.Case_N13)
    g.add((EX.Case_N13, CGAP.processCapabilityCpk, Literal(Decimal("1.4"))))


# --------------------------------------------------------------------------
# N-14: ODRL $50M boundary tiling via SPARQL ASK over the policy graph
# --------------------------------------------------------------------------

# The operator semantics are FILTER comparisons executed by the rdflib SPARQL
# engine (xsd:decimal comparison, no float path). The odrl:operator IRI in the
# graph selects which engine-executed comparison applies; the court never
# compares money in Python.
ASK_HARBOR = """
PREFIX cgap: <https://avec.chatman.dev/ontology/26.10.31#>
PREFIX odrl: <http://www.w3.org/ns/odrl/2/>
PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>
ASK WHERE {
  ?policy a odrl:Policy ;
          odrl:permission ?perm .
  ?perm odrl:action odrl:use ;
        odrl:constraint ?c .
  ?c odrl:leftOperand cgap:assetUnderManagement ;
     odrl:operator odrl:lt ;
     odrl:rightOperand ?limit .
  FILTER ( ?aum < ?limit )
  VALUES ?aum { %s }
}
"""

ASK_DUTY = """
PREFIX cgap: <https://avec.chatman.dev/ontology/26.10.31#>
PREFIX odrl: <http://www.w3.org/ns/odrl/2/>
PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>
ASK WHERE {
  ?policy a odrl:Policy ;
          odrl:permission ?perm .
  ?perm odrl:action odrl:use ;
        odrl:constraint ?c ;
        odrl:duty ?duty .
  ?c odrl:leftOperand cgap:assetUnderManagement ;
     odrl:operator odrl:gte ;
     odrl:rightOperand ?limit .
  ?duty odrl:action ?dutyAction .
  FILTER ( ?aum >= ?limit )
  VALUES ?aum { %s }
}
"""


def ask(g: Graph, query_template: str, aum: Decimal) -> tuple[bool, URIRef | None]:
    """Engine-evaluated ASK; returns (answer, duty action IRI if any)."""
    lit = Literal(aum).n3()  # "49999999.99"^^xsd:decimal — exact decimal path
    q = query_template % lit
    res = g.query(q)
    duty_action = None
    if res.askAnswer:
        sel = g.query(query_template.replace("ASK WHERE",
                                             "SELECT ?dutyAction WHERE") % lit)
        for row in sel:
            duty_action = row.dutyAction
            break
    return bool(res.askAnswer), duty_action


def run_odrl_case(ttl: bytes, base_hash: str, rows: list[dict],
                  findings: list[str]) -> bool:
    """Tiling probes on the base policy + boundary-mutant flip witness."""
    ok = True

    def tiling(g: Graph):
        out = []
        for label, aum, want_harbor in (
            ("AUM=49999999.99", AUM_HARBOR, True),
            ("AUM=50000000.00", AUM_BOUNDARY, False),
            ("AUM=50000000.01", AUM_DUTY, False),
        ):
            harbor, _ = ask(g, ASK_HARBOR, aum)
            duty, duty_action = ask(g, ASK_DUTY, aum)
            # no gap, no overlap: exactly one branch must route the value.
            exact = (int(harbor) + int(duty)) == 1 and harbor == want_harbor
            branch = ("HARBOR(duty-free)" if harbor else
                      f"DUTY(duty={duty_action})" if duty else "GAP")
            if duty and duty_action != CGAP.regularize:
                exact = False
            out.append((label, aum, harbor, duty, branch, exact))
        return out

    base = tiling(parse(ttl))
    restored = canon_hash(parse(ttl)) == base_hash
    base_exact = all(exact for *_x, exact in base)
    for label, aum, harbor, duty, branch, exact in base:
        rows.append(dict(case="N-14", probe=label, verdict=branch,
                         pinned="odrl:lt|odrl:gte tile",
                         observed=f"harbor={harbor} duty={duty}",
                         restore="iso-exact" if restored else "MISMATCH"))
        ok = ok and exact

    # Flip witness: the statutory defect this case exists to catch — an
    # implementation using gt instead of gte leaves 50000000.00 unrouted.
    mutant = parse(ttl)
    perm_gt = None
    for s, _p, o in mutant.triples((None, ODRL.permission, None)):
        for c in mutant.objects(o, ODRL.constraint):
            if (c, ODRL.operator, ODRL.gte) in mutant:
                perm_gt = (o, c)
    if perm_gt is None:
        findings.append("N-14: mutant aborted — no odrl:gte constraint found "
                        "in policy graph")
        return False
    perm, constraint = perm_gt
    mutant.remove((constraint, ODRL.operator, ODRL.gte))
    mutant.add((constraint, ODRL.operator, ODRL.gt))
    m_tiling = tiling(mutant)
    gap_at_boundary = any(
        (aum == AUM_BOUNDARY and harbor == duty)  # neither branch routes it
        for _label, aum, harbor, duty, _branch, _exact in m_tiling
    )
    restored_m = canon_hash(parse(ttl)) == base_hash
    verdict = "FLIP" if gap_at_boundary else "VACUOUS"
    ok = ok and gap_at_boundary and restored_m
    rows.append(dict(
        case="N-14", probe="mutant odrl:gte -> odrl:gt", verdict=verdict,
        pinned="boundary 50000000.00 unrouted", observed="gap detected",
        restore="iso-exact" if restored_m else "MISMATCH"))
    return ok


# --------------------------------------------------------------------------
# main court
# --------------------------------------------------------------------------

def main() -> int:
    if not ARTIFACT_MD.exists():
        print(f"FAIL: artifact missing: {ARTIFACT_MD}")
        return 1

    ttl = extract_ttl(ARTIFACT_MD)
    ttl_sha = sha256(ttl)
    base_g = parse(ttl)
    base_hash = canon_hash(base_g)
    conforms, codes, messages = validate(base_g)
    print("== SHACL negative corpus B (cases N-08..N-14) ==")
    print(f"artifact: {ARTIFACT_MD.relative_to(REPO)}")
    print(f"ttl sha256: {ttl_sha} ({len(ttl)} bytes)")
    print(f"engine: pyshacl {pyshacl.__version__} / rdflib {rdflib.__version__}")
    print(f"baseline self-validation: Conforms={conforms} "
          f"(canon sha256 {base_hash[:16]}..., violations={len(messages)})")
    if not conforms:
        print("FAIL: baseline artifact violates its own shapes — court cannot run.")
        for m in sorted(set(messages)):
            print(f"  baseline violation: {m}")
        return 1

    rows: list[dict] = []
    findings: list[str] = []

    run_flip_case("N-08", "fanoutTableEntries=128", ttl, base_hash,
                  m_n08_fanout, "REFUSAL-08", rows)
    run_absence_probe("N-08", ttl, base_hash, m_n08_absent, rows)
    run_flip_case("N-09", "searchTimeVarianceNs=4.0", ttl, base_hash,
                  m_n09_variance, "REFUSAL-10", rows)
    run_flip_case("N-10", "heapAllocationBytes=4096", ttl, base_hash,
                  m_n10_heap, "REFUSAL-11", rows)
    run_absence_probe("N-10", ttl, base_hash, m_n10_absent, rows)
    run_flip_case("N-11", "branchMissRate=0.003", ttl, base_hash,
                  m_n11_branch, "REFUSAL-12", rows)
    run_absence_probe("N-11", ttl, base_hash, m_n11_absent, rows)
    run_flip_case("N-12", "settlementLatencyNs=40000", ttl, base_hash,
                  m_n12_settlement, "REFUSAL-13", rows)
    run_flip_case("N-13", "processCapabilityCpk=1.4", ttl, base_hash,
                  m_n13_cpk, "REFUSAL-14", rows)

    odrl_ok = run_odrl_case(ttl, base_hash, rows, findings)

    # N-12 cross-ref: the pre-existing sed-based settlement court must agree.
    cross_ok = False
    cross_line = "cross-ref validate_ontology.sh: NOT RUN"
    if VALIDATOR_SH.exists():
        proc = subprocess.run(["bash", str(VALIDATOR_SH)], cwd=REPO,
                              capture_output=True, text=True)
        cross_ok = proc.returncode == 0 and "ANTI-VACUITY: PASS" in proc.stdout
        cross_line = (f"cross-ref validate_ontology.sh: exit {proc.returncode}"
                      f" ({'ANTI-VACUITY PASS' if cross_ok else 'NO PASS LINE'})")
    else:
        cross_line = "cross-ref validate_ontology.sh: MISSING"
    print(cross_line)

    # Positive control (spec: the code pinning must not be decorative).
    pc_g = parse(ttl)
    m_n08_fanout(pc_g)
    pc_conforms, pc_codes, _ = validate(pc_g)
    corrupted_pin = "REFUSAL-99"
    pc_detected = (not pc_conforms) and corrupted_pin not in pc_codes \
        and "REFUSAL-08" in pc_codes
    rows.append(dict(case="PC-1",
                     probe=f"corrupted pin {corrupted_pin} vs observed "
                           f"{sorted(pc_codes)}",
                     verdict="DETECTED" if pc_detected else "PIN-VACUOUS",
                     pinned=corrupted_pin, observed=sorted(pc_codes)))
    if not pc_detected:
        findings.append("positive control FAILED: corrupted expected-code pin "
                        "was accepted — code pinning is decorative")

    # ---------------- flip table ----------------
    print()
    print(f"{'case':6} {'probe':38} {'verdict':18} {'pinned':20} observed")
    hard_fail = False
    flips = 0
    required_flips = {"N-08", "N-09", "N-10", "N-11", "N-12", "N-13"}
    seen_flips: set[str] = set()
    for r in rows:
        print(f"{r['case']:6} {r['probe']:38} {r['verdict']:18} "
              f"{str(r['pinned']):20} {r['observed']}"
              + (f" [{r.get('restore')}]" if r.get("restore") else ""))
        if r["verdict"] == "FLIP":
            seen_flips.add(r["case"])
        if r["verdict"] in ("VACUOUS", "CODE-MISS", "CODE-EXTRA",
                            "RESTORE-MISMATCH", "FAIL-BASELINE",
                            "PIN-VACUOUS"):
            hard_fail = True
    ok = odrl_ok and not hard_fail and not findings
    flips_ok = required_flips.issubset(seen_flips) and \
        any(r["case"] == "N-14" and r["verdict"] == "FLIP" for r in rows)
    if not flips_ok:
        print("FAIL: missing witnessed flip for a required case")
        hard_fail = True

    restores = [r for r in rows if r.get("restore")]
    exact = sum(1 for r in restores if r["restore"] == "iso-exact")
    print(f"restores: {exact}/{len(restores)} isomorphism-exact "
          "(bnode-stable graph hash == baseline; artifact bytes pinned by "
          "ttl sha256)")
    for f in findings:
        print(f"FINDING: {f}")

    # hasValue-absence reality — recorded, never silently accepted (suite
    # spec implementer note claims hasValue is vacuous on absence; the
    # engine decides, this court records whichever way it observed).
    absence = [r for r in rows if r["probe"] == "property absent"]
    binding = [r for r in absence if r["verdict"] == "BINDING-ON-ABSENCE"]
    nonbinding = [r for r in absence if r["verdict"] == "NOT-BINDING"]
    if binding:
        codes = sorted({c for r in binding for c in r["observed"]
                        if c.startswith("REFUSAL-")})
        print(f"hasValue ABSENCE finding: engine BINDS presence on absence "
              f"({[r['case'] for r in binding]} rejected with {codes}) — the "
              "30-shacl-validation-suite.md implementer note "
              "(hasValue vacuous on absence) is REFUTED by "
              f"pyshacl {pyshacl.__version__}; no sh:minCount companion needed "
              "for presence.")
    if nonbinding:
        print(f"hasValue ABSENCE finding: engine does NOT bind presence "
              f"({[r['case'] for r in nonbinding]}) — the implementer note "
              "holds for these shapes; presence needs a sh:minCount 1 "
              "companion.")

    verdict = "PASS" if (ok and not hard_fail and cross_ok) else "FAIL"
    print()
    print(f"VERDICT: {verdict} — corpus B (N-08..N-14) "
          f"flips={len(seen_flips)}/7 families=Git,Turnstile,ODRL")
    print(json.dumps(dict(
        verdict=verdict, artifact=str(ARTIFACT_MD.relative_to(REPO)),
        ttl_sha256=ttl_sha, canon_base_sha256=base_hash,
        engine=f"pyshacl {pyshacl.__version__} / rdflib {rdflib.__version__}",
        rows=rows, findings=findings, cross_ref=cross_line), default=str))
    return 0 if verdict == "PASS" else 1


if __name__ == "__main__":
    sys.exit(main())
