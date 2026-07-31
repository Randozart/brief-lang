// Accumulator Flush — C reference. Same computation as accumulator_flush.bv.
#include <stdio.h>
#include <stdlib.h>

int main(void) {
    long total = 50000000;
    char *env = getenv("BOUND");
    if (env) total = atol(env);
    float sum = 0.0f, sumsq = 0.0f;
    long n = 0;
    long count = 0;
    while (count < total) {
        float x = (float)(count % 101) * 0.5f;
        sum = sum + x;
        sumsq = sumsq + x * x;
        n++;
        count++;
        if (count % 100000 == 0) {
            printf("%lld %g %g\n", (long long)n, (double)sum, (double)sumsq);
            sum = 0.0f;
            sumsq = 0.0f;
            n = 0;
        }
    }
    return 0;
}
