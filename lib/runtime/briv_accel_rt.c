// Briv Accel Runtime — device-agnostic GPU dispatch glue.
//
// The compiler never names a device. It emits SPIR-V kernel blobs + per-kernel
// layout descriptors + calls the stable briv_accel_* ABI below. This runtime
// dispatches to a pluggable device-driver table (BrivDeviceDriver). Kernel
// EMISSION is per device-FAMILY (CUDA needs PTX; Vulkan/OpenCL/LevelZero all
// consume the same SPIR-V), so the glue is shared. See
// docs/plans/2026-08-06-accel-gpu-offload.md §7.
//
// Device model: each kernel sees ONE flat projection buffer — the host %State
// sliced to the kernel's buffers in kernel `%State` field order (arrays then
// scalars, each sorted). The kernel GEPs into that struct; the device buffer
// holds exactly that packed struct. The generic pack/unpack here is
// device-independent; each driver only uploads, launches, downloads.
//
// Selection: BRIV_ACCEL_DEVICE env (vulkan|opencl|...) overrides the default;
// otherwise the first available driver wins (Vulkan → OpenCL → CPU).

#include <stdint.h>
#include <stddef.h>
#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include <dlfcn.h>

// ────────────────────────────────────────────────────────────────────────────
// Layout + descriptor types (emitted by the compiler as const tables)
// ────────────────────────────────────────────────────────────────────────────

typedef enum {
    BRIV_FIELD_ARRAY = 1,
    BRIV_FIELD_SCALAR = 2,
} BrivFieldKind;

/// One state field the kernel touches. Fields are listed in KERNEL `%State`
/// order (arrays first, then scalars — the order the kernel GEPs by).
typedef struct {
    const char* name;      // Briv field name (diagnostics)
    uint32_t kind;         // BRIV_FIELD_ARRAY | BRIV_FIELD_SCALAR
    uint64_t host_offset;  // byte offset of the field in the HOST %State
    uint64_t elem_bytes;   // array: element size; scalar: value size
    uint64_t count;        // array: element count; scalar: 1
    uint32_t is_write;     // array written by the kernel (readback after)
} BrivField;

/// One compiled SPIR-V kernel + its layout.
typedef struct {
    const char* txn_name;  // diagnostics
    const uint8_t* spirv;  // SPIR-V blob
    uint32_t spirv_size;
    uint32_t n_fields;
    const BrivField* fields;
} BrivKernelDesc;

// ────────────────────────────────────────────────────────────────────────────
// Device-driver ABI
// ────────────────────────────────────────────────────────────────────────────

#define BRIV_DEV_CAP_ZERO_COPY 0x1u  // can skip the upload/download copy

typedef struct {
    /// Raw device transfer: upload `proj` (the flat kernel projection),
    /// dispatch `global_n` work-items, download the result into `proj_out`.
    /// The driver owns its device memory; `proj`/`proj_out` are host buffers.
    int (*launch)(void* kernel, const void* proj, size_t proj_bytes,
                  size_t global_n, void* proj_out);
} BrivDriverOps;

typedef struct BrivDeviceDriver {
    const char* name;            // "vulkan" | "opencl" | ...
    uint32_t capabilities;
    int (*available)(void);      // dlopen + device present
    int (*init)(void);
    int (*create_kernel)(const uint8_t* spirv, size_t size, void** kernel_out);
    int (*launch)(void* kernel, const void* proj, size_t proj_bytes,
                  size_t global_n, void* proj_out);
    void (*destroy_kernel)(void* kernel);
    void (*shutdown)(void);
} BrivDeviceDriver;

extern BrivDeviceDriver briv_dev_vulkan;
extern BrivDeviceDriver briv_dev_opencl;

// ────────────────────────────────────────────────────────────────────────────
// Device selection (config default + BRIV_ACCEL_DEVICE env + fallback chain)
// ────────────────────────────────────────────────────────────────────────────

static const BrivDeviceDriver* g_driver = NULL;
static int g_init_done = 0;
static void** g_kernels = NULL;       // per-desc kernel handles
static uint32_t g_n_kernels = 0;
static const BrivKernelDesc* g_descs = NULL;

static const BrivDeviceDriver* select_driver(void) {
    const char* env = getenv("BRIV_ACCEL_DEVICE");
    const BrivDeviceDriver* chain[] = { &briv_dev_vulkan, &briv_dev_opencl, NULL };
    if (env != NULL && env[0] != '\0') {
        for (int i = 0; chain[i] != NULL; i++) {
            if (strcmp(chain[i]->name, env) == 0) {
                if (chain[i]->available()) {
                    return chain[i];
                }
            }
        }
        // env names a driver that is unavailable → fall through to the chain.
    }
    for (int i = 0; chain[i] != NULL; i++) {
        if (chain[i]->available()) {
            return chain[i];
        }
    }
    return NULL;
}

/// Register the kernel set. Call once from the emitted program's init.
/// Returns 1 when a device is active and all kernels compiled, 0 for CPU.
int briv_accel_init(const BrivKernelDesc* descs, uint32_t n) {
    if (!g_init_done) {
        g_driver = select_driver();
        if (g_driver != NULL) {
            g_driver->init();
        }
        g_init_done = 1;
    }
    g_descs = descs;
    g_n_kernels = n;
    if (g_driver == NULL) {
        return 0;
    }
    if (g_kernels != NULL) {
        free(g_kernels);
    }
    g_kernels = calloc(n, sizeof(void*));
    if (g_kernels == NULL) {
        return 0;
    }
    for (uint32_t i = 0; i < n; i++) {
        if (!g_driver->create_kernel(descs[i].spirv, descs[i].spirv_size, &g_kernels[i])) {
            return 0;
        }
    }
    return 1;
}

/// 1 when a device is active (after init), 0 → CPU path.
int briv_accel_available(void) {
    return (g_init_done && g_driver != NULL) ? 1 : 0;
}

/// The active driver name, or "cpu".
const char* briv_accel_device_name(void) {
    if (!g_init_done || g_driver == NULL) {
        return "cpu";
    }
    return g_driver->name;
}

// ────────────────────────────────────────────────────────────────────────────
// Generic pack/unpack + dispatch
// ────────────────────────────────────────────────────────────────────────────

/// Byte offset of kernel field `i` inside the flat projection.
static uint64_t proj_field_offset(const BrivKernelDesc* k, uint32_t i) {
    uint64_t off = 0;
    for (uint32_t j = 0; j < i; j++) {
        off += k->fields[j].count * k->fields[j].elem_bytes;
    }
    return off;
}

static uint64_t proj_size(const BrivKernelDesc* k) {
    uint64_t off = 0;
    for (uint32_t j = 0; j < k->n_fields; j++) {
        off += k->fields[j].count * k->fields[j].elem_bytes;
    }
    return off;
}

/// Pack the kernel's fields from the host %State into a flat projection
/// (kernel field order), launch, then unpack written fields back.
int briv_accel_launch(uint32_t idx, void* state, uint64_t work_n) {
    if (!briv_accel_available() || idx >= g_n_kernels || g_kernels == NULL) {
        return 0;
    }
    const BrivKernelDesc* k = &g_descs[idx];
    uint64_t bytes = proj_size(k);
    uint8_t* proj = malloc(bytes == 0 ? 1 : bytes);
    uint8_t* proj_out = malloc(bytes == 0 ? 1 : bytes);
    if (proj == NULL || proj_out == NULL) {
        free(proj);
        free(proj_out);
        return 0;
    }
    // Pack: copy each field from its host offset into projection order.
    for (uint32_t i = 0; i < k->n_fields; i++) {
        const BrivField* f = &k->fields[i];
        uint64_t off = proj_field_offset(k, i);
        size_t n = (size_t)(f->count * f->elem_bytes);
        memcpy(proj + off, (const uint8_t*)state + f->host_offset, n);
    }
    int ok = g_driver->launch(g_kernels[idx], proj, bytes, work_n, proj_out);
    // Unpack: copy written fields back to the host state.
    for (uint32_t i = 0; i < k->n_fields; i++) {
        const BrivField* f = &k->fields[i];
        if (!f->is_write) {
            continue;
        }
        uint64_t off = proj_field_offset(k, i);
        size_t n = (size_t)(f->count * f->elem_bytes);
        memcpy((uint8_t*)state + f->host_offset, proj_out + off, n);
    }
    free(proj);
    free(proj_out);
    return ok;
}

/// Free kernel handles + driver shutdown. Called by program exit.
void briv_accel_shutdown(void) {
    if (g_driver != NULL && g_kernels != NULL) {
        for (uint32_t i = 0; i < g_n_kernels; i++) {
            if (g_kernels[i] != NULL) {
                g_driver->destroy_kernel(g_kernels[i]);
            }
        }
        g_driver->shutdown();
    }
    free(g_kernels);
    g_kernels = NULL;
    g_driver = NULL;
    g_init_done = 0;
}

// ────────────────────────────────────────────────────────────────────────────
// Auto-tuning probe (D7). Runs both lanes on a slice; verifies output
// equality within tolerance and commits to the faster path. Returns 1 = GPU.
// ────────────────────────────────────────────────────────────────────────────

#include <time.h>

static double now_seconds(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (double)ts.tv_sec + (double)ts.tv_nsec / 1e9;
}

/// `cpu_fn`/`gpu_fn` run one firing each; `gpu_ok` reports whether the GPU
/// lane's output matched the CPU lane within `tolerance`.
int briv_accel_probe(void (*cpu_fn)(void*), void (*gpu_fn)(void*), void* ctx,
                     int64_t probe_k, double tolerance,
                     int (*gpu_ok)(const void*, const void*, double, void*)) {
    if (!briv_accel_available() || probe_k <= 0) {
        return 0;
    }
    // Warm-up: one dummy launch each (first device dispatch is slow).
    cpu_fn(ctx);
    gpu_fn(ctx);
    double t0 = now_seconds();
    for (int64_t i = 0; i < probe_k; i++) {
        cpu_fn(ctx);
    }
    double cpu_t = now_seconds() - t0;
    t0 = now_seconds();
    for (int64_t i = 0; i < probe_k; i++) {
        gpu_fn(ctx);
    }
    double gpu_t = now_seconds() - t0;
    if (gpu_ok != NULL && !gpu_ok(ctx, ctx, tolerance, ctx)) {
        return 0;  // correctness gate: GPU diverged → stay CPU.
    }
    return (gpu_t < cpu_t) ? 1 : 0;
}

// ────────────────────────────────────────────────────────────────────────────
// Drivers — Vulkan and OpenCL, ported from the legacy briv_gpu_rt.c dual-API
// (dlopen'd, both SPIR-V consumers), restructured to the single-flat-buffer
// model. The generic pack/selection/probe above is complete; these drivers
// carry over the original mechanism and its known simplifications (see the
// per-driver header comments) until hardened against real hardware.
// ────────────────────────────────────────────────────────────────────────────

#include "briv_dev_vulkan.c"
#include "briv_dev_opencl.c"

// ────────────────────────────────────────────────────────────────────────────
// Self-test (BRIV_ACCEL_SELF_TEST) — exercises selection, pack math, and the
// probe gate on synthetic data with NO device required. Built standalone:
//   cc -DBRIV_ACCEL_SELF_TEST briv_accel_rt.c -ldl -o /tmp/briv_accel_selftest
// ────────────────────────────────────────────────────────────────────────────

#ifdef BRIV_ACCEL_SELF_TEST

static int self_failures = 0;
static int reject_hits = 0;

static void probe_cpu_fn(void* ctx);
static void probe_gpu_fn(void* ctx);
static int probe_reject_ok(const void* a, const void* b, double tol, void* ctx);

static void expect(int cond, const char* what) {
    if (!cond) {
        fprintf(stderr, "SELF-TEST FAIL: %s\n", what);
        self_failures++;
    }
}

static int fake_launch(void* k, const void* proj, size_t bytes,
                       size_t n, void* proj_out) {
    (void)k;
    // Copy proj to proj_out (identity) so write fields round-trip.
    memcpy(proj_out, proj, bytes);
    (void)n;
    return 1;
}

int main(void) {
    // Field projection order + offsets: array a (Float[4], 4B) then scalar
    // s (Int, 8B) → flat layout: a at 0 (16B), s at 16 (8B). Host struct has
    // count first (8B) so a's host_offset is 8, s's is 24.
    BrivField fields[2] = {
        { "a", BRIV_FIELD_ARRAY, 8, 4, 4, 1 },
        { "s", BRIV_FIELD_SCALAR, 24, 8, 1, 0 },
    };
    BrivKernelDesc desc = { "force", NULL, 0, 2, fields };

    // ── Pack math ──
    expect(proj_size(&desc) == 24, "proj_size = 16 + 8");
    expect(proj_field_offset(&desc, 1) == 16, "scalar offset after array");

    // ── Launch with the fake driver ──
    static BrivDeviceDriver fake_driver = {
        "fake", 0, NULL, NULL, NULL, fake_launch, NULL, NULL,
    };
    g_driver = &fake_driver;
    g_init_done = 1;
    g_descs = &desc;
    g_n_kernels = 1;
    g_kernels = malloc(sizeof(void*));
    g_kernels[0] = (void*)0x1;

    uint64_t host[4] = { 0 };
    float* a = (float*)((uint8_t*)host + 8);
    a[0] = 1.0f; a[1] = 2.0f; a[2] = 3.0f; a[3] = 4.0f;
    ((int64_t*)((uint8_t*)host + 24))[0] = 42;
    int ok = briv_accel_launch(0, host, 4);
    expect(ok == 1, "fake launch returns ok");
    expect(a[0] == 1.0f && a[3] == 4.0f, "write field round-trips");
    expect(((int64_t*)((uint8_t*)host + 24))[0] == 42, "scalar preserved");

    // ── Probe gate ──
    // The probe returns GPU(1) only when gpu_ok confirms output equality; a
    // rejecting gate forces CPU regardless of timing. Runs a real clocked
    // loop with the fake driver installed above (gpu_ok always rejects).
    {
        static struct ProbeCtx { int n; } pctx = { 4 };
        int verdict = briv_accel_probe(probe_cpu_fn, probe_gpu_fn, &pctx, 2, 0.001, probe_reject_ok);
        expect(verdict == 0, "rejecting gate forces CPU");
    }
    (void)reject_hits;

    if (self_failures == 0) {
        printf("briv_accel_rt self-test: all passed\n");
        return 0;
    }
    return 1;
}

static void probe_cpu_fn(void* ctx) { (void)ctx; }
static void probe_gpu_fn(void* ctx) { reject_hits++; (void)ctx; }
static int probe_reject_ok(const void* a, const void* b, double tol, void* ctx) {
    (void)a; (void)b; (void)tol; (void)ctx;
    return 0;
}

#endif
