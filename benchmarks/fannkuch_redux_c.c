// Fannkuch-Redux C reference — symmetric with Brief benchmark.
// 12 flat fields, clockwise rotation, checksum via modulo-13.
// Compile: clang -O3 -march=native -o fannkuch_redux_c fannkuch_redux_c.c

#include <stdlib.h>

#define IM 139968
#define IA 3877
#define IC 29573

int main(void) {
    long count = 0;
    long N = 50000000;
    char *env = getenv("BOUND");
    if (env) N = atol(env);

    long seed = 42;
    long max_flips = 0;
    long checksum = 0;

    long p0=0, p1=1, p2=2, p3=3, p4=4, p5=5;
    long p6=6, p7=7, p8=8, p9=9, p10=10, p11=11;

    while (count < N) {
        seed = (seed * IA + IC) % IM;

        long saved = p0;
        p0 = p1; p1 = p2; p2 = p3; p3 = p4; p4 = p5;
        p5 = p6; p6 = p7; p7 = p8; p8 = p9; p9 = p10;
        p10 = p11; p11 = saved;

        checksum = checksum + saved % 13;
        max_flips = max_flips + checksum % 17;

        count++;
    }
    return (int)(checksum & 0xFF);
}
