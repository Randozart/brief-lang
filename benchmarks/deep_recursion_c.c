// deep_recursion_c — C reference for deep_recursion.bv
#include <stdlib.h>
#include <stdio.h>

long sum(long n) {
    if (n == 0) return 0;
    return n + sum(n - 1);
}

int main(void) {
    const char* env = getenv("BOUND");
    long N = env ? atol(env) : 50000000L;
    fprintf(stdout, "%ld\n", sum(N));
    return 0;
}
