// queue_drain_sym_c — C reference for queue_drain_sym.bv.
// Symmetric counter-only loop. No collection ops.
//
// clang -O3 -march=native -ffast-math -o benchmarks/queue_drain_sym_c benchmarks/queue_drain_sym_c.c

#include <stdlib.h>
#include <stdio.h>

int main(void) {
    const char* env = getenv("BOUND");
    long N = env ? atol(env) : 50000000L;
    long count = 0;

    for (; count < N; ) {
        count++;
        if (count % 5000000 == 0)
            fprintf(stderr, "%ld\n", count);
    }

    return 0;
}