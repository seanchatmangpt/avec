#!/usr/bin/env escript
%% avec-beam NIF membrane load test (lane BEAM).
%%
%% The module MUST be named avec_beam: erlang:load_nif/2 requires the calling
%% module's name to equal the ErlNifEntry.name. Loading happens in -on_load,
%% the canonical load_nif context.

-module(avec_beam).
-export([main/1]).
-on_load(init/0).

init() ->
    %% OTP's load_nif always appends ".so" (never tries .dylib), so expose the
    %% cargo cdylib under the name the BEAM looks for. Symlink is refreshed on
    %% every run so it always targets the current build. Build profile comes
    %% from AVEC_BEAM_PROFILE ("release") and defaults to debug. os:getenv/1
    %% returns {ok, Val} on OTP <= 27 and the bare Val on OTP 28+; both
    %% shapes are accepted.
    {Dylib, NifBase} =
        case os:getenv("AVEC_BEAM_PROFILE") of
            Rel when Rel =:= {ok, "release"}; Rel =:= "release" ->
                {"/Users/sac/avec/target/release/libavec_beam.dylib",
                 "/Users/sac/avec/target/release/libavec_beam"};
            _ ->
                {"/Users/sac/avec/target/debug/libavec_beam.dylib",
                 "/Users/sac/avec/target/debug/libavec_beam"}
        end,
    So = NifBase ++ ".so",
    case file:read_link_info(So) of
        {ok, _} -> file:delete(So);
        _ -> ok
    end,
    ok = file:make_symlink(Dylib, So),
    case erlang:load_nif(NifBase, 0) of
        ok ->
            io:format("nif loaded: ~s~n", [So]),
            ok;
        {error, {Reason, Txt}} ->
            io:format("LOAD FAILED: ~p: ~ts~n", [Reason, Txt]),
            erlang:halt(1)
    end.

expect(Actual, Expected, Label) ->
    case Actual of
        Expected ->
            io:format("ok   ~s => ~p~n", [Label, Actual]);
        _ ->
            io:format("FAIL ~s: got ~p, want ~p~n", [Label, Actual, Expected]),
            erlang:halt(1)
    end.

main(_Args) ->
    %% --- contract asserts -------------------------------------------------
    Id = avec_new(1000, 5),
    io:format("avec_new(1000, 5) => ~p~n", [Id]),
    true = is_integer(Id),

    Len = avec_len(Id),
    io:format("avec_len => ~p~n", [Len]),
    1000 = Len,

    G0 = avec_get(Id, 0),
    io:format("avec_get(0) => ~p~n", [G0]),
    5 = G0,

    G999 = avec_get(Id, 999),
    io:format("avec_get(999) => ~p~n", [G999]),
    1004 = G999,

    Free = avec_free(Id),
    io:format("avec_free => ~p~n", [Free]),
    ok = Free,

    Free2 = avec_free(Id),
    io:format("avec_free (second) => ~p~n", [Free2]),
    error = Free2,

    Ver = avec_version(),
    io:format("avec_version => ~p~n", [Ver]),
    %% Version law: the membrane must report the workspace release version.
    <<"26.10.29">> = Ver,

    %% --- typed-error probes (beyond contract) ------------------------------
    expect(avec_len(Id), error, "avec_len(freed id)"),
    expect(avec_get(Id, 0), error, "avec_get(freed id, 0)"),
    expect(avec_get(999_999, 0), error, "avec_get(unknown id, 0)"),
    expect(avec_get(Id2 = avec_new(3, -1), 3), error, "avec_get(id, out of range)"),
    expect(avec_get(Id2, 2), 1, "avec_get(id2, 2) of ramp 3@-1"),
    ok = avec_free(Id2),

    case catch avec_get(not_an_integer, 0) of
        {'EXIT', {badarg, _}} ->
            io:format("ok   avec_get(atom, 0) raised badarg~n");
        Other ->
            io:format("FAIL avec_get(atom, 0): got ~p, want badarg exit~n", [Other]),
            erlang:halt(1)
    end,

    %% --- large-n dirty path -------------------------------------------------
    IdBig = avec_new(20000, 7),
    expect(avec_len(IdBig), 20000, "avec_len(dirty 20000)"),
    expect(avec_get(IdBig, 16383), 16390, "avec_get(dirty, 16383)"),
    ok = avec_free(IdBig),

    %% --- i64 path -----------------------------------------------------------
    Id64 = avec_new_i64(3, 4000000000),
    io:format("avec_new_i64(3, 4000000000) => ~p~n", [Id64]),
    true = is_integer(Id64),
    expect(avec_len(Id64), 3, "avec_len(i64 3 rows)"),
    expect(avec_get_i64(Id64, 0), 4000000000, "avec_get_i64(0) exact"),
    expect(avec_get_i64(Id64, 1), 4000000001, "avec_get_i64(1) exact"),
    expect(avec_get_i64(Id64, 2), 4000000002, "avec_get_i64(2) exact"),
    expect(avec_get(Id64, 0), error, "avec_get on i64 batch is error"),
    expect(avec_stats(Id64), {3, 64}, "avec_stats(i64)"),
    ok = avec_free(Id64),
    expect(avec_free(Id64), error, "avec_free(i64 id, second)"),
    expect(avec_get_i64(Id64, 0), error, "avec_get_i64(freed i64 id)"),

    %% i64 beyond-i32 reach, negative direction
    IdNeg = avec_new_i64(2, -4000000000),
    expect(avec_get_i64(IdNeg, 1), -3999999999, "avec_get_i64(negative fill, 1)"),
    expect(avec_stats(IdNeg), {2, 64}, "avec_stats(negative i64)"),
    ok = avec_free(IdNeg),

    %% ramp must not wrap: fill + (n-1) overflows i64 -> refused
    expect(avec_new_i64(2, 9223372036854775807), error,
           "avec_new_i64(overflow ramp) refused"),

    %% --- mixed workload: i32 and i64 alive simultaneously --------------------
    IdA = avec_new(2, 10),
    IdB = avec_new_i64(2, 4000000000),
    true = is_integer(IdA) andalso is_integer(IdB),
    expect(IdA =/= IdB, true, "mixed ids independent"),
    expect(avec_len(IdA), 2, "avec_len(mixed i32)"),
    expect(avec_len(IdB), 2, "avec_len(mixed i64)"),
    expect(avec_get(IdA, 1), 11, "avec_get(mixed i32, 1)"),
    expect(avec_get_i64(IdB, 1), 4000000001, "avec_get_i64(mixed i64, 1)"),
    expect(avec_get_i64(IdA, 0), error, "avec_get_i64 on i32 batch is error"),
    expect(avec_stats(IdA), {2, 32}, "avec_stats(mixed i32)"),
    expect(avec_stats(IdB), {2, 64}, "avec_stats(mixed i64)"),
    ok = avec_free(IdA),
    ok = avec_free(IdB),
    expect(avec_len(IdA), error, "avec_len(mixed i32 freed)"),
    expect(avec_len(IdB), error, "avec_len(mixed i64 freed)"),

    io:format("ALL BEAM ASSERTS PASSED~n").

%% --- NIF stubs (replaced at load_nif time) ---------------------------------

avec_new(_N, _Fill) ->
    erlang:nif_error(nif_library_not_loaded).
avec_len(_Id) ->
    erlang:nif_error(nif_library_not_loaded).
avec_get(_Id, _I) ->
    erlang:nif_error(nif_library_not_loaded).
avec_free(_Id) ->
    erlang:nif_error(nif_library_not_loaded).
avec_version() ->
    erlang:nif_error(nif_library_not_loaded).
avec_new_i64(_N, _Fill) ->
    erlang:nif_error(nif_library_not_loaded).
avec_get_i64(_Id, _I) ->
    erlang:nif_error(nif_library_not_loaded).
avec_stats(_Id) ->
    erlang:nif_error(nif_library_not_loaded).
