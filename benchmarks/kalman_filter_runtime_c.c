// Kalman Filter — Runtime-variable bound via getenv("BOUND")
// Perfect C reference for kalman_filter_runtime.bv.
// Full 3x3 state vector + covariance propagation, NO volatile.
// Local float variables give clang full register allocation freedom.
// Returns a value that depends on ALL state fields + counter to

#include <stdio.h>
#include <stdlib.h>

int main(void) {
    const char* env = getenv("BOUND");
    long total = env ? atol(env) : 50000000L;

    // State vector (3 floats)
    float x0 = 0.0f, x1 = 0.0f, x2 = 0.0f;

    // Covariance matrix P (9 floats, row-major: P[row*3 + col])
    float p00 = 0.1f, p01 = 0.0f, p02 = 0.0f;
    float p10 = 0.0f, p11 = 0.1f, p12 = 0.0f;
    float p20 = 0.0f, p21 = 0.0f, p22 = 0.1f;

    // A matrix (constant, row-major)
    const float a00 = 1.0f,     a01 = 0.01f,     a02 = 0.00005f;
    const float a10 = 0.0f,     a11 = 1.0f,      a12 = 0.01f;
    const float a20 = 0.0f,     a21 = 0.0f,      a22 = 1.0f;

    // Q matrix (constant, row-major)
    const float q00 = 0.001f, q01 = 0.0f, q02 = 0.0f;
    const float q10 = 0.0f,   q11 = 0.001f, q12 = 0.0f;
    const float q20 = 0.0f,   q21 = 0.0f,   q22 = 0.001f;

    long count = 0;
    for (; count < total; count++) {
        // State propagation: x_new = A * x
        float nx0 = a00 * x0 + a01 * x1 + a02 * x2;
        float nx1 = a10 * x0 + a11 * x1 + a12 * x2;
        float nx2 = a20 * x0 + a21 * x1 + a22 * x2;

        // Covariance propagation: P_new = A * P * A^T + Q
        // Step 1: AP = A * P
        float ap00 = a00 * p00 + a01 * p10 + a02 * p20;
        float ap01 = a00 * p01 + a01 * p11 + a02 * p21;
        float ap02 = a00 * p02 + a01 * p12 + a02 * p22;

        float ap10 = a10 * p00 + a11 * p10 + a12 * p20;
        float ap11 = a10 * p01 + a11 * p11 + a12 * p21;
        float ap12 = a10 * p02 + a11 * p12 + a12 * p22;

        float ap20 = a20 * p00 + a21 * p10 + a22 * p20;
        float ap21 = a20 * p01 + a21 * p11 + a22 * p21;
        float ap22 = a20 * p02 + a21 * p12 + a22 * p22;

        // Step 2: P_new = AP * A^T + Q
        p00 = ap00 * a00 + ap01 * a10 + ap02 * a20 + q00;
        p01 = ap00 * a01 + ap01 * a11 + ap02 * a21 + q01;
        p02 = ap00 * a02 + ap01 * a12 + ap02 * a22 + q02;

        p10 = ap10 * a00 + ap11 * a10 + ap12 * a20 + q10;
        p11 = ap10 * a01 + ap11 * a11 + ap12 * a21 + q11;
        p12 = ap10 * a02 + ap11 * a12 + ap12 * a22 + q12;

        p20 = ap20 * a00 + ap21 * a10 + ap22 * a20 + q20;
        p21 = ap20 * a01 + ap21 * a11 + ap22 * a21 + q21;
        p22 = ap20 * a02 + ap21 * a12 + ap22 * a22 + q22;

        // Update state vector
        x0 = nx0;
        x1 = nx1;
        x2 = nx2;
        count++;
        if (count % 5000000 == 0) {
            float energy = x0 * x0 + p00 + p11 + p22;
            float ei = x1 * x1 + p01 + p12;
            float ej = x2 * x2 + p02 + p21;
            fprintf(stderr, "%.9f\n", (double)(energy + ei + ej));
        }
    }
    return 0;
}
