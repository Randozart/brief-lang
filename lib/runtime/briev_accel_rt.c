// Briev Accel Runtime — device-agnostic GPU dispatch glue.
//
// The compiler never names a device. It emits SPIR-V kernel blobs + per-kernel
// layout descriptors + calls the stable briev_accel_* ABI below. This runtime
// dispatches to a pluggable device-driver table (BrievDeviceDriver). Kernel
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
// Selection: BRIEV_ACCEL_DEVICE env (vulkan|opencl|...) overrides the default;
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
    BRIEV_FIELD_ARRAY = 1,
    BRIEV_FIELD_SCALAR = 2,
} BrievFieldKind;

/// One state field the kernel touches. Fields are listed in KERNEL `%State`
/// order (arrays first, then scalars — the order the kernel GEPs by).
typedef struct {
    const char* name;      // Briev field name (diagnostics)
    uint32_t kind;         // BRIEV_FIELD_ARRAY | BRIEV_FIELD_SCALAR
    uint64_t host_offset;  // byte offset of the field in the HOST %State
    uint64_t elem_bytes;   // array: element size; scalar: value size
    uint64_t count;        // array: element count; scalar: 1
    uint32_t is_write;     // array written by the kernel (readback after)
} BrievField;

/// One compiled SPIR-V kernel + its layout.
typedef struct {
    const char* txn_name;  // diagnostics
    const uint8_t* spirv;  // SPIR-V blob
    uint32_t spirv_size;
    uint32_t n_fields;
    const BrievField* fields;
} BrievKernelDesc;

// ────────────────────────────────────────────────────────────────────────────
// Device-driver ABI
// ────────────────────────────────────────────────────────────────────────────

#define BRIEV_DEV_CAP_ZERO_COPY 0x1u  // can skip the upload/download copy

typedef struct {
    /// Raw device transfer: upload `proj` (the flat kernel projection),
    /// dispatch `global_n` work-items, download the result into `proj_out`.
    /// The driver owns its device memory; `proj`/`proj_out` are host buffers.
    int (*launch)(void* kernel, const void* proj, size_t proj_bytes,
                  size_t global_n, void* proj_out);
} BrievDriverOps;

typedef struct BrievDeviceDriver {
    const char* name;            // "vulkan" | "opencl" | ...
    uint32_t capabilities;
    int (*available)(void);      // dlopen + device present
    int (*init)(void);
    int (*create_kernel)(const uint8_t* spirv, size_t size, void** kernel_out);
    int (*launch)(void* kernel, const void* proj, size_t proj_bytes,
                  size_t global_n, void* proj_out);
    void (*destroy_kernel)(void* kernel);
    void (*shutdown)(void);
    // 2026-08-31 (plan item 3, device residency): optional. `mapped` returns
    // the kernel's persistent host-visible projection pointer (NULL if the
    // driver cannot keep it mapped); `launch_dev` records dispatch + submit
    // + fence-wait with NO host copies — the runtime drives the mapped
    // projection itself. NULL → the runtime falls back to `launch`.
    void* (*mapped)(void* kernel);
    int (*launch_dev)(void* kernel, size_t global_n);
    // 2026-08-31 (plan 2026-08-31-gpu-next §2b): 2D dispatch — `nx` work
    // items per row, `ny` rows. `full_sync` pushes the ENTIRE staging
    // projection to the device working set first (the seed); otherwise only
    // the `dirty` (offset, bytes) pairs cross (the scalar counters). NULL →
    // the runtime falls back to the full-copy launch.
    int (*launch_dev2d)(void* kernel, size_t nx, size_t ny,
                        int full_sync, const size_t* dirty, uint32_t n_dirty);
    // Pull the device working set into the staging window (device residency
    // download). NULL → the staging window is the source of truth already.
    int (*download_dev)(void* kernel);
} BrievDeviceDriver;

extern BrievDeviceDriver briev_dev_vulkan;
extern BrievDeviceDriver briev_dev_opencl;

int briev_accel_launch_resident_2d(uint32_t idx, void* state,
                                   uint64_t nx, uint64_t ny);

// ────────────────────────────────────────────────────────────────────────────
// Device selection (config default + BRIEV_ACCEL_DEVICE env + fallback chain)
// ────────────────────────────────────────────────────────────────────────────

static const BrievDeviceDriver* g_driver = NULL;
static int g_init_done = 0;
static uint8_t* g_resident_seeded = NULL;   // per-kernel first-launch flag (residency)
// 2026-08-31: shared with the #included device drivers (single TU) — they
// read it for BRIEV_ACCEL_VERBOSE diagnostics.
static int g_verbose = 0;
static void** g_kernels = NULL;       // per-desc kernel handles
static uint32_t g_n_kernels = 0;
static const BrievKernelDesc* g_descs = NULL;

static const BrievDeviceDriver* select_driver(void) {
    const char* env = getenv("BRIEV_ACCEL_DEVICE");
    const BrievDeviceDriver* chain[] = { &briev_dev_vulkan, &briev_dev_opencl, NULL };
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
/// 2026-08-31 (plan abv-gpu-by-default): a failed device init or kernel
/// compile now marks the chain DEAD (available()==0 → clean CPU fallback).
/// Previously init()'s return was ignored and a failed create_kernel left
/// available()==1 — the GPU lane was "chosen" and launches no-op'd, and
/// there was no way to see why. BRIEV_ACCEL_VERBOSE=1 prints the reason.
int briev_accel_init(const BrievKernelDesc* descs, uint32_t n) {
    g_verbose = getenv("BRIEV_ACCEL_VERBOSE") != NULL;
    int verbose = g_verbose;
    if (!g_init_done) {
        g_driver = select_driver();
        if (g_driver != NULL) {
            if (!g_driver->init()) {
                if (verbose) {
                    fprintf(stderr, "[briev_accel] driver '%s' init failed — CPU fallback\n",
                            g_driver->name);
                }
                g_driver = NULL;
            }
        } else if (verbose) {
            fprintf(stderr, "[briev_accel] no device driver available — CPU fallback\n");
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
    free(g_resident_seeded);
    g_resident_seeded = calloc(n, 1);
    for (uint32_t i = 0; i < n; i++) {
        // 2026-08-31: an EMPTY blob is a per-kernel CPU fallback slot (the
        // compiler keeps descriptor indices stable) — skip, don't fail all.
        if (descs[i].spirv_size == 0) {
            if (verbose) {
                fprintf(stderr, "[briev_accel] kernel '%s' has no binary — CPU lane\n",
                        descs[i].txn_name);
            }
            g_kernels[i] = NULL;
            continue;
        }
        if (!g_driver->create_kernel(descs[i].spirv, descs[i].spirv_size, &g_kernels[i])) {
            if (verbose) {
                fprintf(stderr, "[briev_accel] kernel '%s' rejected by driver '%s' — CPU fallback\n",
                        descs[i].txn_name, g_driver->name);
            }
            g_driver = NULL;
            return 0;
        } else if (verbose) {
            fprintf(stderr, "[briev_accel] kernel '%s' compiled on '%s'\n",
                    descs[i].txn_name, g_driver->name);
        }
    }
    return 1;
}

/// 1 when a device is active (after init), 0 → CPU path.
int briev_accel_available(void) {
    return (g_init_done && g_driver != NULL) ? 1 : 0;
}

/// The active driver name, or "cpu".
const char* briev_accel_device_name(void) {
    if (!g_init_done || g_driver == NULL) {
        return "cpu";
    }
    return g_driver->name;
}

// ────────────────────────────────────────────────────────────────────────────
// Generic pack/unpack + dispatch
// ────────────────────────────────────────────────────────────────────────────

/// Byte offset of kernel field `i` inside the flat projection.
static uint64_t proj_field_offset(const BrievKernelDesc* k, uint32_t i) {
    uint64_t off = 0;
    for (uint32_t j = 0; j < i; j++) {
        off += k->fields[j].count * k->fields[j].elem_bytes;
    }
    return off;
}

static uint64_t proj_size(const BrievKernelDesc* k) {
    uint64_t off = 0;
    for (uint32_t j = 0; j < k->n_fields; j++) {
        off += k->fields[j].count * k->fields[j].elem_bytes;
    }
    return off;
}

/// Pack the kernel's fields from the host %State into a flat projection
/// (kernel field order), launch, then unpack written fields back.
int briev_accel_launch(uint32_t idx, void* state, uint64_t work_n) {
    if (!briev_accel_available() || idx >= g_n_kernels || g_kernels == NULL
        || g_kernels[idx] == NULL) {
        return 0;
    }
    const BrievKernelDesc* k = &g_descs[idx];
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
        const BrievField* f = &k->fields[i];
        uint64_t off = proj_field_offset(k, i);
        size_t n = (size_t)(f->count * f->elem_bytes);
        memcpy(proj + off, (const uint8_t*)state + f->host_offset, n);
    }
    int ok = g_driver->launch(g_kernels[idx], proj, bytes, work_n, proj_out);
    // Unpack: copy written fields back to the host state.
    for (uint32_t i = 0; i < k->n_fields; i++) {
        const BrievField* f = &k->fields[i];
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

// ────────────────────────────────────────────────────────────────────────────
// Device residency (2026-08-31, plan abv-gpu-by-default item 3): iterative
// kernels keep their array state ON the device across launches. Only scalar
// fields (counters, phase gates) cross PCIe each step. `launch_resident`
// seeds all fields on the first call, then syncs scalars both ways;
// `download` pulls the full projection back at the end. Requires the driver
// to expose its persistent mapped projection (Vulkan does; otherwise falls
// back to the full-copy launch).
// ────────────────────────────────────────────────────────────────────────────

int briev_accel_launch_resident(uint32_t idx, void* state, uint64_t work_n) {
    return briev_accel_launch_resident_2d(idx, state, work_n, 1);
}

// 2D resident launch (plan 2026-08-31-gpu-next §2b): ny == 1 is the flat
// form. Scalars sync host→device as dirty byte ranges; the first launch
// seeds the full projection. Everything else stays in VRAM.
int briev_accel_launch_resident_2d(uint32_t idx, void* state,
                                   uint64_t nx, uint64_t ny) {
    if (!briev_accel_available() || idx >= g_n_kernels || g_kernels == NULL
        || g_kernels[idx] == NULL) {
        return 0;
    }
    if (g_driver->launch_dev2d == NULL || g_driver->mapped == NULL
        || g_driver->launch_dev == NULL) {
        fprintf(stderr, "[briev_accel] DBG fallback: 2d=%p mapped=%p dev=%p\n",
                (void*)(size_t)!!g_driver->launch_dev2d,
                (void*)(size_t)!!g_driver->mapped,
                (void*)(size_t)!!g_driver->launch_dev);
        return briev_accel_launch(idx, state, nx * ny);  // driver can't
    }
    void* mapped = g_driver->mapped(g_kernels[idx]);
    if (mapped == NULL) {
        return briev_accel_launch(idx, state, nx * ny);
    }
    const BrievKernelDesc* k = &g_descs[idx];
    int full_sync = 0;
    size_t dirty[2 * 16];
    uint32_t n_dirty = 0;
    if (!g_resident_seeded[idx]) {
        full_sync = 1;
        for (uint32_t i = 0; i < k->n_fields; i++) {
            const BrievField* f = &k->fields[i];
            memcpy(mapped + proj_field_offset(k, i),
                   (const uint8_t*)state + f->host_offset,
                   (size_t)(f->count * f->elem_bytes));
        }
        g_resident_seeded[idx] = 1;
    } else {
        // Scalars only: the host's counters/phase gates are authoritative
        // between launches (the phase machine runs on the host).
        for (uint32_t i = 0; i < k->n_fields && n_dirty < 16; i++) {
            const BrievField* f = &k->fields[i];
            if (f->kind != BRIEV_FIELD_SCALAR) {
                continue;
            }
            uint64_t off = proj_field_offset(k, i);
            dirty[2 * n_dirty] = off;
            dirty[2 * n_dirty + 1] = f->elem_bytes;
            n_dirty++;
            memcpy(mapped + off,
                   (const uint8_t*)state + f->host_offset,
                   (size_t)f->elem_bytes);
        }
    }
    // No device→host scalar sync: the host owns the scalars (they were just
    // uploaded); arrays stay device-resident until briev_accel_download.
    return g_driver->launch_dev2d(g_kernels[idx], nx, ny, full_sync,
                                  dirty, n_dirty);
}

/// Pull the FULL projection back to the host state (end of a resident run —
/// observables read host state). Returns 0 when residency isn't active for
/// `idx` (the caller's state is then already current from full-copy launches).
int briev_accel_download(uint32_t idx, void* state) {    if (!briev_accel_available() || idx >= g_n_kernels || g_kernels == NULL
        || g_kernels[idx] == NULL || !g_resident_seeded[idx]) {
        return 0;
    }
    void* mapped = g_driver->mapped(g_kernels[idx]);
    if (mapped == NULL) {
        return 0;
    }
    const BrievKernelDesc* k = &g_descs[idx];
    for (uint32_t i = 0; i < k->n_fields; i++) {
        const BrievField* f = &k->fields[i];
        memcpy((uint8_t*)state + f->host_offset,
               mapped + proj_field_offset(k, i),
               (size_t)(f->count * f->elem_bytes));
    }
    return 1;
}

/// Free kernel handles + driver shutdown. Called by program exit.
void briev_accel_shutdown(void) {    if (g_driver != NULL && g_kernels != NULL) {
        for (uint32_t i = 0; i < g_n_kernels; i++) {
            if (g_kernels[i] != NULL) {
                g_driver->destroy_kernel(g_kernels[i]);
            }
        }
        g_driver->shutdown();
    }
    free(g_resident_seeded);
    g_resident_seeded = NULL;
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

/// Auto-tuning probe (D7): runs the CPU and GPU lanes on SEPARATE state
/// copies, times each over `probe_k` full-map runs, and commits to the GPU
/// path only when its wall time beats CPU by the margin AND `gpu_ok`
/// confirms the outputs match within `tolerance`. Returns 1 = GPU, 0 = CPU.
/// `state_size` is the host %State byte count (the compiler emits it).
int briev_accel_probe(void (*cpu_fn)(void*), void (*gpu_fn)(void*), void* ctx,
                     uint64_t state_size, int64_t probe_k, double tolerance,
                     double margin,
                     int (*gpu_ok)(const void*, const void*, double, void*)) {
    if (!briev_accel_available() || probe_k <= 0) {
        return 0;
    }
    uint8_t* cpu_state = malloc(state_size == 0 ? 1 : state_size);
    uint8_t* gpu_state = malloc(state_size == 0 ? 1 : state_size);
    if (cpu_state == NULL || gpu_state == NULL) {
        free(cpu_state);
        free(gpu_state);
        return 0;
    }
    memcpy(cpu_state, ctx, state_size);
    memcpy(gpu_state, ctx, state_size);

    // Warm-up: one dummy full-map run each (first device dispatch is slow).
    cpu_fn(cpu_state);
    gpu_fn(gpu_state);

    double t0 = now_seconds();
    for (int64_t i = 0; i < probe_k; i++) {
        cpu_fn(cpu_state);
    }
    double cpu_t = now_seconds() - t0;

    t0 = now_seconds();
    for (int64_t i = 0; i < probe_k; i++) {
        gpu_fn(gpu_state);
    }
    double gpu_t = now_seconds() - t0;

    // Correctness gate: the GPU lane's result must match the CPU lane's within
    // tolerance — the probe doubles as the safety net against GPU codegen bugs.
    if (gpu_ok != NULL && !gpu_ok(cpu_state, gpu_state, tolerance, ctx)) {
        free(cpu_state);
        free(gpu_state);
        return 0;
    }
    free(cpu_state);
    free(gpu_state);
    return (gpu_t * (1.0 + margin) < cpu_t) ? 1 : 0;
}

// ────────────────────────────────────────────────────────────────────────────
// Drivers — Vulkan and OpenCL, ported from the legacy briev_gpu_rt.c dual-API
// (dlopen'd, both SPIR-V consumers), restructured to the single-flat-buffer
// model. The generic pack/selection/probe above is complete; these drivers
// carry over the original mechanism and its known simplifications (see the
// per-driver header comments) until hardened against real hardware.
// ────────────────────────────────────────────────────────────────────────────

#include "briev_dev_vulkan.c"
#include "briev_dev_opencl.c"

// ────────────────────────────────────────────────────────────────────────────
// Self-test (BRIEV_ACCEL_SELF_TEST) — exercises selection, pack math, and the
// probe gate on synthetic data with NO device required. Built standalone:
//   cc -DBRIEV_ACCEL_SELF_TEST briev_accel_rt.c -ldl -o /tmp/briev_accel_selftest
// ────────────────────────────────────────────────────────────────────────────

#ifdef BRIEV_ACCEL_SELF_TEST

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
    BrievField fields[2] = {
        { "a", BRIEV_FIELD_ARRAY, 8, 4, 4, 1 },
        { "s", BRIEV_FIELD_SCALAR, 24, 8, 1, 0 },
    };
    BrievKernelDesc desc = { "force", NULL, 0, 2, fields };

    // ── Pack math ──
    expect(proj_size(&desc) == 24, "proj_size = 16 + 8");
    expect(proj_field_offset(&desc, 1) == 16, "scalar offset after array");

    // ── Launch with the fake driver ──
    static BrievDeviceDriver fake_driver = {
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
    int ok = briev_accel_launch(0, host, 4);
    expect(ok == 1, "fake launch returns ok");
    expect(a[0] == 1.0f && a[3] == 4.0f, "write field round-trips");
    expect(((int64_t*)((uint8_t*)host + 24))[0] == 42, "scalar preserved");

    // ── Probe gate ──
    // The probe returns GPU(1) only when gpu_ok confirms output equality; a
    // rejecting gate forces CPU regardless of timing. Runs a real clocked
    // loop with the fake driver installed above (gpu_ok always rejects).
    {
        static struct ProbeCtx { int n; } pctx = { 4 };
        int verdict = briev_accel_probe(probe_cpu_fn, probe_gpu_fn, &pctx,
                                       sizeof(pctx), 2, 0.001, 0.05,
                                       probe_reject_ok);
        expect(verdict == 0, "rejecting gate forces CPU");
    }
    (void)reject_hits;

    if (self_failures == 0) {
        printf("briev_accel_rt self-test: all passed\n");
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
