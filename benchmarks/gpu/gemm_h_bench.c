// gemm_h_bench.c — correctness + perf for the Float16 GEMM lane (plan
// 2026-09-02, resuming 2026-08-31-vitriol-gemm-comparison). The f16 twin of
// gemm_bench.c: y[m*N+n] = sum_k A[m*K+k] * B[k*N+n], f16 storage /
// f32 compute (the kernel widens at the SSBO boundary and stores round
// back to f16 — one final rounding).
// Usage: gemm_h_bench <kernel.spv> [M] [N] [K] [iters]
// State layout mirrors the generated gemm_h_runner.c exactly
// (name-sorted: a, b, i, y; f16 arrays 2B/elem, proj == host offsets):
//   a @ 0          (M*K * 2 bytes)
//   b @ a_end      (K*N * 2)
//   i @ b_end (8-aligned) (8 bytes)
//   y @ i_end + 8  (M*N * 2)
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <math.h>
#include <time.h>
#include "briev_accel_rt.c"

#define WARMUP 5
#define VERIFY_ROWS 16

static double now_ms(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (double)ts.tv_sec * 1e3 + (double)ts.tv_nsec / 1e6;
}

// IEEE-754 binary16 encode, round-to-nearest-even — mirrors the backend's
// f32_to_f16_hex (plan fundamental-parent-membership). Seeds must be f16
// EXACT so the reference is exact.
static uint16_t f32_to_f16(float v) {
    uint32_t bits;
    memcpy(&bits, &v, 4);
    uint16_t sign = (uint16_t)((bits >> 16) & 0x8000u);
    int32_t exp = (int32_t)((bits >> 23) & 0xff);
    uint32_t mant = bits & 0x007fffffu;
    if (exp == 255) {
        return (uint16_t)(mant == 0 ? sign | 0x7c00u : sign | 0x7e00u | (uint16_t)((mant >> 13) & 0x3ff));
    }
    int32_t unbiased = exp - 127;
    if (unbiased > 15) return (uint16_t)(sign | 0x7c00u);
    if (unbiased >= -14) {
        uint32_t m = mant >> 13;
        uint32_t rem = mant & 0x1fffu;
        if (rem > 0x1000u || (rem == 0x1000u && (m & 1u) == 1u)) m += 1;
        uint32_t e = (uint32_t)(unbiased + 15);
        if (m == 0x800u) { m = 0; e += 1; }
        if (e >= 31) return (uint16_t)(sign | 0x7c00u);
        return (uint16_t)(sign | (e << 10) | m);
    }
    if (unbiased >= -25) {
        uint32_t combined = 0x00800000u | mant;
        uint32_t d = (uint32_t)(-(unbiased + 1));
        uint32_t f10 = combined >> d;
        uint32_t rem = combined & ((1u << d) - 1u);
        uint32_t half = 1u << (d - 1);
        if (rem > half || (rem == half && (f10 & 1u) == 1u)) f10 += 1;
        if (f10 >= 0x400u) return (uint16_t)(sign | (1u << 10));
        return (uint16_t)(sign | f10);
    }
    return sign;
}

// f16 decode (exact — for the reference comparison).
static double f16_to_f64(uint16_t h) {
    uint32_t sign = (uint32_t)(h & 0x8000u) << 16;
    uint32_t exp = (h >> 10) & 0x1fu;
    uint32_t mant = h & 0x3ffu;
    uint32_t bits;
    if (exp == 0) {
        if (mant == 0) { bits = sign; }
        else {
            int32_t e = -1;
            uint32_t m = mant;
            while ((m & 0x400u) == 0) { m <<= 1; e -= 1; }
            m &= 0x3ffu;
            bits = sign | (uint32_t)((127 - 15 + e + 1 + 10) << 23) | (m << 13);
        }
    } else if (exp == 31) {
        bits = sign | 0x7f800000u | (mant << 13);
    } else {
        bits = sign | ((exp - 15 + 127) << 23) | (mant << 13);
    }
    float f;
    memcpy(&f, &bits, 4);
    return (double)f;
}

static unsigned char* state = NULL;

int main(int argc, char** argv) {
    if (argc < 2) { fprintf(stderr, "usage: %s <kernel.spv> [M] [N] [K] [iters]\n", argv[0]); return 2; }
    const uint64_t M = argc > 2 ? strtoull(argv[2], NULL, 10) : 4096;
    const uint64_t N = argc > 3 ? strtoull(argv[3], NULL, 10) : 4096;
    const uint64_t K = argc > 4 ? strtoull(argv[4], NULL, 10) : 4096;
    const int iters = argc > 5 ? atoi(argv[5]) : 20;
    // 0 = naive tier (M*N work items). The TENSOR tier's geometry differs
    // — mirror the generated runner: (M*N / 256) * 32 work items.
    uint64_t dispatch_override = argc > 6 ? strtoull(argv[6], NULL, 10) : 0;

    FILE* f = fopen(argv[1], "rb");
    if (f == NULL) { perror("spv"); return 2; }
    fseek(f, 0, SEEK_END);
    long spv_len = ftell(f);
    fseek(f, 0, SEEK_SET);
    uint8_t* spv = malloc((size_t)spv_len);
    if (spv == NULL || fread(spv, 1, (size_t)spv_len, f) != (size_t)spv_len) {
        fprintf(stderr, "short read\n"); return 2;
    }
    fclose(f);

    uint64_t off_a = 0;
    uint64_t off_b = off_a + M * K * 2;
    uint64_t off_i = (off_b + K * N * 2 + 7) & ~(uint64_t)7;
    uint64_t off_y = off_i + 8;
    uint64_t state_bytes = off_y + M * N * 2 + 64;
    state = calloc(1, state_bytes);
    if (state == NULL) { fprintf(stderr, "oom\n"); return 2; }

    BrievField fields[] = {
        { "a", 1, off_a, 2, M * K, 1, off_a },
        { "b", 1, off_b, 2, K * N, 1, off_b },
        { "i", 2, off_i, 8, 1, 0, off_i },
        { "y", 1, off_y, 2, M * N, 1, off_y },
    };
    BrievKernelDesc desc = { "gemm", spv, (uint32_t)spv_len, 4, fields };

    uint16_t* a = (uint16_t*)(state + off_a);
    uint16_t* b = (uint16_t*)(state + off_b);
    uint16_t* y = (uint16_t*)(state + off_y);
    // f16-EXACT seeds: 0.25/0.5 multiples of small ints — the reference is
    // exact and the f32 kernel's single store-rounding is the only error.
    for (uint64_t j = 0; j < M * K; j++) a[j] = f32_to_f16((float)((int)(j % 7)) * 0.25f);
    for (uint64_t j = 0; j < K * N; j++) b[j] = f32_to_f16((float)((int)(j % 5)) * 0.5f);

    if (!briev_accel_init(&desc, 1)) {
        fprintf(stderr, "no GPU device\n"); return 1;
    }
    printf("# fingerprint: device=%s spv=%ldB M=%llu N=%llu K=%llu warmup=%d iters=%d\n",
           briev_accel_device_name(), spv_len,
           (unsigned long long)M, (unsigned long long)N, (unsigned long long)K,
           WARMUP, iters);

    // Dispatch: naive = one work item per output element; a nonzero
    // override mirrors the generated runner's tier geometry.
    uint64_t dispatch_n = dispatch_override != 0 ? dispatch_override : M * N;

    // Warm-up + download (the state after warm-up holds real outputs).
    for (int w = 0; w < WARMUP; w++) {
        *(int64_t*)(state + off_i) = 0;
        if (!briev_accel_launch_resident(0, state, dispatch_n)) {
            fprintf(stderr, "dispatch failed\n"); return 1;
        }
    }
    if (!briev_accel_download(0, state)) {
        fprintf(stderr, "download failed\n"); return 1;
    }

    // Sampled correctness against a double reference. f16 storage rounds
    // once per output — bound the relative error at ~2^-10 (f16 epsilon)
    // plus slack for the ~K-term f32 accumulation.
    double max_rel = 0.0;
    uint64_t rows[VERIFY_ROWS];
    for (int r = 0; r < VERIFY_ROWS; r++) {
        rows[r] = (uint64_t)(r * 7919) % M;
    }
    for (int r = 0; r < VERIFY_ROWS; r++) {
        const uint64_t m = rows[r];
        for (uint64_t n = 0; n < N; n++) {
            double ref = 0.0;
            for (uint64_t k = 0; k < K; k++) {
                ref += f16_to_f64(a[m * K + k]) * f16_to_f64(b[k * N + n]);
            }
            double got = f16_to_f64(y[m * N + n]);
            double rel = ref == 0.0 ? got : fabs(got - ref) / fabs(ref);
            if (rel > max_rel) max_rel = rel;
        }
    }
    printf("# correctness: max_rel_err = %.3e (%s)\n", max_rel,
           max_rel <= 5e-3 ? "OK" : "FAIL");

    // Steady-state perf.
    double sum = 0, mn = 1e30, mx = 0;
    for (int it = 0; it < iters; it++) {
        *(int64_t*)(state + off_i) = 0;
        double t1 = now_ms();
        int ok = briev_accel_launch_resident(0, state, dispatch_n);
        double dt = now_ms() - t1;
        if (!ok) { fprintf(stderr, "dispatch failed\n"); return 1; }
        sum += dt; if (dt < mn) mn = dt; if (dt > mx) mx = dt;
    }
    printf("GPU  avg %.3f ms  min %.3f ms  max %.3f ms  %.2f GFLOP/s\n",
           sum / iters, mn, mx, 2.0 * (double)M * N * K / ((sum / iters) * 1e6));

    briev_accel_shutdown();
    return 0;
}
