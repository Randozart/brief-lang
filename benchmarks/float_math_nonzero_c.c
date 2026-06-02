// Float Math (Non-Zero) — C reference for non-trivial float computation.
// Non-zero initial values + coupled matrix prevent clang from eliminating
// the matrix multiply via constant propagation.
//
// Brief version: float_math_nonzero.bv — same computation in reactive model
// with enum dispatch + SSA-mode folded loop.

#include <stdlib.h>
#include <stdio.h>

int main(int argc, char **argv) {
    long bound = 50000000;
    char *env = getenv("BOUND");
    if (env) bound = atol(env);

    float x0 = 1.0f, x1 = 0.5f, x2 = 0.2f;
    float p00 = 0.0f, p11 = 0.0f, p22 = 0.0f;

    const float A00 = 1.0f, A01 = 0.01f, A02 = 0.001f;
    const float A10 = 0.01f, A11 = 1.0f, A12 = 0.01f;
    const float A20 = 0.001f, A21 = 0.01f, A22 = 1.0f;
    const float Q00 = 0.1f, Q11 = 0.1f, Q22 = 0.1f;

    long count = 0;
    while (count < bound) {
        float nx0 = A00 * x0 + A01 * x1 + A02 * x2;
        float nx1 = A10 * x0 + A11 * x1 + A12 * x2;
        float nx2 = A20 * x0 + A21 * x1 + A22 * x2;
        x0 = nx0; x1 = nx1; x2 = nx2;
        p00 += Q00; p11 += Q11; p22 += Q22;
        count++;
    }

    return (int)(count + x0 + x1 + x2 + p00 + p11 + p22);
}
