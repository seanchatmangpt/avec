/* branchy.c — POSITIVE-CONTROL fixture for audit_branchless.sh.
 * Deliberately contains a data-dependent if-branch; at -O0 this must lower
 * to b.cond/b (ARM64) or jcc (x86). The gate REQUIRES detection of >=1
 * banned opcode here; zero detections = VACUOUS GATE (exit 2). */
int control_branchy(int x, int y) {
    int r = 0;
    if (x > y) {
        r = x - y;
    } else {
        r = y - x;
    }
    return r;
}
