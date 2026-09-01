// ggml_gemv_bench.c — ledger anchor (plan 2026-08-31-o3-float4 → split-K):
// bare GEMV y = A·x at M=K=4096 via ggml_mul_mat on the llama.cpp build the
// VITRIOL project ships (same box). Rows: ggml-cpu (1T / NT) and, when the
// CUDA backend initializes, ggml-cuda — the cuBLAS-class race target for the
// .abv GPU lane.
//
// Usage: ggml_gemv_bench [M] [K] [n_threads]

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <time.h>
#include "ggml.h"
#include "ggml-cpu.h"
#include "ggml-backend.h"
#include "ggml-cuda.h"

static double now_ms(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (double)ts.tv_sec * 1e3 + (double)ts.tv_nsec / 1e6;
}

// ha/hx: host data buffers (the CPU-context tensors' payloads) for the
// device upload when `be` is a GPU backend.
static void bench_backend(ggml_backend_t be, struct ggml_context* ctx,
                          struct ggml_tensor* a, struct ggml_tensor* x,
                          struct ggml_tensor* y, int n_threads,
                          uint64_t M, uint64_t K,
                          const float* ha, const float* hx) {
    struct ggml_tensor* yt = ggml_mul_mat(ctx, a, x);
    struct ggml_cgraph* gf = ggml_new_graph(ctx);
    ggml_build_forward_expand(gf, yt);
    const char* bname = ggml_backend_name(be);
    const int is_cuda = bname != NULL
        && (strncmp(bname, "CUDA", 4) == 0 || strncmp(bname, "cuda", 4) == 0);
    if (is_cuda) {
        ggml_backend_buffer_t buf = ggml_backend_alloc_ctx_tensors_from_buft(
            ctx, ggml_backend_get_default_buffer_type(be));
        if (buf == NULL) {
            fprintf(stderr, "# cuda: device alloc failed\n");
            return;
        }
        ggml_backend_tensor_set(a, ha, 0, ggml_nbytes(a));
        ggml_backend_tensor_set(x, hx, 0, ggml_nbytes(x));
        if (ggml_backend_graph_compute(be, gf) != GGML_STATUS_SUCCESS) {
            fprintf(stderr, "# cuda: compute failed\n");
            return;
        }
        double sum = 0.0, min = 1e30;
        for (int it = 0; it < 20; it++) {
            double t0 = now_ms();
            ggml_backend_graph_compute(be, gf);
            double dt = now_ms() - t0;
            sum += dt;
            if (dt < min) min = dt;
        }
        double avg = sum / 20;
        double gflop = 2.0 * (double)M * (double)K / 1e9;
        printf("ggml-cuda    avg %8.3f ms  min %8.3f ms  %8.2f GFLOP/s\n",
               avg, min, gflop / (avg / 1e3));
        return;
    }
    ggml_graph_compute_with_ctx(ctx, gf, n_threads);
    double sum = 0.0, min = 1e30;
    for (int it = 0; it < 20; it++) {
        double t0 = now_ms();
        ggml_graph_compute_with_ctx(ctx, gf, n_threads);
        double dt = now_ms() - t0;
        sum += dt;
        if (dt < min) min = dt;
    }
    double avg = sum / 20;
    double gflop = 2.0 * (double)M * (double)K / 1e9;
    printf("ggml-cpu(%dT) avg %8.3f ms  min %8.3f ms  %8.2f GFLOP/s\n",
           n_threads, avg, min, gflop / (avg / 1e3));
}

int main(int argc, char** argv) {
    const uint64_t M = argc > 1 ? strtoull(argv[1], NULL, 10) : 4096;
    const uint64_t K = argc > 2 ? strtoull(argv[2], NULL, 10) : 4096;
    const int n_threads = argc > 3 ? atoi(argv[3]) : 1;

    struct ggml_init_params ip = {
        .mem_size = 512 * 1024 * 1024,
        .mem_buffer = NULL,
        .no_alloc = 0,
    };
    struct ggml_context* ctx = ggml_init(ip);
    if (ctx == NULL) { fprintf(stderr, "ggml_init failed\n"); return 2; }

    struct ggml_tensor* a = ggml_new_tensor_2d(ctx, GGML_TYPE_F32, K, M);
    struct ggml_tensor* x = ggml_new_tensor_1d(ctx, GGML_TYPE_F32, K);
    struct ggml_tensor* y = ggml_new_tensor_1d(ctx, GGML_TYPE_F32, M);
    float* ad = (float*)a->data;
    float* xd = (float*)x->data;
    for (uint64_t j = 0; j < M * K; j++) ad[j] = (float)(j % 7) * 0.25f;
    for (uint64_t k = 0; k < K; k++) xd[k] = (float)(k % 5) * 0.5f;

    // CPU row (host tensors).
    bench_backend(ggml_backend_cpu_init(), ctx, a, x, y, n_threads, M, K, ad, xd);

    // CUDA row when the backend initializes (device tensors, host data).
    ggml_backend_t cuda = ggml_backend_cuda_init(0);
    if (cuda != NULL) {
        struct ggml_init_params ip2 = {
            .mem_size = 16 * 1024 * 1024,
            .mem_buffer = NULL,
            .no_alloc = 1,
        };
        struct ggml_context* dctx = ggml_init(ip2);
        struct ggml_tensor* da = ggml_new_tensor_2d(dctx, GGML_TYPE_F32, K, M);
        struct ggml_tensor* dx = ggml_new_tensor_1d(dctx, GGML_TYPE_F32, K);
        struct ggml_tensor* dy = ggml_new_tensor_1d(dctx, GGML_TYPE_F32, M);
        bench_backend(cuda, dctx, da, dx, dy, n_threads, M, K, ad, xd);
        ggml_free(dctx);
    } else {
        printf("# cuda backend unavailable (no GGML_CUDA in this build or no device)\n");
    }
    printf("fingerprint: ggml mul_mat F32 M=%llu K=%llu iters=20\n",
           (unsigned long long)M, (unsigned long long)K);
    ggml_free(ctx);
    return 0;
}
