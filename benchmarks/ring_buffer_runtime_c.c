// Ring Buffer — Runtime-variable bound (BOUND env var)
// C reference for ring_buffer_runtime.bv.
// Loop bound unknown at compile time — must emit actual while-loop.
//
// Build:
//   clang -O3 -march=native -o benchmarks/ring_buffer_runtime_c benchmarks/ring_buffer_runtime_c.c

#include <stdlib.h>

int main(void) {
    const char* env = getenv("BOUND");
    long bound = env ? atol(env) : 50000000L;
    volatile long ops = 0;
    for (; ops < bound; ops++) {}
    return 0;
}
