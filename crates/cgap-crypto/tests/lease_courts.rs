//! Lane A16 lease courts (`docs/jira/v26.11.1/lanes/A16-chatman-pqc.md`
//! §5 mechanics, §8.1/§8.2 courts).
//!
//! Chicago discipline: a REAL lease, REAL subprocess traps, assertions on
//! final state — no mocks, no sleeps. Each court names its §8 anti-vacuity
//! mutation; the mutations are executed as shim-courts over the source
//! (baseline → shim → court MUST flip → byte-exact restore) by the
//! replayable in-repo battery `scripts/wave7_shim_battery.py`.
//!
//! Flip receipts (latest full run 2026-11-01 wave 7, exit 0, sandbox ==
//! repo after battery): M1 flips `monotonicity_court_tick_sequence_
//! strictly_increasing` + `current_tick_read_is_pure_never_advances` AND
//! NOW the exhaustion court (wave-7 repair of a16-wave6 audit D-A16-4: the
//! court previously stayed green for the WRONG reason — the probe's own
//! sanity assert died first; it now demands the trap's stderr signature),
//! M2 the exhaustion court (primary, `VACUOUS GATE` on warning-return),
//! M3d the constructor court (M3/M3b/M3c compile-aborts, recorded as such —
//! not failure counts). Restores sha256 byte-exact after every mutation.

use cgap_crypto::lease::Lease;

/// §8.1 (tick side): tick a real lease N times through the public API; the
/// observed sequence must be strictly increasing and the post-advance read
/// must agree.
///
/// Anti-vacuity mutation (shim-court executed, wave 6 / M1): `tick()` that
/// returns the current value WITHOUT advancing (a rewind-capable path) MUST
/// flip this court to failure.
#[test]
fn monotonicity_court_tick_sequence_strictly_increasing() {
    const CEILING: u64 = 64;
    let mut lease = Lease::new([7u8; 32], CEILING, [9u8; 32]);
    let mut previous: Option<u64> = None;
    for expected in 0..CEILING {
        let t = lease.tick();
        assert_eq!(t, expected, "tick() must emit the pre-advance value");
        assert_eq!(
            lease.current_tick(),
            expected + 1,
            "current_tick after advance"
        );
        if let Some(prev) = previous {
            assert!(t > prev, "tick sequence must be strictly increasing");
        }
        previous = Some(t);
    }
    assert_eq!(lease.current_tick(), CEILING);
}

/// §8.2 exhaustion court, REAL trap observation: ceiling = k, tick k times,
/// tick once more — the PROCESS must trap. Observed by re-invoking this test
/// binary as a subprocess with a probe env var (real OS process, real panic
/// exit status; profile-independent — `panic = "abort"` in dev would turn a
/// panic into SIGABRT, and `!status.success()` flips either way).
///
/// Anti-vacuity mutation (shim-court executed, wave 6 / M2; re-executed
/// wave 7): downgrading the trap to a warning return MUST make the probe
/// child exit 0 and this court FAIL. A court satisfied by a warning is
/// vacuous.
///
/// Wave-7 repair (a16-wave6 audit D-A16-4): a nonzero child exit alone does
/// not attribute the death to the §5 trap — under the M1 no-advance shim the
/// probe's own `current_tick == 8` sanity assert died first and this court
/// stayed GREEN for the WRONG reason (nonzero exit, no trap). The court now
/// additionally demands the trap's own signature on stderr, so only the
/// exhaustion trap satisfies it; under M1 it now flips (attributable
/// collateral) instead of passing for the wrong reason.
#[test]
fn exhaustion_court_trap_observed_in_real_process() {
    if std::env::var("CGAP_A16_LEASE_PROBE").as_deref() == Ok("exhaust") {
        let mut lease = Lease::new([7u8; 32], 8, [9u8; 32]);
        for _ in 0..8 {
            lease.tick();
        }
        assert_eq!(lease.current_tick(), 8);
        let _ = lease.tick(); // MUST TRAP (§5) — if it returns, exit 0 flips the court
        eprintln!("SHIM-COURT WITNESS: tick() survived exhaustion (trap missing)");
        return; // normal test exit 0 -> parent court fails
    }
    let exe = std::env::current_exe().expect("current_exe");
    let output = std::process::Command::new(exe)
        .args(["exhaustion_court_trap_observed_in_real_process", "--exact", "--nocapture"])
        .env("CGAP_A16_LEASE_PROBE", "exhaust")
        .output()
        .expect("spawn probe subprocess");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "§8.2 VACUOUS GATE: exhaustion did NOT trap the process (status \
         {status:?}); stderr: {stderr}",
        status = output.status,
    );
    assert!(
        stderr.contains("authority EXHAUSTED"),
        "§8.2 WRONG-REASON DEATH: probe exited nonzero but its stderr does \
         not carry the §5 exhaustion-trap signature (\"authority \
         EXHAUSTED\") — a non-trap panic died instead; stderr: {stderr}",
    );
}

/// §5 constructor invariant: `tick_ceiling == 0` is exhausted-at-birth — the
/// constructor TRAPS. Same subprocess observation as the exhaustion court.
///
/// Anti-vacuity mutation (shim-court executed, wave 6 / M3d; M3, M3b, M3c
/// were compile-aborts, recorded as such — not failure counts; re-executed
/// wave 7): replacing the constructor trap with a warning default-ceiling
/// MUST flip this court (probe child exits 0).
///
/// Wave-7 repair (D-A16-4 class, same session): the parent additionally
/// demands the constructor trap's own stderr signature, so a nonzero child
/// exit from any OTHER panic cannot satisfy this court either.
#[test]
fn constructor_court_zero_ceiling_traps() {
    if std::env::var("CGAP_A16_LEASE_PROBE").as_deref() == Ok("ceiling0") {
        let _ = Lease::new([1u8; 32], 0, [3u8; 32]); // MUST TRAP (§5 "ceiling > 0")
        eprintln!("SHIM-COURT WITNESS: zero-ceiling lease constructed (trap missing)");
        return;
    }
    let exe = std::env::current_exe().expect("current_exe");
    let output = std::process::Command::new(exe)
        .args(["constructor_court_zero_ceiling_traps", "--exact", "--nocapture"])
        .env("CGAP_A16_LEASE_PROBE", "ceiling0")
        .output()
        .expect("spawn probe subprocess");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "VACUOUS GATE: Lease::new accepted tick_ceiling = 0 (status {status:?})",
        status = output.status,
    );
    assert!(
        stderr.contains("tick_ceiling must be > 0"),
        "WRONG-REASON DEATH: probe exited nonzero but its stderr does not \
         carry the §5 constructor-trap signature (\"tick_ceiling must be \
         > 0\"); stderr: {stderr}",
    );
}

/// §5 rewinding is unrepresentable: the only mutating path is `tick()`
/// (advance by one, trap at exhaustion); `current_tick` is a pure read.
/// The court pins the observable surface: reading never advances.
#[test]
fn current_tick_read_is_pure_never_advances() {
    let mut lease = Lease::new([7u8; 32], 16, [9u8; 32]);
    assert_eq!(lease.current_tick(), 0, "fresh lease starts at tick 0 (§3.2)");
    for _ in 0..3 {
        let before = lease.current_tick();
        let _ = lease.current_tick();
        let _ = lease.current_tick();
        assert_eq!(lease.current_tick(), before, "reads must not advance");
    }
    let _ = lease.tick();
    assert_eq!(lease.current_tick(), 1);
}

/// §3.2/§5 accessors return the constructed state unchanged.
#[test]
fn accessors_return_constructed_state() {
    let lease_id = [0xAAu8; 32];
    let issued_root = [0x55u8; 32];
    let lease = Lease::new(lease_id, 42, issued_root);
    assert_eq!(lease.lease_id(), &lease_id);
    assert_eq!(lease.issued_root(), &issued_root);
    assert_eq!(lease.tick_ceiling(), 42);
    assert_eq!(lease.current_tick(), 0);
}

/// §3.2 codec: the 80-byte canonical encoding round-trips; byte offsets
/// match the §3.2 concatenation (`lease_id || tick || tick_ceiling ||
/// issued_root`, multi-byte fields LE).
#[test]
fn phi_lease_codec_roundtrip_and_offsets() {
    use cgap_crypto::lease::{PhiLease, PHI_LEASE_LEN};

    let phi = PhiLease {
        lease_id: [1u8; 32],
        tick: 0x0102_0304_0506_0708,
        tick_ceiling: 0x0807_0605_0403_0201,
        issued_root: [2u8; 32],
    };
    let bytes = phi.to_bytes();
    assert_eq!(bytes.len(), PHI_LEASE_LEN);
    assert_eq!(&bytes[0..32], &[1u8; 32]);
    assert_eq!(&bytes[32..40], &[0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01]); // LE
    assert_eq!(&bytes[40..48], &[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]); // LE
    assert_eq!(&bytes[48..80], &[2u8; 32]);
    assert_eq!(PhiLease::from_bytes(&bytes), phi);
}
