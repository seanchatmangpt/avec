/* clean.c — NEGATIVE-CONTROL fixture, cfg_homology.sh --mutant court.
 * Kernel-shaped: flat arithmetic, exactly 2 decision constructs, no loops.
 * NOTE: the textual construct counter reads block-comment text too, so this
 * header deliberately avoids the counted keywords.
 * At -O0 clang lowers each construct to 1 conditional branch + 2 straight-
 * line plumbing jumps, so the delta floor here is 2x2=4; the mutant slack
 * (4) admits exactly that floor and nothing beyond it. A delta above slack
 * means the counter over-reports and the court is not trustworthy. */
long homology_clean_kernel(long a, long b) {
    long r = 0;
    if (a > b) {
        r = a - b;
    }
    if (b > 0) {
        r += b;
    }
    return r;
}
