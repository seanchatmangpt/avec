#!/usr/bin/env escript
%% cgap-beam resource-envelope smoke test (lane BEAM-L1).
%%
%% Proves in the real VM, against the real enif resource machinery:
%%   open  — load_nif succeeds only after enif_open_resource_type_x opened the
%%           BatchEnvelope type in the load callback (fail-closed boot),
%%   alloc — cgap_envelope_new/1 wraps a registry batch into a VM-owned
%%           resource, consuming the integer handle,
%%   read  — cgap_envelope_len/1 reads cached metadata off the resource term,
%%   dtor  — dropping thousands of resource terms and forcing GC runs the
%%           dtor (which drops the Rust-side payload); the VM stays alive and
%%           the resource allocator keeps working afterwards.
%%
%% The module MUST be named cgap_beam: erlang:load_nif/2 requires the calling
%% module's name to equal the ErlNifEntry.name.

-module(cgap_beam).
%% load_nif requires every NIF the library exports to have an EXPORTED
%% Erlang function in the loading module (local-only stubs fail with
%% bad_lib "Function not found" — witnessed on OTP 28.3.1, lane BEAM-L1).
-export([main/1, cgap_new/2, cgap_new_i64/2, cgap_len/1, cgap_stats/1,
         cgap_free/1, cgap_envelope_new/1, cgap_envelope_len/1,
         cgap_get/2, cgap_get_i64/2, cgap_version/0,
         cgap_new_fast/2, cgap_batch_binary/1]).
-on_load(init/0).

init() ->
    %% OTP's load_nif always appends ".so", so expose the cargo cdylib under
    %% the name the BEAM looks for (symlink refreshed on every run).
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

gc_all() ->
    _ = erlang:garbage_collect(),
    [erlang:garbage_collect(P) || P <- processes()],
    ok.

main(_Args) ->
    %% --- open + alloc: wrap a registry batch --------------------------------
    Id = cgap_new(4, 10),
    true = is_integer(Id),
    expect(cgap_len(Id), 4, "pre-wrap cgap_len"),
    expect(cgap_stats(Id), {4, 32}, "pre-wrap cgap_stats"),

    Env32 = cgap_envelope_new(Id),
    io:format("envelope term: ~p~n", [Env32]),
    true = is_reference(Env32),

    %% --- ownership transfer: the integer handle is consumed -----------------
    expect(cgap_len(Id), error, "handle consumed by wrap"),
    expect(cgap_free(Id), error, "cgap_free(consumed id)"),
    expect(cgap_envelope_new(Id), error, "double wrap refused"),

    %% --- read back through the envelope -------------------------------------
    expect(cgap_envelope_len(Env32), 4, "envelope len i32"),

    %% i64 class through the same envelope path
    Id64 = cgap_new_i64(3, 5000000000),
    Env64 = cgap_envelope_new(Id64),
    true = is_reference(Env64),
    expect(cgap_envelope_len(Env64), 3, "envelope len i64"),

    %% --- typed refusals ------------------------------------------------------
    expect(cgap_envelope_len(not_a_resource), error, "len(atom)"),
    expect(cgap_envelope_len(42), error, "len(integer)"),
    expect(cgap_envelope_len(make_ref()), error, "len(foreign ref)"),
    expect(cgap_envelope_len([1, 2]), error, "len(list)"),
    expect(cgap_envelope_new(not_an_integer), error, "wrap(atom)"),
    expect(cgap_envelope_new(-1), error, "wrap(negative id)"),
    expect(cgap_envelope_new(999_999), error, "wrap(unknown id)"),
    expect(cgap_envelope_new(0.5), error, "wrap(float)"),

    %% --- live-resource GC survival -------------------------------------------
    %% A term still referencing the envelope pins it across GC: the resource
    %% must stay readable after a full sweep of every process.
    ok = gc_all(),
    expect(cgap_envelope_len(Env32), 4, "len after GC (live resource survives)"),
    expect(cgap_envelope_len(Env64), 3, "len after GC (live i64 survives)"),

    %% --- dtor proof -----------------------------------------------------------
    %% 20_000 envelopes dropped in dead stack frames (the resource term never
    %% escapes churn/1's callee), then a forced full GC: 20_000 dtors fire,
    %% each dropping a Rust-side payload. A dtor bug (double drop, bad
    %% drop_in_place) aborts the VM here; the proof is the VM living on and
    %% the resource allocator still working afterwards.
    ok = churn(20_000),
    ok = gc_all(),
    IdAfter = cgap_new(2, 100),
    EnvAfter = cgap_envelope_new(IdAfter),
    expect(cgap_envelope_len(EnvAfter), 2, "post-dtor alloc+read"),
    expect(cgap_len(IdAfter), error, "post-dtor wrap consumed handle"),
    expect(cgap_envelope_len(Env32), 4, "old envelope still intact post-churn"),

    io:format("ALL RESOURCE SMOKE ASSERTS PASSED~n").

%% One envelope per call; the resource term is discarded when wrap_one
%% returns, so only the GC can reach it — its dtor must fire there.
churn(0) -> ok;
churn(N) -> _ = wrap_one(N), churn(N - 1).

wrap_one(N) ->
    _ = cgap_envelope_new(cgap_new(1, N)),
    dropped.

%% --- NIF stubs (replaced at load_nif time) ----------------------------------

cgap_new(_N, _Fill) ->
    erlang:nif_error(nif_library_not_loaded).
cgap_new_i64(_N, _Fill) ->
    erlang:nif_error(nif_library_not_loaded).
cgap_len(_Id) ->
    erlang:nif_error(nif_library_not_loaded).
cgap_stats(_Id) ->
    erlang:nif_error(nif_library_not_loaded).
cgap_free(_Id) ->
    erlang:nif_error(nif_library_not_loaded).
cgap_envelope_new(_Id) ->
    erlang:nif_error(nif_library_not_loaded).
cgap_envelope_len(_Env) ->
    erlang:nif_error(nif_library_not_loaded).

%% Pre-existing exports: load_nif requires a stub for every NIF the library
%% exports, even ones this smoke test never calls.
cgap_get(_Id, _I) ->
    erlang:nif_error(nif_library_not_loaded).
cgap_get_i64(_Id, _I) ->
    erlang:nif_error(nif_library_not_loaded).
cgap_version() ->
    erlang:nif_error(nif_library_not_loaded).

%% Lane BEAM-L2 (resbin): stub for the zero-copy projection export —
%% load_nif requires a stub for every NIF the library exports. No asserts
%% touched.
cgap_batch_binary(_IdOrEnvelope) ->
    erlang:nif_error(nif_library_not_loaded).

%% Minimal unblock fix (compile-freeze SLA, disclosed in BEAM-L2's receipt):
%% BEAM-L4's export cgap_new_fast had no stub in this file, so load_nif
%% refused the whole library (bad_lib "Function not found") and every smoke
%% assert was frozen. Stub only — BEAM-L4's asserts are untouched.
cgap_new_fast(_N, _Fill) ->
    erlang:nif_error(nif_library_not_loaded).
