# The Pull Request as Universal Mechanical Turnstile

## The Insight

graphlaw's formal kernel is rigorous: typed plans, SHACL shapes, deterministic
admission, BLAKE3 receipts. But no external agent knows how to drive it out of
the box — the ABI is bespoke (`Plan::admit`, op codes, JSON plan syntax, a
refusal enum to interpret), and teaching it costs in-context tokens and error
retries on every new agent.

Meanwhile, every frontier LLM is pre-trained on millions of git and pull
request workflows. Wrapping the same machinery in standard PR mechanics gives
**identical admission semantics with zero in-context teaching** — and makes the
safety model mechanical rather than probabilistic. The agent does what it was
born knowing; the turnstile does what the kernel already does.

## Contrast: Custom Agent Protocol vs PR Boundary

| Concern | Custom graphlaw protocol | PR boundary |
|---|---|---|
| Agent action | `graphlaw_admit(plan_json)` RPC | `git checkout -b` / `commit` / `gh pr create` |
| Cognitive load | High — teach AST syntax, op codes, JSON ABI | Zero — pre-trained behavior |
| Sandbox | In-process WASM | Isolated branch; zero blast radius on main |
| Pre-flight | `Plan::admit` + court runs | CI / pre-receive hook running the same courts |
| Refusal arm | `PlanRefused { index, .. }` returned to caller | Red CI check + typed refusal in PR
comment |
| Admission record | BLAKE3 receipt in state arena | Merge commit OID = `R_receipt` |
| Rollback | Custom journal rewind | `git revert` |

Every row is the same semantics in a different syntax. The right column's
syntax costs the agent nothing and costs us no new admission machinery — the
hook invokes the kernel, git supplies the ledger, the forge supplies the
refusal surface.

## Execution Flow

1. **Untrusted agent proposes diffs on an ephemeral branch**
   (`agent/admit-sensor-delta`). It has no write access to `main` — only to
   its own branch and the PR it opens against it.
2. **PR opened** through the standard forge flow.
3. **Court arbitrator runs**, in kernel order: dialect sniff, SHACL pre-flight,
   sieve courts, Chatman-equation check. This is the same admission function
   the WASM kernel would run in-process, now gated on the PR head SHA.
4. **RED**: typed refusal comment (`PlanRefused { index, reason, .. }` rendered
   into the PR), merge blocked. The agent reads the refusal like any other CI
   failure and retries on a new commit — the retry loop it already knows.
5. **GREEN**: signed receipt attached, auto-merge. The merge commit OID is the
   receipt pointer.

## Why the Code Is the Receipt

A merge that passed the courts binds the commit hash to its inputs, its
shapes, and its diff. The commit OID is content-addressed by exactly the same
argument as the kernel's BLAKE3 receipt: changing any input changes the hash,
so the DAG edge from red-to-green is unverifiable-by-construction to tamper.

The Git DAG is the ledger. `git log` is the replay surface. A checkout of the
merge commit reproduces the admitted state without any auxiliary receipt
store; branch protection settings are the standing register (who may admit,
under which court set).

## The Fail-Closed Triad

1. **Deterministic pre-receive gates.** The pre-receive hook runs the kernel
   directly. A SHACL violation or policy-mask breach rejects at the network
   boundary — before the ref exists on the server — with a standard git error
   plus a validation receipt. Nothing probabilistic: the same plan either
   passes shapes or it does not, and the rejection carries the typed reason.
2. **Instant rollback.** `git revert` is mechanically guaranteed blast-radius
   reversal: it produces the inverse diff against the exact recorded inputs.
   No custom journal to implement, no rewind protocol to trust — the same
   operation every developer uses, applied to an agent's admission.
3. **Branch-isolated execution arenas.** All speculative planning happens off
   `main`, on per-agent branches. Merge into the admitted state happens only
   on a signed proof-of-correctness receipt from the court arbitrator. A
   hallucinating agent can litter the branch namespace; it cannot touch the
   admitted graph.

## The "100% Safe" Claim Discipline

Safety here means **containment**, not a proof of agent correctness.

- Contained: the agent has zero write access to `main`. Its entire output
  surface is diffs on ephemeral branches, and every diff must pass the
  deterministic courts to enter the admitted state. Worst case is a noisy PR,
  not a corrupted graph.
- Not claimed: that the agent's diffs are correct, its plans optimal, or its
  retries convergent. Quality of the proposed code is the courts' business,
  and the courts check what they check — nothing more.

The honest boundary: PR mechanics make the *safety model* mechanical
(deterministic gates, guaranteed rollback, isolated arenas). They do not make
agent behavior reliable. Any "100% safe" phrasing must be read as "100%
contained," and stated that way.

## Standing

PARTIAL_ALIVE kernel-side, NOT-BUILT at the transport. The court
machinery the pattern calls for is real in this repo: `fsck_court`, 24
LIVE court families per `52-courts-audit.md` (incl. the wave-9 addendum),
and the
`scripts/verify_v26.10.31.sh` 10-gate battery. The pre-receive /
PR-forge hook transport that would drive them from a PR boundary is not
built — no hook wiring exists.

## See Also

- `09-stardf-hooks.md`
- `01-stardf-reconstitution.md`
