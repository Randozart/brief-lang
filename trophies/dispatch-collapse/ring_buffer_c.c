// Ring Buffer — C reference for Brief LLVM backend Path 4 benchmark
//
// Counts iterations and prints every 5M. Symmetric with Brief.
//
// Build:
//   clang -O3 -march=native -o benchmarks/ring_buffer_c benchmarks/ring_buffer_c.c

#include <stdio.h>
#include <stdlib.h>

int main(void) {
    long ops = 0;
    long N = 50000000L;
    char *env = getenv("BOUND");
    if (env) N = atol(env);

    while (ops < N) {
        ops++;
        if (ops % 5000000 == 0) {
            printf("%ld\n", ops);
        }
    }
    return 0;
}
