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
    %% every run so it always targets the current build.
    Dylib = "/Users/sac/avec/target/debug/libavec_beam.dylib",
    So = "/Users/sac/avec/target/debug/libavec_beam.so",
    case file:read_link_info(So) of
        {ok, _} -> file:delete(So);
        _ -> ok
    end,
    ok = file:make_symlink(Dylib, So),
    case erlang:load_nif("/Users/sac/avec/target/debug/libavec_beam", 0) of
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
    true = is_binary(Ver),

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
