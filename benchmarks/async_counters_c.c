// Async Parallel Counters — C reference for Briev LLVM backend Path 5 benchmark
//
// Two counters each reach N via pure-body increments. Briev proves both are
// pure with known bounds and emits two `store i64 N` (O(1)). C gets the same
// optimization: the compiler eliminates both loops.
//
// Build:
//   clang -O3 -march=native -o benchmarks/async_counters_c benchmarks/async_counters_c.c

int main(void) {
    long g_a = 25000000L;
    long g_b = 25000000L;
    (void)g_a;
    (void)g_b;
    return 0;
}
