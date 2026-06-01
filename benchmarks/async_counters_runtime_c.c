// Async Parallel Counters — Runtime-variable bound (BOUND env var)
// Perfect C reference for async_counters_runtime.bv.
//
// Single combined for-loop (no mid-loop branch), local variables,
// no volatile, returns combined final value.
//
// Build:
//   clang -O3 -march=native -o benchmarks/async_counters_runtime_c \
//       benchmarks/async_counters_runtime_c.c

#include <stdlib.h>

int main(void) {
    const char* env = getenv("BOUND");
    long bound = env ? atol(env) : 25000000L;
    long a = 0, b = 0;
    for (; a < bound && b < bound; a++, b++) {}
    return (int)(a + b);
}
