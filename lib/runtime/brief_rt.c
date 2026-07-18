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
// 2026-07-15: Still needed by __print, __read_file__, __write_file__,
// and other infrastructure functions that interface with C strings.

char* brief_str_to_c(int64_t bstr);

char* brief_str_to_c(int64_t bstr) {
    if (bstr == 0) return NULL;
    int64_t len = *(int64_t*)(uintptr_t)bstr;
    if (len < 0 || len > 1024 * 1024 * 1024) return NULL;
    char* c_str = malloc((size_t)(len + 1));
    if (!c_str) return NULL;
    if (len > 0) memcpy(c_str, (void*)(uintptr_t)(bstr + 8), (size_t)len);
    c_str[len] = '\0';
    return c_str;
}

// ── Core intrinsics (kept) ────────────────────────────────────────────

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

// 2026-07-18: All __utf8_* functions now implemented as pure Brief in utf8view.bv
// (uses Load# + convergent txn). Find byte substring in byte string.
// Returns offset or -1.
// (implemented in pure Brief in lib/std/types/utf8view.bv)
