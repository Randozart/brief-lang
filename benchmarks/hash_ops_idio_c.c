// hash_ops_idio_c — C reference for hash_ops_idio.bv
// Mirrors the flat open-addressing insert + read-back sum.
#include <stdlib.h>
#include <stdio.h>

int main(void) {
    const char* env = getenv("BOUND");
    long N = env ? atol(env) : 50000000L;
    const long CAP = 256;
    long* keys = calloc(CAP, sizeof(long));
    long* vals = calloc(CAP, sizeof(long));
    long count = 0;
    long i, sum = 0;
    for (i = 0; i < N; ) {
        long h = i % CAP;
        keys[h] = i;
        vals[h] = i * 2;
        sum += vals[h];
        count++;
        i++;
        if (i % 5000000 == 0)
            fprintf(stdout, "%ld\n", sum);
    }
    free(keys); free(vals);
    return 0;
}
