// Async Parallel Counters — Runtime-variable bound (BOUND env var)
// Sequential dispatch (no threads) to match Brief's sequential path.
//
// Build:
//   clang -O3 -march=native -o benchmarks/async_counters_runtime_c \
//       benchmarks/async_counters_runtime_c.c

#include <stdlib.h>

int main(void) {
    const char* env = getenv("BOUND");
    long bound = env ? atol(env) : 25000000L;
    volatile long a = 0;
    volatile long b = 0;

    for (; a < bound && b < bound;) {
        a++;
        if (a >= bound || b >= bound) break;
        b++;
    }

    return 0;
}
