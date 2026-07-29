// minmax C reference
#include <stdio.h>

static long min(long x, long y) {
    return y ^ ((x ^ y) & -(x < y));
}

static long max(long x, long y) {
    return x ^ ((x ^ y) & -(x < y));
}

int main() {
    long N = 50000000;
    long total = 0;
    for (long i = 0; i < N; i++) {
        total += min(i, N - i) + max(i, N - i);
    }
    printf("%ld\n", total);
    return 0;
}
