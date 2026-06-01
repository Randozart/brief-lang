// Biquad IIR Filter — Runtime-variable bound (BOUND env var)
// Perfect C reference for iir_filter_runtime.bv.
//
// Float state kept local (optimal SSE register allocation).
// Count is local, no volatile. Returns count + y1 to make both
// the loop count and the biquad result observable.
//
// Build:
//   clang -O3 -march=native -o benchmarks/iir_filter_runtime_c \
//       benchmarks/iir_filter_runtime_c.c -lm

#include <stdlib.h>

int main(void) {
    const char* env = getenv("BOUND");
    long total = env ? atol(env) : 50000000L;

    const float b0 = 0.003916126444f;
    const float b1 = 0.007832252889f;
    const float b2 = 0.003916126444f;
    const float a1 = -1.815341082700f;
    const float a2 = 0.831005589300f;

    float x1 = 0.0f, x2 = 0.0f, y1 = 0.0f, y2 = 0.0f;
    const float input = 1.0f;

    long count = 0;
    for (; count < total; count++) {
        float ff = b0 * input + b1 * x1 + b2 * x2;
        float fb = a1 * y1 + a2 * y2;
        float out = ff - fb;
        x2 = x1; x1 = input; y2 = y1; y1 = out;
    }
    return (int)(count + y1);
}
