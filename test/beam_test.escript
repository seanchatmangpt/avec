#!/usr/bin/env escript
%% cgap-beam NIF membrane load test (lane BEAM).
%%
%% The module MUST be named cgap_beam: erlang:load_nif/2 requires the calling
%% module's name to equal the ErlNifEntry.name. Loading happens in -on_load,
%% the canonical load_nif context.

-module(cgap_beam).
%% load_nif requires every NIF the library exports to have an EXPORTED
%% Erlang function in the loading module (local-only stubs fail with
%% bad_lib "Function not found" — witnessed on OTP 28.3.1, lane BEAM-L1).
-export([main/1, cgap_new/2, cgap_len/1, cgap_get/2, cgap_free/1,
         cgap_version/0, cgap_new_i64/2, cgap_get_i64/2, cgap_stats/1,
         cgap_envelope_new/1, cgap_envelope_len/1, cgap_new_fast/2,
         cgap_batch_binary/1, cgap_binary_data_pointer/1, cgap_dtor_stats/0,
         cgap_alloc_stats/0, cgap_flush_to_parquet/1, cgap_seal_to_packfile/1,
         cgap_receipt_query/1, cgap_open/1, cgap_scan/3, pointer_of/1]).
-on_load(init/0).

init() ->
    %% OTP's load_nif always appends ".so" (never tries .dylib), so expose the
    %% cargo cdylib under the name the BEAM looks for. Symlink is refreshed on
    %% every run so it always targets the current build. Build profile comes
    %% from CGAP_BEAM_PROFILE ("release") and defaults to debug. os:getenv/1
    %% returns {ok, Val} on OTP <= 27 and the bare Val on OTP 28+; both
    %% shapes are accepted.
    {Dylib, NifBase} =
        case os:getenv("CGAP_BEAM_PROFILE") of
            Rel when Rel =:= {ok, "release"}; Rel =:= "release" ->
                {"/Users/sac/cgap/target/release/libcgap_beam.dylib",
                 "/Users/sac/cgap/target/release/libcgap_beam"};
            _ ->
                {"/Users/sac/cgap/target/debug/libcgap_beam.dylib",
                 "/Users/sac/cgap/target/debug/libcgap_beam"}
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
    Id = cgap_new(1000, 5),
    io:format("cgap_new(1000, 5) => ~p~n", [Id]),
    true = is_integer(Id),

    Len = cgap_len(Id),
    io:format("cgap_len => ~p~n", [Len]),
    1000 = Len,

    G0 = cgap_get(Id, 0),
    io:format("cgap_get(0) => ~p~n", [G0]),
    5 = G0,

    G999 = cgap_get(Id, 999),
    io:format("cgap_get(999) => ~p~n", [G999]),
    1004 = G999,

    Free = cgap_free(Id),
    io:format("cgap_free => ~p~n", [Free]),
    ok = Free,

    Free2 = cgap_free(Id),
    io:format("cgap_free (second) => ~p~n", [Free2]),
    error = Free2,

    Ver = cgap_version(),
    io:format("cgap_version => ~p~n", [Ver]),
    %% Version law: the membrane must report the workspace release version.
    <<"26.10.29">> = Ver,

    %% --- typed-error probes (beyond contract) ------------------------------
    expect(cgap_len(Id), error, "cgap_len(freed id)"),
    expect(cgap_get(Id, 0), error, "cgap_get(freed id, 0)"),
    expect(cgap_get(999_999, 0), error, "cgap_get(unknown id, 0)"),
    expect(cgap_get(Id2 = cgap_new(3, -1), 3), error, "cgap_get(id, out of range)"),
    expect(cgap_get(Id2, 2), 1, "cgap_get(id2, 2) of ramp 3@-1"),
    ok = cgap_free(Id2),

    case catch cgap_get(not_an_integer, 0) of
        {'EXIT', {badarg, _}} ->
            io:format("ok   cgap_get(atom, 0) raised badarg~n");
        Other ->
            io:format("FAIL cgap_get(atom, 0): got ~p, want badarg exit~n", [Other]),
            erlang:halt(1)
    end,

    %% --- large-n dirty path -------------------------------------------------
    IdBig = cgap_new(20000, 7),
    expect(cgap_len(IdBig), 20000, "cgap_len(dirty 20000)"),
    expect(cgap_get(IdBig, 16383), 16390, "cgap_get(dirty, 16383)"),
    ok = cgap_free(IdBig),

    %% --- i64 path -----------------------------------------------------------
    Id64 = cgap_new_i64(3, 4000000000),
    io:format("cgap_new_i64(3, 4000000000) => ~p~n", [Id64]),
    true = is_integer(Id64),
    expect(cgap_len(Id64), 3, "cgap_len(i64 3 rows)"),
    expect(cgap_get_i64(Id64, 0), 4000000000, "cgap_get_i64(0) exact"),
    expect(cgap_get_i64(Id64, 1), 4000000001, "cgap_get_i64(1) exact"),
    expect(cgap_get_i64(Id64, 2), 4000000002, "cgap_get_i64(2) exact"),
    expect(cgap_get(Id64, 0), error, "cgap_get on i64 batch is error"),
    expect(cgap_stats(Id64), {3, 64}, "cgap_stats(i64)"),
    ok = cgap_free(Id64),
    expect(cgap_free(Id64), error, "cgap_free(i64 id, second)"),
    expect(cgap_get_i64(Id64, 0), error, "cgap_get_i64(freed i64 id)"),

    %% i64 beyond-i32 reach, negative direction
    IdNeg = cgap_new_i64(2, -4000000000),
    expect(cgap_get_i64(IdNeg, 1), -3999999999, "cgap_get_i64(negative fill, 1)"),
    expect(cgap_stats(IdNeg), {2, 64}, "cgap_stats(negative i64)"),
    ok = cgap_free(IdNeg),

    %% ramp must not wrap: fill + (n-1) overflows i64 -> refused
    expect(cgap_new_i64(2, 9223372036854775807), error,
           "cgap_new_i64(overflow ramp) refused"),

    %% --- mixed workload: i32 and i64 alive simultaneously --------------------
    IdA = cgap_new(2, 10),
    IdB = cgap_new_i64(2, 4000000000),
    true = is_integer(IdA) andalso is_integer(IdB),
    expect(IdA =/= IdB, true, "mixed ids independent"),
    expect(cgap_len(IdA), 2, "cgap_len(mixed i32)"),
    expect(cgap_len(IdB), 2, "cgap_len(mixed i64)"),
    expect(cgap_get(IdA, 1), 11, "cgap_get(mixed i32, 1)"),
    expect(cgap_get_i64(IdB, 1), 4000000001, "cgap_get_i64(mixed i64, 1)"),
    expect(cgap_get_i64(IdA, 0), error, "cgap_get_i64 on i32 batch is error"),
    expect(cgap_stats(IdA), {2, 32}, "cgap_stats(mixed i32)"),
    expect(cgap_stats(IdB), {2, 64}, "cgap_stats(mixed i64)"),
    ok = cgap_free(IdA),
    ok = cgap_free(IdB),
    expect(cgap_len(IdA), error, "cgap_len(mixed i32 freed)"),
    expect(cgap_len(IdB), error, "cgap_len(mixed i64 freed)"),

    io:format("ALL BEAM ASSERTS PASSED~n").

%% --- NIF stubs (replaced at load_nif time) ---------------------------------

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

%% Lane BEAM-L1 (resource envelope): stubs for the two new exports —
%% load_nif requires a stub for every NIF the library exports. No asserts
%% touched.
cgap_envelope_new(_Id) ->
    erlang:nif_error(nif_library_not_loaded).
cgap_envelope_len(_Env) ->
    erlang:nif_error(nif_library_not_loaded).

%% Lane BEAM-L4 (scheduling-regime split): stub for the new export —
%% load_nif requires a stub for every NIF the library exports. No asserts
%% touched.
cgap_new_fast(_N, _Fill) ->
    erlang:nif_error(nif_library_not_loaded).

%% Cross-lane stub (resbin lane's export): load_nif requires a stub for
%% every NIF the library exports. No asserts touched.
cgap_batch_binary(_Id) ->
    erlang:nif_error(nif_library_not_loaded).

%% Rename-repair (2026-11-01): load_nif requires a stub for EVERY registered
%% NIF; the pre-existing stub list covered only 12 of 21 (witnessed: bad_lib
%% "Function not found cgap_beam:pointer_of/1"). No asserts touched.
cgap_binary_data_pointer(_Id) ->
    erlang:nif_error(nif_library_not_loaded).
cgap_dtor_stats() ->
    erlang:nif_error(nif_library_not_loaded).
cgap_alloc_stats() ->
    erlang:nif_error(nif_library_not_loaded).
cgap_flush_to_parquet(_Id) ->
    erlang:nif_error(nif_library_not_loaded).
cgap_seal_to_packfile(_Id) ->
    erlang:nif_error(nif_library_not_loaded).
cgap_receipt_query(_Id) ->
    erlang:nif_error(nif_library_not_loaded).
cgap_open(_Path) ->
    erlang:nif_error(nif_library_not_loaded).
cgap_scan(_A, _B, _C) ->
    erlang:nif_error(nif_library_not_loaded).
pointer_of(_A) ->
    erlang:nif_error(nif_library_not_loaded).
