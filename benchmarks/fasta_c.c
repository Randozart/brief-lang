// Fasta C reference — symmetric with Briv benchmark.
// Compile: clang -O3 -march=native -ffast-math -o fasta_c fasta_c.c

#include <stdio.h>
#include <stdlib.h>

#define IM 139968
#define IA 3877
#define IC 29573

int main(void) {
    long count = 50000000;
    char *env = getenv("BOUND");
    if (env) count = atol(env);

    long seed = 42;

    while (count-- > 0) {
        seed = (seed * IA + IC) % IM;
        fprintf(stdout, "%c", (int)(seed % 26 + 'a'));
    }
    return 0;
}
