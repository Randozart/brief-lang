// knucleotide C reference — symmetric with Briev benchmark.
// Rolling 2-bit hash (k=3) + checksum, printf every 5M.
// Compile: clang -O3 -march=native -o knucleotide_c knucleotide_c.c

#include <stdio.h>
#include <stdlib.h>

#define IM 139968
#define IA 3877
#define IC 29573
#define MASK 63

int main(void) {
    long N = 50000000;
    char *env = getenv("BOUND");
    if (env) N = atol(env);

    long seed = 42, hash = 0, chksum = 0;

    for (long count = 0; count < N; count++) {
        seed = (seed * IA + IC) % IM;
        hash = ((hash << 2) | (seed & 3)) & MASK;
        chksum += hash % 13;

        if (count % 5000000 == 0)
            fprintf(stdout, "%lld\n", (long long)chksum);
    }
    return 0;
}
