// hash_ops_idio_c — C reference for hash_ops_idio.bv.
// The IDIOMATIC workload: a REAL open-addressing hash table sized 2*N (never
// fills) — `map.insert((i, i*2))` + `map.get(i)` per iteration, sum of the
// read-backs observable. The C side mirrors the Briev HashMap structure:
// three 2*N columns (keys/vals/occupied), direct hash `i % (2*N)` with linear
// probe, early-exit on the matched/free slot. (The old reference was a
// 256-entry RING — a different workload: it overwrote the same 256 slots, so
// it measured 4KB-cache-resident direct writes vs the map's full-table
// traffic. The `_sym` flat-table form is hash_ops.bv.)
#include <stdlib.h>
#include <stdio.h>

int main(void) {
    const char* env = getenv("BOUND");
    long N = env ? atol(env) : 50000000L;
    const long CAP = 2 * N;
    long* keys = calloc(CAP, sizeof(long));
    long* vals = calloc(CAP, sizeof(long));
    char* occupied = calloc(CAP, sizeof(char));
    long i, sum = 0;
    for (i = 0; i < N; ) {
        // insert((i, i*2)): probe for a free or matching slot, early-exit.
        long h = i % CAP;
        long q = 0;
        long p;
        do {
            p = (h + q) % CAP;
            q++;
        } while (occupied[p] && keys[p] != i);
        keys[p] = i; vals[p] = i * 2; occupied[p] = 1;
        // get(i): probe for the matching slot, early-exit.
        q = 0;
        do {
            p = (h + q) % CAP;
            q++;
        } while (!(occupied[p] && keys[p] == i));
        sum += vals[p];
        i++;
        if (i % 5000000 == 0)
            fprintf(stdout, "%ld\n", sum);
    }
    free(keys); free(vals); free(occupied);
    return 0;
}
