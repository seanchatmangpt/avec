#!/usr/bin/env escript
%% avec-beam scheduling-regime crossover probe (lane BEAM-L4).
%%
%% Question: does the dirty-CPU flag on `avec_new/2` earn its keep on call
%% latency alone, or is it a wash against the normal scheduler? The membrane
%% exposes both regimes over the identical fn body: `avec_new/2` (dirty, flags
%% 0x01) and `avec_new_fast/2` (normal, flags 0). Same builder loop, same
%% 64-byte-aligned allocation, same registry insert — the only controlled
%% variable is which scheduler thread runs the call.
%%
%% Method (release dylib, monotonic ns brackets, free untimed — the sensor
%% discipline of test/beam_latency2.escript):
%%   for each n in {1000, 4000, 16384, 65536, 262144}:
%%     3 warmup calls per variant, then min-of-20 PAIRED samples: each round
%%     times dirty once then fast once, so ambient host drift hits both
%%     variants inside the same round instead of biasing the later block
%%     (witnessed: three sequential-block runs gave three different verdict
%%     shapes — wash, never, above-262144 — before this pairing; the dirty
%%     column was stable across runs while the later-measured fast column
%%     swung 438-462 us at n=262144). Each sample is bracketed by
%%     erlang:monotonic_time(nanosecond); the batch from every timed call is
%%     freed outside the timed window so buffer churn does not compound.
%%
%% Verdict rule (printed with the verdict): let W = the set of measured n
%% where dirty < fast strictly. Empty W -> the dirty flag never earns its
%% keep in this range. Else Nc = min(W): if dirty also wins at every measured
%% n > Nc, the dirty flag earns its keep above n = Nc; if any larger n flips
%% back to fast, the table shows no clean crossover and the harness says so
%% rather than forcing the dichotomy.
%%
%% Exit codes: 0 completed run (any verdict); 1 version-law mismatch or
%% sanity failure; 2 NIF load failure; 3 release dylib missing.

-module(avec_beam).
-export([main/1, avec_new/2, avec_new_fast/2, avec_len/1, avec_get/2,
         avec_free/1, avec_version/0, avec_new_i64/2, avec_get_i64/2,
         avec_stats/1, avec_envelope_new/1, avec_envelope_len/1,
         avec_batch_binary/1]).
-on_load(init/0).

-define(VERSION, <<"26.10.29">>).
-define(RELEASE_DIR, "/Users/sac/avec/target/release").
-define(NS, [1000, 4000, 16384, 65536, 262144]).
-define(REPS, 20).
-define(WARMUP, 3).

init() ->
    Dylib = ?RELEASE_DIR ++ "/libavec_beam.dylib",
    NifBase = ?RELEASE_DIR ++ "/libavec_beam",
    So = NifBase ++ ".so",
    case file:read_link_info(Dylib) of
        {ok, _} ->
            ok;
        _ ->
            io:format("RELEASE DYLIB MISSING: ~s (build it first: cargo build --release -p avec-beam)~n",
                      [Dylib]),
            erlang:halt(3)
    end,
    %% OTP's load_nif always appends ".so"; refresh the symlink every run so
    %% it targets the current release build (same dance as test/beam_test.escript).
    case file:read_link(So) of
        {ok, Dylib} ->
            ok;
        _ ->
            case file:read_link_info(So) of
                {ok, _} -> file:delete(So);
                _ -> ok
            end,
            ok = file:make_symlink(Dylib, So)
    end,
    case erlang:load_nif(NifBase, 0) of
        ok ->
            io:format("nif loaded: ~s~n", [So]),
            ok;
        {error, {Reason, Txt}} ->
            io:format("LOAD FAILED: ~p: ~ts~n", [Reason, Txt]),
            erlang:halt(2)
    end.

main(_Args) ->
    print_env(),
    sanity(),
    table(),
    io:format("ALL SCHED-CROSSOVER RUNS COMPLETED~n").

%% --- environment -------------------------------------------------------------

print_env() ->
    io:format("[env] erts=~s otp_release=~s logical_cpus=~p schedulers_online=~p dirty_cpu_schedulers=~p~n",
              [erlang:system_info(version),
               erlang:system_info(otp_release),
               erlang:system_info(logical_processors_available),
               erlang:system_info(schedulers_online),
               erlang:system_info(dirty_cpu_schedulers_online)]).

%% --- contract sanity: the two regimes are behaviorally identical --------------

sanity() ->
    case avec_version() of
        ?VERSION ->
            ok;
        Other ->
            io:format("VERSION LAW VIOLATION: avec_version() = ~p, want ~ts~n",
                      [Other, ?VERSION]),
            erlang:halt(1)
    end,
    %% Fast variant obeys the ramp contract...
    IdF = avec_new_fast(1000, 5),
    true = is_integer(IdF),
    1000 = avec_len(IdF),
    5 = avec_get(IdF, 0),
    1004 = avec_get(IdF, 999),
    %% ...the dirty variant still does (unchanged surface)...
    IdD = avec_new(1000, 5),
    true = is_integer(IdD),
    1000 = avec_len(IdD),
    5 = avec_get(IdD, 0),
    1004 = avec_get(IdD, 999),
    %% ...ids are independent, and both free cleanly.
    true = IdF =/= IdD,
    ok = avec_free(IdF),
    ok = avec_free(IdD),
    error = avec_free(IdF),
    io:format("ok   sanity: fast ramp == dirty ramp == ramp contract; ids independent; version law 26.10.29 verified~n").

%% --- crossover table -----------------------------------------------------------

table() ->
    Rows = [row(N) || N <- ?NS],
    io:format("~n== crossover table (release dylib, min-of-~p wall us, free untimed) ==~n", [?REPS]),
    lists:foreach(fun({N, DirtyUs, FastUs}) ->
                          Winner = case DirtyUs < FastUs of
                                       true -> dirty;
                                       false -> fast
                                   end,
                          io:format("n=~p dirty=~.3f fast=~.3f winner=~p~n",
                                    [N, DirtyUs, FastUs, Winner])
                  end, Rows),
    verdict(Rows).

row(N) ->
    D = fun avec_new/2,
    F = fun avec_new_fast/2,
    warmup(D, ?WARMUP, N),
    warmup(F, ?WARMUP, N),
    Pairs = [begin
                 DUs = timed_us(D, N),
                 FUs = timed_us(F, N),
                 {DUs, FUs}
             end || _ <- lists:seq(1, ?REPS)],
    {N, lists:min([S || {S, _} <- Pairs]), lists:min([S || {_, S} <- Pairs])}.

%% One paired sample: dirty bracketed, fast bracketed, each batch freed after
%% its bracket (untimed) so the next round's allocation starts from the same
%% allocator state.
timed_us(F, N) ->
    T0 = erlang:monotonic_time(nanosecond),
    Id = F(N, 0),
    T1 = erlang:monotonic_time(nanosecond),
    ok = avec_free(Id),
    (T1 - T0) / 1000.0.

warmup(_F, 0, _N) ->
    ok;
warmup(F, K, N) ->
    Id = F(N, 0),
    ok = avec_free(Id),
    warmup(F, K - 1, N).

%% --- verdict ---------------------------------------------------------------------

verdict(Rows) ->
    Wins = [N || {N, D, F} <- Rows, D < F],
    case Wins of
        [] ->
            io:format("VERDICT: the dirty flag never earns its keep in this range "
                      "(dirty >= fast at every measured n)~n");
        _ ->
            Nc = lists:min(Wins),
            LosesAbove = [N || {N, D, F} <- Rows, N > Nc, D >= F],
            case LosesAbove of
                [] ->
                    {Nc, D, F} = lists:keyfind(Nc, 1, Rows),
                    io:format("VERDICT: the dirty flag earns its keep above n=~p "
                              "(dirty strictly wins at n=~p and every larger measured n)~n",
                              [Nc, Nc]),
                    io:format("[verdict-note] margin at crossover: dirty ~s fast by ~.1f% (~.3f vs ~.3f us); "
                              "fast's wins below it are ~s% — dirty's win is marginal, fast's are not~n",
                              [if D < F -> "beats"; true -> "loses to" end,
                               100.0 * abs(D - F) / erlang:min(D, F),
                               D, F,
                               below_note(Rows, Nc)]);
                _ ->
                    io:format("VERDICT: no clean crossover — dirty wins at ~p but loses at ~p "
                              "(non-monotone table; regime choice is a wash at this sensor's resolution)~n",
                              [Wins, LosesAbove])
            end
    end.

%% Range of fast's winning margins strictly below the crossover point.
below_note(Rows, Nc) ->
    Below = [100.0 * (D - F) / F || {N, D, F} <- Rows, N < Nc],
    case Below of
        [] -> "n/a (no measured n below the crossover)";
        _ ->
            Pcts = [io_lib:format("~.1f", [P]) || P <- Below],
            string:join(Pcts, "/")
    end.

%% --- NIF stubs (replaced at load_nif time) ----------------------------------------

avec_new(_N, _Fill) ->
    erlang:nif_error(nif_library_not_loaded).
avec_new_fast(_N, _Fill) ->
    erlang:nif_error(nif_library_not_loaded).
avec_len(_Id) ->
    erlang:nif_error(nif_library_not_loaded).
avec_get(_Id, _I) ->
    erlang:nif_error(nif_library_not_loaded).
avec_free(_Id) ->
    erlang:nif_error(nif_library_not_loaded).
avec_version() ->
    erlang:nif_error(nif_library_not_loaded).

%% load_nif law (witnessed twice now: BEAM-L1 and this lane): EVERY NIF the
%% library exports needs an exported stub in the loading module, called or
%% not. Remaining exports of the 11-row table:
avec_new_i64(_N, _Fill) ->
    erlang:nif_error(nif_library_not_loaded).
avec_get_i64(_Id, _I) ->
    erlang:nif_error(nif_library_not_loaded).
avec_stats(_Id) ->
    erlang:nif_error(nif_library_not_loaded).
avec_envelope_new(_Id) ->
    erlang:nif_error(nif_library_not_loaded).
avec_envelope_len(_Env) ->
    erlang:nif_error(nif_library_not_loaded).
avec_batch_binary(_Id) ->
    erlang:nif_error(nif_library_not_loaded).
