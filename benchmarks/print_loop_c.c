// Print Loop — C reference for print_loop.bv
// Structured identically: 50M iterations, print every 100Kth value
// No volatile, no benchmark hacks — structurally symmetric

#include <stdio.h>
#include <stdlib.h>

int main(void) {
    long ops = 0;
    long N = 50000000;
    const long PRINT_INTERVAL = 100000;
    char *env = getenv("BOUND");
    if (env) N = atol(env);

    while (ops < N) {
        ops++;
        if (ops % PRINT_INTERVAL == 0) {
            fprintf(stdout, "%ld\n", ops);
        }
    }

    return 0;
}
