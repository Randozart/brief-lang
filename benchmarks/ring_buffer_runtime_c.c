// Ring Buffer — Runtime-variable bound (BOUND env var)
// Perfect C reference for ring_buffer_runtime.bv.
//
// Uses local variable (optimal register allocation), no volatile,
// returns final value to make loop observable.
//
// Build:
//   clang -O3 -march=native -o benchmarks/ring_buffer_runtime_c \
//       benchmarks/ring_buffer_runtime_c.c

#include <stdlib.h>

int main(void) {
    const char* env = getenv("BOUND");
    long bound = env ? atol(env) : 50000000L;
    long ops = 0;
    for (; ops < bound; ops++) {}
    return (int)ops;
}
