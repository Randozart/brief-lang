// stack_push_pop_c — C reference for stack_push_pop.bv
// Fixed-size stack: push the counter, pop it, each tick.
// The stack starts with one element [0] (len = 1) via init.

#include <stdlib.h>
#include <stdio.h>

int main(void) {
    const char* env = getenv("BOUND");
    long N = env ? atol(env) : 50000000L;
    long count = 0;
    long st[256];
    long len = 1;
    st[0] = 0;

    for (; count < N; ) {
        st[len] = count; len++;
        len--; (void) st[len];
        count++;
        if (count % 5000000 == 0)
            fprintf(stdout, "%ld\n", count);
    }

    return 0;
}
