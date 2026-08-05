// Precompute Sum — Runtime-variable bound (BOUND env var)
// Perfect C reference for precompute_sum_runtime.bv.
//
// Local variables, no volatile, returns accumulated sum.
// The interleaved accumulation pattern is what Briv's two-txn
// dispatch produces; clang cannot fold this to O(1).
//
// Build:
//   clang -O3 -march=native -o benchmarks/precompute_sum_runtime_c \
//       benchmarks/precompute_sum_runtime_c.c

#include <stdlib.h>

int main(void) {
    const char* env = getenv("BOUND");
    long bound = env ? atol(env) : 500L;
    long count = 0, acc_a = 0, acc_b = 0;
    for (; count < bound;) {
        acc_a += count; count++;
        if (count >= bound) break;
        acc_b += count; count++;
    }
    return (int)(acc_a + acc_b);
}
