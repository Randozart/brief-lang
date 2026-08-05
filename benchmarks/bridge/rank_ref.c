// rank_ref.c — C reference for the GLUE native-speed benchmark.
// Mirrors examples/glue-host/rank.bv feature_hash (FNV-1a folding) so
// Briv and C produce identical output on the same workload.
#include <stdint.h>

int64_t feature_hash_c(int64_t count, int64_t seed) {
    int64_t h = seed;
    for (int64_t i = 0; i < count; i++) {
        h = (h ^ (i * 2654435761)) * 1099511628211;
    }
    return h;
}

int64_t add_c(int64_t a, int64_t b) {
    return a + b;
}
