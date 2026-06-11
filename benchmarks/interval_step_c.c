// interval_step_c — C reference for interval_step.bv
// Symmetric loop: accumulates acc with count, increments count by
// (R1 - R2) = 1 via the interval formula. In C this is just count++
// with extra arithmetic that clang will optimize away.
//
// clang -O3 -march=native -ffast-math -o benchmarks/interval_step_c benchmarks/interval_step_c.c

#include <stdlib.h>
#include <stdio.h>

#define R1 200
#define R2 199

int main(void) {
    const char* env = getenv("BOUND");
    long N = env ? atol(env) : 50000000L;
    long count = 0;
    long acc = 0;

    for (; count < N; ) {
        acc += count;
        count = (count + R1) - R2;

        if (count % 5000000 == 0)
            fprintf(stdout, "%ld\n", acc);
    }

    return 0;
}
