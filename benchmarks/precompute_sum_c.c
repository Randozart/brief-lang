// Precompute Sum — C reference for Brief LLVM backend Path 3 benchmark
//
// Computes 0..500 sum (twin accumulators). Both C and Brief produce the
// same output: 249500.
//
// Build:
//   clang -O3 -march=native -o benchmarks/precompute_c benchmarks/precompute_sum_c.c

#include <stdio.h>
#include <stdlib.h>

int main(void) {
    long count = 0;
    long total = 500;
    long acc_a = 0;
    long acc_b = 0;

    while (count < total) {
        acc_a += count;
        acc_b += count;
        count++;
    }

    printf("%ld\n", acc_a + acc_b);
    return 0;
}
