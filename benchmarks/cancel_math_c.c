// cancel_math_c — C reference for cancel_math.bv
// Symmetric loop: accumulates acc with count, increments count.
// Guard fires when count % 5M == 0 (pre-increment), matching Brief's
// rct txn atomic pre-tick read semantics.
//
// clang -O3 -march=native -ffast-math -o benchmarks/cancel_math_c benchmarks/cancel_math_c.c

#include <stdlib.h>
#include <stdio.h>

int main(void) {
    const char* env = getenv("BOUND");
    long N = env ? atol(env) : 50000000L;
    long count = 0;
    long acc = 0;

    while (count < N) {
        acc += count;
        if (count % 5000000 == 0)
            fprintf(stderr, "%ld\n", acc);
        count++;
    }

    return (int)(acc + count);
}
