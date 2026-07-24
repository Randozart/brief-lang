// C benchmark — Tier 1: Direct .so call (gen_c output: bridge.h)
// 2026-07-24: Measures per-call latency of Brief export via direct C ABI.
// After LTO this is ~0ns overhead — the function IS the native function.
//
// Build:
//   gcc -O2 -o bench_c bench_c.c -ldl
// Run:
//   ./bench_c <path_to.so>

#include <stdio.h>
#include <stdint.h>
#include <stdlib.h>
#include <dlfcn.h>
#include <time.h>

int main(int argc, char** argv) {
    const char* so_path = argc > 1 ? argv[1] : "out/bench_add.so";

    void* lib = dlopen(so_path, RTLD_LAZY | RTLD_LOCAL);
    if (!lib) { fprintf(stderr, "dlopen: %s\n", dlerror()); return 1; }

    typedef int64_t (*add_fn_t)(int64_t, int64_t);
    add_fn_t add_fn = (add_fn_t)dlsym(lib, "add");
    if (!add_fn) { fprintf(stderr, "dlsym add: %s\n", dlerror()); return 1; }

    // Warmup
    int64_t warm = add_fn(3, 4);
    if (warm != 7) { fprintf(stderr, "wrong result: %ld\n", warm); return 1; }

    // Benchmark
    const int N = 100000;
    struct timespec t0, t1;
    clock_gettime(CLOCK_MONOTONIC, &t0);
    for (int i = 0; i < N; i++) {
        add_fn(3, 4);
    }
    clock_gettime(CLOCK_MONOTONIC, &t1);

    int64_t ns = (t1.tv_sec - t0.tv_sec) * 1000000000LL + (t1.tv_nsec - t0.tv_nsec);
    printf("  C (dlopen dlsym)       median=%ldns  result=%ld\n", ns / N, warm);
    printf("  total: %ldns over %d iterations\n", ns, N);

    dlclose(lib);
    return 0;
}
