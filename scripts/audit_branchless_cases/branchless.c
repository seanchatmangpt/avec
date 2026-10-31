/* branchless.c — SCOPE-DEMO clean fixture for audit_branchless.sh.
 * Pure arithmetic, no data-dependent control flow; must produce zero banned
 * opcodes at -O2 on arm64 (compilers emit b for the return-plumbing on
 * arm64 only when control flow exists; this body is straight-line). */
long demo_branchless(long a, long b, long c) {
    long mask = -(long)(a > b);   /* comparison -> arithmetic, not branch   */
    long sel  = (b ^ c) & mask;   /* branchless select of c when a > b      */
    return (a + b) ^ sel ^ c;
}
