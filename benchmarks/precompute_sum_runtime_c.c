// Precompute Sum — Runtime-variable bound (BOUND env var)
// C reference for precompute_sum_runtime.bv.
// Loop bound unknown at compile time — must emit actual while-loop.
//
// Build:
//   clang -O3 -march=native -o benchmarks/precompute_sum_runtime_c \
//       benchmarks/precompute_sum_runtime_c.c

#include <stdlib.h>

int main(void) {
    const char* env = getenv("BOUND");
    long bound = env ? atol(env) : 500L;
    volatile long count = 0;
    volatile long acc_a = 0;
    volatile long acc_b = 0;

    for (; count < bound;) {
        acc_a += count;
        count++;
        if (count >= bound) break;
        acc_b += count;
        count++;
    }

    return 0;
}
