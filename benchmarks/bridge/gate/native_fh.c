// native_fh.c — native C reference for the zero-friction gate.
// Same FNV-1a folding as examples/glue-host/bench.bv feature_hash (64-bit
// wrapping). Compiled -O3; the "native" number a C host would get writing it
// itself.
#include <stdint.h>
int64_t native_fh(int64_t count, int64_t seed) {
    int64_t h = seed;
    for (int64_t i = 0; i < count; i++) {
        h = (h ^ (i * 2654435761LL)) * 1099511628211LL;
    }
    return h;
}
