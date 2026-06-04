// Print Loop — C reference for print_loop.bv
// Structured identically: 50M iterations, print every 100Kth value
// No volatile, no benchmark hacks — structurally symmetric

#include <stdio.h>

int main(void) {
    long ops = 0;
    const long N = 50000000;
    const long PRINT_INTERVAL = 100000;

    while (ops < N) {
        ops++;
        if (ops % PRINT_INTERVAL == 0) {
            fprintf(stderr, "%ld\n", ops);
        }
    }

    return 0;
}
