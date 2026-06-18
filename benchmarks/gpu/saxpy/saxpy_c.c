// SAXPY C reference: y = a * x + y (single-precision float)
// Compile with: clang -O3 -ffast-math -o saxpy_c saxpy_c.c
// Run with: N=10000000 ./saxpy_c

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

int main(int argc, char** argv) {
    const char* n_str = getenv("N");
    if (!n_str) { fprintf(stderr, "N not set\n"); return 1; }
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

    // SAXPY kernel
    for (size_t i = 0; i < N; i++) {
        y[i] = a * x[i] + y[i];
    }

    // Observable output to prevent dead-code elimination
    unsigned long sum = 0;
    for (size_t i = 0; i < N; i++) {
        sum += (unsigned long)y[i];
    }
    printf("%lu\n", sum);

    free(x);
    free(y);
    return 0;
}
