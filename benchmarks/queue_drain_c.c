// queue_drain_c — C reference for queue_drain.bv
// Symmetric loop: 50M iterations, push and pop via counter manipulations.
// Counter goes up then down, matching Brief's push/pop cycle per tick.
//
// clang -O3 -march=native -ffast-math -o benchmarks/queue_drain_c benchmarks/queue_drain_c.c

#include <stdlib.h>
#include <stdio.h>

int main(void) {
    const char* env = getenv("BOUND");
    long N = env ? atol(env) : 50000000L;
    long count = 0;

    for (; count < N; ) {
        count++;
        if (count % 5000000 == 0)
            fprintf(stdout, "%ld\n", count);
    }

    return 0;
}