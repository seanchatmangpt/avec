//! Concurrent-kill stress court (wave-14 lane 4): 100 create/kill/verify
//! iterations, each storming 100 BEAM worker processes.
//!
//! Modeled on `bridge_contract.rs`'s escript pattern: one Rust integration
//! test drives the REAL dylib in a REAL BEAM. Per iteration the escript
//! spawns 100 workers; each worker creates 4 envelopes (`cgap_new` →
//! `cgap_envelope_new` → `cgap_batch_binary`), garbage-collects while holding
//! them, then keeps GC-stressing while awaiting orders. A random exact half
//! of the cohort is killed mid-flight with the untrappable `exit(kill)`.
//! The survivors' handles are then verified on a dedicated verifier process:
//!
//! 1. every surviving handle still serves reads (metadata, pointer
//!    stability, payload identity, exact ramp bytes, zero-copy
//!    re-projection);
//! 2. the dtor counter delta equals EXACTLY the dead envelopes — once per
//!    iteration for the killed half, once for the released survivors. The
//!    wave-5 GC-lag caveat applies: reads happen after the relevant exits
//!    are DOWN-collected (process-exit semantics, as in
//!    `t_dtor_exactly_once`) and then pass a bounded convergence barrier
//!    that asserts the at-most-once ceiling at every read and requires the
//!    counter to land exactly on the dead count (witnessed on this OTP: a
//!    raw post-DOWN read can lag, e.g. 196 of 200);
//! 3. the alloc-spy counters move by exactly {0, 0} across the
//!    surviving-handle read sweep;
//! 4. a full round-trip (create → project → content → typed i64 read →
//!    free) works after the storm.
//!
//! Accounting discipline: the iteration manager never holds a resource term
//! (handles flow worker → verifier only), and the verifier is itself
//! DOWN-collected before the final dtor read — a surviving pin anywhere
//! would suppress a dtor and read as a leak. The BEAM process is the
//! collaborator; no mocks. Exit code 0 from the escript = all courts hold
//! for all 100 iterations.
//!
//! Budget: a healthy run is well under a minute; the escript self-limits at
//! a 240 s wall-clock watchdog (inside the crate's 600 s test gate).

use std::path::PathBuf;
use std::process::Command;

/// Locate the built NIF dylib (`target/{debug,release}/libcgap_beam.{dylib,so}`),
/// building it first if absent. Honors `CARGO_TARGET_DIR`. (Same helper as
/// `bridge_contract.rs` — integration tests are separate crates.)
fn nif_dylib_path() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let target_root = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest_dir.join("../../target").canonicalize().unwrap_or(manifest_dir.join("../../target")));
    for profile in ["debug", "release"] {
        for ext in ["dylib", "so"] {
            let p = target_root.join(profile).join(format!("libcgap_beam.{ext}"));
            if p.exists() {
                return p;
            }
        }
    }
    // Not built yet: build it (test profile) with a clean cargo environment.
    let status = Command::new("cargo")
        .args(["build", "-p", "cgap-beam"])
        .current_dir(&manifest_dir)
        .env_remove("CARGO")
        .env_remove("CARGO_MANIFEST_DIR")
        .env_remove("CARGO_PKG_VERSION")
        .env_remove("CARGO_TARGET_DIR")
        .status()
        .expect("spawn cargo build -p cgap-beam");
    assert!(status.success(), "cargo build -p cgap-beam failed: {status}");
    let p = target_root.join("debug").join("libcgap_beam.dylib");
    assert!(p.exists(), "dylib missing after build: {}", p.display());
    p
}

/// The BEAM-side kill-storm program. The module name must be `cgap_beam` —
/// `erlang:load_nif/2` refuses a NIF whose entry module name differs from the
/// calling module — and the escript file stem must match the module (escript
/// compiles the script as that module). The temp dir disambiguates.
const ESCRIPT_SRC: &str = r##"#! /usr/bin/env escript
%% Concurrent-kill stress court (wave-14 lane 4).
%% Usage: escript <this> <nif-dylib-path>. Exit 0 = all courts hold.
-module(cgap_beam).
-export([main/1]).
%% load_nif's eager check refuses any library entry the module cannot be
%% found to have (witnessed on this OTP: an uncalled, unexported stub fails
%% with bad_lib "Function not found"); this escript exports the full stub
%% surface explicitly, which alone satisfies the check (witnessed).
-export([cgap_new/2, cgap_new_fast/2, cgap_new_i64/2, cgap_len/1, cgap_stats/1,
         cgap_get/2, cgap_get_i64/2, cgap_free/1, cgap_version/0,
         cgap_envelope_new/1, cgap_envelope_len/1, cgap_batch_binary/1,
         pointer_of/1, cgap_binary_data_pointer/1, cgap_dtor_stats/0,
         cgap_alloc_stats/0, cgap_open/1, cgap_scan/3,
         cgap_flush_to_parquet/1, cgap_seal_to_packfile/1,
         cgap_receipt_query/1]).

-define(ITERATIONS, 100).
-define(N_WORKERS, 100).
-define(N_KILL, 50).                  %% random exact half, killed mid-flight
-define(E, 4).                        %% envelopes per worker
-define(N_SURV, (?N_WORKERS - ?N_KILL)).
-define(GC_ROUNDS, 40).               %% create-phase GC stress rounds
-define(BUSY_POLLS, 2000).            %% mid-flight await budget before blocking
-define(SETTLE_ROUNDS, 1000).         %% bounded dtor-counter convergence barrier
-define(WALL_BUDGET_MS, 240000).      %% self-limit, inside the 600s test gate

%% NIF stubs — the full 21-function surface (load_nif checks eagerly that
%% every library function has a module export, used or not).
cgap_new(_, _) -> erlang:nif_error(nif_not_loaded).
cgap_new_fast(_, _) -> erlang:nif_error(nif_not_loaded).
cgap_new_i64(_, _) -> erlang:nif_error(nif_not_loaded).
cgap_len(_) -> erlang:nif_error(nif_not_loaded).
cgap_stats(_) -> erlang:nif_error(nif_not_loaded).
cgap_get(_, _) -> erlang:nif_error(nif_not_loaded).
cgap_get_i64(_, _) -> erlang:nif_error(nif_not_loaded).
cgap_free(_) -> erlang:nif_error(nif_not_loaded).
cgap_version() -> erlang:nif_error(nif_not_loaded).
cgap_envelope_new(_) -> erlang:nif_error(nif_not_loaded).
cgap_envelope_len(_) -> erlang:nif_error(nif_not_loaded).
cgap_batch_binary(_) -> erlang:nif_error(nif_not_loaded).
pointer_of(_) -> erlang:nif_error(nif_not_loaded).
cgap_binary_data_pointer(_) -> erlang:nif_error(nif_not_loaded).
cgap_dtor_stats() -> erlang:nif_error(nif_not_loaded).
cgap_alloc_stats() -> erlang:nif_error(nif_not_loaded).
cgap_open(_) -> erlang:nif_error(nif_not_loaded).
cgap_scan(_, _, _) -> erlang:nif_error(nif_not_loaded).
cgap_flush_to_parquet(_) -> erlang:nif_error(nif_not_loaded).
cgap_seal_to_packfile(_) -> erlang:nif_error(nif_not_loaded).
cgap_receipt_query(_) -> erlang:nif_error(nif_not_loaded).

main([NifPath]) ->
    ok = case erlang:load_nif(NifPath, 0) of
             ok -> ok;
             {error, R} -> erlang:error({load_nif_failed, R})
         end,
    erlang:send_after(?WALL_BUDGET_MS, self(), wall_clock_expired),
    Res = try storm_all(?ITERATIONS, []) of
              ok -> 0
          catch Class:Reason:Stack ->
              io:format("KILL_STRESS_FAIL ~p:~p~n~p~n", [Class, Reason, Stack]),
              1
          end,
    halt(Res).

%% --- iteration driver --------------------------------------------------------

storm_all(0, Infos) -> finish(Infos);
storm_all(K, Infos) ->
    Iter = ?ITERATIONS - K + 1,
    Self = self(),
    {Mgr, Mon} = spawn_monitor(fun() -> storm_iter_run(Self, Iter) end),
    Info = receive
        {storm_done, I, Info0} when I =:= Iter ->
            receive
                {'DOWN', Mon, process, Mgr, normal} -> ok;
                {'DOWN', Mon, process, Mgr, MReason} -> erlang:error({manager_died, MReason});
                wall_clock_expired -> erlang:error(wall_clock_budget_exceeded)
            after 10000 -> erlang:error(manager_down_timeout)
            end,
            Info0;
        {storm_failed, I, C, R, S} when I =:= Iter ->
            erlang:error({storm_failed, I, C, R, S});
        {'DOWN', Mon, process, Mgr, MReason} ->
            erlang:error({storm_crashed, Iter, MReason});
        wall_clock_expired ->
            erlang:error(wall_clock_budget_exceeded)
    after 180000 -> erlang:error({storm_timeout, Iter})
    end,
    {Dead, Served} = Info,
    io:format("iter ~3B ok dead_dtors=~B served_reads=~B alloc_delta={0,0}~n",
              [Iter, Dead, Served]),
    storm_all(K - 1, [Info | Infos]).

storm_iter_run(Main, Iter) ->
    try storm_iter(Iter) of
        Counts -> Main ! {storm_done, Iter, Counts}
    catch Class:Reason:Stack ->
        Main ! {storm_failed, Iter, Class, Reason, Stack},
        exit({storm_failed, Iter})
    end.

%% One create/kill/verify cycle. Returns {DeadDtorDelta, ServedEnvs}; both are
%% ?N_KILL * ?E by the exactly-once law, re-added across iterations in finish/1.
storm_iter(Iter) ->
    {D0} = cgap_dtor_stats(),
    true = is_integer(D0) andalso D0 >= 0,
    Self = self(),
    Ws = [spawn_monitor(fun() -> worker(Self, Iter, W) end)
          || W <- lists:seq(1, ?N_WORKERS)],
    Pids = [P || {P, _} <- Ws],
    KillSet = kill_half(Pids),
    Survivors = collect_readies(Pids, KillSet, []),
    ?N_KILL = length(Pids) - length(Survivors),
    %% Court 2a (killed half): process-exit semantics. Every killed worker
    %% held exactly ?E envelopes; its termination releases each EXACTLY ONCE,
    %% before its 'DOWN' reaches us (witnessed semantics of
    %% t_dtor_exactly_once on this OTP). Survivors still pin theirs, so the
    %% counter must move by exactly the dead count — no leak (delta too
    %% small) and no double destruction (delta too large).
    ok = await_downs([M || {P, M} <- Ws, lists:member(P, KillSet)], killed),
    DeadEnvs = ?N_KILL * ?E,
    D1 = settle_dtor(D0, DeadEnvs, ?SETTLE_ROUNDS),
    %% Courts 1+3 (survivors): reads still serve and the read hot path
    %% allocates {0,0} at counted sites. Verified in a dedicated process so
    %% THIS manager's heap never touches a resource term — keeps the D2
    %% boundary pure process-exit.
    {V, VM} = spawn_monitor(fun() -> verifier(Self) end),
    [P ! {send, V} || P <- Survivors],
    ExpectedServed = ?N_SURV * ?E,
    Served = receive
        {verified, V, N} -> N;
        {verify_failed, V, C, R, S} -> erlang:error({verifier_failed, C, R, S});
        {'DOWN', VM, process, V, VReason} -> erlang:error({verifier_died, VReason})
    after 30000 -> erlang:error(verifier_timeout)
    end,
    ExpectedServed = Served,
    %% Court 2b (released survivors): each survivor envelope must die exactly
    %% once once BOTH of its pins are gone — the worker's (its exit) and the
    %% verifier's (its heap destruction). Both exits are DOWN-collected, then
    %% the counter converges through the same bounded barrier as Court 2a:
    %% the ceiling (no double destruction) holds at every read, and the
    %% at-least-once side must land exactly on the dead count.
    [P ! {release} || P <- Survivors],
    ok = await_downs([M || {P, M} <- Ws, not lists:member(P, KillSet)], normal),
    ok = receive
        {'DOWN', VM, process, V, normal} -> ok;
        {'DOWN', VM, process, V, VReason2} -> erlang:error({verifier_died, VReason2})
    after 30000 -> erlang:error(verifier_down_timeout)
    end,
    D2 = settle_dtor(D1, DeadEnvs, ?SETTLE_ROUNDS),
    %% Court 4: full round-trip after the storm. The envelope minted here is
    %% pinned only by this manager and dies with it — after every counter
    %% read of this iteration, so it cannot pollute any delta.
    Id = cgap_new(64, Iter),
    64 = cgap_len(Id),
    RB = cgap_batch_binary(Id),
    256 = byte_size(RB),
    ExpectedRamp = ramp_bin(Iter, 64),
    ExpectedRamp = RB,
    error = cgap_len(Id),            %% projection CONSUMED the handle
    Id64 = cgap_new_i64(2, -Iter),
    NegFill = cgap_get_i64(Id64, 0),
    true = NegFill =:= -Iter,
    ok = cgap_free(Id64),
    {DeadEnvs, Served}.

%% --- workers -----------------------------------------------------------------

%% Create ?E envelopes (distinct lengths, distinct fills), project each as a
%% zero-copy resource binary, GC-stress while holding, then check in and keep
%% GC-stressing while awaiting orders — the kill lands in that mid-flight
%% loop for the kill-set half.
worker(Manager, Iter, W) ->
    Hs = [begin
              Id = cgap_new(Len, Fill),
              true = is_integer(Id),
              Env = cgap_envelope_new(Id),
              true = is_reference(Env),
              error = cgap_envelope_new(Id),      %% id was consumed
              Bin = cgap_batch_binary(Env),       %% envelope path: non-consuming
              true = is_binary(Bin) andalso byte_size(Bin) =:= Len * 4,
              Expected = ramp_bin(Fill, Len),
              Expected = Bin,
              {Env, Bin, Len, Fill}
          end || {Len, Fill} <- specs(Iter, W)],
    ok = gc_hold(Hs, ?GC_ROUNDS),
    Manager ! {ready, self()},
    await_orders(Hs, ?BUSY_POLLS).

specs(Iter, W) ->
    [{8 * (1 + ((W + K) rem 4)), 1 + ((Iter * 97 + W * 13 + K * 7) rem 5000)}
     || K <- lists:seq(1, ?E)].

gc_hold(_Hs, 0) -> ok;
gc_hold(Hs, N) ->
    garbage_collect(),
    ok = check_holdings(Hs),
    gc_hold(Hs, N - 1).

check_holdings(Hs) ->
    case lists:all(fun({Env, Bin, Len, _Fill}) ->
                       (Len =:= cgap_envelope_len(Env))
                           andalso (Len * 4 =:= byte_size(Bin))
                   end,
                   Hs) of
        true -> ok;
        false -> erlang:error(holding_lost_across_gc)
    end.

await_orders(Hs, 0) ->
    receive
        {send, V} -> handle_orders(V, Hs)
    after 60000 -> exit(orders_timeout)
    end;
await_orders(Hs, N) ->
    receive
        {send, V} -> handle_orders(V, Hs)
    after 0 ->
        garbage_collect(),
        ok = check_holdings(Hs),
        await_orders(Hs, N - 1)
    end.

handle_orders(V, Hs) ->
    V ! {handles, self(), Hs},
    receive {release} -> exit(normal) after 60000 -> exit(release_timeout) end.

%% --- verifier ----------------------------------------------------------------

%% Collects the survivors' handles and runs the two survivor courts. It holds
%% resource terms, so it MUST be DOWN-collected before the final dtor read —
%% a surviving pin would suppress a dtor and read as a leak.
verifier(Manager) ->
    try
        HSets = recv_handles(?N_SURV, []),
        {A0, B0} = cgap_alloc_stats(),
        [ok = verify_handle(H) || HS <- HSets, H <- HS],
        {A1, B1} = cgap_alloc_stats(),
        0 = A1 - A0,
        0 = B1 - B0,
        Manager ! {verified, self(), lists:sum([length(HS) || HS <- HSets])}
    catch Class:Reason:Stack ->
        Manager ! {verify_failed, self(), Class, Reason, Stack}
    end.

recv_handles(0, Acc) -> Acc;
recv_handles(K, Acc) ->
    receive
        {handles, _Worker, HS} -> recv_handles(K - 1, [HS | Acc])
    after 30000 -> erlang:error(handles_timeout)
    end.

%% One surviving handle: metadata reads, pointer stability across calls,
%% payload identity (the binary's data pointer IS the exposed envelope
%% address), exact ramp bytes, and a zero-copy re-projection byte-equal to
%% the original binary.
verify_handle({Env, Bin, Len, Fill}) ->
    Len = cgap_envelope_len(Env),
    P = pointer_of(Env),
    true = is_integer(P) andalso P > 0,
    P = pointer_of(Env),
    Size = Len * 4,
    Size = byte_size(Bin),
    P = cgap_binary_data_pointer(Bin),
    Expected = ramp_bin(Fill, Len),
    Expected = Bin,
    B2 = cgap_batch_binary(Env),
    Size = byte_size(B2),
    Expected = B2,
    ok.

%% --- helpers -------------------------------------------------------------------

kill_half(Pids) ->
    Ranked = lists:sort([{rand:uniform(), P} || P <- Pids]),
    {Half, _Rest} = lists:split(?N_KILL, Ranked),
    [P || {_R, P} <- Half].

collect_readies([], _KillSet, Acc) -> lists:reverse(Acc);
collect_readies(Remaining, KillSet, Acc) ->
    receive
        {ready, P} ->
            case lists:member(P, KillSet) of
                true ->
                    exit(P, kill),   %% brutal, untrappable: mid-flight death
                    collect_readies(lists:delete(P, Remaining), KillSet, Acc);
                false ->
                    collect_readies(lists:delete(P, Remaining), KillSet, [P | Acc])
            end
    after 30000 -> erlang:error(ready_timeout)
    end.

await_downs([], _Reason) -> ok;
await_downs([M | Rest], Reason) ->
    receive
        {'DOWN', M, process, _, Reason} -> await_downs(Rest, Reason)
    after 30000 -> erlang:error({down_timeout, Reason})
    end.

%% Bounded convergence barrier for the dtor counter (no sleeps): the counter
%% is re-read across synchronous garbage_collect() rounds until the delta is
%% EXACTLY the dead count. The at-most-once ceiling (delta never exceeds the
%% dead count) is asserted at EVERY read — a double destruction fails
%% immediately; a genuine pin (a leak) fails loudly after the bounded rounds.
%% Witnessed on this OTP: ~1 in ~600 process exits releases its envelope
%% resources a few scheduler passes after its 'DOWN' is delivered, so a raw
%% post-DOWN read lags (witnessed 196 of 200 on iteration 6) — the wave-5
%% GC-lag caveat, surfacing on the exit path itself.
settle_dtor(Before, Expected, Rounds) ->
    {D} = cgap_dtor_stats(),
    Delta = D - Before,
    if
        Delta =:= Expected -> D;
        Delta > Expected -> erlang:error({dtor_overshoot, Expected, Delta});
        Rounds =:= 0 -> erlang:error({dtor_lag_no_convergence, Expected, Delta});
        true ->
            garbage_collect(),
            settle_dtor(Before, Expected, Rounds - 1)
    end.

ramp_bin(Fill, Len) ->
    << <<(Fill + I):32/little-signed>> || I <- lists:seq(0, Len - 1) >>.

finish(Infos) ->
    ?ITERATIONS = length(Infos),
    PerIter = ?N_KILL * ?E,
    TotalDead = lists:sum([D || {D, _S} <- Infos]),
    TotalServed = lists:sum([S || {_D, S} <- Infos]),
    ExpectedTotal = ?ITERATIONS * PerIter,
    ExpectedTotal = TotalDead,
    ExpectedTotal = TotalServed,
    io:format("ok kill_stress iterations=~B workers=~B kill_half=~B envs_per_worker=~B "
              "killed_dtors=~B survivor_dtors=~B roundtrips=~B alloc_delta={0,0}~n",
              [?ITERATIONS, ?N_WORKERS, ?N_KILL, ?E, TotalDead, TotalServed,
               ?ITERATIONS]),
    ok.
"##;

#[test]
fn beam_kill_stress_100_iterations() {
    let dylib = nif_dylib_path();
    let erl = std::env::var("ESCRIPT").unwrap_or_else(|_| "escript".to_string());

    let script_dir = std::env::temp_dir().join(format!("cgap_beam_kill_stress_{}", std::process::id()));
    std::fs::create_dir_all(&script_dir).expect("create temp script dir");
    let script = script_dir.join("cgap_beam");
    std::fs::write(&script, ESCRIPT_SRC).expect("write escript");

    // This OTP appends ".so" to the load_nif path unconditionally, so the
    // staged file is named `libcgap_beam.so` while the passed path is the stem.
    let staged = script_dir.join("libcgap_beam.so");
    std::fs::copy(&dylib, &staged).expect("copy dylib into temp dir");
    let nif_path = script_dir.join("libcgap_beam");

    let t0 = std::time::Instant::now();
    let out = Command::new(&erl)
        .args([script.to_str().unwrap(), nif_path.to_str().unwrap()])
        .output()
        .expect("spawn escript (set ESCRIPT=... if escript is not on PATH)");
    let elapsed = t0.elapsed();

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    print!("{stdout}");
    eprint!("{stderr}");
    std::fs::remove_dir_all(&script_dir).ok();

    assert!(
        out.status.success(),
        "kill-stress run failed (exit {:?}) after {elapsed:?}:\n{stdout}\n{stderr}",
        out.status.code()
    );
    assert!(
        !stdout.contains("KILL_STRESS_FAIL") && !stdout.contains("{storm_failed"),
        "kill-stress escript reported a failed court:\n{stdout}"
    );
    // Per-iteration witness: 100 iterations, each killing 50 workers x 4
    // envelopes exactly once (dead_dtors=200) and serving reads over the
    // surviving 50 x 4 handles (served_reads=200) with alloc delta {0,0}.
    assert_eq!(
        stdout.matches("dead_dtors=200 served_reads=200").count(),
        100,
        "expected 100 iteration lines witnessing dead_dtors=200 served_reads=200"
    );
    let summary = "ok kill_stress iterations=100 workers=100 kill_half=50 envs_per_worker=4";
    assert!(
        stdout.contains(summary),
        "missing kill-stress summary proof `{summary}` in:\n{stdout}"
    );
    eprintln!("kill_stress escript wall time: {elapsed:?}");
}
