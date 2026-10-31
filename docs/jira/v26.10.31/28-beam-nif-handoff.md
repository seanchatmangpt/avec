# Sub-BEAM Handoff: Zero-Copy NIF Resource Specification

FR-07 handoff spec between the BEAM runtime and `avec`. Governs every byte that
crosses the NIF boundary: nothing is serialized, ever.

## Boundary Contract

No serialization across the NIF boundary. An `avec` buffer is exposed to the
BEAM as an immutable resource binary via `enif_make_resource_binary(env, obj,
data, size)`: the returned binary term **aliases** the avec buffer's memory
rather than copying it. Witnessed export at `erl_nif_api_funcs.h:134` and in
`beam.smp`'s export table (OTP 28.3.1); the contract's other name,
`enif_inspect_resource_binary`, does not exist in this OTP (full
extern-verification receipt: `crates/avec-beam/src/resbin.rs`).

Lifecycle ownership sits with the BEAM: the binary term holds the resource
alive for its whole life; `enif_release_resource` drops only the NIF-local
reference created at allocation. Because the binary aliases live memory, avec
never frees memory the BEAM may still reference. An avec-side free is lawful
only as the resource-destructor act, and the dual-ownership turnstile
(AVEC-105) gates it to exact-once at turnstile state 0b00.

## Ownership & Lifetime Rules

Two admissible allocation forms, each with a matching destructor law:

- **Host-passed chunk** (avec wraps foreign memory). The resource holds a raw
  pointer + length; the destructor is a no-op because avec does not own the
  allocation. Two sub-forms:
  - Host binary wrap: the BEAM binary's own refcount governs the memory; the
    resource is a view, not an owner.
  - mmap region: the host owns the mapping; avec's resource destructor drops
    its Arc to the mapping handle (handle release, not memory free).
- **Avec-allocated aligned wrap**: avec allocates 64-byte-aligned buffers
  (Arrow canonical in-memory form per `02-architecture-planes.md`) and wraps
  them in a resource. Destructor = free, exact-once, turnstile-gated.

**Alignment propagation.** The 64-byte alignment guarantee is carried in the
resource metadata and asserted at wrap time; a host chunk that is not 64-byte
aligned is refused (`{:error, {:align, Got}}`), never silently re-aligned. A
zero-copy subresource (scan result) inherits the parent's alignment offset:
`parent_offset % 64 == 0` is a constructor precondition, not a runtime hope.

**Read-only vs mutable.** The default resource variant is read-only. A mutable
variant exists only under exclusive ownership: the turnstile must show a
single holder (BEAM bit set, Arrow bit clear, refcount 1) before a mutable
view is minted. Any second holder demotes to read-only, fail-closed.

## API Surface Sketch

Five operations. Allocation contract is 0 hot-path bytes for every op; all
failures are typed error tuples, fail-closed.

- **`open/attach`** — wrap existing chunk or mmap; 0 bytes copied, 0 allocated
  on the hot path. Fails `{:error, {:align, Got}}` on alignment miss,
  `{:error, {:mmap, Errno}}` on mapping failure.
- **`scan`** — returns a zero-copy subresource: an offset+len view into the
  parent resource, no new buffer. Fails `{:error, {:range, Off, Len}}`
  out-of-bounds, `{:error, :frozen}` if the parent was mutated.
- **`flush_to_parquet`** — writes from the aliased buffer; allocates only the
  destination page buffer (cold path, not hot). Fails
  `{:error, {:io, Reason}}`; the parent resource is untouched on failure.
- **`seal_to_packfile`** — flush plus content-address digest (BLAKE3,
  `crates/avec-blake3`); the digest is computed in place, no payload copy.
  Fails `{:error, {:seal, Reason}}`; a failed seal leaves the packfile
  absent, never partial.
- **`receipt_query`** — reads resource metadata only; allocates only the
  small return term. Fails `{:error, :not_found}` for unknown receipts.

Every error path returns before any partial write is visible: fail-closed, no
half-mutated resource is ever observable from the BEAM side.

## Scheduling Discipline

- No NIF may hold a normal scheduler dirty. Every op above is a candidate for
  the dirty CPU scheduler (`ERL_NIF_DIRTY_JOB_CPU_BOUND`) if its input exceeds
  a bounded work quantum; `receipt_query` and `scan` (view construction) are
  fast enough for normal schedulers, `flush_to_parquet` and `seal_to_packfile`
  are CPU-bound by construction and must run dirty.
- No unbounded loops inside a NIF. The 16-stage ladder is bounded by
  construction: stage count is fixed, per-stage work is proportional to input
  length, and no stage recurses. A kernel that cannot state its bound up front
  is refused at registration, not at runtime.
- No blocking I/O on a scheduler thread; I/O-bound work takes
  `ERL_NIF_DIRTY_JOB_IO_BOUND`.

## Prior Art (audit items, not claims)

- AVEC-105 (`../v26.10.29/AVEC-105-beam-arrow-dual-turnstile.md`): the
  `TurnstileEnvelope<T>` exact-once free law is the destructor gate this spec
  assumes. Audit item: confirm `crates/avec-beam/src/resources.rs` implements
  the AtomicU8 turnstile on both the BEAM-dtor and `ArrowArray.release` edges
  before relying on it here.
- `crates/avec-beam` (in-repo): current bridge. Audit items for aligning with
  this spec:
  - `lib.rs` registry is a `Mutex<Vec<Option<Batch>>>` slot table — a slot
    lease model, not the resource-binary alias model of this spec; reconcile
    or document the two regimes.
  - `resbin.rs` already carries the `enif_make_resource_binary` extern
    verification and the frozen-builder (no mutation under aliasing) law —
    consistent with this spec's read-only default.
  - `shm.rs` / `spsc_ring.rs` provide the cross-process mmap wrap path; check
    their destructor semantics against the host-owned no-op law above.

## Acceptance Tests

1. **Cross-boundary zero-copy**: assert pointer identity — the address
   returned by the NIF for a resource binary equals the avec buffer's data
   pointer (no copy). Verify via the resource metadata and a BEAM-side
   `:erlang.using/0`-style probe or a dedicated `pointer_of/1` NIF; the
   assertion fails on any allocation between wrap and inspect.
2. **Allocation assertion during scan**: wrap an allocation counter
   (allocator hook or `enif_alloc_binary` spy) around a scan of a large
   buffer; assert zero hot-path allocations for the subresource mint itself.
3. **Supervisor-kill safety**: kill the owning process (and the supervisor
   subtree) while a resource binary is live; assert the resource destructor
   runs exactly once (turnstile reaches 0b00) and no leak is reported by the
   allocator counter.
4. **Concurrency**: N concurrent readers hold read-only resource binaries
   while one writer under exclusive ownership attempts a mutable view; assert
   readers see stable bytes, the writer's mutable mint is refused
   (fail-closed), and the refusal is typed, not a crash.

## Standing

UNVERIFIED as a full-surface spec, with acceptance-test status split as
follows (wave 7):

- **Acceptance 1 (cross-boundary zero-copy): DONE.**
  `crates/avec-beam/src/pointer.rs` implements `avec_pointer_of` plus
  `avec_binary_data_pointer`; pointer identity asserted across GC in
  `crates/avec-beam/tests/bridge_contract.rs`.
- **Acceptance 2 (allocation assertion during scan): DONE.**
  `crates/avec-beam/src/alloc_spy.rs`; read-path allocation delta is
  {0, 0}; envelope creation is exactly 1 alloc / 80 bytes.
- **Acceptance 3 (supervisor-kill dtor exactly-once): DONE at M=64** via
  the process-exit path, with the GC-lag caveat: minor-GC residue read
  24/64 — a collection lag, never a double-drop.
- **Acceptance 4 (concurrency / mutable-mint refusal) and the five-op
  surface** (open/attach, scan, flush_to_parquet, seal_to_packfile,
  receipt_query): UNIMPLEMENTED.
- **Prior-art audit item on `resources.rs` AtomicU8 turnstile:
  DISCHARGED** — implemented (`batch_envelope_dtor`, `alloc_envelope`).

## See Also

- `02-architecture-planes.md` — logic/physical plane split; avec as physical
  data-plane substrate with 64-byte-aligned Arrow canonical form.
- `../v26.10.29/AVEC-105-beam-arrow-dual-turnstile.md` — BEAM/Arrow dual
  turnstile; exact-once free law this spec's destructor rules depend on.
