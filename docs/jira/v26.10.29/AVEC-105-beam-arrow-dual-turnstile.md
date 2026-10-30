# AVEC-105: Alternating Current: Atomic Dual-Ownership Turnstile for BEAM GC & Arrow C-ABI

**Component**: `crates/avec-beam`, `crates/avec-ffi`  
**Priority**: High  
**Reporter**: Antigravity Scan Pass  
**Status**: IN PROGRESS  

## Description
Erlang BEAM garbage collector sweeps resource envelopes independently of external downstream C-ABI consumers holding exported Arrow pointers. If BEAM collects the resource while a C-ABI consumer is using it, memory is freed prematurely. Conversely, if C-ABI releases memory while BEAM holds the envelope, a double free occurs.

## Turnstile State Machine
```
                      +---------------------------------------+
                      |         Turnstile State: 0b11         |
                      |   (Bit 0: BEAM Held, Bit 1: Arrow)    |
                      +---------------------------------------+
                                  /               \
       BEAM GC / dtor fires      /                 \    ArrowArray.release() invoked
      (fetch_and(!0b01))        /                   \  (fetch_and(!0b10))
                               v                     v
              +-----------------------+       +-----------------------+
              | State: 0b10 (Arrow)   |       | State: 0b01 (BEAM)    |
              | Awaiting C-ABI release|       | Awaiting BEAM GC dtor |
              +-----------------------+       +-----------------------+
                               \                     /
       ArrowArray.release()     \                   /   BEAM GC / dtor fires
      (fetch_and(!0b10) == 1)    \                 /   (fetch_and(!0b01) == 1)
                                  v               v
                      +---------------------------------------+
                      |          Turnstile State: 0b00        |
                      |     --> Exact-Once Memory Free <--    |
                      +---------------------------------------+
```

## Acceptance Criteria
- [ ] Implement `TurnstileEnvelope<T>` in `crates/avec-beam/src/resources.rs` with `AtomicU8` state.
- [ ] Wire both BEAM resource destructor and `ArrowArray.release` callback to the turnstile.
- [ ] Run `escript test/beam_test.escript` verifying sub-50 ns dispatch without leaks.
