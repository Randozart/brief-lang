// series_converge_c — C reference for series_converge.bv
#include <stdlib.h>
#include <stdio.h>

int main(void) {
    const char* env = getenv("BOUND");
    long N = env ? atol(env) : 50000000L;
    long i = 0;
    float last = 0.0f;
    float x = 0.5f;
    for (; i < N; ) {
        last = x;
        x = last * 0.9999f + 0.0001f;
        i++;
        if ((x - last) * (x - last) <= 0.000001f) {
            fprintf(stdout, "%.9g\n", x);
            return 0;
        }
    }
    return 0;
}
