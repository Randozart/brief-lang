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
// 2026-08-01 (B0): A Brief String value is a ptr to a length-prefixed
// [len][bytes] buffer. The old int64_t "handle" params were the address in
// disguise; they are now typed as pointers so clang's IR (ptr) matches the
// compiler's `ptr`-based frgn declares (String ABI = ptr). int64_t and
// pointers are ABI-identical on x86-64 — this is a typing change only.
char* brief_str_to_c(const char* handle) {
    // 2026-07-22: Strip tag bits (bottom 2 bits) — they mark temporary
    // flags (bit 0 = SSO inline, bit 1 = temporary concat result).
    // After stripping, we have the raw data pointer.
    uintptr_t u = (uintptr_t)handle;
    uintptr_t ptr = u & ~3ULL;
    if (u & 1) {
        // SSO string — inline data packed at bits >= 3
        // The LLVM SSO encoding: handle0 = (raw_data << 3) | 1
        // where raw_data has bytes packed in LE order.
        uint64_t raw_data = ((uint64_t)u) >> 3;
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
        uint8_t first = *(uint8_t*)ptr;
        if (first >= 32 && first < 127) {
            // Looks like a C string — strlen it
            int64_t len = (int64_t)strlen((const char*)ptr);
            if (len > 0 && len < 4096) {
                char* c_str = malloc((size_t)(len + 1));
                if (!c_str) return NULL;
                memcpy(c_str, (void*)ptr, (size_t)len);
                c_str[len] = '\0';
                return c_str;
            }
        }
    }
    // Heap Brief string: ptr is a pointer to [8-byte length][data].
    if (ptr == 0) return NULL;
    int64_t len = *(int64_t*)ptr;
    if (len < 0 || len > 1024 * 1024 * 1024) return NULL;
    char* c_str = malloc((size_t)(len + 1));
    if (!c_str) return NULL;
    if (len > 0) memcpy(c_str, (void*)(ptr + 8), (size_t)len);
    c_str[len] = '\0';
    return c_str;
}

/// Convert a C string (null-terminated) to a Brief string.
/// Returns a heap-allocated Brief string (8-byte length prefix + data).
/// Caller should free via brief_free_brief_str().
char* brief_cstr_to_brief(const char* c_str) {
    if (!c_str) return 0;
    int64_t len = (int64_t)strlen(c_str);
    if (len > 1024 * 1024 * 1024) return 0; // sanity check
    // Allocate: 8 bytes for length + len bytes for data + 1 for null terminator
    char* buf = (char*)malloc((size_t)(len + 9));
    if (!buf) return 0;
    *(int64_t*)buf = len;               // write length prefix
    if (len > 0) memcpy(buf + 8, c_str, (size_t)len);
    buf[8 + len] = '\0';                // null terminator for C compatibility
    return buf;
}

/// Free a Brief string allocated by brief_cstr_to_brief or similar.
void brief_free_brief_str(void* handle) {
    if (handle) free(handle);
}

// 2026-08-01 (B2): The #Bit → #String ENCODING DOOR default. The bits are a
// Brief `[len: i64][bytes]` buffer (the content view of a String — see
// #String→#Bit). This re-materializes a String from those bits by copying the
// length header + payload into a fresh heap buffer. This is NOT brief_cstr_to_brief
// (which reads a null-terminated C string) — the bits carry their own length.
// The header is created by construction (copied from the bits), never inherited
// by aliasing. Returns a heap [len][bytes] String; caller frees via
// brief_free_brief_str. Sub-protocols override the lane via CastFrom(#Bit).
char* brief_bits_to_str(const char* bits) {
    if (!bits) return 0;
    int64_t len = *(const int64_t*)bits;
    if (len < 0 || len > 1024 * 1024 * 1024) return 0; // sanity
    char* buf = (char*)malloc((size_t)(len + 9));
    if (!buf) return 0;
    *(int64_t*)buf = len;
    if (len > 0) memcpy(buf + 8, bits + 8, (size_t)len);
    buf[8 + len] = '\0';
    return buf;
}

// 2026-08-01 (B3): UTF8 character count of a Brief String value (String ABI =
// ptr to [len: i64][bytes]). Bytes are valid UTF8, so the count is the number
// of codepoints (skip continuation bytes 0b10xxxxxx). This is the `#String`
// `Size` prop default (the O(1) byte-length header read is the `Bytes` prop).
// Sub-protocols override the lane via their own prop bindings.
int64_t brief_char_len(const char* str) {
    if (!str) return 0;
    int64_t len = *(const int64_t*)str;
    if (len < 0) return 0;
    const unsigned char* p = (const unsigned char*)(str + 8);
    int64_t chars = 0;
    for (int64_t i = 0; i < len; i++) {
        // A UTF8 continuation byte is 0b10xxxxxx (0x80–0xBF). Count only
        // lead bytes (including ASCII 0x00–0x7F).
        if ((p[i] & 0xC0) != 0x80) chars++;
    }
    return chars;
}

// 2026-08-01 (B1): Content equality for Brief String values (String ABI = ptr
// to a length-prefixed [len: i64][bytes] buffer). Compares lengths first, then
// payload bytes. Returns 1 if equal, 0 otherwise. This is the runtime half of
// B1's content Eq/Ne — the compiler emits a call to this instead of comparing
// the two addresses. Both arguments must be valid [len][bytes] buffers (as all
// Brief String values are under the bits model); handles are converted to
// content by the caller when needed.
int64_t brief_str_eq(const char* a, const char* b) {
    if (a == b) return 1;
    if (!a || !b) return 0;
    int64_t la = *(const int64_t*)a;
    int64_t lb = *(const int64_t*)b;
    if (la != lb) return 0;
    if (la <= 0) return 1;  // both empty
    return memcmp(a + 8, b + 8, (size_t)la) == 0;
}

// 2026-08-01 (B1): Content bitwise ops for Brief String values (String ABI =
// ptr to [len: i64][bytes]). The result is a NEW heap buffer with the same
// length and the per-byte op applied to the payloads (band/bor/bxor) or to a
// single payload (bnot). Length must match for binary ops (asserted by the
// compiler; a mismatch returns the empty string defensively). Caller frees via
// brief_free_brief_str. These are the runtime half of the #String bitwise
// defaults; the compiler emits a call instead of treating String ptrs as ints.
static char* brief_str_bitop(const char* a, const char* b, int op) {
    if (!a) return 0;
    int64_t la = *(const int64_t*)a;
    int64_t lb = b ? *(const int64_t*)b : 0;
    if (op != 3 && la != lb) return 0;  // binary ops need equal length
    if (la < 0) return 0;
    char* out = (char*)malloc((size_t)(la + 8 + 1));
    if (!out) return 0;
    *(int64_t*)out = la;
    for (int64_t i = 0; i < la; i++) {
        unsigned char x = (unsigned char)a[8 + i];
        unsigned char y = b ? (unsigned char)b[8 + i] : 0;
        switch (op) {
            case 0: out[8 + i] = (char)(x & y); break;
            case 1: out[8 + i] = (char)(x | y); break;
            case 2: out[8 + i] = (char)(x ^ y); break;
            default: out[8 + i] = (char)(~x); break;
        }
    }
    out[8 + la] = '\0';
    return out;
}
char* brief_str_band(const char* a, const char* b) { return brief_str_bitop(a, b, 0); }
char* brief_str_bor(const char* a, const char* b)  { return brief_str_bitop(a, b, 1); }
char* brief_str_bxor(const char* a, const char* b) { return brief_str_bitop(a, b, 2); }
char* brief_str_bnot(const char* a)                { return brief_str_bitop(a, 0, 3); }

// ── CLI argv capture (Phase 3, 2026-08-01) ────────────────────────────
// The compiler's emitted `main(i32 %argc, ptr %argv)` stores its arguments
// into these module globals (see emit_main_header); the helpers below read
// them. String results follow the String ABI = ptr to [len: i64][bytes]
// (heap-allocated; caller frees via brief_free_brief_str).
// The compiler's emitted `main(i32 %argc, ptr %argv)` stores its arguments
// into these globals (see emit_main_header) — the compiler OWNS them, so the
// runtime declares them extern (not defines them). String results follow the
// String ABI = ptr to [len: i64][bytes] (heap-allocated; caller frees via
// brief_free_brief_str).
extern int32_t __brief_argc;
extern void* __brief_argv;

int64_t __argv_count(void) {
    return (int64_t)__brief_argc;
}

// argv[i] as a Brief string (empty for out-of-range i).
char* __argv_get(int64_t i) {
    if (!__brief_argv || i < 0 || i >= __brief_argc) {
        return brief_cstr_to_brief("");
    }
    char* s = ((char**)__brief_argv)[i];
    return brief_cstr_to_brief(s);
}

// Whether any argv token equals `flag` (a Brief string). Returns 1/0.
// Skips argv[0] (the program name) — flags/commands live in argv[1..].
int64_t __argv_has(const char* flag_bstr) {
    char* c_flag = brief_str_to_c(flag_bstr);
    if (!c_flag) return 0;
    int64_t found = 0;
    if (__brief_argv) {
        for (int64_t i = 1; i < __brief_argc; i++) {
            if (strcmp(((char**)__brief_argv)[i], c_flag) == 0) {
                found = 1;
                break;
            }
        }
    }
    free(c_flag);
    return found;
}

// The value following `flag` (e.g. `--out file` → "file"), or "" if absent.
char* __argv_value(const char* flag_bstr) {
    char* c_flag = brief_str_to_c(flag_bstr);
    if (!c_flag) return brief_cstr_to_brief("");
    char* result = NULL;
    if (__brief_argv) {
        for (int64_t i = 1; i + 1 < __brief_argc; i++) {
            if (strcmp(((char**)__brief_argv)[i], c_flag) == 0) {
                result = ((char**)__brief_argv)[i + 1];
                break;
            }
        }
    }
    free(c_flag);
    if (!result) return brief_cstr_to_brief("");
    return brief_cstr_to_brief(result);
}

// The first non-flag token in argv[1..] — the subcommand. `<prog> --verbose
// build` → "build"; "" if none. Honors $BRIEF_ENTRY_CMD (test/embedded path
// without argv) as the sole environment fallback.
char* __argv_command(void) {
    const char* env_cmd = getenv("BRIEF_ENTRY_CMD");
    if (env_cmd && env_cmd[0]) {
        return brief_cstr_to_brief(env_cmd);
    }
    if (__brief_argv) {
        for (int64_t i = 1; i < __brief_argc; i++) {
            const char* tok = ((char**)__brief_argv)[i];
            if (tok[0] != '-') {
                return brief_cstr_to_brief(tok);
            }
        }
    }
    return brief_cstr_to_brief("");
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
// 2026-08-01 (B0): key_bstr is a ptr to a Brief [len][bytes] buffer; returns
// a ptr to the same layout (String ABI = ptr, matching the compiler declares).
char* __getenv_brief(const char* key_bstr) {
    char* c_key = brief_str_to_c(key_bstr);
    if (!c_key) return 0;
    char* val = getenv(c_key);
    free(c_key);
    if (!val) return 0;
    int64_t len = (int64_t)strlen(val);
    char* bstr = (char*)malloc((size_t)(len + 8 + 1));
    if (!bstr) return 0;
    *(int64_t*)bstr = len;
    memcpy(bstr + 8, val, (size_t)len);
    bstr[8 + len] = '\0';
    return bstr;
}

// 2026-07-19: Returns the value of an env var parsed as Int.
int64_t __getenv_int(const char* key_bstr) {
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

int64_t __print(const char* msg_bstr) {
    char* c_msg = brief_str_to_c(msg_bstr);
    if (c_msg) { fputs(c_msg, stdout); free(c_msg); }
    return 0;
}

int64_t __print_int(int64_t n) {
    printf("%ld", (long)n);
    return 0;
}

// 2026-07-31: %.9g — round-trips any float32 uniquely (~7 sig decimal digits).
// The prior %g (6 sig digits) truncated precision, making Brief's float output
// differ from C references that print %.9f even for identical values.
int64_t __print_float(float f) {
    printf("%.9g", (double)f);
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

// 2026-08-01: String printer for the PrintStr# intrinsic — the target of
// format-string literal segments in print!/println!. Mirrors __print: takes
// a ptr to a length-prefixed [len][bytes] buffer (String ABI = ptr), prints
// it without a trailing newline, and frees the C copy. Defined here
// because the print plugin expands literal segments to PrintStr# calls.
int64_t __print_str(const char* msg_bstr) {
    char* c_msg = brief_str_to_c(msg_bstr);
    if (c_msg) { fputs(c_msg, stdout); free(c_msg); }
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
// 2026-08-01 (B0): path_bstr/data_bstr are ptrs to Brief [len][bytes] buffers
// (String ABI = ptr).

int64_t __read_file__(const char* path_bstr) {
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

int64_t __write_file__(const char* path_bstr, const char* data_bstr) {
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

// 2026-08-01 (C3): required-watchdog failure exit. A `![cond]` watchdog that
// fires without an on-fire handler is a fatal program error — the loop engine
// calls this on the fire path.
void __watchdog_fail(void) {
    fprintf(stderr, "brief: required watchdog fired\n");
    exit(1);
}

// 2026-08-01 (D2): garbage-scheduling calibration. The scheduler's scheduled
// frees route through __brief_free so a benchmark can assert frees == allocs
// (no premature free, no leak). __brief_free_count() is the observable getter.
static long __brief_free_total = 0;

void __brief_free(void* p) {
    if (p) __brief_free_total++;
    free(p);
}

long __brief_free_count(void) {
    return __brief_free_total;
}
