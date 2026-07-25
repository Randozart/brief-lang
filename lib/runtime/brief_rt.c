/*
 * brief_rt.c — Minimal runtime for Brief LLVM backend
 *
 * 2026-07-15: Stripped ~70 brief_* wrapper functions that were replaced
 * by SysCall#/SysConf#/Atomic*# intrinsics. Only keeps infrastructure
 * functions (__rt_init, __rt_wait, barriers, threads, triggers) and
 * the two remaining intrinsics: brief_syscall, brief_sysconf.
 */

#define _GNU_SOURCE
#include <stddef.h>
#include <stdint.h>
#include <signal.h>
#include <time.h>
#include <string.h>
#include <errno.h>
#include <stdio.h>
#include <stdlib.h>
#include <math.h>
#include <dirent.h>
#include <unistd.h>
#include <fcntl.h>
#include <pthread.h>
#include <sys/stat.h>
#include <sys/syscall.h>
#ifdef __linux__
#include <sys/utsname.h>
#include <sys/random.h>
#include <execinfo.h>
#endif
#ifdef __APPLE__
#include <mach/mach_time.h>
#include <sys/sysctl.h>
#endif
#include <sys/ioctl.h>

// ── Integer type for Brief C ABI ──────────────────────────────────────
#ifndef _BRIEF_INT_DEFINED
#define _BRIEF_INT_DEFINED
#if defined(__LP64__) || defined(_WIN64)
typedef int64_t brief_int;
#else
typedef int32_t brief_int;
#endif
#endif

// ── String conversion helpers (internal) ──────────────────────────────
// 2026-07-19: Convert a Brief handle or C string pointer to a C string.
// With SSO, the argument may be an SSO handle (bit 0 = 1), a Brief heap
// handle (pointer to [length][data]), or a raw C string pointer (null-term).
// Detects C strings by checking if the value looks like a valid pointer
// (not SSO, not a small integer, starts with printable ASCII).
// Returns a heap-allocated string; caller must free(). NULL on error.
char* brief_str_to_c(int64_t handle) {
    // 2026-07-22: Strip tag bits (bottom 2 bits) — they mark temporary
    // flags (bit 0 = SSO inline, bit 1 = temporary concat result).
    // After stripping, we have the raw data pointer.
    int64_t ptr = handle & ~3ULL;
    if (handle & 1) {
        // SSO string — inline data packed at bits >= 3
        // The LLVM SSO encoding: handle0 = (raw_data << 3) | 1
        // where raw_data has bytes packed in LE order.
        uint64_t raw_data = ((uint64_t)handle) >> 3;
        int64_t len = 0;
        for (int i = 0; i < 6; i++) {
            if ((raw_data >> (i * 8)) & 0xFF) len = i + 1;
        }
        if (len == 0) len = 1;
        char* c_str = malloc((size_t)(len + 1));
        if (!c_str) return NULL;
        for (int64_t i = 0; i < len; i++) {
            c_str[i] = (char)((raw_data >> (i * 8)) & 0xFF);
        }
        c_str[len] = '\0';
        return c_str;
    }
    // Check for C string pointer: ptr looks like a valid pointer
    // (not zero, not a small integer, first byte is printable ASCII).
    if (ptr > 4096 && ptr < 0x800000000000) {
        uint8_t first = *(uint8_t*)(uintptr_t)ptr;
        if (first >= 32 && first < 127) {
            // Looks like a C string — strlen it
            int64_t len = (int64_t)strlen((const char*)(uintptr_t)ptr);
            if (len > 0 && len < 4096) {
                char* c_str = malloc((size_t)(len + 1));
                if (!c_str) return NULL;
                memcpy(c_str, (void*)(uintptr_t)ptr, (size_t)len);
                c_str[len] = '\0';
                return c_str;
            }
        }
    }
    // Heap Brief string: ptr is a pointer to [8-byte length][data].
    if (ptr == 0) return NULL;
    int64_t len = *(int64_t*)(uintptr_t)ptr;
    if (len < 0 || len > 1024 * 1024 * 1024) return NULL;
    char* c_str = malloc((size_t)(len + 1));
    if (!c_str) return NULL;
    if (len > 0) memcpy(c_str, (void*)(uintptr_t)(ptr + 8), (size_t)len);
    c_str[len] = '\0';
    return c_str;
}

/// Convert a C string (null-terminated) to a Brief string handle.
/// Returns a heap-allocated Brief string (8-byte length prefix + data).
/// Caller should free via brief_free_brief_str().
int64_t brief_cstr_to_brief(const char* c_str) {
    if (!c_str) return 0;
    int64_t len = (int64_t)strlen(c_str);
    if (len > 1024 * 1024 * 1024) return 0; // sanity check
    // Allocate: 8 bytes for length + len bytes for data + 1 for null terminator
    char* buf = (char*)malloc((size_t)(len + 9));
    if (!buf) return 0;
    *(int64_t*)buf = len;               // write length prefix
    if (len > 0) memcpy(buf + 8, c_str, (size_t)len);
    buf[8 + len] = '\0';                // null terminator for C compatibility
    return (int64_t)(uintptr_t)buf;
}

/// Free a Brief string allocated by brief_cstr_to_brief or similar.
void brief_free_brief_str(int64_t handle) {
    if (handle) free((void*)(uintptr_t)handle);
}

// ── Core intrinsics (kept) ────────────────────────────────────────────

// 2026-07-19: Returns the environ pointer (char **environ) as an Int.
// Used by pure-Brief getenv to scan the environment block.
int64_t __get_environ(void) {
    extern char **environ;
    return (int64_t)(uintptr_t)environ;
}

// 2026-07-19: Returns the value of an env var as a heap-allocated Brief string
// (null-terminated UTF-8 data preceded by 8-byte length header).
// Caller takes ownership of the returned pointer.
int64_t __getenv_brief(int64_t key_bstr) {
    char* c_key = brief_str_to_c(key_bstr);
    if (!c_key) return 0;
    char* val = getenv(c_key);
    free(c_key);
    if (!val) return 0;
    int64_t len = (int64_t)strlen(val);
    int64_t* bstr = (int64_t*)malloc((size_t)(len + 8 + 1));
    if (!bstr) return 0;
    bstr[0] = len;
    memcpy((char*)(bstr + 1), val, (size_t)len);
    ((char*)(bstr + 1))[len] = '\0';
    return (int64_t)(uintptr_t)bstr;
}

// 2026-07-19: Returns the value of an env var parsed as Int.
int64_t __getenv_int(int64_t key_bstr) {
    char* c_key = brief_str_to_c(key_bstr);
    if (!c_key) return 0;
    char* val = getenv(c_key);
    free(c_key);
    if (!val) return 0;
    return atol(val);
}

int64_t brief_syscall(int64_t num, int64_t a1, int64_t a2, int64_t a3, int64_t a4, int64_t a5, int64_t a6) {
    return syscall((long)num, (long)a1, (long)a2, (long)a3, (long)a4, (long)a5, (long)a6);
}

int64_t brief_sysconf(int64_t name) {
    return sysconf((int)name);
}

// ── Print / Exit runtime (used by LLVM codegen) ───────────────────────

int64_t __print(int64_t msg_bstr) {
    char* c_msg = brief_str_to_c(msg_bstr);
    if (c_msg) { fputs(c_msg, stdout); free(c_msg); }
    return 0;
}

int64_t __print_int(int64_t n) {
    printf("%ld", (long)n);
    return 0;
}

int64_t __print_float(float f) {
    printf("%g", (double)f);
    return 0;
}

// 2026-07-21: always_inline + LTO enables inlining into main() hot loop.
__attribute__((always_inline)) int64_t __print_char(int64_t c) {
    if (c == 10) {
        puts("");
    } else {
        putchar((int)c);
    }
    return 0;
}

void __exit(int64_t code) {
    exit((int)code);
}

// ── Timer infrastructure (used by trigger system) ─────────────────────

int32_t __trg_timerfd_open(int64_t hz) {
    return 0;
}

int32_t __trg_timerfd_read(int32_t fd) {
    (void)fd;
    return 0;
}

int32_t __trg_signalfd_open(const char* name) {
    (void)name;
    return 0;
}

int32_t __trg_signalfd_read(int32_t fd) {
    (void)fd;
    return 0;
}

// ── Async runtime infrastructure ─────────────────────────────────────
// Forward declaration for atexit cleanup handler
void brief_thread_pool_shutdown(void);
// 2026-07-17: Real thread pool implementation using pthreads.
// Protocol:
//   1. __thread_pool_init__ creates N worker threads, each pinned to a
//      function pointer from the fn_ptrs array.
//   2. Each tick: main calls __set_async_state__, __barrier_release__
//      (workers run their body), then reactor_tick + __barrier_wait__.
//   3. brief_thread_pool_shutdown joins all workers.

typedef struct {
    unsigned id;
    void (*fn)(void*);
} WorkerArg;

static pthread_t* workers = NULL;
static WorkerArg* worker_args = NULL;
static unsigned worker_count = 0;
static void* async_state = NULL;

static pthread_mutex_t barrier_mutex = PTHREAD_MUTEX_INITIALIZER;
static pthread_cond_t barrier_cond = PTHREAD_COND_INITIALIZER;
static volatile int barrier_phase = 0;  // 0 = wait, 1 = go
static volatile int workers_done = 0;

static void* worker_thread(void* arg) {
    WorkerArg* wa = (WorkerArg*)arg;
    while (1) {
        pthread_mutex_lock(&barrier_mutex);
        while (barrier_phase == 0) {
            pthread_cond_wait(&barrier_cond, &barrier_mutex);
        }
        // barrier_phase == 1: workers are released
        pthread_mutex_unlock(&barrier_mutex);

        // Execute the async body function with the current state
        if (wa->fn && async_state) {
            wa->fn(async_state);
        }

        // Signal completion
        pthread_mutex_lock(&barrier_mutex);
        workers_done++;
        pthread_cond_signal(&barrier_cond);
        pthread_mutex_unlock(&barrier_mutex);
    }
    return NULL;
}

// 2026-07-17: Thread pool shutdown registered as atexit handler so worker
// threads are cleaned up when main() returns.
static void __rt_cleanup(void) {
    brief_thread_pool_shutdown();
}

void __rt_init(void) {
    signal(SIGPIPE, SIG_IGN);
    atexit(__rt_cleanup);
}

void __set_async_state__(void* state) {
    async_state = state;
}

void __thread_pool_init__(unsigned num_workers, void** fn_ptrs) {
    if (num_workers == 0) return;
    worker_count = num_workers;
    workers = (pthread_t*)calloc(num_workers, sizeof(pthread_t));
    worker_args = (WorkerArg*)calloc(num_workers, sizeof(WorkerArg));
    for (unsigned i = 0; i < num_workers; i++) {
        worker_args[i].id = i;
        worker_args[i].fn = (void (*)(void*))fn_ptrs[i];
        pthread_create(&workers[i], NULL, worker_thread, &worker_args[i]);
    }
}

void __barrier_release__(void) {
    pthread_mutex_lock(&barrier_mutex);
    workers_done = 0;
    barrier_phase = 1;
    pthread_cond_broadcast(&barrier_cond);
    pthread_mutex_unlock(&barrier_mutex);
}

void __barrier_wait__(void) {
    pthread_mutex_lock(&barrier_mutex);
    while (workers_done < worker_count) {
        pthread_cond_wait(&barrier_cond, &barrier_mutex);
    }
    barrier_phase = 0;
    pthread_mutex_unlock(&barrier_mutex);
}

void brief_thread_pool_shutdown(void) {
    if (!workers) return;
    // Workers loop forever — cancel them on shutdown
    for (unsigned i = 0; i < worker_count; i++) {
        pthread_cancel(workers[i]);
        pthread_join(workers[i], NULL);
    }
    free(workers);
    free(worker_args);
    workers = NULL;
    worker_args = NULL;
    worker_count = 0;
}

void __wait_for_trigger__(void) {
    // 2026-07-15: Async event loop — wait for next trigger event
    pause();
}

// ── File I/O (used by stdlib) ─────────────────────────────────────────

int64_t __read_file__(int64_t path_bstr) {
    char* c_path = brief_str_to_c(path_bstr);
    if (!c_path) return -1;
    FILE* f = fopen(c_path, "r");
    free(c_path);
    if (!f) return -1;
    fseek(f, 0, SEEK_END);
    long size = ftell(f);
    fseek(f, 0, SEEK_SET);
    char* buf = malloc((size_t)(size + 1));
    if (!buf) { fclose(f); return -1; }
    size_t n = fread(buf, 1, (size_t)size, f);
    fclose(f);
    buf[n] = '\0';
    return (int64_t)(uintptr_t)buf;
}

int64_t __write_file__(int64_t path_bstr, int64_t data_bstr) {
    char* c_path = brief_str_to_c(path_bstr);
    char* c_data = brief_str_to_c(data_bstr);
    if (!c_path || !c_data) { free(c_path); free(c_data); return -1; }
    FILE* f = fopen(c_path, "w");
    free(c_path);
    if (!f) { free(c_data); return -1; }
    size_t len = strlen(c_data);
    size_t written = fwrite(c_data, 1, len, f);
    free(c_data);
    fclose(f);
    return (int64_t)written;
}

// ── Event loop ───────────────────────────────────────────────────────

void __rt_wait(void) {
    pause();
}

void __rt_poll(void) {
    pause();
}

// ── TTY / Terminal (used by stdlib) ──────────────────────────────────

int64_t tty_raw_mode(int64_t enable) {
    (void)enable;
    return -1;
}

int64_t tty_size(void) {
    return -1;
}

int64_t __tty_raw_mode__(int64_t enable) {
    return tty_raw_mode(enable);
}

int64_t __tty_size__(void) {
    return tty_size();
}

int64_t __tty_read_key__(void) {
    return -1;
}

int64_t __readln__(void) {
    return -1;
}

int64_t __sort_list__(int64_t list_bstr) {
    (void)list_bstr;
    return -1;
}

int64_t __reverse_list__(int64_t list_bstr) {
    (void)list_bstr;
    return -1;
}

int64_t brief_ttyname(int64_t fd) {
    return (int64_t)(uintptr_t)ttyname((int)fd);
}

// 2026-07-25: ShellCmd# runtime implementation.
// Runs a shell command via popen() and returns stdout as a Brief String.
// Expected LLVM signature: call i64 @ShellCmd(i64 %cmd_bstr)
int64_t ShellCmd(int64_t cmd_bstr) {
    // Extract C string from Brief String handle
    int64_t handle = cmd_bstr & ~3ULL;  // strip tag bits
    int64_t len = *(int64_t*)(uintptr_t)handle;  // read length prefix
    char* cstr = (char*)(uintptr_t)(handle + 8);  // data starts after length
    char* buf = (char*)calloc(len + 32, 1);
    if (!buf) return 0;
    memcpy(buf, cstr, len);
    
    // Run command via popen
    FILE* f = popen(buf, "r");
    free(buf);
    if (!f) return 0;
    
    // Read output into a growing buffer
    size_t out_cap = 4096;
    size_t out_len = 0;
    char* out = (char*)malloc(out_cap);
    if (!out) { pclose(f); return 0; }
    while (fgets(out + out_len, out_cap - out_len, f) != NULL) {
        out_len = strlen(out);
        if (out_len + 1024 > out_cap) {
            out_cap *= 2;
            out = (char*)realloc(out, out_cap);
            if (!out) { pclose(f); return 0; }
        }
    }
    pclose(f);
    
    // Pack as Brief String: {i64 length, i8 data[]}
    int64_t total = 8 + out_len;
    int64_t* result = (int64_t*)malloc(total + 8);  // extra padding
    if (!result) { free(out); return 0; }
    result[0] = out_len;
    memcpy(result + 1, out, out_len);
    free(out);
    return (int64_t)(uintptr_t)result;
}

// 2026-07-18: All __utf8_* functions now implemented as pure Brief in utf8view.bv
// (uses Load# + convergent txn). Find byte substring in byte string.
// Returns offset or -1.
// (implemented in pure Brief in lib/std/types/utf8view.bv)
