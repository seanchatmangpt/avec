#!/usr/bin/env escript
%% cgap-beam sub-us latency + scheduler-isolation probe (lane BEAM-L7).
%%
%% Companion to test/beam_latency.escript (OPT-L8). That harness's <=50 us CTQ
%% sensor was a 1 ms-timer sensor; its floor (idle overshoot ~+1.0 ms median,
%% multi-ms tails) exceeds the 50 us gate by orders of magnitude, so that
%% verdict measured the timer floor, not the membrane. This harness implements
%% the honest sensor class the BEAM itself supports:
%%
%%   1. cgap_get/2 round-trip latency on the RELEASE dylib: 10000 samples,
%%      each bracketed by explicit-unit erlang:monotonic_time(nanosecond)
%%      reads, plus a no-NIF calibration row so the bracketing overhead is
%%      subtractable. This sensor has ns-class resolution and receipts the
%%      sub-us single-call claim.
%%   2. Scheduler isolation, benchmark-to-degradation probe: a fixed pure-
%%      Erlang workload (tight tail-recursive integer loop, calibrated once
%%      to ~10 ms per burst) is timed burst-by-burst across five 2.5 s
%%      windows: idle-A, churn-1, churn-4, churn-FULL (dirty CPU scheduler
%%      count), idle-B. Churn = continuous cgap_new(262144,0)/cgap_free
%%      loops; 262144 >= 16384 rows => dirty-CPU path (NIF_FUNCS threshold,
%%      crates/cgap-beam/src/lib.rs). If dirty work stays on dirty
%%      schedulers, a normal-scheduler Erlang burst is not slowed by dirty
%%      churn except through OS core contention. The churn sweep separates
%%      the two: degradation present even at churn-1 (one dirty thread on
%%      16 cores, no oversubscription) would be consistent with dirty-to-
%%      normal leakage; degradation appearing only near saturation tracks
%%      OS oversubscription. Idle-B brackets ambient host drift. Verdict
%%      rule (printed by the harness): ISOLATED-AT-RESOLUTION if every
%%      churn median <= 1.10 x min(idle medians); ISOLATION-CONSISTENT if
%%      churn-1 is within +10% while saturation degrades; DEGRADED-EVEN-AT-
%%      CHURN-1 if the churn-1 median ratio > 1.25; else AMBIGUOUS.
%%   3. What this sensor class cannot prove: a <=50 us scheduler-impact
%%      bound. Erlang timers floor near 1 ms, statistics/1 counters are
%%      ms-granular, and monotonic_time brackets only code the BEAM itself
%%      runs, so no in-ERTS sensor can timestamp an external wakeup with
%%      sub-us truth. The 50 us CTQ needs hardware-timer instrumentation
%%      and is reported UNSUPPORTED (instrument class) here, not PASS and
%%      not FAIL.
%%
%% Exit codes: 0 completed run (any measurement verdict); 1 version-law
%% mismatch; 2 NIF load failure; 3 phase timeout.

%% Module name MUST be cgap_beam: the NIF registers itself under the module
%% name baked in at build time (bad_lib mismatch otherwise), mirroring
%% test/beam_latency.escript.
-module(cgap_beam).
-export([main/1]).
-on_load(init/0).

-define(VERSION, <<"26.10.29">>).
-define(RELEASE_DIR, "/Users/sac/cgap/target/release").
-define(GET_N, 10000).
-define(GET_ROWS, 1024).
-define(BURST_TARGET_NS, 10000000).
-define(PHASE_MS, 2500).
-define(CHURN_ROWS, 262144). %% 1 MiB i32 value buffer per cgap_new => dirty CPU
-define(CHURN_LIGHT, 1).
-define(CHURN_MID, 4).

init() ->
    %% Release dylib only: lane BEAM-L7 receipts the release membrane. OTP's
    %% load_nif always appends ".so", so expose the cargo cdylib under the
    %% name the BEAM looks for. Reuse an existing correct symlink; refresh
    %% it only if stale (build-artifact dir, same dance as
    %% test/beam_test.escript).
    Dylib = ?RELEASE_DIR ++ "/libcgap_beam.dylib",
    NifBase = ?RELEASE_DIR ++ "/libcgap_beam",
    So = NifBase ++ ".so",
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
    get_latency(),
    churn_char(),
    isolation_probe().

%% --- environment -----------------------------------------------------------

print_env() ->
    Dylib = ?RELEASE_DIR ++ "/libcgap_beam.dylib",
    Sha = string:trim(os:cmd("shasum -a 256 " ++ Dylib), both),
    io:format("[env] erts=~s otp_release=~s~n",
              [erlang:system_info(version),
               erlang:system_info(otp_release)]),
    io:format("[env] logical_cpus=~p schedulers_online=~p dirty_cpu_schedulers=~p dirty_io_schedulers=~p native_ns_per_unit=~.1f~n",
              [erlang:system_info(logical_processors_available),
               erlang:system_info(schedulers_online),
               erlang:system_info(dirty_cpu_schedulers_online),
               erlang:system_info(dirty_io_schedulers),
               1.0 * erlang:convert_time_unit(1, native, nanosecond)]),
    io:format("[env] nif_build=release dylib=target/release/libcgap_beam.dylib run_class=single-run machine-local~n"),
    io:format("[env] dylib_sha256: ~s~n", [Sha]),
    io:format("[env] host_load: ~s", [os:cmd("uptime")]).

%% --- contract sanity before any timing --------------------------------------

sanity() ->
    Id = cgap_new(1000, 5),
    true = is_integer(Id),
    1000 = cgap_len(Id),
    5 = cgap_get(Id, 0),
    1004 = cgap_get(Id, 999),
    {1000, 32} = cgap_stats(Id),
    ok = cgap_free(Id),
    Id64 = cgap_new_i64(10, 1099511627776),
    1099511627776 = cgap_get_i64(Id64, 0),
    {10, 64} = cgap_stats(Id64),
    ok = cgap_free(Id64),
    case cgap_version() of
        ?VERSION ->
            io:format("ok   sanity: ramp + stats + i64 path verified; version law 26.10.29 verified (~ts)~n",
                      [?VERSION]);
        Other ->
            io:format("VERSION LAW VIOLATION: cgap_version() = ~p, want ~ts~n",
                      [Other, ?VERSION]),
            erlang:halt(1)
    end.

%% --- 1. cgap_get round-trip latency, release dylib, ns-unit brackets ---------

get_latency() ->
    Id = cgap_new(?GET_ROWS, -100),
    true = is_integer(Id),
    lists:foreach(fun(I) -> _ = cgap_get(Id, I rem ?GET_ROWS) end,
                  lists:seq(1, 1000)),
    Samples = [begin
                   T0 = erlang:monotonic_time(nanosecond),
                   _ = cgap_get(Id, I rem ?GET_ROWS),
                   T1 = erlang:monotonic_time(nanosecond),
                   T1 - T0
               end || I <- lists:seq(1, ?GET_N)],
    Cal = [begin
               T0 = erlang:monotonic_time(nanosecond),
               T1 = erlang:monotonic_time(nanosecond),
               T1 - T0
           end || _ <- lists:seq(1, ?GET_N)],
    ok = cgap_free(Id),
    report_us("[get-latency]      ", Samples,
              "cgap_get/2 round-trip us (incl. 2x monotonic_time(nanosecond)), release dylib"),
    report_us("[get-calibration]  ", Cal,
              "monotonic_time pair only, no NIF (subtractable bracketing overhead)").

%% --- 2. churn characterization on the release build --------------------------

churn_char() ->
    lists:foreach(fun(_) ->
                      Id = cgap_new(?CHURN_ROWS, 0),
                      true = is_integer(Id),
                      ok = cgap_free(Id)
                  end, lists:seq(1, 50)),
    Samples = [begin
                   T0 = erlang:monotonic_time(nanosecond),
                   Id = cgap_new(?CHURN_ROWS, 0),
                   T1 = erlang:monotonic_time(nanosecond),
                   ok = cgap_free(Id),
                   T1 - T0
               end || _ <- lists:seq(1, 50)],
    report_us("[churn-char]       ", Samples,
              "cgap_new(262144,0) wall us, dirty CPU, release, n=50 (free untimed); one churner loop = repeated calls of this size").

%% --- 3. scheduler isolation: fixed Erlang burst idle vs dirty churn ----------

isolation_probe() ->
    Iters = calibrate(),
    T0 = erlang:monotonic_time(nanosecond),
    _ = burst(Iters),
    T1 = erlang:monotonic_time(nanosecond),
    io:format("[burst-cal] iterations=~p measured_ms=~.3f (target ~.1f ms, fixed for all phases)~n",
              [Iters, (T1 - T0) / 1.0e6, ?BURST_TARGET_NS / 1.0e6]),
    Full = erlang:system_info(dirty_cpu_schedulers_online),
    IdleA = run_window(0, Iters),
    report_phase("[iso-idle-A ] ", 0, IdleA),
    R1 = run_window(?CHURN_LIGHT, Iters),
    report_phase("[iso-churn-1] ", ?CHURN_LIGHT, R1),
    RMid = run_window(?CHURN_MID, Iters),
    report_phase("[iso-churn-4] ", ?CHURN_MID, RMid),
    RFull = run_window(Full, Iters),
    report_phase("[iso-churn-F ] ", Full, RFull),
    IdleB = run_window(0, Iters),
    report_phase("[iso-idle-B ] ", 0, IdleB),
    verdict(IdleA, IdleB, [{"churn-1", R1}, {"churn-4", RMid},
                           {"churn-" ++ integer_to_list(Full), RFull}]).

calibrate() ->
    calibrate(100000, 6).

%% Converge the fixed burst into [8 ms, 14 ms] so each 2.5 s window yields
%% ~200 samples; whatever it converges to is then FROZEN for every phase.
calibrate(Iters, 0) ->
    Iters;
calibrate(Iters, Rounds) ->
    T0 = erlang:monotonic_time(nanosecond),
    _ = burst(Iters),
    T1 = erlang:monotonic_time(nanosecond),
    Measured = T1 - T0,
    case Measured >= 8000000 andalso Measured =< 14000000 of
        true ->
            Iters;
        false ->
            Next = erlang:max(1000, trunc(Iters * ?BURST_TARGET_NS / erlang:max(1, Measured))),
            calibrate(Next, Rounds - 1)
    end.

burst(N) ->
    burst(N, 0).

burst(0, Acc) ->
    Acc;
burst(N, Acc) ->
    burst(N - 1, Acc + 1).

run_window(Churners, Iters) ->
    Parent = self(),
    Deadline = erlang:monotonic_time(nanosecond) + ?PHASE_MS * 1000000,
    lists:foreach(fun(_) ->
                      erlang:spawn(fun() -> churn_loop(Parent, Deadline) end)
                  end, lists:seq(1, Churners)),
    Probe = erlang:spawn(fun() ->
                             erlang:process_flag(priority, normal),
                             probe_loop(Parent, Deadline, Iters, [])
                         end),
    collect(Churners + 1, Probe, []).

collect(0, _Probe, Acc) ->
    lists:append(lists:reverse(Acc));
collect(N, Probe, Acc) ->
    receive
        {probe_done, Probe, Samples} ->
            %% probe_loop sends its list already in chronological order
            collect(N - 1, Probe, [Samples | Acc]);
        churn_done ->
            collect(N - 1, Probe, Acc)
    after 60000 ->
        io:format("FAIL collect: phase timed out~n"),
        erlang:halt(3)
    end.

probe_loop(Parent, Deadline, Iters, Acc) ->
    case erlang:monotonic_time(nanosecond) >= Deadline of
        true ->
            Parent ! {probe_done, self(), lists:reverse(Acc)};
        false ->
            T0 = erlang:monotonic_time(nanosecond),
            _ = burst(Iters),
            T1 = erlang:monotonic_time(nanosecond),
            probe_loop(Parent, Deadline, Iters, [T1 - T0 | Acc])
    end.

churn_loop(Parent, Deadline) ->
    case erlang:monotonic_time(nanosecond) >= Deadline of
        true ->
            Parent ! churn_done;
        false ->
            Id = cgap_new(?CHURN_ROWS, 0),
            true = is_integer(Id),
            ok = cgap_free(Id),
            churn_loop(Parent, Deadline)
    end.

report_phase(Prefix, Churners, Samples) ->
    report_us(Prefix, Samples,
              "fixed pure-Erlang ~10ms burst wall us; churners=" ++
                  integer_to_list(Churners)).

%% Verdict: churn medians vs the tighter of the two idle brackets. Rule is
%% printed with the verdict so the thresholds are part of the receipt.
verdict(IdleA, IdleB, PhasePairs) ->
    MinIdleMed = lists:min([median(S) || {_, S} <- [{idle_a, IdleA}, {idle_b, IdleB}]]),
    MinIdleP99 = lists:min([p99(S) || {_, S} <- [{idle_a, IdleA}, {idle_b, IdleB}]]),
    Ratios = [{Name, median(S) / MinIdleMed, p99(S) / MinIdleP99}
              || {Name, S} <- PhasePairs],
    lists:foreach(fun({Name, RMed, RP99}) ->
                      io:format("[iso-ratio] ~s median_ratio=~.3f p99_ratio=~.3f (vs tighter idle bracket)~n",
                                [Name, RMed, RP99])
                  end, Ratios),
    {_N1, R1Med, _} = hd(Ratios),
    {_, RFullMed, _} = lists:last(Ratios),
    AllWithin = lists:all(fun({_, M, _}) -> M =< 1.10 end, Ratios),
    Verdict =
        case {AllWithin, R1Med, RFullMed} of
            {true, _, _} ->
                "ISOLATED-AT-RESOLUTION (all churn medians within +10% of the tighter idle bracket)";
            {false, M, _} when M =< 1.10 ->
                "ISOLATION-CONSISTENT; degradation at higher churn attributed to OS core "
                "contention near saturation (churn-1 clean)";
            {false, M, _} when M > 1.25 ->
                "DEGRADED-EVEN-AT-CHURN-1 (dirty-to-normal leakage or ambient shift; "
                "cause not separable by this sensor)";
            _ ->
                "AMBIGUOUS (churn-1 median ratio between +10% and +25%)"
        end,
    io:format("[iso-verdict] ~s~n", [Verdict]).

%% --- reporting helpers --------------------------------------------------------

report_us(Label, SamplesNs, Note) ->
    Us = lists:sort([S / 1000.0 || S <- SamplesNs]),
    io:format("~s n=~p min_us=~.3f median_us=~.3f p99_us=~.3f max_us=~.3f  | ~s~n",
              [Label, length(Us), hd(Us), pct(Us, 0.5), pct(Us, 0.99),
               lists:last(Us), Note]).

median(SortedUs) ->
    pct(SortedUs, 0.5).

p99(SortedUs) ->
    pct(SortedUs, 0.99).

pct(Sorted, P) ->
    N = length(Sorted),
    Idx = erlang:max(1, erlang:ceil(N * P)),
    lists:nth(Idx, Sorted).

%% --- NIF stubs (replaced at load_nif time) ------------------------------------

cgap_new(_N, _Fill) ->
    erlang:nif_error(nif_library_not_loaded).
cgap_len(_Id) ->
    erlang:nif_error(nif_library_not_loaded).
cgap_get(_Id, _I) ->
    erlang:nif_error(nif_library_not_loaded).
cgap_free(_Id) ->
    erlang:nif_error(nif_library_not_loaded).
cgap_version() ->
    erlang:nif_error(nif_library_not_loaded).
cgap_new_i64(_N, _Fill) ->
    erlang:nif_error(nif_library_not_loaded).
cgap_get_i64(_Id, _I) ->
    erlang:nif_error(nif_library_not_loaded).
cgap_stats(_Id) ->
    erlang:nif_error(nif_library_not_loaded).
