//! avec-beam: raw erl_nif membrane for BEAM/AtomVM.
//!
//! Zero external crates: the erl_nif C ABI is declared by hand in Rust,
//! transcribed from the OTP 28.3.1 header
//! `/Users/sac/.erlmcp/otp-28.3.1/lib/erlang/usr/include/erl_nif.h`
//! (`ErlNifFunc` lines 124-130, `ErlNifEntry` lines 133-156) and
//! `erl_nif_api_funcs.h` for the `enif_*` signatures. The dylib leaves every
//! `enif_*` symbol undefined; the BEAM resolves them when `erlang:load_nif/2`
//! dlopens the library (build.rs supplies the macOS `-undefined dynamic_lookup`
//! link argument).
//!
//! Integer-handle registry design: `static Mutex<Vec<Option<Batch>>>`; a
//! handle is a slot index. `avec_free/1` takes the slot to `None`, so freeing
//! a second time returns the atom `error` instead of double-dropping.

#![allow(non_camel_case_types)]

use avec_arrow::{Primitive, PrimitiveBuilder, VERSION};
use std::os::raw::{c_char, c_int, c_void};
use std::sync::{Mutex, OnceLock};

// ---------------------------------------------------------------------------
// erl_nif ABI, transcribed from erl_nif.h (OTP 28.3.1)
// ---------------------------------------------------------------------------

/// `typedef ErlNifUInt ERL_NIF_TERM;` with `ErlNifUInt` = `unsigned long`
/// (erl_nif.h:106 + erl_drv_nif.h:133) — pointer-width on this LP64 target.
type ErlNifTerm = usize;

/// `typedef signed long long ErlNifSInt64;` (erl_drv_nif.h:111)
type ErlNifSInt64 = i64;

/// Opaque `struct enif_environment_t` (erl_nif.h:121-122).
#[repr(C)]
struct ErlNifEnv {
    _data: [u8; 0],
}

const ERL_NIF_MAJOR_VERSION: c_int = 2;
/// OTP 28 header: `#define ERL_NIF_MINOR_VERSION 17`
const ERL_NIF_MINOR_VERSION: c_int = 17;
/// `#define ERL_NIF_MIN_ERTS_VERSION "erts-14.0"` (erl_nif.h:83)
const MIN_ERTS: &[u8] = b"erts-14.0\0";
/// `#define ERL_NIF_VM_VARIANT "beam.vanilla"` (erl_nif.h:105)
const VM_VARIANT: &[u8] = b"beam.vanilla\0";

/// `typedef struct enif_func_t { const char* name; unsigned arity;
/// ERL_NIF_TERM (*fptr)(ErlNifEnv* env, int argc, const ERL_NIF_TERM argv[]);
/// unsigned flags; } ErlNifFunc;` — erl_nif.h:124-130, layout identical.
#[repr(C)]
struct ErlNifFunc {
    name: *const c_char,
    arity: u32,
    fptr: extern "C" fn(*mut ErlNifEnv, c_int, *const ErlNifTerm) -> ErlNifTerm,
    flags: u32,
}

/// `typedef struct enif_entry_t {...} ErlNifEntry;` — erl_nif.h:133-156,
/// field order and types identical (incl. 2.12 `sizeof_ErlNifResourceTypeInit`
/// and 2.14 `min_erts`).
///
/// SAFETY (Sync impl): the entry and func tables are read-only after init;
/// every pointer targets `'static` data owned by this crate.
#[repr(C)]
#[allow(non_snake_case)]
struct ErlNifEntry {
    major: c_int,
    minor: c_int,
    name: *const c_char,
    num_of_funcs: c_int,
    funcs: *mut ErlNifFunc,
    load: Option<extern "C" fn(*mut ErlNifEnv, *mut *mut c_void, ErlNifTerm) -> c_int>,
    reload: Option<extern "C" fn(*mut ErlNifEnv, *mut *mut c_void, ErlNifTerm) -> c_int>,
    upgrade: Option<extern "C" fn(
        *mut ErlNifEnv,
        *mut *mut c_void,
        *mut *mut c_void,
        ErlNifTerm,
    ) -> c_int>,
    unload: Option<extern "C" fn(*mut ErlNifEnv, *mut c_void)>,
    /// Added in 2.1
    vm_variant: *const c_char,
    /// Added in 2.7
    options: u32,
    /// Added in 2.12
    sizeof_ErlNifResourceTypeInit: usize,
    /// Added in 2.14
    min_erts: *const c_char,
}

/// `ErlNifResourceTypeInit` (erl_nif.h:219-225): declared so the entry can
/// report its true size; no resource types are used by this membrane.
#[repr(C)]
struct ErlNifResourceTypeInit {
    dtor: Option<extern "C" fn(*mut ErlNifEnv, *mut c_void)>,
    stop: Option<extern "C" fn(*mut ErlNifEnv, *mut c_void, c_int, c_int)>,
    down: Option<extern "C" fn(*mut ErlNifEnv, *mut c_void, *mut ErlNifPid, *mut ErlNifMonitor)>,
    members: c_int,
    dyncall: Option<extern "C" fn(*mut ErlNifEnv, *mut c_void, *mut c_void)>,
}

/// `typedef void ErlNifResourceDtor(ErlNifEnv*, void*);` internal PIDs and
/// monitors are opaque here (erl_nif.h:201, 208).
#[repr(C)]
struct ErlNifPid {
    pid: ErlNifTerm,
}
#[repr(C)]
struct ErlNifMonitor {
    _opaque: [u8; 0],
}

/// `ErlNifBinary` (erl_nif.h:159-168): `size`, `data`, internals `ref_bin`
/// plus two spare pointers.
#[repr(C)]
struct ErlNifBinary {
    size: usize,
    data: *mut u8,
    ref_bin: *mut c_void,
    __spare__: [*mut c_void; 2],
}

// SAFETY: see ErlNifEntry doc — func table pointers are all 'static.
unsafe impl Sync for ErlNifEntry {}
unsafe impl Sync for ErlNifFunc {}

// ---------------------------------------------------------------------------
// enif_* API: undefined at link time, resolved by the BEAM at load_nif time
// ---------------------------------------------------------------------------

extern "C" {
    // On LP64 (`SIZEOF_LONG == 8`) the header maps enif_get_int64 ->
    // enif_get_long and enif_make_int64 -> enif_make_long
    // (erl_nif_api_funcs.h:663-668). The BEAM exports the *long symbols
    // (verified: nm -gU beam.smp shows _enif_get_long/_enif_make_long, no
    // _enif_get_int64), so bind the *long names directly; ErlNifSInt64 ==
    // signed long on this ABI.
    /// `int enif_get_long(ErlNifEnv*, ERL_NIF_TERM term, long* ip);`
    fn enif_get_long(env: *mut ErlNifEnv, term: ErlNifTerm, ip: *mut ErlNifSInt64) -> c_int;
    /// `ERL_NIF_TERM enif_make_long(ErlNifEnv*, long);`
    fn enif_make_long(env: *mut ErlNifEnv, value: ErlNifSInt64) -> ErlNifTerm;
    /// `ERL_NIF_TERM enif_make_uint(ErlNifEnv*, unsigned i);`
    fn enif_make_uint(env: *mut ErlNifEnv, i: u32) -> ErlNifTerm;
    /// `ERL_NIF_TERM enif_make_atom_len(ErlNifEnv* env, const char* name, size_t len);`
    fn enif_make_atom_len(env: *mut ErlNifEnv, name: *const c_char, len: usize) -> ErlNifTerm;
    /// `int enif_alloc_binary(size_t size, ErlNifBinary* bin);`
    fn enif_alloc_binary(size: usize, bin: *mut ErlNifBinary) -> c_int;
    /// `ERL_NIF_TERM enif_make_binary(ErlNifEnv* env, ErlNifBinary* bin);`
    fn enif_make_binary(env: *mut ErlNifEnv, bin: *mut ErlNifBinary) -> ErlNifTerm;
    /// `ERL_NIF_TERM enif_make_tuple(ErlNifEnv* env, unsigned cnt, ...);`
    /// (erl_nif_api_funcs.h:59; the `enif_make_tuple2..9` forms are macros
    /// over this one, so the variadic is what the BEAM actually exports —
    /// verified `nm -gU beam.smp` shows `_enif_make_tuple`.)
    fn enif_make_tuple(env: *mut ErlNifEnv, cnt: u32, ...) -> ErlNifTerm;
    /// `ERL_NIF_TERM enif_make_badarg(ErlNifEnv* env);`
    fn enif_make_badarg(env: *mut ErlNifEnv) -> ErlNifTerm;
}

// ---------------------------------------------------------------------------
// Batch: avec-arrow primitive i32|i64 batch behind an integer handle
// ---------------------------------------------------------------------------

/// A registered `avec_arrow::Primitive` whose value buffer is a `Buf`
/// allocated 64-byte aligned (charter M-03 invariant). The variant is the
/// registry's type tag: `avec_get/2` answers i32 batches and `avec_get_i64/2`
/// answers i64 batches; a getter against the wrong variant is the atom
/// `error`, never a reinterpret of the bytes.
enum Batch {
    I32(Primitive<i32>),
    I64(Primitive<i64>),
}

impl Batch {
    /// Row count, type-agnostic.
    fn len(&self) -> usize {
        match self {
            Batch::I32(a) => a.len(),
            Batch::I64(a) => a.len(),
        }
    }

    /// Element bit width — the integer type tag `avec_stats/1` reports.
    fn bit_width(&self) -> u32 {
        match self {
            Batch::I32(_) => 32,
            Batch::I64(_) => 64,
        }
    }
}

// SAFETY: Batch owns its aligned buffer allocation outright; moving it to
// another thread moves the allocation with it, and the registry mutex is the
// sole accessor while a Batch is registered. (This toolchain's `NonNull<u8>`
// no longer implements Send/Sync unconditionally, so the impl is explicit.)
unsafe impl Send for Batch {}

/// Handle -> slot registry. A `None` slot is freed; freeing again returns
/// `error`. Mutex poisoning is folded back (no panic crosses the FFI).
fn registry() -> &'static Mutex<Vec<Option<Batch>>> {
    static REGISTRY: OnceLock<Mutex<Vec<Option<Batch>>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(Vec::new()))
}

fn lock_registry() -> std::sync::MutexGuard<'static, Vec<Option<Batch>>> {
    match registry().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

/// Row-count cap: a hostile or typo'd `n` cannot OOM-kill the VM
/// (1 << 28 rows: 1 GiB value buffer at i32, 2 GiB at i64).
const MAX_N: i64 = 1 << 28;

/// ASCII-only helper: byte length == char length for our atom names.
fn make_atom(env: *mut ErlNifEnv, name: &str) -> ErlNifTerm {
    unsafe { enif_make_atom_len(env, name.as_ptr() as *const c_char, name.len()) }
}

// ---------------------------------------------------------------------------
// NIFs
// ---------------------------------------------------------------------------

/// `avec_new(n, fill) -> Id :: integer() | error`
///
/// `n` rows with row `i` holding `fill + i` (ramp), so `avec_get(id, 0) ==
/// fill` and `avec_get(id, n-1) == fill + n - 1`. Runs on the dirty CPU
/// scheduler (see NIF_FUNCS): n >= 16384 rows means a >= 64 KiB aligned
/// allocation, which must not block a normal scheduler.
extern "C" fn avec_new(env: *mut ErlNifEnv, argc: c_int, argv: *const ErlNifTerm) -> ErlNifTerm {
    let args = unsafe { std::slice::from_raw_parts(argv, argc.max(0) as usize) };
    let (mut n, mut fill) = (0i64, 0i64);
    if argc != 2
        || unsafe { enif_get_long(env, args[0], &mut n) } == 0
        || unsafe { enif_get_long(env, args[1], &mut fill) } == 0
        || !(0..=MAX_N).contains(&n)
        || fill < i32::MIN as i64
        || n > 0 && fill + (n - 1) > i32::MAX as i64
    {
        return make_atom(env, "error");
    }
    let mut builder = PrimitiveBuilder::<i32>::new();
    let mut i: i64 = 0;
    while i < n {
        builder.push(Some((fill + i) as i32));
        i += 1;
    }
    let arr = builder.finish();
    debug_assert!(arr.buffers_aligned_64(), "avec-arrow violated M-03 alignment");
    let mut reg = lock_registry();
    let id = reg.len() as i64;
    reg.push(Some(Batch::I32(arr)));
    unsafe { enif_make_long(env, id) }
}

/// `avec_new_i64(n, fill) -> Id :: integer() | error`
///
/// i64 twin of `avec_new/2`: row `i` holds `fill + i`, and `fill` spans the
/// full i64 range. The ramp must not wrap, so `fill + (n - 1)` is
/// overflow-checked here — the i32 gate needs no such check because its
/// `fill` is already clamped to the i32 range. Same dirty-CPU class as
/// `avec_new/2` (see NIF_FUNCS).
extern "C" fn avec_new_i64(env: *mut ErlNifEnv, argc: c_int, argv: *const ErlNifTerm) -> ErlNifTerm {
    let args = unsafe { std::slice::from_raw_parts(argv, argc.max(0) as usize) };
    let (mut n, mut fill) = (0i64, 0i64);
    if argc != 2
        || unsafe { enif_get_long(env, args[0], &mut n) } == 0
        || unsafe { enif_get_long(env, args[1], &mut fill) } == 0
        || !(0..=MAX_N).contains(&n)
        || (n > 0 && fill.checked_add(n - 1).is_none())
    {
        return make_atom(env, "error");
    }
    let mut builder = PrimitiveBuilder::<i64>::new();
    let mut i: i64 = 0;
    while i < n {
        builder.push(Some(fill + i));
        i += 1;
    }
    let arr = builder.finish();
    debug_assert!(arr.buffers_aligned_64(), "avec-arrow violated M-03 alignment");
    let mut reg = lock_registry();
    let id = reg.len() as i64;
    reg.push(Some(Batch::I64(arr)));
    unsafe { enif_make_long(env, id) }
}

/// `avec_len(id) -> Len :: non_neg_integer() | error`
extern "C" fn avec_len(env: *mut ErlNifEnv, argc: c_int, argv: *const ErlNifTerm) -> ErlNifTerm {
    let args = unsafe { std::slice::from_raw_parts(argv, argc.max(0) as usize) };
    let mut id = 0i64;
    if argc != 1 || unsafe { enif_get_long(env, args[0], &mut id) } == 0 || id < 0 {
        return make_atom(env, "error");
    }
    let reg = lock_registry();
    match reg.get(id as usize) {
        Some(Some(batch)) => unsafe { enif_make_uint(env, batch.len() as u32) },
        _ => make_atom(env, "error"),
    }
}

/// `avec_stats(id) -> {Len :: non_neg_integer(), BitWidth :: 32 | 64} | error`
///
/// Length plus element bit width as two integers in a 2-tuple: the API stays
/// integer-based and the type tag is the element width, not an atom.
extern "C" fn avec_stats(env: *mut ErlNifEnv, argc: c_int, argv: *const ErlNifTerm) -> ErlNifTerm {
    let args = unsafe { std::slice::from_raw_parts(argv, argc.max(0) as usize) };
    let mut id = 0i64;
    if argc != 1 || unsafe { enif_get_long(env, args[0], &mut id) } == 0 || id < 0 {
        return make_atom(env, "error");
    }
    let reg = lock_registry();
    match reg.get(id as usize) {
        Some(Some(batch)) => unsafe {
            enif_make_tuple(
                env,
                2,
                enif_make_uint(env, batch.len() as u32),
                enif_make_long(env, i64::from(batch.bit_width())),
            )
        },
        _ => make_atom(env, "error"),
    }
}

/// `avec_get(id, i) -> Value :: integer() | error`
///
/// Non-integer arguments are `badarg`; unknown handle, out-of-range index,
/// or an i64-registered batch is the atom `error` (type-tagged dispatch:
/// use `avec_get_i64/2` for those).
extern "C" fn avec_get(env: *mut ErlNifEnv, argc: c_int, argv: *const ErlNifTerm) -> ErlNifTerm {
    let args = unsafe { std::slice::from_raw_parts(argv, argc.max(0) as usize) };
    let (mut id, mut idx) = (0i64, 0i64);
    if argc != 2
        || unsafe { enif_get_long(env, args[0], &mut id) } == 0
        || unsafe { enif_get_long(env, args[1], &mut idx) } == 0
    {
        return unsafe { enif_make_badarg(env) };
    }
    let reg = lock_registry();
    match reg.get(id as usize) {
        Some(Some(Batch::I32(batch))) if idx >= 0 && (idx as usize) < batch.len() => unsafe {
            enif_make_long(env, batch.values()[idx as usize] as i64)
        },
        _ => make_atom(env, "error"),
    }
}

/// `avec_get_i64(id, i) -> Value :: integer() | error`
///
/// i64 twin of `avec_get/2`, same error split (badarg for non-integer
/// arguments; `error` for unknown handle, out-of-range index, or an
/// i32-registered batch).
extern "C" fn avec_get_i64(env: *mut ErlNifEnv, argc: c_int, argv: *const ErlNifTerm) -> ErlNifTerm {
    let args = unsafe { std::slice::from_raw_parts(argv, argc.max(0) as usize) };
    let (mut id, mut idx) = (0i64, 0i64);
    if argc != 2
        || unsafe { enif_get_long(env, args[0], &mut id) } == 0
        || unsafe { enif_get_long(env, args[1], &mut idx) } == 0
    {
        return unsafe { enif_make_badarg(env) };
    }
    let reg = lock_registry();
    match reg.get(id as usize) {
        Some(Some(Batch::I64(batch))) if idx >= 0 && (idx as usize) < batch.len() => unsafe {
            enif_make_long(env, batch.values()[idx as usize])
        },
        _ => make_atom(env, "error"),
    }
}

/// `avec_free(id) -> ok | error` — second free of the same handle: `error`.
extern "C" fn avec_free(env: *mut ErlNifEnv, argc: c_int, argv: *const ErlNifTerm) -> ErlNifTerm {
    let args = unsafe { std::slice::from_raw_parts(argv, argc.max(0) as usize) };
    let mut id = 0i64;
    if argc != 1 || unsafe { enif_get_long(env, args[0], &mut id) } == 0 || id < 0 {
        return make_atom(env, "error");
    }
    let mut reg = lock_registry();
    match reg.get_mut(id as usize) {
        Some(slot @ Some(_)) => {
            *slot = None;
            make_atom(env, "ok")
        }
        _ => make_atom(env, "error"),
    }
}

/// `avec_version() -> Version :: binary()` — the avec-arrow version string.
extern "C" fn avec_version(env: *mut ErlNifEnv, argc: c_int, argv: *const ErlNifTerm) -> ErlNifTerm {
    let _ = argv;
    if argc != 0 {
        return make_atom(env, "error");
    }
    let bytes = VERSION.as_bytes();
    let mut bin = ErlNifBinary {
        size: 0,
        data: std::ptr::null_mut(),
        ref_bin: std::ptr::null_mut(),
        __spare__: [std::ptr::null_mut(); 2],
    };
    if unsafe { enif_alloc_binary(bytes.len(), &mut bin) } == 0 {
        return make_atom(env, "error");
    }
    // SAFETY: enif_alloc_binary handed us `bytes.len()` writable bytes.
    unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), bin.data, bytes.len()) };
    unsafe { enif_make_binary(env, &mut bin) }
}

// ---------------------------------------------------------------------------
// Entry table
// ---------------------------------------------------------------------------

/// `ERL_NIF_DIRTY_JOB_CPU_BOUND` (erl_drv_nif.h:50: `= 1`), i.e. 0x01.
const ERL_NIF_DIRTY_JOB_CPU_BOUND: u32 = 0x01;

static NIF_FUNCS: [ErlNifFunc; 8] = [
    // n >= 16384 fills build a >= 64 KiB aligned buffer; run it dirty.
    ErlNifFunc {
        name: b"avec_new\0".as_ptr() as *const c_char,
        arity: 2,
        fptr: avec_new,
        flags: ERL_NIF_DIRTY_JOB_CPU_BOUND,
    },
    ErlNifFunc {
        name: b"avec_len\0".as_ptr() as *const c_char,
        arity: 1,
        fptr: avec_len,
        flags: 0,
    },
    ErlNifFunc {
        name: b"avec_get\0".as_ptr() as *const c_char,
        arity: 2,
        fptr: avec_get,
        flags: 0,
    },
    ErlNifFunc {
        name: b"avec_free\0".as_ptr() as *const c_char,
        arity: 1,
        fptr: avec_free,
        flags: 0,
    },
    ErlNifFunc {
        name: b"avec_version\0".as_ptr() as *const c_char,
        arity: 0,
        fptr: avec_version,
        flags: 0,
    },
    // avec_new_i64 builds the same aligned-buffer class; dirty like avec_new.
    ErlNifFunc {
        name: b"avec_new_i64\0".as_ptr() as *const c_char,
        arity: 2,
        fptr: avec_new_i64,
        flags: ERL_NIF_DIRTY_JOB_CPU_BOUND,
    },
    ErlNifFunc {
        name: b"avec_get_i64\0".as_ptr() as *const c_char,
        arity: 2,
        fptr: avec_get_i64,
        flags: 0,
    },
    ErlNifFunc {
        name: b"avec_stats\0".as_ptr() as *const c_char,
        arity: 1,
        fptr: avec_stats,
        flags: 0,
    },
];

/// OTP 26+ no-argument entry point. The BEAM resolves this symbol when
/// `erlang:load_nif/2` dlopens the dylib.
#[no_mangle]
#[allow(private_interfaces)]
pub extern "C" fn nif_init() -> *const ErlNifEntry {
    &NIF_ENTRY
}

static NIF_ENTRY: ErlNifEntry = ErlNifEntry {
    major: ERL_NIF_MAJOR_VERSION,
    minor: ERL_NIF_MINOR_VERSION,
    name: b"avec_beam\0".as_ptr() as *const c_char,
    num_of_funcs: 8,
    funcs: NIF_FUNCS.as_ptr() as *mut ErlNifFunc,
    load: None,
    reload: None,
    upgrade: None,
    unload: None,
    vm_variant: VM_VARIANT.as_ptr() as *const c_char,
    options: 0,
    sizeof_ErlNifResourceTypeInit: std::mem::size_of::<ErlNifResourceTypeInit>(),
    min_erts: MIN_ERTS.as_ptr() as *const c_char,
};
