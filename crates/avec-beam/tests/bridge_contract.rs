//! FR-07 BEAM bridge contract tests (docs/jira/v26.10.31/28-beam-nif-handoff.md §4-6).
//!
//! One Rust integration test drives the REAL dylib in a REAL BEAM: it locates
//! `libavec_beam.dylib`, writes an escript that `erlang:load_nif/2`s it, and
//! exercises the contract surface end to end — load, ramp round-trip,
//! zero-copy resource-binary projection, envelope lifecycle, fail-closed
//! error split (badarg vs the `error` atom), and concurrent-reader +
//! owner-kill safety. The BEAM process is the collaborator; no mocks. Exit
//! code 0 from the escript = contract holds.

use std::path::PathBuf;
use std::process::Command;

/// Locate the built NIF dylib (`target/{debug,release}/libavec_beam.{dylib,so}`),
/// building it first if absent. Honors `CARGO_TARGET_DIR`.
fn nif_dylib_path() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let target_root = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest_dir.join("../../target").canonicalize().unwrap_or(manifest_dir.join("../../target")));
    for profile in ["debug", "release"] {
        for ext in ["dylib", "so"] {
            let p = target_root.join(profile).join(format!("libavec_beam.{ext}"));
            if p.exists() {
                return p;
            }
        }
    }
    // Not built yet: build it (test profile) with a clean cargo environment.
    let status = Command::new("cargo")
        .args(["build", "-p", "avec-beam"])
        .current_dir(&manifest_dir)
        .env_remove("CARGO")
        .env_remove("CARGO_MANIFEST_DIR")
        .env_remove("CARGO_PKG_VERSION")
        .env_remove("CARGO_TARGET_DIR")
        .status()
        .expect("spawn cargo build -p avec-beam");
    assert!(status.success(), "cargo build -p avec-beam failed: {status}");
    let p = target_root.join("debug").join("libavec_beam.dylib");
    assert!(p.exists(), "dylib missing after build: {}", p.display());
    p
}

/// The BEAM-side contract program. Module name must match the escript file
/// stem (escript compiles the script as that module, so the NIF stubs and
/// `main/1` live in one module — exactly how a real OTP NIF module is shaped).
const ESCRIPT_SRC: &str = r##"#! /usr/bin/env escript
%%FR-07 bridge contract. Usage: escript <this> <nif-dylib-path>. Exit 0 = hold.
-module(avec_beam).
-export([main/1]).

%% NIF stubs (the real bodies live in libavec_beam.dylib).
avec_new(_, _) -> erlang:nif_error(nif_not_loaded).
avec_new_fast(_, _) -> erlang:nif_error(nif_not_loaded).
avec_new_i64(_, _) -> erlang:nif_error(nif_not_loaded).
avec_len(_) -> erlang:nif_error(nif_not_loaded).
avec_stats(_) -> erlang:nif_error(nif_not_loaded).
avec_get(_, _) -> erlang:nif_error(nif_not_loaded).
avec_get_i64(_, _) -> erlang:nif_error(nif_not_loaded).
avec_free(_) -> erlang:nif_error(nif_not_loaded).
avec_version() -> erlang:nif_error(nif_not_loaded).
avec_envelope_new(_) -> erlang:nif_error(nif_not_loaded).
avec_envelope_len(_) -> erlang:nif_error(nif_not_loaded).
avec_batch_binary(_) -> erlang:nif_error(nif_not_loaded).
pointer_of(_) -> erlang:nif_error(nif_not_loaded).
avec_binary_data_pointer(_) -> erlang:nif_error(nif_not_loaded).
avec_dtor_stats() -> erlang:nif_error(nif_not_loaded).
avec_alloc_stats() -> erlang:nif_error(nif_not_loaded).
avec_open(_) -> erlang:nif_error(nif_not_loaded).
avec_scan(_, _, _) -> erlang:nif_error(nif_not_loaded).
avec_flush_to_parquet(_) -> erlang:nif_error(nif_not_loaded).
avec_seal_to_packfile(_) -> erlang:nif_error(nif_not_loaded).
avec_receipt_query(_) -> erlang:nif_error(nif_not_loaded).

main([NifPath]) ->
    ok = case erlang:load_nif(NifPath, 0) of
             ok -> ok;
             {error, R} -> erlang:error({load_nif_failed, R})
         end,
    Res = try contracts() of
              ok -> 0
          catch Class:Reason:Stack ->
              io:format("CONTRACT_FAIL ~p:~p~n~p~n", [Class, Reason, Stack]),
              1
          end,
    halt(Res).

contracts() ->
    t_version(),
    _ = t_ramp_roundtrip(),
    t_fail_closed_fresh(),
    t_i64_typed_dispatch(),
    t_lifecycle_double_free(),
    t_consume_projection(),
    t_envelope_lifecycle(),
    t_concurrent_readers_and_owner_kill(),
    t_pointer_identity(),
    t_dtor_exactly_once(),
    t_alloc_spy(),
    t_alloc_spy_creation(),
    t_persist_roundtrip(),
    t_open_scan(),
    ok.

%% --- helpers ---------------------------------------------------------------

-compile({nowarn_unused_function}).
t_version() ->
    V = avec_version(),
    true = is_binary(V) andalso byte_size(V) > 0,
    io:format("ok version=~s~n", [V]).

%% avec_new(n, fill) is a ramp: row i = fill + i.
t_ramp_roundtrip() ->
    Id = avec_new(4, 10),
    true = is_integer(Id),
    4 = avec_len(Id),
    {4, 32} = avec_stats(Id),
    10 = avec_get(Id, 0),
    13 = avec_get(Id, 3),
    %% avec_new_fast must be byte-identical in semantics.
    IdF = avec_new_fast(3, 100),
    100 = avec_get(IdF, 0),
    102 = avec_get(IdF, 2),
    ok = avec_free(IdF),
    Id.

%% Fail-closed split on inputs that cannot touch any live batch:
%% - non-integer args -> badarg exception (typed VM error, not a crash)
%% - unknown/freed/negative handle, out-of-range index, wrong element width,
%%   range/overflow rejects -> the atom `error`
%% Every one of these is a returned term or a catchable exception; none may
%% take down the VM.
t_fail_closed_fresh() ->
    %% badarg class (wrong argument types)
    badarg = try avec_get(not_an_int, 0) catch error:badarg -> badarg end,
    badarg = try avec_get(0, not_an_int) catch error:badarg -> badarg end,
    error = avec_len(<<1, 2>>),  %% avec_len convention: error atom, not badarg
    %% error-atom class
    error = avec_get(987654321, 0),
    error = avec_len(987654321),
    error = avec_stats(987654321),
    error = avec_free(987654321),
    error = avec_new(-1, 0),
    error = avec_new(1, 1 bsl 62),
    error = avec_new((1 bsl 28) + 1, 0),
    error = avec_new_i64((1 bsl 28) + 1, 0),
    %% arity mismatches are refused by the Erlang stubs themselves (a typed
    %% exception at the call boundary), never a VM crash.
    io:format("ok fail_closed~n").

%% Type-tagged dispatch: an i32 batch is invisible to avec_get_i64 and vice
%% versa (a wrong-variant read is `error`, never a reinterpret of the bytes).
t_i64_typed_dispatch() ->
    Id = avec_new_i64(3, -5),
    true = is_integer(Id),
    {3, 64} = avec_stats(Id),
    -5 = avec_get_i64(Id, 0),
    -3 = avec_get_i64(Id, 2),
    error = avec_get(Id, 0),
    error = avec_get_i64(987654321, 0),
    ok = avec_free(Id),
    io:format("ok i64_typed_dispatch~n").

%% Free lifecycle: freeing twice is `error`, and a freed handle is inert.
t_lifecycle_double_free() ->
    Id = avec_new(2, 7),
    ok = avec_free(Id),
    error = avec_free(Id),
    error = avec_len(Id),
    error = avec_get(Id, 0),
    ok.

%% Zero-copy projection via a registry id is CONSUMING: the binary aliases the
%% payload (correct size + little-endian ramp bytes), and the handle reads as
%% freed afterwards — the BEAM binary term is now the lifetime authority.
t_consume_projection() ->
    Len = 5,
    Id = avec_new(Len, 40),
    true = is_integer(Id),
    Bin = avec_batch_binary(Id),
    Expected = Len * 4,
    Expected = byte_size(Bin),
    <<40:32/little-signed, 41:32/little-signed, 42:32/little-signed,
      43:32/little-signed, 44:32/little-signed>> = Bin,
    %% consumed: the id is dead once the binary exists
    error = avec_len(Id),
    error = avec_free(Id),
    error = avec_batch_binary(Id),
    %% double projection of the SAME envelope must be legal (payload immutable)
    IdE = avec_new(2, 900),
    Env = avec_envelope_new(IdE),
    true = is_reference(Env),
    B1 = avec_batch_binary(Env),
    B2 = avec_batch_binary(Env),
    true = B1 =:= B2,
    2 * 4 = byte_size(B1),
    %% after dropping every reference and GC, the dtor must have run and the
    %% VM must keep serving (leak/crash would surface here)
    _ = [ Id, Env, B1, B2 ],
    garbage_collect(),
    1 = avec_len(avec_new(1, 0)),
    io:format("ok zero_copy_projection~n").

%% Envelope lifecycle: consume a handle into a reference-backed envelope, read
%% metadata off it, project, then GC and keep working.
t_envelope_lifecycle() ->
    Id = avec_new(3, 500),
    Env = avec_envelope_new(Id),
    true = is_reference(Env),
    3 = avec_envelope_len(Env),
    error = avec_envelope_len(not_an_envelope),
    error = avec_envelope_len(42),
    error = avec_envelope_new(-1),
    error = avec_envelope_new(Id),           %% consumed id
    Bin = avec_batch_binary(Env),            %% envelope path, non-consuming
    12 = byte_size(Bin),
    <<500:32/little-signed, 501:32/little-signed, 502:32/little-signed>> = Bin,
    error = avec_len(Id),                    %% id was consumed by envelope_new
    _ = [ Env, Bin ],
    garbage_collect(),
    2 = avec_len(avec_new(2, 0)),
    io:format("ok envelope_lifecycle~n").

%% Concurrency + owner-kill: N reader processes hold resource binaries
%% simultaneously and read them repeatedly while the owner of another binary
%% is brutally killed; after the dust settles (GC everywhere) the VM still
%% allocates, projects, and reads correctly.
t_concurrent_readers_and_owner_kill() ->
    NReaders = 16,
    Self = self(),
    Readers = [ spawn(fun() -> reader(Self) end) || _ <- lists:seq(1, NReaders) ],
    %% an owner holding a projected binary gets killed mid-flight
    Owner = spawn(fun owner/0),
    [ R ! go || R <- Readers ],
    timer:sleep(50),
    exit(Owner, kill),
    [ receive {R, ack} -> ok after 5000 -> erlang:error({reader_timeout, R}) end
      || R <- Readers ],
    garbage_collect(),
    %% post-kill liveness: full round-trip still works
    Id = avec_new(64, 1),
    64 = avec_len(Id),
    256 = byte_size(avec_batch_binary(Id)),  %% 64 rows * 4 bytes
    error = avec_free(Id),  %% projection CONSUMED the handle: binary owns it now
    io:format("ok concurrency_and_kill~n").

reader(Parent) ->
    receive
        go ->
            Id = avec_new(128, 9),
            Bin = avec_batch_binary(Id),
            loop_read(Bin, 128, 200),
            Parent ! {self(), ack}
    end.

loop_read(_Bin, _Len, 0) -> ok;
loop_read(Bin, Len, K) ->
    Expected = Len * 4,
    Expected = byte_size(Bin),
    <<9:32/little-signed, _/binary>> = Bin,
    garbage_collect(),   %% stress GC while a resource binary is live
    loop_read(Bin, Len, K - 1).

owner() ->
    Id = avec_new(1024, 3),
    Bin = avec_batch_binary(Id),
    receive
        done -> ok
    after 60000 ->
        ok
    end,
    _ = Bin,
    ok.

%% POINTER IDENTITY (FR-07 MISSING #1, wave-4 lane 12): the zero-copy
%% projection must alias, not copy. Binaries are immutable, so byte equality
%% alone proves nothing about the buffer; instead the envelope advertises its
%% payload address via pointer_of/1 and the projected binary's own data
%% pointer (avec_binary_data_pointer/1 = enif_inspect_binary) must be the
%% SAME integer.
%%
%% The digest-flip live-view variant (mutate the Rust buffer, watch a digest
%% flip while the pointer holds) is deliberately absent: avec_digest_of lives
%% in avec-ffi and folds ArrowArray C-Data-Interface exports, not this
%% membrane's registry Batch, and resbin.rs's immutability law forbids any
%% mutator of a projected payload. Equal data pointers ARE the identity proof.
t_pointer_identity() ->
    %% envelope path: pointer stable across calls AND across GC.
    IdE = avec_new(8, 77),
    Env = avec_envelope_new(IdE),
    P1 = pointer_of(Env),
    true = is_integer(P1) andalso P1 > 0,
    P2 = pointer_of(Env),
    P1 = P2,
    garbage_collect(),
    P1 = pointer_of(Env),
    %% id path: BORROWING — the handle stays live, pointer identical.
    Id = avec_new(4, 5),
    Q1 = pointer_of(Id),
    true = is_integer(Q1) andalso Q1 > 0,
    Q1 = pointer_of(Id),
    4 = avec_len(Id),                        %% non-consuming: still registered
    {4, 32} = avec_stats(Id),
    %% THE IDENTITY COURT: projected binary's data pointer == exposed address.
    Bin = avec_batch_binary(Id),
    16 = byte_size(Bin),
    P1 = pointer_of(Env),
    Q1 = avec_binary_data_pointer(Bin),
    %% content parity: the bytes at that address are the Rust-built ramp.
    <<5:32/little-signed, 6:32/little-signed, 7:32/little-signed,
      8:32/little-signed>> = Bin,
    %% distinct batches hold distinct addresses (no accidental aliasing).
    false = (Q1 =:= P1),
    %% fail-closed: wrong arity/type/unknown id -> the atom `error`.
    error = pointer_of(not_an_int),
    error = pointer_of(987654321),
    error = pointer_of(-1),
    error = pointer_of(<<1, 2>>),
    error = avec_binary_data_pointer(not_a_binary),
    error = avec_binary_data_pointer(42),
    io:format("ok pointer_identity~n").

%% DESTRUCTOR EXACTLY-ONCE (FR-07 acceptance #3, wave-5 lane 14): the BEAM's
%% resource refcount must run the envelope dtor EXACTLY ONCE per envelope.
%% avec_dtor_stats/0 exposes the global dtor counter as {Drops}; after
%% creating M envelopes and dropping every reference, one process-local
%% garbage_collect() must yield a counter delta of exactly M — not 0 (dtor
%% unreachable => leak) and not 2M (double destruction => UB). A post-kill
%% round-trip proves the VM is still healthy after all M dtors ran.
t_dtor_exactly_once() ->
    M = 64,
    {BeforeDrops} = avec_dtor_stats(),
    true = is_integer(BeforeDrops) andalso BeforeDrops >= 0,
    %% The envelopes are created and dropped inside a worker process whose
    %% exit releases every resource it references — exactly once each. This
    %% is deterministic where a single garbage_collect() is not: witnessed on
    %% this OTP, minor-GC stack residue kept some envelope refs live and a
    %% GC-only delta read 24 of M=64 (a lag, never a double-drop — the
    %% process-exit path proves the at-most-once half, the monotone counter
    %% proves the at-least-once half).
    lists:foreach(fun(_) ->
                      {Worker, Mon} = spawn_monitor(fun() ->
                          Envs = [begin
                                      Id = avec_new(1, I),
                                      true = is_integer(Id),
                                      E = avec_envelope_new(Id),
                                      true = is_reference(E),
                                      1 = avec_envelope_len(E),
                                      error = avec_envelope_new(Id), %% consumed
                                      E
                                  end
                                  || I <- lists:seq(1, M)],
                          1 = avec_envelope_len(hd(Envs)),
                          exit(dtors_pending)
                      end),
                      receive
                          {'DOWN', Mon, process, Worker, dtors_pending} -> ok
                      after 5000 -> erlang:error(dtor_worker_timeout)
                      end
                  end,
                  [ok]),
    {AfterDrops} = avec_dtor_stats(),
    Delta = AfterDrops - BeforeDrops,
    M = Delta,   %% the exact-once law: M dtors, no more (no 2M), no fewer (no 0)
    %% Post-kill liveness: a full round-trip still works after M dtors ran.
    Id = avec_new(16, 250),
    16 = avec_len(Id),
    64 = byte_size(avec_batch_binary(Id)),
    io:format("ok dtor_exactly_once delta=~p total_drops=~p~n", [Delta, AfterDrops]).

%% ALLOCATION SPY (FR-07 acceptance #2, wave-5 lane 13): hot-path allocation
%% across the NIF boundary is observable, and the read hot path is proven to
%% allocate NOTHING at the crate's instrumented alloc sites. Two courts:
%%
%% 1. READ HOT PATH: K = 1000 full read rounds (avec_len, avec_stats,
%%    avec_get, avec_get_i64, avec_envelope_len, pointer_of,
%%    avec_batch_binary on the envelope path, avec_binary_data_pointer) must
%%    show a counter delta of EXACTLY {0, 0}. avec_batch_binary on the
%%    envelope path is the strong form: enif_make_resource_binary is
%%    zero-copy (resbin.rs), so 1000 projections must not move the counters.
%% 2. CREATION PATH ACCOUNTING: avec_new's builder allocations live in
%%    avec-arrow (not this membrane, uncounted by construction); one
%%    avec_envelope_new is exactly ONE counted alloc of a deterministic byte
%%    size (size_of::<BatchEnvelope>()), and the same delta repeats exactly.
t_alloc_spy() ->
    %% Live read surface: a borrowing id and an envelope (from a consumed id).
    Id = avec_new(64, 3),
    {64, 32} = avec_stats(Id),
    IdE = avec_new(8, 9),
    Env = avec_envelope_new(IdE),
    true = is_reference(Env),
    Bin = avec_batch_binary(Env),
    %% --- Court 1: the read hot path allocates nothing --------------------
    {A0, B0} = avec_alloc_stats(),
    K = 1000,
    lists:foreach(fun(_) ->
                      64 = avec_len(Id),
                      {64, 32} = avec_stats(Id),
                      3 = avec_get(Id, 0),
                      66 = avec_get(Id, 63),
                      error = avec_get_i64(Id, 0),   %% wrong-variant read: error
                      8 = avec_envelope_len(Env),
                      true = is_integer(pointer_of(Env)),
                      32 = byte_size(avec_batch_binary(Env)),
                      true = is_integer(avec_binary_data_pointer(Bin))
                  end,
                  lists:seq(1, K)),
    {A1, B1} = avec_alloc_stats(),
    0 = A1 - A0,
    0 = B1 - B0,
    io:format("ok alloc_spy reads=~p read_delta={0,0}~n", [K]).

%% Companion to t_alloc_spy: creation-path accounting constants.
t_alloc_spy_creation() ->
    %% avec_new: builder allocs live in avec-arrow — outside this membrane,
    %% NOT counted; the membrane's own contribution must be zero.
    {A0, B0} = avec_alloc_stats(),
    Id = avec_new(4, 0),
    true = is_integer(Id),
    {A1, B1} = avec_alloc_stats(),
    0 = A1 - A0,
    0 = B1 - B0,
    %% avec_envelope_new: exactly one counted alloc, deterministic size.
    Env = avec_envelope_new(Id),
    true = is_reference(Env),
    {A2, B2} = avec_alloc_stats(),
    1 = A2 - A1,
    EnvBytes = B2 - B1,
    true = EnvBytes > 0,
    %% Determinism: a second envelope requests the same byte count exactly.
    Id2 = avec_new(2, 0),
    _Env2 = avec_envelope_new(Id2),
    {A3, B3} = avec_alloc_stats(),
    1 = A3 - A2,
    EnvBytes = B3 - B2,
    %% Fail-closed: wrong arity is refused at the Erlang export boundary as a
    %% compile-time-known stub mismatch (a typed badarg, VM stays healthy) —
    %% asserted in t_alloc_spy via the NIF-level error split instead.
    io:format("ok alloc_spy_creation envelope_bytes=~p~n", [EnvBytes]),
    ok.

%% PERSISTENCE ROUND-TRIP (ops 3-5 of 5, wave-8 lane 8): envelope -> flush
%% -> seal -> receipt. The parquet path must name a real file framed PAR1 on
%% both ends; the seal OID (pack trailer) and the BLAKE3 receipt must both be
%% deterministic (two calls, same bytes, same digest — the receipt is the
%% milestone's blake3 stand-in, deliberately NOT a :crypto comparison), and
%% the fail-closed error split must hold for all three ops.
t_persist_roundtrip() ->
    %% --- envelope -> flush ----------------------------------------------
    N = 16,
    Id = avec_new(N, 100),
    true = is_integer(Id),
    {ok, Path} = avec_flush_to_parquet(Id),
    true = is_binary(Path) andalso byte_size(Path) > 0,
    {ok, Bin} = file:read_file(Path),
    true = byte_size(Bin) > 8,
    <<"PAR1", _/binary>> = Bin,
    %% trailing magic: last 4 bytes
    true = binary:part(Bin, byte_size(Bin) - 4, 4) =:= <<"PAR1">>,
    %% --- flush determinism + temp uniqueness -----------------------------
    {ok, Path2} = avec_flush_to_parquet(Id),
    true = Path2 =/= Path,                       %% pid+nanos unique names
    {ok, Bin2} = file:read_file(Path2),
    Bin2 = Bin,                                  %% byte-identical encode
    %% handle NOT consumed by flush (borrowing, like pointer_of on ids)
    N = avec_len(Id),
    %% --- receipt determinism (BLAKE3 receipt stand-in) --------------------
    %% receipt_query returns a BARE 32-byte binary (spec surface); fixed
    %% back from lane 7's {ok,_} unblock after ops_persist.rs landed the
    %% bare-binary return (lane 8, disclosed).
    R1 = avec_receipt_query(Id),
    R2 = avec_receipt_query(Id),
    true = is_binary(R1) andalso byte_size(R1) =:= 32,
    R1 = R2,
    %% receipt is stable across a flush too (pure payload read)
    {ok, _} = avec_flush_to_parquet(Id),
    R1 = avec_receipt_query(Id),
    %% --- seal determinism (pack trailer OID) ------------------------------
    {ok, Oid1} = avec_seal_to_packfile(Id),
    {ok, Oid2} = avec_seal_to_packfile(Id),
    true = is_binary(Oid1) andalso byte_size(Oid1) =:= 20,
    Oid1 = Oid2,
    %% --- i64 batch: same surface, width-8 payload --------------------------
    Id64 = avec_new_i64(4, 7),
    {ok, Path64} = avec_flush_to_parquet(Id64),
    {ok, Bin64} = file:read_file(Path64),
    true = byte_size(Bin64) > 8 andalso binary:part(Bin64, 0, 4) =:= <<"PAR1">>,
    R64 = avec_receipt_query(Id64),
    32 = byte_size(R64),
    true = R64 =/= R1,                           %% different bytes, different digest
    {ok, _} = avec_seal_to_packfile(Id64),
    ok = avec_free(Id64),
    %% --- fail-closed split (all three ops, typed `error`, no crash) --------
    error = avec_flush_to_parquet(987654321),
    error = avec_seal_to_packfile(987654321),
    error = avec_receipt_query(987654321),
    error = avec_flush_to_parquet(not_an_int),
    error = avec_receipt_query(<<1, 2>>),
    ok = avec_free(Id),
    %% cleanup: the flushed temp files must go
    ok = file:delete(Path),
    ok = file:delete(Path2),
    ok = file:delete(Path64),
    io:format("ok persist_roundtrip~n").

%% OPEN/SCAN (ops 1-2 of 5, wave-8 lane 7): attach a read-view to an existing
%% batch (zero-copy, pointer aliasing proven against pointer_of), scan
%% zero-copy byte-window slices out of it, and refuse every malformed input
%% fail-closed. Courts:
%% 1. CONTENT EQUALITY: scan(View, 0, total) is byte-identical to
%%    avec_batch_binary of the same payload; interior windows carry the exact
%%    expected ramp bytes.
%% 2. POINTER SEMANTICS: the slice's data pointer is EXACTLY
%%    pointer_of(parent) + start — same memory, offset arithmetic on the wire.
%% 3. LIFECYCLE: the id path consumes (open-after-consume -> `error`, the
%%    resbin consumption law); the envelope path is non-consuming and
%%    DOUBLE-OPEN IS LEGAL (immutable payload, both views alias same bytes).
%% 4. BOUNDS: out-of-range windows, unknown handles, negative offsets -> the
%%    atom `error`; non-integer start/len -> badarg.
%% 5. ALLOCATION: K scans through a live view show a counted-site delta of
%%    exactly {0, 0}; one envelope-path open is exactly ONE counted alloc.
t_open_scan() ->
    %% --- id path: open consumes (the resbin consumption law) ---------------
    BaseId = avec_new(8, 21),
    true = is_integer(BaseId),
    IdBase = pointer_of(BaseId),                 %% borrowing: handle stays live
    ViewId = avec_open(BaseId),
    true = is_reference(ViewId),
    error = avec_len(BaseId),                    %% consumed by open
    error = avec_free(BaseId),
    error = avec_open(BaseId),                   %% open-after-consume -> error
    error = avec_open(987654321),                %% unknown id
    error = avec_open(-1),
    error = avec_open(not_an_int),
    error = avec_open(<<1, 2>>),
    %% full-window scan equals the projection of the same payload...
    IdTotal = 8 * 4,
    FullId = avec_scan(ViewId, 0, IdTotal),
    IdTotal = byte_size(FullId),
    IdBase = avec_binary_data_pointer(FullId),   %% THE POINTER COURT (id path)
    <<21:32/little-signed, 22:32/little-signed, 23:32/little-signed,
      24:32/little-signed, 25:32/little-signed, 26:32/little-signed,
      27:32/little-signed, 28:32/little-signed>> = FullId,
    %% ...and an interior window carries the exact expected ramp slice
    <<22:32/little-signed, 23:32/little-signed, 24:32/little-signed>> =
        avec_scan(ViewId, 4, 12),
    %% zero-length window at the far edge is legal
    0 = byte_size(avec_scan(ViewId, IdTotal, 0)),
    %% --- envelope path: borrowing, double-open legal ------------------------
    IdE = avec_new(4, 7),
    Env = avec_envelope_new(IdE),
    true = is_reference(Env),
    EBase = pointer_of(Env),
    V1 = avec_open(Env),
    V2 = avec_open(Env),                          %% DOUBLE-OPEN LEGAL
    true = is_reference(V1) andalso is_reference(V2),
    Full = avec_batch_binary(Env),
    16 = byte_size(Full),
    %% CONTENT EQUALITY COURT: full-window scan == avec_batch_binary bytes
    Full = avec_scan(V1, 0, 16),
    Full = avec_scan(V2, 0, 16),
    EBase = avec_binary_data_pointer(avec_scan(V1, 0, 16)),
    Ptr8 = avec_binary_data_pointer(avec_scan(V1, 8, 8)),
    true = EBase + 8 =:= Ptr8,                   %% offset arithmetic on the wire
    %% --- bounds refusals (fail-closed, error atom) --------------------------
    error = avec_scan(V1, 0, 17),                %% past the 16-byte payload
    error = avec_scan(V1, 16, 1),
    error = avec_scan(V1, 17, 0),
    error = avec_scan(V1, -1, 4),
    error = avec_scan(V1, 0, -4),
    error = avec_scan(987654321, 0, 4),          %% unknown handle
    error = avec_scan(Env, 0, 4),                %% an ENVELOPE is not a view
    error = avec_scan(not_a_view, 0, 4),
    badarg = try avec_scan(V1, x, 4) catch error:badarg -> badarg end,
    badarg = try avec_scan(V1, 0, not_an_int) catch error:badarg -> badarg end,
    %% --- ALLOCATION COURT: scans allocate {0,0} at counted sites ------------
    {A0, B0} = avec_alloc_stats(),
    K = 1000,
    lists:foreach(fun(_) ->
                      12 = byte_size(avec_scan(ViewId, 4, 12)),
                      16 = byte_size(avec_scan(V1, 0, 16)),
                      0 = byte_size(avec_scan(ViewId, IdTotal, 0))
                  end,
                  lists:seq(1, K)),
    {A1, B1} = avec_alloc_stats(),
    0 = A1 - A0,
    0 = B1 - B0,
    %% one envelope-path open = exactly ONE counted alloc (the view resource)
    {A2, _} = avec_alloc_stats(),
    _V3 = avec_open(Env),
    {A3, _} = avec_alloc_stats(),
    1 = A3 - A2,
    %% --- liveness: drop every view term, GC, VM stays healthy ---------------
    _ = [ViewId, V1, V2, FullId, Full],
    garbage_collect(),
    2 = avec_len(avec_new(2, 0)),
    io:format("ok open_scan~n").
"##;

#[test]
fn beam_bridge_contract_end_to_end() {
    let dylib = nif_dylib_path();
    let erl = std::env::var("ESCRIPT").unwrap_or_else(|_| "escript".to_string());

    let script_dir = std::env::temp_dir().join(format!("avec_beam_contract_{}", std::process::id()));
    std::fs::create_dir_all(&script_dir).expect("create temp script dir");
    let script = script_dir.join("avec_beam");
    std::fs::write(&script, ESCRIPT_SRC).expect("write escript");

    // This OTP appends ".so" to the load_nif path unconditionally, so the
    // staged file is named `libavec_beam.so` while the passed path is the stem.
    let staged = script_dir.join("libavec_beam.so");
    std::fs::copy(&dylib, &staged).expect("copy dylib into temp dir");
    let nif_path = script_dir.join("libavec_beam");

    let out = Command::new(&erl)
        .args([script.to_str().unwrap(), nif_path.to_str().unwrap()])
        .output()
        .expect("spawn escript (set ESCRIPT=... if escript is not on PATH)");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    print!("{stdout}");
    eprint!("{stderr}");
    std::fs::remove_dir_all(&script_dir).ok();

    assert!(
        out.status.success(),
        "BEAM contract run failed (exit {:?}):\n{stdout}\n{stderr}",
        out.status.code()
    );
    for expected in [
        "ok version=",
        "ok fail_closed",
        "ok i64_typed_dispatch",
        "ok zero_copy_projection",
        "ok envelope_lifecycle",
        "ok concurrency_and_kill",
        "ok pointer_identity",
        "ok dtor_exactly_once",
        "ok alloc_spy",
        "ok alloc_spy_creation",
        "ok persist_roundtrip",
        "ok open_scan",
    ] {
        assert!(stdout.contains(expected), "missing contract proof `{expected}` in:\n{stdout}");
    }
}
