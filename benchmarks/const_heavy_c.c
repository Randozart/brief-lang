// Const Heavy — C reference for constant inlining benchmark.
// 20 compile-time constants summed per iteration, 50M iterations.
//
// Brief version: const_heavy.bv — same computation in reactive model
// with folded pure-counter loop (O(1) store).

#include <stdlib.h>
#include <stdio.h>

#define C00 100
#define C01 200
#define C02 300
#define C03 400
#define C04 500
#define C05 600
#define C06 700
#define C07 800
#define C08 900
#define C09 1000
#define C10 1100
#define C11 1200
#define C12 1300
#define C13 1400
#define C14 1500
#define C15 1600
#define C16 1700
#define C17 1800
#define C18 1900
#define C19 2000

int main(int argc, char **argv) {
    long bound = 50000000;
    char *env = getenv("BOUND");
    if (env) bound = atol(env);

    long count = 0;
    long acc = 0;
    while (count < bound) {
        acc = acc + count / 100 + C00 + C01 + C02 + C03 + C04
            + C05 + C06 + C07 + C08 + C09
            + C10 + C11 + C12 + C13 + C14
            + C15 + C16 + C17 + C18 + C19;
        count++;
    }
    return (int)(count + acc);
}
