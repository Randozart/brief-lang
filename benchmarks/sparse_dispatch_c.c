// Sparse Dispatch — C reference.
// Equivalent to the Brief-native cyclic dispatch benchmark.
// Measures: direct switch (count % 8) overhead, 50M iterations.
//
// Brief version: sparse_dispatch.bv — 8 reactive txns gated by
//   io_pending && (count % 8) == N, enum switch dispatch path.

#include <stdlib.h>
#include <stdio.h>

int main(int argc, char **argv) {
    long bound = 50000000;
    char *env = getenv("BOUND");
    if (env) bound = atol(env);

    long count = 0;
    while (count < bound) {
        switch (count % 8) {
            case 0: break; case 1: break; case 2: break; case 3: break;
            case 4: break; case 5: break; case 6: break; case 7: break;
            default: break;
        }
        count++;
        if (count % 5000000 == 0) {
            fprintf(stderr, "%ld\n", count);
        }
    }
    return 0;
}
