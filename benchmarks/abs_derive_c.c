// abs C reference — branchless absolute value
#include <stdio.h>

static long abs(long x) {
    long mask = x >> 63;
    return (x ^ mask) - mask;
}

int main() {
    long N = 50000000;
    long total = 0;
    for (long i = 0; i < N; i++) {
        total += abs(i - N/2);
    }
    printf("%ld\n", total);
    return 0;
}
