#!/usr/bin/env escript
%% test/shm_ring.escript — lane BEAM-L9. BEAM-side court for the POSIX-shm
%% SPSC slot ring in crates/cgap-beam/src/shm.rs. Version law 26.10.29.
%%
%% The ring is self-contained Rust (zero crates, shm_open/mmap externs).
%% src/lib.rs is owned by another lane this wave, so there is NO NIF surface
%% here: the BEAM exercises the ring through the shm_helper binary that the
%% same shm.rs source compiles into. The escript is the orchestrator and
%% assertion layer:
%%
%%   Stage R (rustc): compile shm.rs two ways into /tmp/cgap-l9 —
%%     --test  -> shm_test   (in-process #[cfg(test)] suite, 7 tests)
%%     bin     -> shm_helper (serve/child CLI)
%%   Stage T (suite): run shm_test; require exit 0, "test result: ok",
%%     "0 failed". Collect THROUGHPUT/LATENCY evidence lines verbatim.
%%   Stage X (cross-process throughput): open_port spawn_executable on
%%     `shm_helper serve-throughput /avl9es_thr 8192`. The serve process
%%     creates + sizes + maps the ring and spawns two REAL child processes
%%     (std::process::Command) that receive the ring fd NUMBER in argv
%%     (F_DUPFD-duplicated, CLOEXEC-free — darwin marks shm_open fds CLOEXEC
%%     by default, probe7 receipt in shm.rs platform law). Producer child
%%     writes 8192 x 64 KiB = 512 MiB; consumer child verifies every byte.
%%     Require exit 0 + `SERVE THROUGHPUT ... ok=true` + n/verified match.
%%   Stage P (cross-process latency): open_port on `serve-pingpong`: two
%%     rings (req/rsp), ping and pong children exchange 2000 64-KiB round
%%     trips as two OS processes. Require exit 0 + `ok=true` + a parsed
%%     p50_us > 0.
%%
%% Every gate is a real observable: process exit status, verified byte
%% counts, ring counters drained to zero. A fixed sleep appears nowhere;
%% waits are bounded receives on port exit messages.
%%
%% Single-run class: one invocation, one machine; numbers are machine-local.
%% Concurrency noise (other lanes on this box) is part of every number by
%% construction.

-module(shm_ring).
-export([main/1]).

-define(VERSION, <<"26.10.29">>).
-define(ROOT, "/Users/sac/cgap").
-define(SHM_RS, ?ROOT ++ "/crates/cgap-beam/src/shm.rs").
-define(BUILD_DIR, "/tmp/cgap-l9").
-define(TEST_BIN, ?BUILD_DIR ++ "/shm_test").
-define(HELPER_BIN, ?BUILD_DIR ++ "/shm_helper").
-define(N_THR, 8192).  %% 8192 x 64 KiB = 512 MiB cross-process
-define(N_PING, 2000).
-define(PORT_TIMEOUT_MS, 180000).

main(_Args) ->
    io:format("== shm_ring lane BEAM-L9 v~s ==~n", [?VERSION]),
    ok = stage_rustc(),
    Suite = stage_suite(),
    Thr = stage_throughput(),
    Ping = stage_pingpong(),
    summary(Suite, Thr, Ping),
    erlang:halt(0).

%% ---------- Stage R: rustc ----------

stage_rustc() ->
    filelib:ensure_dir(filename:join(?BUILD_DIR, "x")),
    CmdTest = io_lib:format(
        "rustc --edition 2021 --test ~s -C opt-level=3 -o ~s",
        [?SHM_RS, ?TEST_BIN]),
    {0, _} = rustc(CmdTest, "test-bin"),
    CmdBin = io_lib:format(
        "rustc --edition 2021 ~s -C opt-level=3 -o ~s",
        [?SHM_RS, ?HELPER_BIN]),
    {0, _} = rustc(CmdBin, "helper-bin"),
    io:format("[R] PASS rustc test-bin + helper-bin~n"),
    ok.

rustc(Cmd, Label) ->
    case run_port("/bin/zsh", ["-c", lists:flatten(Cmd)], 120000) of
        {0, Out} ->
            {0, Out};
        {Status, Out} ->
            io:format("rustc ~s failed (exit ~p):~n~s", [Label, Status, Out]),
            erlang:halt(1)
    end.

%% ---------- Stage T: in-process suite ----------

stage_suite() ->
    io:format("[T] running shm_test suite...~n"),
    {Status, Out} =
        run_port("/bin/zsh", ["-c",
            lists:flatten(io_lib:format("~s --nocapture --test-threads=1",
                                        [?TEST_BIN]))],
            120000),
    Lines = binary_lines(Out),
    %% libtest glues a test's first println onto its "test ... " line, so
    %% match the evidence tags by substring, not by line prefix.
    Evidence = [L || L <- Lines,
                     has_sub(L, <<"THROUGHPUT ">>) orelse has_sub(L, <<"LATENCY ">>)],
    case {Status, has_sub(Out, <<"test result: ok">>), has_sub(Out, <<"0 failed">>)} of
        {0, true, true} ->
            io:format("[T] PASS suite exit=0 result=ok~n"),
            lists:foreach(fun(L) -> io:format("    ~s~n", [L]) end, Evidence),
            #{status => pass, evidence => Evidence};
        {0, _, _} ->
            io:format("[T] FAIL suite exit 0 but result lines missing:~n~s", [Out]),
            erlang:halt(1);
        {_, _, _} ->
            io:format("[T] FAIL suite exit ~p:~n~s", [Status, Out]),
            erlang:halt(1)
    end.

%% ---------- Stage X: cross-process throughput ----------

stage_throughput() ->
    Name = <<"/avl9es_thr">>,
    {Status, Out} = run_port(?HELPER_BIN,
        [<<"serve-throughput">>, Name, integer_to_binary(?N_THR)],
        ?PORT_TIMEOUT_MS),
    case {Status, serve_ok(Out, <<"SERVE THROUGHPUT">>)} of
        {0, true} ->
            Prod = kv(Out, <<"PRODUCED">>, <<"gibps">>),
            Cons = kv(Out, <<"CONSUMED">>, <<"gibps">>),
            Verified = kv(Out, <<"CONSUMED">>, <<"verified">>),
            case Verified =:= integer_to_binary(?N_THR) of
                true ->
                    io:format("[X] PASS serve-throughput exit=0 verified=~s/~p ~n",
                              [Verified, ?N_THR]),
                    #{status => pass, prod_gibps => Prod, cons_gibps => Cons};
                false ->
                    io:format("[X] FAIL verified=~s expected ~p:~n~s",
                              [Verified, ?N_THR, Out]),
                    erlang:halt(1)
            end;
        {Status, _} ->
            io:format("[X] FAIL serve-throughput exit ~p:~n~s", [Status, Out]),
            erlang:halt(1)
    end.

%% ---------- Stage P: cross-process ping-pong latency ----------

stage_pingpong() ->
    Req = <<"/avl9es_req">>,
    Rsp = <<"/avl9es_rsp">>,
    {Status, Out} = run_port(?HELPER_BIN,
        [<<"serve-pingpong">>, Req, Rsp, integer_to_binary(?N_PING)],
        ?PORT_TIMEOUT_MS),
    case {Status, serve_ok(Out, <<"SERVE PINGPONG">>)} of
        {0, true} ->
            P50 = kv(Out, <<"RTT">>, <<"p50_us">>),
            case {P50 =/= undefined, safe_float(P50)} of
                {true, {ok, V}} when V > 0.0 ->
                    io:format("[P] PASS serve-pingpong exit=0 p50_us=~s~n", [P50]),
                    #{status => pass, rtt_p50_us => P50};
                {_, _} ->
                    io:format("[P] FAIL RTT p50_us unparsable/nonpositive:~n~s", [Out]),
                    erlang:halt(1)
            end;
        {Status, _} ->
            io:format("[P] FAIL serve-pingpong exit ~p:~n~s", [Status, Out]),
            erlang:halt(1)
    end.

%% ---------- summary ----------

summary(Suite, Thr, Ping) ->
    io:format("~n== SHM RING RESULT (lane BEAM-L9, v~s) ==~n", [?VERSION]),
    io:format("suite:          PASS (~p evidence lines)~n",
              [length(maps:get(evidence, Suite))]),
    io:format("cross-proc thr: PASS prod=~s GiB/s cons=~s GiB/s (8192x64KiB, byte-verified)~n",
              [maps:get(prod_gibps, Thr), maps:get(cons_gibps, Thr)]),
    io:format("cross-proc RTT: PASS p50=~s us (n=~p, 64KiB each way)~n",
              [maps:get(rtt_p50_us, Ping), ?N_PING]),
    io:format("gate: ALL STAGES PASS~n").

%% ---------- plumbing ----------

run_port(Exe, Args, Timeout) ->
    Port = open_port({spawn_executable, Exe},
                     [{args, Args}, binary, exit_status, stderr_to_stdout, hide]),
    collect(Port, <<>>, Timeout).

collect(Port, Buf, Timeout) ->
    receive
        {Port, {data, D}} -> collect(Port, <<Buf/binary, D/binary>>, Timeout);
        {Port, {exit_status, S}} -> {S, Buf}
    after Timeout ->
        catch port_close(Port),
        {timeout, Buf}
    end.

binary_lines(Bin) ->
    [L || L <- binary:split(Bin, <<"\n">>, [global, trim_all]), L =/= <<>>].

has_prefix(Bin, P) -> binary:match(Bin, P) =:= {0, byte_size(P)}.
has_sub(Bin, S) -> binary:match(Bin, S) =/= nomatch.

serve_ok(Out, Tag) ->
    Lines = binary_lines(Out),
    case [L || L <- Lines, has_prefix(L, Tag)] of
        [] -> false;
        TagLines -> lists:all(fun(L) -> has_sub(L, <<"ok=true">>) end, TagLines)
    end.

%% Value of Key inside the first line with prefix LineTag; undefined if absent.
kv(Bin, LineTag, Key) ->
    Lines = binary_lines(Bin),
    case [L || L <- Lines, has_prefix(L, LineTag)] of
        [] -> undefined;
        [L | _] ->
            Pairs = [binary:split(P, <<"=">>) ||
                     P <- binary:split(L, <<" ">>, [global, trim_all])],
            case [V || [K, V] <- Pairs, K =:= Key] of
                [V | _] -> V;
                [] -> undefined
            end
    end.

safe_float(undefined) -> {error, undefined};
safe_float(Bin) ->
    try
        {ok, binary_to_float(Bin)}
    catch
        _:_ ->
            try
                {ok, binary_to_integer(Bin) * 1.0}
            catch
                _:_ -> {error, badint}
            end
    end.
