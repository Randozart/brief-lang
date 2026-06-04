// Mandelbrot C reference — symmetric with Brief benchmark.
// Complex integer arithmetic (fixed-point), LCG, escape tracking.
// Compile: clang -O3 -march=native -o mandelbrot_c mandelbrot_c.c

#include <stdlib.h>

#define IM 139968
#define IA 3877
#define IC 29573
#define SCALE 100

int main(void) {
    long count = 0;
    long N = 50000000;
    char *env = getenv("BOUND");
    if (env) N = atol(env);

    long seed = 42;
    long zr = 100, zi = 0, cr = -75, ci = 10;
    long t1 = 0, t2 = 0, t3 = 0, escapes = 0;

    while (count < N) {
        // LCG for next c
        seed = (seed * IA + IC) % IM;
        cr = seed % 200 - 100;
        seed = (seed * IA + IC) % IM;
        ci = seed % 200 - 100;

        // Complex multiply: z = z * c
        t1 = zr * cr / SCALE;
        t2 = zi * ci / SCALE;
        t3 = zr * ci / SCALE;
        zr = t1 - t2;
        zi = t3 + zi * cr / SCALE;

        // Track norm
        escapes = escapes + zr * zr / SCALE + zi * zi / SCALE;

        count++;
    }
    return (int)(escapes & 0xFF);
}
