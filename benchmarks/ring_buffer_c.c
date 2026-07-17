// Ring Buffer — C reference for Brief LLVM backend
//
// Allocates a 1024-slot ring buffer, enqueues TOTAL items,
// prints fill level every 5M ops. Symmetric with ring_buffer.bv.

#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>

int main(void) {
    long N = 50000000L;
    char *env = getenv("BOUND");
    if (env) N = atol(env);

    const long cap = 1024;
    int64_t* data = (int64_t*)malloc(cap * sizeof(int64_t));
    long head = 0;
    long tail = 0;
    long ops = 0;

    while (ops < N) {
        data[tail % cap] = ops;
        tail++;
        ops++;
        if (ops % 5000000 == 0) {
            long filled = tail - head;
            printf("%ld\n", filled);
        }
    }

    free(data);
    return 0;
}
