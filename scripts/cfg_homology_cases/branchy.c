/* branchy.c — POSITIVE-CONTROL fixture, cfg_homology.sh --mutant court.
 * Deliberate chained conditionals plus nested iteration: 4 counted source
 * constructs lower at -O0 to 17 banned branches (each conditional carries 2
 * plumbing jumps; each iteration level adds a conditional exit AND an
 * unconditional back-edge), so the delta (13) must exceed the tight slack.
 * A delta at/below slack here means the verdict never flips => VACUOUS gate.
 * Header avoids the counted keywords on purpose (see clean.c). */
long homology_branchy_kernel(const long *a, long n) {
    long s = 0;
    for (long i = 0; i < n; ++i) {
        for (long j = 0; j < n; ++j) {
            if (a[i] > a[j]) {
                s += a[i] - a[j];
            } else {
                s -= 1;
            }
            if (s > 1000) {
                s = 1000;
            }
        }
    }
    return s;
}
