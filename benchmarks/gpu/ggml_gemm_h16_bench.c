// ggml_gemm_h16_bench.c — THE CUDA RACE ANCHOR (plan 2026-09-02-cuda-race
// Phase A): 4096³ F16 GEMM (the tensor-core race: A,B = GGML_TYPE_F16, f32 accumulate)
// via ggml_mul_mat on the llama.cpp build the VITRIOL project ships (same
// box). Rows: ggml-cpu (1T / NT) and, when the CUDA backend initializes,
// ggml-cuda — the bar our tiled .abv kernel (25.3ms / 5250 GFLOP/s) must be
// judged against on the same silicon.
//
// ggml mul_mat(A, B): A ne={K,M} row-major, B ne={K,N} row-major,
// C[m*N+n] = Σ_k A[m*K+k] * B[k*N+n] — the SAME math as examples/gpu/gemm.abv.
//
// Usage: ggml_gemm_bench [M] [N] [K] [n_threads]

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

static void bench_backend(ggml_backend_t be, struct ggml_context* ctx,
                          struct ggml_tensor* a, struct ggml_tensor* b,
                          struct ggml_tensor* y, int n_threads,
                          uint64_t M, uint64_t N, uint64_t K,
                          const void* ha, const void* hb) {
    struct ggml_tensor* yt = ggml_mul_mat(ctx, a, b);
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
        ggml_backend_tensor_set(b, hb, 0, ggml_nbytes(b));
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
        double tflop = 2.0 * (double)M * (double)N * (double)K / 1e12;
        printf("ggml-cuda    avg %9.3f ms  min %9.3f ms  %9.2f GFLOP/s\n",
               avg, min, tflop / (avg * 1e-6));
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
    double tflop = 2.0 * (double)M * (double)N * (double)K / 1e12;
    printf("ggml-cpu(%dT) avg %9.3f ms  min %9.3f ms  %9.2f GFLOP/s\n",
           n_threads, avg, min, tflop / (avg * 1e-6));
}

int main(int argc, char** argv) {
    const uint64_t M = argc > 1 ? strtoull(argv[1], NULL, 10) : 4096;
    const uint64_t N = argc > 2 ? strtoull(argv[2], NULL, 10) : 4096;
    const uint64_t K = argc > 3 ? strtoull(argv[3], NULL, 10) : 4096;
    const int n_threads = argc > 4 ? atoi(argv[4]) : 1;

    struct ggml_init_params ip = {
        .mem_size = 1024 * 1024 * 1024,
        .mem_buffer = NULL,
        .no_alloc = 0,
    };
    struct ggml_context* ctx = ggml_init(ip);
    if (ctx == NULL) { fprintf(stderr, "ggml_init failed\n"); return 2; }

    // ggml row-major: a ne={K, M}, b ne={K, N} (K contiguous per row).
    struct ggml_tensor* a = ggml_new_tensor_2d(ctx, GGML_TYPE_F16, K, M);
    struct ggml_tensor* b = ggml_new_tensor_2d(ctx, GGML_TYPE_F16, K, N);
    struct ggml_tensor* y = ggml_new_tensor_2d(ctx, GGML_TYPE_F32, N, M);
    uint16_t* ad = (uint16_t*)a->data;
    uint16_t* bd = (uint16_t*)b->data;
    for (uint64_t j = 0; j < M * K; j++) ad[j] = ggml_fp32_to_fp16((float)(j % 7) * 0.25f);
    for (uint64_t j = 0; j < K * N; j++) bd[j] = ggml_fp32_to_fp16((float)(j % 5) * 0.5f);

    bench_backend(ggml_backend_cpu_init(), ctx, a, b, y, n_threads, M, N, K, ad, bd);

    ggml_backend_t cuda = ggml_backend_cuda_init(0);
    if (cuda != NULL) {
        struct ggml_init_params ip2 = {
            .mem_size = 64 * 1024 * 1024,
            .mem_buffer = NULL,
            .no_alloc = 1,
        };
        struct ggml_context* dctx = ggml_init(ip2);
        struct ggml_tensor* da = ggml_new_tensor_2d(dctx, GGML_TYPE_F16, K, M);
        struct ggml_tensor* db = ggml_new_tensor_2d(dctx, GGML_TYPE_F16, K, N);
        struct ggml_tensor* dy = ggml_new_tensor_2d(dctx, GGML_TYPE_F32, N, M);
        bench_backend(cuda, dctx, da, db, dy, n_threads, M, N, K, ad, bd);
        ggml_free(dctx);
    } else {
        printf("# cuda backend unavailable (no GGML_CUDA in this build or no device)\n");
    }
    printf("fingerprint: ggml mul_mat F16xF16->F32 M=%llu N=%llu K=%llu iters=20\n",
           (unsigned long long)M, (unsigned long long)N, (unsigned long long)K);
    ggml_free(ctx);
    return 0;
}
