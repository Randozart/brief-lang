// Async Parallel Counters — Symmetric Runtime C Reference
//
// History:
//   2026-07-01: Created as symmetric companion to async_counters_sym.bv.
//     The original async_counters_c.c was CHANGED IN ad67b83 from a real
//     2-thread pthread program to a trivial fold-target (long g_a = 25000000L;
//     (void)g_a;), breaking the runtime comparison. This restores the original
//     intent: two worker threads with dual-barrier synchronization matching
//     Briev's thread pool implementation.
//
// Matches Briev's runtime pattern (briev_rt.c):
//   - Dual barriers (enter: main releases workers, exit: workers signal done)
//   - main participates in both barriers (3 participants total: 2 workers + main)
//   - Workers print progress every 5M iterations via __sync_fetch_and_add
//   - No explicit thread pool shutdown — process exit kills workers (matches
//     Briev's compiled behavior where main returns without briev_thread_pool_shutdown)
//
// Build:
//   clang -O3 -march=native -o benchmarks/async_counters_sym_c \
//     benchmarks/async_counters_sym_c.c -lpthread

#include <stdio.h>
#include <stdlib.h>
#include <pthread.h>

static long N;
static volatile long a = 0;
static volatile long b = 0;
static pthread_barrier_t barrier_enter;
static pthread_barrier_t barrier_exit;

static void *worker(void *arg) {
    volatile long *counter = (volatile long *)arg;
    while (1) {
        pthread_barrier_wait(&barrier_enter);
        long old = __sync_fetch_and_add(counter, 1) + 1;
        if (old % 5000000 == 0) printf("%ld\n", old);
        pthread_barrier_wait(&barrier_exit);
    }
    return NULL;
}

int main(void) {
    const char *env = getenv("BOUND");
    N = env ? atol(env) : 50000000;

    pthread_barrier_init(&barrier_enter, NULL, 3);
    pthread_barrier_init(&barrier_exit, NULL, 3);

    pthread_t ta, tb;
    pthread_create(&ta, NULL, worker, (void *)&a);
    pthread_create(&tb, NULL, worker, (void *)&b);

    while (a < N || b < N) {
        pthread_barrier_wait(&barrier_enter);
        pthread_barrier_wait(&barrier_exit);
    }

    // Process exit kills workers (same as Briev's ret without shutdown).
    // Calling pthread_barrier_destroy while workers are blocked on the
    // barrier is undefined behavior (may hang). Just return — the OS
    // cleans up on process exit.
    return 0;
}
