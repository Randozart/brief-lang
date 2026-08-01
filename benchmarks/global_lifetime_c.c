// global_lifetime_c — C reference for global_lifetime.bv
#include <stdlib.h>
#include <stdio.h>

int main(void) {
    const char* env = getenv("BOUND");
    long N = env ? atol(env) : 50000000L;
    long* buf = malloc(64 * sizeof(long));
    long sum = 0;
    for (; sum < N; ) {
        buf[sum % 64] = sum;
        sum++;
        if (sum % 5000000 == 0)
            fprintf(stdout, "%ld\n", buf[sum % 64]);
    }
    free(buf);
    return 0;
}
