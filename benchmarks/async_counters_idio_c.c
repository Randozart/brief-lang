// Async Parallel Counters — Idiomatic Optimizer C Reference
//
// History:
//   2026-07-01: Created as optimizer companion to async_counters_idio.bv.
//     Uses const N matching the Briv version's const bound. clang -O3
//     eliminates the dead stores (g_a, g_b never read), producing the
//     same O(1) fold as Briv's multi-txn pure fold path.
//
// Build:
//   clang -O3 -march=native -o benchmarks/async_counters_idio_c \
//     benchmarks/async_counters_idio_c.c

#define N 50000000L

int main(void) {
    long g_a = N;
    long g_b = N;
    (void)g_a;
    (void)g_b;
    return 0;
}
