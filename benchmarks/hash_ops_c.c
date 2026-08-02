// hash_ops_c — C reference for hash_ops.bv
#include <stdlib.h>
#include <stdio.h>

int main(void) {
    const char* env = getenv("BOUND");
    long N = env ? atol(env) : 50000000L;
    const long CAP = 16777216L;
    long* table = calloc(CAP, sizeof(long));
    long i = 0, found = 0;
    for (; i < N; ) {
        long h = (i * 2654435761UL) % CAP;
        table[h] = table[h] + i + 1;
        found += table[h];
        i++;
        if (i % 5000000 == 0)
            fprintf(stdout, "%ld\n", found);
    }
    free(table);
    return 0;
}
