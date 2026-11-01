#!/usr/bin/env python3
"""scripts/shacl_negative_corpus.py — SHACL negative corpus court (N-01..N-07).

Wave-14 lane 14. Loads the canonical ontology artifact
(docs/jira/v26.10.31/19-ontology-ttl.md), extracts the Turtle block the same
way scripts/validate_ontology.sh does, and runs each negative-corpus case:

  * build a minimal instance graph = full ontology + ONE typed node of the
    targeted family class, fully populated with the shape's pinned values
    except the single perturbed property (mutant) or none perturbed (control);
  * run pySHACL self-validation (data graph == shapes graph, same mode as
    validate_ontology.sh `-e`);
  * FLIPPED requires: mutant non-conformant, the artifact-derived REFUSAL code
    fires on the case node, no other REFUSAL code fires anywhere, and the
    control run carries zero REFUSAL codes.

Non-flipping cases are recorded as NOT-BINDING with the observed codes and a
reason. Exit 0 only when every case is FLIPPED and the code-pinning self-check
passes; otherwise exit 1 (fail-closed, court states which cell did not bind).

Engine-semantics facts this design is pinned to (probed on pyshacl 0.31.0):
  * sh:hasValue VIOLATES when the property is absent (it is not vacuous on
    absence) — hence fully-populated mutation nodes;
  * sh:closed true flags the rdf:type triple of any targeted instance (no
    ignoredProperties on the plane shapes) — hence the binding assertion is on
    the REFUSAL-code set, not the raw sh:conforms boolean of control runs.
"""

import hashlib
import re
import sys
from pathlib import Path

from pyshacl import validate
from rdflib import Graph, Literal, Namespace, RDF
from rdflib.namespace import XSD

ROOT = Path(__file__).resolve().parent.parent
ARTIFACT = ROOT / "docs/jira/v26.10.31/19-ontology-ttl.md"

SH = Namespace("http://www.w3.org/ns/shacl#")
CGAP = Namespace("https://avec.chatman.dev/ontology/26.10.31#")
NC = Namespace("https://avec.chatman.dev/negative-corpus#")

CODE_RE = re.compile(r"REFUSAL-\d+")

# The 30-shacl-validation-suite.md table's provisional case -> code mapping.
# The artifact TTL is the register of truth; divergences are reported, and the
# artifact-derived code is what the court pins to.
BRIEF_CODES = {
    "N-01": "REFUSAL-01",
    "N-02": "REFUSAL-02",
    "N-03": "REFUSAL-03",
    "N-04": "REFUSAL-04",
    "N-05": "REFUSAL-05",
    "N-06": "REFUSAL-06",
    "N-07": "REFUSAL-07",
}

# id, shape, perturbed path, mutant python value, description
CASES = [
    ("N-01", CGAP.ArrowExecutionPlaneShape, CGAP.cacheLineAlignmentBytes,
     32, "cacheLineAlignmentBytes 64 -> 32"),
    ("N-02", CGAP.ArrowExecutionPlaneShape, CGAP.nullBitmapPresent,
     True, "nullBitmapPresent false -> true"),
    ("N-03", CGAP.ArrowExecutionPlaneShape, CGAP.cyclomaticComplexity,
     2, "cyclomaticComplexity 1 -> 2 (kernel M=2)"),
    ("N-04", CGAP.ParquetPersistencePlaneShape, CGAP.definitionLevelByteLength,
     4, "definitionLevelByteLength 0 -> 4"),
    ("N-05", CGAP.ParquetPersistencePlaneShape, CGAP.repetitionLevelByteLength,
     1, "repetitionLevelByteLength 0 -> 1"),
    ("N-06", CGAP.ParquetPersistencePlaneShape, CGAP.decodeThroughputGiBs,
     Literal("60", datatype=XSD.decimal), "decodeThroughputGiBs 74.8 -> 60 (< LSL 70)"),
    ("N-07", CGAP.GitSequencingPlaneShape, CGAP.searchLadderStages,
     32, "searchLadderStages 16 -> 32"),
]

# Paths constrained by sh:minInclusive/sh:maxInclusive have no sh:hasValue term
# to copy; their control (conforming) values are pinned here.
CONTROL_OVERRIDES = {
    CGAP.decodeThroughputGiBs: Literal("74.8", datatype=XSD.decimal),
    CGAP.searchTimeVarianceNs: Literal("1.0", datatype=XSD.decimal),
}


def extract_ttl(text: str) -> str:
    """Same fence logic as validate_ontology.sh:
    awk '/^```turtle$/{f=1;next} /^```$/{f=0} f'."""
    out, inside = [], False
    for line in text.splitlines(keepends=True):
        stripped = line.rstrip("\n")
        if not inside and stripped == "```turtle":
            inside = True
            continue
        if inside and stripped == "```":
            inside = False
            continue
        if inside:
            out.append(line)
    return "".join(out)


def read_family(g: Graph, shape: Namespace) -> dict:
    """Walk a sh:NodeShape's sh:property blocks: pinned hasValue terms, the
    constraint kind per path, target class, and the artifact-pinned REFUSAL
    code from each sh:message."""
    family = {"class": g.value(shape, SH.targetClass), "paths": [], "by_path": {}}
    for pshape in g.objects(shape, SH.property):
        path = g.value(pshape, SH.path)
        message = str(g.value(pshape, SH.message, default="") or "")
        match = CODE_RE.search(message)
        if match is None:
            raise SystemExit(f"FATAL: no REFUSAL code in message for {path}: {message!r}")
        has_value = g.value(pshape, SH.hasValue)
        if has_value is not None:
            kind, pinned = "hasValue", has_value
        elif g.value(pshape, SH.minInclusive) is not None:
            kind, pinned = "minInclusive", g.value(pshape, SH.minInclusive)
        elif g.value(pshape, SH.maxInclusive) is not None:
            kind, pinned = "maxInclusive", g.value(pshape, SH.maxInclusive)
        else:
            raise SystemExit(f"FATAL: unsupported constraint kind on {path}")
        family["paths"].append(path)
        family["by_path"][path] = {
            "code": match.group(0), "kind": kind, "pinned": pinned,
            "message": message,
        }
    return family


def mutant_term(case_value, pinned: Literal) -> Literal:
    """Same datatype as the shape's pinned term so sh:hasValue term equality
    is exercised exactly; python bool -> xsd:boolean."""
    if isinstance(case_value, Literal):
        return case_value
    return Literal(case_value, datatype=pinned.datatype)


def build_graph(base: Graph, family: dict, node: Namespace, path_override, value: Literal) -> Graph:
    """Ontology + one fully-populated typed instance of the family class.
    path_override gets `value`; every other family path gets its pinned value."""
    g = Graph()
    for t in base:
        g.add(t)
    g.add((node, RDF.type, family["class"]))
    for path in family["paths"]:
        if path == path_override:
            g.add((node, path, value))
        elif path in CONTROL_OVERRIDES:
            g.add((node, path, CONTROL_OVERRIDES[path]))
        else:
            g.add((node, path, family["by_path"][path]["pinned"]))
    return g


def run_engine(g: Graph) -> dict:
    """pySHACL self-validation (data graph == shapes graph, the -e mode of
    validate_ontology.sh). Returns the conforms boolean, the set of REFUSAL
    codes over the whole results graph, and per-focus-node code sets."""
    conforms, results_graph, _text = validate(data_graph=g, shacl_graph=None)
    codes_all, codes_by_node = set(), {}
    for result in results_graph.subjects(RDF.type, SH.ValidationResult):
        focus = results_graph.value(result, SH.focusNode)
        message = str(results_graph.value(result, SH.resultMessage, default="") or "")
        match = CODE_RE.search(message)
        if match is None:
            continue
        code = match.group(0)
        codes_all.add(code)
        codes_by_node.setdefault(focus, set()).add(code)
    return {
        "conforms": conforms,
        "codes_all": codes_all,
        "codes_by_node": codes_by_node,
    }


def classify(case_id, node, expected, mutant_run, control_run) -> tuple:
    """FLIPPED / NOT-BINDING + reason. The reason strings are the honest
    record for cells that do not bind."""
    if mutant_run["conforms"]:
        return "NOT-BINDING", "engine ACCEPTED the mutant (sh:conforms true)"
    if expected not in mutant_run["codes_all"]:
        return ("NOT-BINDING",
                f"mutant rejected but pinned code {expected} absent; observed "
                f"{sorted(mutant_run['codes_all'])}")
    extra = mutant_run["codes_all"] - {expected}
    if extra:
        return ("NOT-BINDING",
                f"mutant fired extra REFUSAL codes beyond the pinned one: {sorted(extra)}")
    if node not in mutant_run["codes_by_node"] or expected not in mutant_run["codes_by_node"].get(node, set()):
        return ("NOT-BINDING",
                f"pinned code fired only off the case node; per-node map "
                f"{ {str(k): sorted(v) for k, v in mutant_run['codes_by_node'].items()} }")
    if expected in control_run["codes_all"]:
        return ("NOT-BINDING",
                f"control node (all pinned values) also fires {expected}; "
                f"shape cannot distinguish mutant from conforming instance")
    return "FLIPPED", "mutant rejected with exactly the pinned code on the case node; control clean"


def main() -> int:
    if not ARTIFACT.is_file():
        print(f"FATAL: artifact missing: {ARTIFACT}")
        return 1
    ttl = extract_ttl(ARTIFACT.read_text())
    if not ttl.strip():
        print(f"FAIL: empty TTL extraction from {ARTIFACT}")
        return 1
    ttl_sha = hashlib.sha256(ttl.encode()).hexdigest()

    try:
        base = Graph().parse(data=ttl, format="turtle")
    except Exception as exc:  # noqa: BLE001 — fail closed on parse errors
        print(f"FAIL: TTL parse error: {exc}")
        return 1

    engine = f"pyshacl {__import__('pyshacl').__version__} / rdflib {__import__('rdflib').__version__}"

    # Precondition: the unmutated artifact self-validates with zero REFUSAL codes.
    base_run = run_engine(base)
    if not base_run["conforms"] or base_run["codes_all"]:
        print(f"FAIL: base artifact not conforming: conforms={base_run['conforms']} "
              f"codes={sorted(base_run['codes_all'])}")
        return 1

    print("== SHACL negative corpus court (N-01..N-07) ==")
    print(f"engine: {engine}")
    print(f"artifact: docs/jira/v26.10.31/19-ontology-ttl.md")
    print(f"ttl sha256: {ttl_sha}")
    print(f"base self-validation: Conforms True, REFUSAL codes: {sorted(base_run['codes_all'])}")

    # Read pinned values + artifact-derived codes from the shapes at runtime.
    shapes = {}
    for case_id, shape, _path, _val, _desc in CASES:
        if shape not in shapes:
            shapes[shape] = read_family(base, shape)

    rows, all_flipped, divergences = [], True, []
    run_cache = {}
    for case_id, shape, path, case_value, desc in CASES:
        family = shapes[shape]
        if path not in family["by_path"]:
            print(f"FAIL: {case_id}: path {path} not constrained by {shape}")
            return 1
        expected = family["by_path"][path]["code"]
        if BRIEF_CODES.get(case_id) != expected:
            divergences.append(
                f"{case_id}: suite-table code {BRIEF_CODES.get(case_id)} -> artifact code {expected} (artifact wins)")
        node = NC[f"node_{case_id}"]
        pinned = family["by_path"][path]["pinned"]
        mutant_value = mutant_term(case_value, pinned)
        control_value = (CONTROL_OVERRIDES[path] if path in CONTROL_OVERRIDES else pinned)

        mutant_run = run_engine(build_graph(base, family, node, path, mutant_value))
        control_run = run_engine(build_graph(base, family, node, path, control_value))
        run_cache[case_id] = (node, expected, mutant_run)
        status, reason = classify(case_id, node, expected, mutant_run, control_run)
        if status != "FLIPPED":
            all_flipped = False
        rows.append((case_id, desc, expected, sorted(mutant_run["codes_all"]),
                     sorted(control_run["codes_all"]), status, reason))

    # Anti-vacuity positive control on the code-pinning itself: a deliberately
    # corrupted expectation must be classified NOT-BINDING, never FLIPPED.
    node, expected, mutant_run = run_cache["N-01"]
    selfcheck_status, selfcheck_reason = classify("N-01", node, "REFUSAL-99", mutant_run, {"codes_all": set()})
    selfcheck_ok = selfcheck_status == "NOT-BINDING"
    if not selfcheck_ok:
        all_flipped = False
    print("self-check (code-pinning positive control): "
          + ("PASS — corrupted expectation REFUSAL-99 -> NOT-BINDING, court not vacuous"
             if selfcheck_ok else f"VACUOUS COURT — matcher accepted REFUSAL-99 ({selfcheck_reason})"))

    print()
    print(f"{'CASE':<6}{'MUTATION':<44}{'EXPECTED':<12}{'OBSERVED':<24}{'CONTROL':<10}STATUS")
    for case_id, desc, expected, observed, control, status, _reason in rows:
        control_str = "{" + "/".join(control) + "}"
        print(f"{case_id:<6}{desc:<44}{expected:<12}{','.join(observed):<24}"
              f"{control_str:<10}{status}")
    print()
    for divergence in divergences:
        print(f"NOTE code-mapping divergence: {divergence}")
    print("NOTE control runs may carry ClosedConstraintComponent results on rdf:type "
          "(plane shapes are sh:closed with no ignoredProperties); these carry no REFUSAL "
          "code and the binding assertion is on the pinned-code set.")
    print("NOTE probed on pyshacl 0.31.0: sh:hasValue VIOLATES on absent property (not "
          "vacuous) — mutation nodes are fully populated with pinned values except the "
          "perturbed path; the 30-shacl-validation-suite.md vacuity note is contradicted.")

    flipped = sum(1 for r in rows if r[5] == "FLIPPED")
    print()
    for case_id, desc, expected, observed, control, status, reason in rows:
        if status != "FLIPPED":
            print(f"NOT-BINDING {case_id}: {reason}")
    print(f"VERDICT: {flipped}/{len(rows)} cases FLIPPED, "
          f"self-check {'PASS' if selfcheck_ok else 'FAIL'} — "
          f"exit {'0' if all_flipped else '1'}")
    return 0 if all_flipped else 1


if __name__ == "__main__":
    sys.exit(main())
