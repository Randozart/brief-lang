// Enum Dispatch Counter — C reference for Brief LLVM backend Path 4 benchmark
//
// Simple counter loop with volatile guard (prevents O3 elimination).
// Brief uses switch-dispatch entry + folded while-loop.
// C uses a plain while-loop.
//
// Build:
//   clang -O3 -march=native -o benchmarks/ring_buffer_c benchmarks/ring_buffer_c.c

#include <stdio.h>
#include <stdlib.h>

int main(void) {
    volatile long ops = 0;
    const long N = 50000000L;
    for (; ops < N; ops++) {
        // empty counter body — matches the Brief txn body
    }
    return 0;
}
