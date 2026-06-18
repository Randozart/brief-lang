// SAXPY C reference — compiled via clang -O3 -ffast-math
// Output must match Brief version for correctness check.

#include <stdio.h>
#include <stdlib.h>

int main(int argc, char** argv) {
    const char* n_str = getenv("BOUND");
    if (!n_str) { fprintf(stderr, "BOUND not set\n"); return 1; }
    size_t N = (size_t)atol(n_str);
    if (N == 0) return 0;

    float* x = (float*)malloc(N * sizeof(float));
    float* y = (float*)malloc(N * sizeof(float));
    if (!x || !y) { fprintf(stderr, "malloc failed\n"); return 1; }

    const float a = 2.0f;
    for (size_t i = 0; i < N; i++) {
        x[i] = (float)i;
        y[i] = (float)(i * 2);
    }

    for (size_t i = 0; i < N; i++) {
        y[i] = a * x[i] + y[i];
    }

    unsigned long sum = 0;
    for (size_t i = 0; i < N; i++) {
        sum += (unsigned long)y[i];
    }
    printf("%lu\n", sum);

    free(x);
    free(y);
    return 0;
}
