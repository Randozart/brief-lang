// Precompute Sum — C reference for Brief LLVM backend Path 3 benchmark
//
// Computes the sum of integers 0..N in a loop, equivalent to the work
// the 2-txn Brief chain would perform at runtime (but Brief eliminates
// the loop entirely via compile-time precompute).
//
// Build:
//   clang -O3 -march=native -o benchmarks/precompute_c benchmarks/precompute_sum_c.c

#include <stdio.h>
#include <stdlib.h>

int main(void) {
    // Use volatile to prevent the compiler from eliminating the loop
    // at O3. The Brief compiler does the elimination at the source level.
    volatile long count = 0;
    volatile long acc_a = 0;
    volatile long acc_b = 0;
    const long total = 500;

    for (; count < total;) {
        acc_a += count;
        count++;
        if (count >= total) break;
        acc_b += count;
        count++;
    }

    return 0;
}
