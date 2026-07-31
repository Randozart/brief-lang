// Sweep Sparse — C reference. Same computation as sweep_sparse.bv.
#include <stdio.h>
#include <stdlib.h>

int main(void) {
    long total = 50000000;
    char *env = getenv("BOUND");
    if (env) total = atol(env);
    float f0=1.0f, f1=0.5f, f2=0.25f, f3=0.125f;
    long count = 0;
    while (count < total) {
        f0 = f0 * 0.999f + f1 * 0.001f;
        f1 = f1 * 0.999f + f2 * 0.001f;
        f2 = f2 * 0.999f + f3 * 0.001f;
        f3 = f3 * 0.999f + f0 * 0.001f;
        count++;
        if (count % 100000 == 0) {
            printf("%.9g\n", (double)(f0 + f1 + f2 + f3));
        }
    }
    return 0;
}
