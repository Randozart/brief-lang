// arena_churn_c — C reference for arena_churn.bv
// Mirrors the bump-arena churn: a fixed 64KB bump buffer that grows via
// realloc when exhausted (equivalent to the arena's realloc-grow).

#include <stdlib.h>
#include <stdio.h>

int main(void) {
    const char* env = getenv("BOUND");
    long N = env ? atol(env) : 50000000L;
    long* buf = malloc(65536);
    long cap = 8192; /* slots */
    long used = 0;
    long ops = 0, sum = 0;
    for (; ops < N; ) {
        if (used + 1 > cap) {
            cap *= 2;
            buf = realloc(buf, cap * sizeof(long));
        }
        buf[used++] = ops;
        sum += buf[used - 1];
        ops++;
        if (ops % 5000000 == 0)
            fprintf(stdout, "%ld\n", sum);
    }
    free(buf);
    return 0;
}
