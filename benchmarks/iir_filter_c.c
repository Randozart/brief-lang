// Biquad IIR Filter Cascade — C reference for Briev LLVM backend benchmark
//
// Matches benchmarks/iir_filter.bv exactly:
//   - single-precision float (f32) — same as Briev LLVM `float` type
//   - 50M iterations of the biquad difference equation
//   - impulse input (constant 1.0)
//   - float state in registers (no volatile) — matches Briev's register promotion
//   - only `count` is volatile to prevent dead-code elimination of the entire loop
//
// Build:
//   clang -O3 -march=native -o benchmarks/iir_filter_c benchmarks/iir_filter_c.c -lm

int main(void) {
    const float b0 = 0.003916126444f;
    const float b1 = 0.007832252889f;
    const float b2 = 0.003916126444f;
    const float a1 = -1.815341082700f;
    const float a2 = 0.831005589300f;

    float x1 = 0.0f;
    float x2 = 0.0f;
    float y1 = 0.0f;
    float y2 = 0.0f;

    const float input = 1.0f;

    const long total = 50000000L;
    volatile long count = 0;
    for (; count < total; count++) {
        const float f0 = b0 * input;
        const float f1 = b1 * x1;
        const float f2 = b2 * x2;
        const float ff = f0 + f1 + f2;

        const float fb1 = a1 * y1;
        const float fb2 = a2 * y2;
        const float fb = fb1 + fb2;

        const float out = ff - fb;

        x2 = x1;
        x1 = input;
        y2 = y1;
        y1 = out;
    }

    return 0;
}
