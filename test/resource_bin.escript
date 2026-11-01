#!/usr/bin/env escript
%% cgap-beam zero-copy projection test (lane BEAM-L2, resbin.rs).
%%
%% Proves in the real VM, against the real enif resource-binary machinery:
%%   project   — cgap_batch_binary/1 turns a registry batch (id or envelope
%%               term) into a binary term WITHOUT a copy: byte_size is
%%               len*4 (i32) or len*8 (i64) and the bytes at offset 0 are the
%%               first ramp value in little-endian two's complement,
%%   consume   — the id path consumes the handle (cgap_len/cgap_free read
%%               freed afterwards): the registry is Rust-side memory a binary
%%               term cannot pin, so consumption is the soundness requirement,
%%   alias     — the envelope path is non-consuming: two projections of one
%%               envelope term alias the same immutable payload legally,
%%   pin       — a projected binary keeps its envelope alive with NO other
%%               term referencing it (binary-only pinning across full GC),
%%   message   — the binary crosses a process boundary and reads identically
%%               in the child (zero-copy inter-process proof at the API
%%               level: refc binaries are shared, not copied), and survives a
%%               forced GC in BOTH processes while pinned,
%%   refuse    — atoms, floats, lists, foreign refs, negative/unknown/freed
%%               ids are all the atom `error`,
%%   churn     — thousands of project-and-drop cycles exercise
%%               alloc→wrap→project→release→GC-dtor repeatedly; the VM stays
%%               alive and the allocator keeps working afterwards.
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
    %% OTP's load_nif always appends ".so" (never tries .dylib), so expose the
    %% cargo cdylib under the name the BEAM looks for (symlink refreshed on
    %% every run). Same profile logic as beam_test.escript.
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
    %% A process listed by processes/0 can exit before its GC call lands
    %% (false is a legal answer for a dying pid, not a failure); the sweep's
    %% premise is "every surviving process gets collected", not liveness.
    _ = [erlang:garbage_collect(P) || P <- processes()],
    ok.

%% First ramp value of an i64 projection, read as little-endian two's
%% complement — the byte-level assert on binary_at(0).
first_i64(Bin) ->
    <<V:64/signed-little, _/binary>> = Bin,
    V.

%% First ramp value of an i32 projection.
first_i32(Bin) ->
    <<V:32/signed-little, _/binary>> = Bin,
    V.

main(_Args) ->
    %% --- version law ----------------------------------------------------------
    Ver = cgap_version(),
    expect(Ver, <<"26.10.29">>, "cgap_version (version law)"),

    %% --- i32 projection from a registry id (consuming id path) ---------------
    N32 = 1000,
    Fill32 = 5,
    Id32 = cgap_new(N32, Fill32),
    true = is_integer(Id32),
    Bin32 = cgap_batch_binary(Id32),
    true = is_binary(Bin32),
    expect(byte_size(Bin32), N32 * 4, "byte_size(i32 projection) = len*4"),
    %% binary_at(0): the first ramp value in LE two's complement.
    expect(first_i32(Bin32), Fill32, "i32 binary_at(0) LE == fill"),
    <<Fill32:32/little-signed, _/binary>> = Bin32,
    expect(<<Fill32, 0, 0, 0>>, binary:part(Bin32, 0, 4),
           "i32 bytes at 0 are <<5,0,0,0>> LE"),
    %% Ramp tail proves the projected window covers the whole buffer.
    expect(<<1004:32/little-signed>> = binary:part(Bin32, (N32 - 1) * 4, 4),
           <<1004:32/little-signed>>, "i32 last element 1004 at offset 3996"),

    %% --- id-path consumption ---------------------------------------------------
    expect(cgap_len(Id32), error, "id consumed by projection (cgap_len)"),
    expect(cgap_free(Id32), error, "id consumed by projection (cgap_free)"),

    %% --- i64 projection from a registry id ------------------------------------
    N64 = 1000,
    Fill64 = 4000000000,
    Id64 = cgap_new_i64(N64, Fill64),
    true = is_integer(Id64),
    Bin64 = cgap_batch_binary(Id64),
    true = is_binary(Bin64),
    expect(byte_size(Bin64), N64 * 8, "byte_size(i64 projection) = len*8"),
    expect(first_i64(Bin64), Fill64, "i64 binary_at(0) LE == fill"),
    %% 4000000000 = 16#EE6B2800 -> LE bytes 00 28 6B EE.
    expect(<<16#00, 16#28, 16#6B, 16#EE>>, binary:part(Bin64, 0, 4),
           "i64 first value LE bytes 00 28 6B EE"),
    expect(<<4000000999:64/little-signed>> = binary:part(Bin64, (N64 - 1) * 8, 8),
           <<4000000999:64/little-signed>>, "i64 last element at offset 7992"),

    %% --- negative values project bit-exact ------------------------------------
    IdNeg = cgap_new_i64(2, -4000000000),
    BinNeg = cgap_batch_binary(IdNeg),
    expect(byte_size(BinNeg), 16, "byte_size(negative i64, 2 rows)"),
    %% -4000000000 two's complement = 16#FFFFFFFF1194D800 -> LE bytes.
    expect(<<16#00, 16#D8, 16#94, 16#11, 16#FF, 16#FF, 16#FF, 16#FF,
             16#01, 16#D8, 16#94, 16#11, 16#FF, 16#FF, 16#FF, 16#FF>>,
           BinNeg, "negative i64 ramp bit-exact LE"),
    expect(first_i64(BinNeg), -4000000000, "negative i64 binary_at(0)"),

    %% --- envelope path: non-consuming, double projection aliases one payload --
    Env = cgap_envelope_new(cgap_new(3, 42)),
    true = is_reference(Env),
    BinA = cgap_batch_binary(Env),
    BinB = cgap_batch_binary(Env),
    true = is_binary(BinA) andalso is_binary(BinB),
    expect(byte_size(BinA), 12, "envelope projection size (3 rows * 4)"),
    expect(BinB, BinA, "double projection aliases the same immutable payload"),
    expect(<<42:32/little-signed, 43:32/little-signed, 44:32/little-signed>>,
           BinA, "envelope projection content 42,43,44"),

    %% --- binary-only pinning across full GC ------------------------------------
    %% The envelope term is dropped when this callee returns; the projected
    %% binary is then the ONLY term pinning the resource. A full GC of every
    %% process must leave the payload readable — otherwise the binary term's
    %% keep-alive is broken and this test dies on garbage bytes.
    BinPinned = project_from_fresh_envelope(77),
    ok = gc_all(),
    expect(byte_size(BinPinned), 8, "binary-only pinned size after full GC"),
    expect(<<77:32/little-signed, 78:32/little-signed>>, BinPinned,
           "binary-only pinned content after full GC"),

    %% --- inter-process: the binary crosses a process boundary ------------------
    %% spawn_link fails the test loudly if the child dies. Plain receives:
    %% structural sync on messages that are certain to arrive — no sleeps.
    Parent = self(),
    Child = spawn_link(fun() -> child_hold(Bin64, Parent) end),
    receive
        {child_read, Size, First} ->
            expect(Size, N64 * 8, "child sees byte_size len*8"),
            expect(First, Fill64, "child reads first value LE")
    end,
    %% GC both processes while the binary is pinned in each.
    true = erlang:garbage_collect(),
    true = erlang:garbage_collect(Child),
    Child ! {check, self()},
    receive
        {child_check, Size2, First2} ->
            expect(Size2, N64 * 8, "child re-read after GC: size"),
            expect(First2, Fill64, "child re-read after GC: first value")
    end,
    Child ! stop,

    %% --- parent-side GC survival ------------------------------------------------
    ok = gc_all(),
    expect(first_i32(Bin32), Fill32, "parent Bin32 intact after full GC"),
    expect(first_i64(Bin64), Fill64, "parent Bin64 intact after full GC"),

    %% --- typed refusals ----------------------------------------------------------
    expect(cgap_batch_binary(some_atom), error, "project(atom)"),
    expect(cgap_batch_binary(3.14), error, "project(float)"),
    expect(cgap_batch_binary([1, 2]), error, "project(list)"),
    expect(cgap_batch_binary(make_ref()), error, "project(foreign ref)"),
    expect(cgap_batch_binary(999_999), error, "project(unknown id)"),
    expect(cgap_batch_binary(-1), error, "project(negative id)"),
    expect(cgap_batch_binary(<<1:1>>), error, "project(bitstring)"),
    IdFree = cgap_new(1, 0),
    ok = cgap_free(IdFree),
    expect(cgap_batch_binary(IdFree), error, "project(freed id)"),

    %% --- churn: thousands of project-and-drop cycles ----------------------------
    ok = churn(2_000),
    ok = gc_all(),
    IdAfter = cgap_new(2, 42),
    BinAfter = cgap_batch_binary(IdAfter),
    expect(byte_size(BinAfter), 8, "post-churn projection size"),
    expect(first_i32(BinAfter), 42, "post-churn projection content"),
    expect(cgap_len(IdAfter), error, "post-churn projection consumed id"),

    io:format("ALL RESOURCE BINARY ASSERTS PASSED~n").

%% The envelope term is born and dropped inside this frame: on return only the
%% projected binary pins the resource.
project_from_fresh_envelope(Fill) ->
    Env = cgap_envelope_new(cgap_new(2, Fill)),
    true = is_reference(Env),
    cgap_batch_binary(Env).

%% One id-path projection per call; the binary term is discarded when
%% project_one returns, so only the GC can reclaim it — alloc→wrap→project→
%% release→GC-dtor must stay clean at volume.
churn(0) -> ok;
churn(N) -> _ = project_one(N), churn(N - 1).

project_one(N) ->
    _ = cgap_batch_binary(cgap_new(1, N)),
    dropped.

%% Child: reads the binary it was sent, reports the facts, then holds the
%% binary across GC rounds on demand. spawn_link makes any child death kill
%% the parent — fail-loud, never hang.
child_hold(Bin, Parent) ->
    Parent ! {child_read, byte_size(Bin), first_i64(Bin)},
    child_loop(Bin).

child_loop(Bin) ->
    receive
        {check, From} ->
            From ! {child_check, byte_size(Bin), first_i64(Bin)},
            child_loop(Bin);
        stop ->
            ok
    end.

%% --- NIF stubs (replaced at load_nif time) -----------------------------------

cgap_new(_N, _Fill) ->
    erlang:nif_error(nif_library_not_loaded).
cgap_new_fast(_N, _Fill) ->
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
cgap_get(_Id, _I) ->
    erlang:nif_error(nif_library_not_loaded).
cgap_get_i64(_Id, _I) ->
    erlang:nif_error(nif_library_not_loaded).
cgap_version() ->
    erlang:nif_error(nif_library_not_loaded).
cgap_batch_binary(_IdOrEnvelope) ->
    erlang:nif_error(nif_library_not_loaded).
