// Async Parallel Counters — C reference for Brief LLVM backend Path 5 benchmark
//
// Two pthreads each increment their own counter 25M times.
// Brief dispatches them concurrently via the built-in thread pool.
// C creates two pthreads manually with pthread_create/pthread_join.
//
// Build:
//   clang -O3 -march=native -o benchmarks/async_counters_c benchmarks/async_counters_c.c -lpthread

#include <stdio.h>
#include <stdlib.h>
#include <pthread.h>

#define N 25000000L

static volatile long g_a = 0;
static volatile long g_b = 0;

static void *inc_a_thread(void *arg) {
    (void)arg;
    for (; g_a < N; g_a++) {}
    return NULL;
}

static void *inc_b_thread(void *arg) {
    (void)arg;
    for (; g_b < N; g_b++) {}
    return NULL;
}

int main(void) {
    pthread_t ta, tb;
    pthread_create(&ta, NULL, inc_a_thread, NULL);
    pthread_create(&tb, NULL, inc_b_thread, NULL);
    pthread_join(ta, NULL);
    pthread_join(tb, NULL);
    return 0;
}
