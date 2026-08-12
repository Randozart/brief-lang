/*
 * timing_bridge.c — Shared monotonic clock FFI for Briev benchmarks
 *
 * Provides:
 *   __monotonic_ns() → uint64_t — nanosecond timestamp from CLOCK_MONOTONIC
 *   __run_benchmark() — runs a void(void) payload N times
 *
 * All benchmarks link against this file for in-benchmark timing.
 * Declare in .bv as:  frgn __monotonic_ns() -> Result<Int, TimeError>;
 */

#include <time.h>
#include <stdint.h>

uint64_t __monotonic_ns(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (uint64_t)ts.tv_sec * 1000000000ULL + (uint64_t)ts.tv_nsec;
}
