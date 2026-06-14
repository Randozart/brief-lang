/*
 * brief_rt.c — Single-file runtime for Brief LLVM backend
 *
 * Provides:
 *   1. @ link global definitions (__io_pending, __sigint_flag, etc.)
 *   2. __rt_init() — signal handlers, timers, epoll/kqueue setup (called by main())
 *   3. __rt_wait() — per-platform blocking sleep (called by main())
 *   4. __wait_for_event() — user-callable FFI wrapper over __rt_wait()
 *
 * Compile once per target:
 *   cc -c brief_rt.c -o brief_rt.o
 *   ld program.o brief_rt.o -o program
 *
 * The C preprocessor handles platform detection. No manual per-system config.
 */

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
#include <sys/stat.h>

/* ===================================================================
 * 1. @ link Global Definitions
 *
 * These symbols are referenced by the LLVM IR as `external global`.
 * The resolver links them to these definitions.
 *
 * Users declare them in .bv files as:
 *   trg name: Type @ link __io_pending;
 *
 * Multiple .bv files can share the same @ link name — the linker
 * deduplicates.
 * =================================================================== */

volatile char      __io_pending    __attribute__((section("brief_trg")));
volatile char      __sigint_flag   __attribute__((section("brief_trg")));
volatile char      __sigterm_flag  __attribute__((section("brief_trg")));
volatile char      __sighup_flag   __attribute__((section("brief_trg")));
volatile int64_t   __timer_1hz     __attribute__((section("brief_trg")));
volatile int64_t   __timer_100hz   __attribute__((section("brief_trg")));
volatile char      __stdin_ready   __attribute__((section("brief_trg")));
volatile char*     __stdin_buffer  __attribute__((section("brief_trg")));
/* ===================================================================
 * 1.5 Environment Variable Reader
 *
 * Called at init_state() time to read a 64-bit integer from an
 * environment variable. Used by runtime-mode benchmarks where the
 * compiler cannot constant-fold the bound.
 * =================================================================== */

int64_t __get_env_int(const char* name) {
    const char* val = getenv(name);
    if (!val) return 0;
    char* end = NULL;
    long v = strtol(val, &end, 10);
    if (end == val) return 0;
    return (int64_t)v;
}

/* ===================================================================
 * 2. Signal Handlers
 *
 * Each handler writes to the corresponding @ link global.
 * The reactor tick's `load volatile` samples these on the next tick.
 * =================================================================== */

static void handle_sigint(int sig) {
    (void)sig;
    __sigint_flag = 1;
    __io_pending = 1;
}

static void handle_sigterm(int sig) {
    (void)sig;
    __sigterm_flag = 1;
    __io_pending = 1;
}

static void handle_sighup(int sig) {
    (void)sig;
    __sighup_flag = 1;
    __io_pending = 1;
}

/* ===================================================================
 * 3. Timer Setup
 *
 * A real-time timer fires periodically and increments __timer_* counters
 * and sets __io_pending to wake the reactor.
 * =================================================================== */

static timer_t g_timer_1hz;
static timer_t g_timer_100hz;

static void handle_timer(int sig, siginfo_t* si, void* uc) {
    (void)sig; (void)si; (void)uc;
    __timer_1hz++;
    __io_pending = 1;
}

static int setup_timer(timer_t* tid, int signo, long sec, long nsec) {
    struct sigevent sev = {0};
    sev.sigev_notify = SIGEV_SIGNAL;
    sev.sigev_signo = signo;
    if (timer_create(CLOCK_REALTIME, &sev, tid) != 0)
        return -1;
    struct itimerspec its = {0};
    its.it_value.tv_sec = sec;
    its.it_value.tv_nsec = nsec;
    its.it_interval.tv_sec = sec;
    its.it_interval.tv_nsec = nsec;
    return timer_settime(*tid, 0, &its, NULL);
}

/* ===================================================================
 * 4. __rt_wait — Platform blocking sleep
 *
 * Called by the LLVM backend after each reactor_tick() to block
 * until the next event. Uses per-platform primitives:
 *   - Linux:   epoll_wait with signalfd, timerfd, stdin
 *   - BSD/mac: kqueue with EVFILT_SIGNAL, EVFILT_TIMER, EVFILT_READ
 *   - ARM:     WFI (Wait For Interrupt)
 *   - x86:     STI; HLT
 *   - WASM:    host yield
 *   - Other:   nanosleep poll
 * =================================================================== */

#if defined(__linux__)
#include <sys/epoll.h>
#include <unistd.h>
#include <fcntl.h>

#define MAX_EPOLL_EVENTS 64

static int g_epoll_fd = -1;

static int ensure_epoll(void) {
    if (g_epoll_fd < 0) {
        g_epoll_fd = epoll_create1(0);
        if (g_epoll_fd < 0) return -1;
        /* Watch stdin for __stdin_ready */
        struct epoll_event ev = {0};
        ev.events = EPOLLIN | EPOLLRDHUP;
        ev.data.fd = STDIN_FILENO;
        epoll_ctl(g_epoll_fd, EPOLL_CTL_ADD, STDIN_FILENO, &ev);
    }
    return 0;
}

void __rt_wait(void) {
    if (ensure_epoll() == 0) {
        struct epoll_event events[MAX_EPOLL_EVENTS];
        int n = epoll_wait(g_epoll_fd, events, MAX_EPOLL_EVENTS, 100);
        if (n > 0) {
            for (int i = 0; i < n; i++) {
                if (events[i].data.fd == STDIN_FILENO
                    && (events[i].events & EPOLLIN)) {
                    __stdin_ready = 1;
                    __io_pending = 1;
                }
            }
        }
        /* Timeout or EINTR: signal handlers may have updated globals.
           Set io_pending so the reactor re-samples all triggers. */
        if (n <= 0) {
            __io_pending = 1;
        }
        return;
    }
    /* Fallback: poll stdin with 100ms timeout */
    fd_set rfds;
    FD_ZERO(&rfds);
    FD_SET(STDIN_FILENO, &rfds);
    struct timeval tv = {0, 100000};
    select(1, &rfds, NULL, NULL, &tv);
    if (FD_ISSET(STDIN_FILENO, &rfds)) {
        __stdin_ready = 1;
        __io_pending = 1;
    }
    __io_pending = 1;
}

/* Non-blocking poll: drains any already-pending events without sleeping.
   Called once at main() entry, before the first tick, to eliminate the
   100ms wasted first tick on programs that already have events ready.
   Also sets io_pending = 1 so the first reactor tick always runs
   (same as __rt_wait signals on timeout). */
void __rt_poll(void) {
    if (ensure_epoll() == 0) {
        struct epoll_event events[MAX_EPOLL_EVENTS];
        int n = epoll_wait(g_epoll_fd, events, MAX_EPOLL_EVENTS, 0);
        if (n > 0) {
            for (int i = 0; i < n; i++) {
                if (events[i].data.fd == STDIN_FILENO
                    && (events[i].events & EPOLLIN)) {
                    __stdin_ready = 1;
                    __io_pending = 1;
                }
            }
        }
        __io_pending = 1;
        return;
    }
    /* Fallback: try select with zero timeout */
    fd_set rfds;
    FD_ZERO(&rfds);
    FD_SET(STDIN_FILENO, &rfds);
    struct timeval tv = {0, 0};
    select(1, &rfds, NULL, NULL, &tv);
    if (FD_ISSET(STDIN_FILENO, &rfds)) {
        __stdin_ready = 1;
        __io_pending = 1;
    }
    __io_pending = 1;
}

#elif defined(__APPLE__) || defined(__FreeBSD__) || defined(__OpenBSD__) || defined(__NetBSD__)
#include <sys/types.h>
#include <sys/event.h>
#include <sys/time.h>
#include <unistd.h>
#include <fcntl.h>

#define MAX_KQUEUE_EVENTS 64

static int g_kq = -1;

static int ensure_kqueue(void) {
    if (g_kq < 0) {
        g_kq = kqueue();
        if (g_kq < 0) return -1;
        struct kevent ev;
        EV_SET(&ev, STDIN_FILENO, EVFILT_READ, EV_ADD, 0, 0, NULL);
        kevent(g_kq, &ev, 1, NULL, 0, NULL);
    }
    return 0;
}

void __rt_wait(void) {
    if (ensure_kqueue() == 0) {
        struct kevent events[MAX_KQUEUE_EVENTS];
        struct timespec ts = {1, 0};
        int n = kevent(g_kq, NULL, 0, events, MAX_KQUEUE_EVENTS, &ts);
        if (n > 0) {
            for (int i = 0; i < n; i++) {
                if (events[i].ident == STDIN_FILENO
                    && events[i].filter == EVFILT_READ) {
                    __stdin_ready = 1;
                    __io_pending = 1;
                }
            }
        }
        return;
    }
    /* Fallback: same as Linux — poll stdin */
    fd_set rfds;
    FD_ZERO(&rfds);
    FD_SET(STDIN_FILENO, &rfds);
    struct timeval tv = {1, 0};
    select(1, &rfds, NULL, NULL, &tv);
    if (FD_ISSET(STDIN_FILENO, &rfds)) {
        __stdin_ready = 1;
        __io_pending = 1;
    }
}

void __rt_poll(void) {
    if (ensure_kqueue() == 0) {
        struct kevent events[MAX_KQUEUE_EVENTS];
        struct timespec ts = {0, 0};
        int n = kevent(g_kq, NULL, 0, events, MAX_KQUEUE_EVENTS, &ts);
        if (n > 0) {
            for (int i = 0; i < n; i++) {
                if (events[i].ident == STDIN_FILENO
                    && events[i].filter == EVFILT_READ) {
                    __stdin_ready = 1;
                    __io_pending = 1;
                }
            }
        }
        __io_pending = 1;
        return;
    }
    fd_set rfds;
    FD_ZERO(&rfds);
    FD_SET(STDIN_FILENO, &rfds);
    struct timeval tv = {0, 0};
    select(1, &rfds, NULL, NULL, &tv);
    if (FD_ISSET(STDIN_FILENO, &rfds)) {
        __stdin_ready = 1;
        __io_pending = 1;
    }
    __io_pending = 1;
}

#elif defined(__arm__) || defined(__aarch64__) || defined(_ARM_) || defined(_M_ARM)
void __rt_wait(void) {
    /* ARM Wait For Interrupt — CPU halts until interrupt/event */
    __asm__ volatile("wfi" ::: "memory");
    __io_pending = 1;
}

void __rt_poll(void) {
    /* ARM: no non-blocking equivalent. Just set io_pending so the
       first tick runs instead of wasting 100ms in __rt_wait. */
    __sync_synchronize();
    __io_pending = 1;
}

#elif defined(__x86_64__) || defined(__i386__) || defined(_M_IX86) || defined(_M_X64)
void __rt_wait(void) {
    /* x86: enable interrupts then halt — CPU sleeps until IRQ */
    __asm__ volatile("sti; hlt" ::: "memory");
    __io_pending = 1;
}

void __rt_poll(void) {
    /* x86: no non-blocking equivalent. Just set io_pending so the
       first tick runs instead of wasting 100ms in __rt_wait. */
    __sync_synchronize();
    __io_pending = 1;
}

#elif defined(__wasm__) || defined(__EMSCRIPTEN__)
void __rt_wait(void) {
    /* WASM: yield to host event loop. Returns when re-entered. */
    __builtin_wasm_memory_grow(0, 0);
    __io_pending = 1;
}

void __rt_poll(void) {
    /* WASM: no-op — events are delivered asynchronously. */
}

#else
/* Fallback: busy-sleep with 1ms polling */
void __rt_wait(void) {
    struct timespec ts = {0, 1000000}; /* 1ms */
    nanosleep(&ts, NULL);
    __io_pending = 1;
}

void __rt_poll(void) {
    /* Fallback: no-op — events collected on next wait. */
}
#endif

/* ===================================================================
 * 4b. __wait_for_event — User-callable FFI wrapper
 *
 * Thin wrapper over __rt_wait() for user code that declares:
 *   frgn __wait_for_event() -> Void from "libruntime";
 * =================================================================== */
void __wait_for_event(void) {
    __rt_wait();
}

/* ===================================================================
 * 4c. FFI I/O Functions — Transparent, no compiler magic
 *
 * These are called through the generic FFI path in the LLVM backend.
 * No hardcoded Rust string matches.
 *
 * Signatures:
 *   frgn __print(msg: String) -> Bool    — prints to stdout
 *   frgn __print_int(n: Int) -> Bool     — prints int to stderr
 *   frgn __exit() -> Void                — terminates the program
 *
 * The LLVM backend marshals String as i8*, Int as i64, Bool as i64.
 * =================================================================== */

int64_t __print(const char* msg) {
    fputs(msg, stdout);
    return 1;
}

int64_t __print_int(int64_t n) {
    fprintf(stderr, "%lld\n", (long long)n);
    return 1;
}

int64_t __print_float(float d) {
    fprintf(stderr, "%.9f\n", (double)d);
    return 1;
}

float __sqrtf(float x) {
    return sqrtf(x);
}

void __exit(void) {
    exit(0);
}

static inline int64_t __print_str_len(const char* buf, int64_t len) {
    fwrite(buf, 1, (size_t)len, stdout);
    return len;
}

static inline int64_t __write_bytes(const char* buf, int64_t len) {
    fwrite(buf, 1, (size_t)len, stdout);
    return len;
}

int64_t __read_stdin(char* buf, int64_t max_len) {
    size_t n = fread(buf, 1, (size_t)max_len, stdin);
    return (int64_t)n;
}

int64_t __putchar(int64_t c) {
    fprintf(stderr, "%c", (int)c);
    return c;
}

/* ===================================================================
 * 5. Initialization — __rt_init() and constructor wrapper
 *
 * __rt_init() is called by the LLVM backend at the start of main()
 * before the reactor loop begins. It sets up signal handlers, timers,
 * and OS event sources.
 *
 * The constructor wrapper ensures init also runs for FFI-only usage
 * (e.g. when the user calls __wait_for_event via frgn without the
 * generated main()).
 * =================================================================== */

void __rt_init(void) {
    /* Signal handlers */
    signal(SIGINT,  handle_sigint);
    signal(SIGTERM, handle_sigterm);
    signal(SIGHUP,  handle_sighup);

    /* Timers (100Hz and 1Hz) using SIGRTMIN + 1 / +2 */
    struct sigaction sa = {0};
    sa.sa_sigaction = handle_timer;
    sa.sa_flags = SA_SIGINFO;
    sigaction(SIGRTMIN + 1, &sa, NULL);
    sigaction(SIGRTMIN + 2, &sa, NULL);
    setup_timer(&g_timer_1hz,   SIGRTMIN + 1, 1,   0);
    setup_timer(&g_timer_100hz, SIGRTMIN + 2, 0,   10000000); /* 10ms */

    /* Ensure stdout buffer is line-buffered for __print */
    setvbuf(stdout, NULL, _IOLBF, 0);

    /* Mark io as pending initially so the first tick checks for work */
    __io_pending = 1;
}

/* ===================================================================
 * 6. Thread Pool — Concurrent async transaction dispatch
 *
 * Workers are spawned once at init and synchronize via two barriers
 * per tick: one for releasing workers (main → workers), one for
 * collecting them back (workers → main).
 *
 *   main loop:      workers (each):
 *     barrier_release  →  barrier_enter
 *     (enum/seq work)     fire_body()
 *     barrier_wait    ←  barrier_exit
 *
 * On macOS, pthread_barrier_t is unavailable. We use a portable
 * implementation with mutex+cond+counter.
 *
 * Gated behind #if defined(BRIEF_THREAD_POOL). The LLVM backend
 * emits @llvm.thread_pool metadata when async txns exist, and the
 * compiler driver adds -DBRIEF_THREAD_POOL -lpthread.
 * =================================================================== */

#if defined(BRIEF_THREAD_POOL)
#include <pthread.h>
#include <stdlib.h>

/* Portable barrier — works on all pthread platforms */
typedef struct {
    pthread_mutex_t mutex;
    pthread_cond_t  cond;
    unsigned count;
    unsigned target;
    unsigned generation;
} brief_barrier_t;

static void brief_barrier_init(brief_barrier_t *b, unsigned n) {
    pthread_mutex_init(&b->mutex, NULL);
    pthread_cond_init(&b->cond, NULL);
    b->count = 0;
    b->target = n;
    b->generation = 0;
}

static void brief_barrier_wait_impl(brief_barrier_t *b) {
    pthread_mutex_lock(&b->mutex);
    unsigned my_gen = b->generation;
    b->count++;
    if (b->count >= b->target) {
        b->count = 0;
        b->generation++;
        pthread_cond_broadcast(&b->cond);
    } else {
        while (b->generation == my_gen) {
            pthread_cond_wait(&b->cond, &b->mutex);
        }
    }
    pthread_mutex_unlock(&b->mutex);
}

/* Per-worker state */
typedef void (*brief_work_fn)(void);

static int              g_thread_pool_active = 0;
static unsigned         g_num_workers = 0;
static pthread_t       *g_workers = NULL;
static brief_work_fn   *g_work_fns = NULL;
static brief_work_fn    *g_work_fns_base = NULL;  /* pointer to first slot */
static int              g_shutdown = 0;
static brief_barrier_t  g_barrier_enter;  /* main releases → workers start */
static brief_barrier_t  g_barrier_exit;   /* workers finish → main continues */

static void* brief_worker_main(void* arg) {
    unsigned idx = (unsigned)(uintptr_t)arg;
    while (1) {
        brief_barrier_wait_impl(&g_barrier_enter);  /* wait for tick start */
        if (g_shutdown) return NULL;
        if (idx < g_num_workers && g_work_fns[idx]) {
            g_work_fns[idx]();
        }
        brief_barrier_wait_impl(&g_barrier_exit);   /* signal tick done */
    }
    return NULL;
}
#endif /* BRIEF_THREAD_POOL */

void brief_thread_pool_init(unsigned num_workers, void** fn_ptrs) {
#if defined(BRIEF_THREAD_POOL)
    if (g_thread_pool_active) return;
    if (num_workers == 0) return;
    g_num_workers = num_workers;
    g_workers = (pthread_t*)calloc(num_workers, sizeof(pthread_t));
    g_work_fns = (brief_work_fn*)calloc(num_workers, sizeof(brief_work_fn));
    g_work_fns_base = g_work_fns;
    for (unsigned i = 0; i < num_workers; i++) {
        g_work_fns[i] = (brief_work_fn)(uintptr_t)fn_ptrs[i];
    }
    brief_barrier_init(&g_barrier_enter, num_workers + 1);  /* +1 for main */
    brief_barrier_init(&g_barrier_exit, num_workers + 1);
    g_shutdown = 0;
    for (unsigned i = 0; i < num_workers; i++) {
        pthread_create(&g_workers[i], NULL, brief_worker_main,
                       (void*)(uintptr_t)i);
    }
    g_thread_pool_active = 1;
#else
    (void)num_workers; (void)fn_ptrs;
#endif
}

void brief_barrier_release(void) {
#if defined(BRIEF_THREAD_POOL)
    if (!g_thread_pool_active || g_num_workers == 0) return;
    brief_barrier_wait_impl(&g_barrier_enter);
#endif
}

void brief_barrier_wait(void) {
#if defined(BRIEF_THREAD_POOL)
    if (!g_thread_pool_active || g_num_workers == 0) return;
    brief_barrier_wait_impl(&g_barrier_exit);
#endif
}

void brief_thread_pool_shutdown(void) {
#if defined(BRIEF_THREAD_POOL)
    g_shutdown = 1;
    brief_barrier_wait_impl(&g_barrier_enter);  /* wake workers */
    for (unsigned i = 0; i < g_num_workers; i++) {
        pthread_join(g_workers[i], NULL);
    }
    free(g_workers); g_workers = NULL;
    free(g_work_fns_base); g_work_fns = NULL; g_work_fns_base = NULL;
    g_thread_pool_active = 0;
#endif
}

/* ===================================================================
 * 5. Brief String I/O — read_file intrinsic
 *
 * The LLVM backend marshals String as i8*, so both path and return
 * value are C strings (null-terminated char*). The interpreter uses
 * Rust std::fs::read_to_string and never calls this function.
 * =================================================================== */

char* brief_read_file(const char* path) {
    if (!path) return NULL;

    FILE* fp = fopen(path, "rb");
    if (!fp) return NULL;

    fseek(fp, 0, SEEK_END);
    long file_size = ftell(fp);
    fseek(fp, 0, SEEK_SET);

    if (file_size <= 0) {
        fclose(fp);
        return NULL;
    }

    char* data = malloc((size_t)file_size + 1);
    if (!data) {
        fclose(fp);
        return NULL;
    }

    size_t bytes_read = fread(data, 1, (size_t)file_size, fp);
    fclose(fp);

    if (bytes_read == 0) {
        free(data);
        return NULL;
    }

    data[bytes_read] = '\0';
    return data;
}

/* ===================================================================
 * 6. Wrapper symbols — bridge frgn names to runtime implementations
 *
 * The LLVM backend emits calls using the frgn declaration names directly
 * (e.g. "print", "string_trim"). These wrappers bridge to the internal
 * __-prefixed implementations.
 *
 * Brief string format for all String parameters and returns:
 *   int64_t header[2 + len]
 *   header[0] = data pointer (address of header[2])
 *   header[1] = length (int64_t)
 *   header[2..2+len] = character data (one int64_t per char, low byte only)
 * =================================================================== */

#include <ctype.h>
#include <string.h>
#include <sys/wait.h>

/* ── Helpers (forward declarations from section 7) ─────────────────── */
static char*   brief_str_to_c(int64_t bstr);
static int64_t cstr_to_brief(const char* s);
static int64_t buf_to_brief(const char* buf, int64_t len);

/* ── Terminal ─────────────────────────────────────────────────────── */

int64_t tty_raw_mode(int64_t enable) {
    int64_t r = *((volatile int64_t*)&enable); // read arg to prevent elimination
    return r ? 1 : 0; // placeholder — real termios requires platform includes
}

int64_t tty_size(void) {
    return (int64_t)(80 * 10000 + 24); // placeholder — real ioctl
}

int64_t tty_read_key(void) {
    unsigned char ch = 0;
    if (fread(&ch, 1, 1, stdin) > 0) return (int64_t)ch;
    return 0;
}

/* ── Stdlib __ functions ──────────────────────────────────────────── */

int64_t __trim_left(int64_t s_val) {
    char* s = brief_str_to_c(s_val);
    if (!s) return 0;
    char* start = s;
    while (*start && (unsigned char)*start <= 32) start++;
    int64_t result = cstr_to_brief(start);
    free(s);
    return result;
}

int64_t __trim_right(int64_t s_val) {
    char* s = brief_str_to_c(s_val);
    if (!s) return 0;
    char* end = s + strlen(s);
    while (end > s && (unsigned char)*(end - 1) <= 32) end--;
    *end = '\0';
    int64_t result = cstr_to_brief(s);
    free(s);
    return result;
}

int64_t __to_lower(int64_t s_val) {
    char* s = brief_str_to_c(s_val);
    if (!s) return 0;
    for (char* p = s; *p; p++) *p = (char)tolower((unsigned char)*p);
    int64_t result = cstr_to_brief(s);
    free(s);
    return result;
}

int64_t __contains_at(int64_t haystack_val, int64_t needle_val, int64_t start_val) {
    char* haystack = brief_str_to_c(haystack_val);
    char* needle = brief_str_to_c(needle_val);
    if (!haystack || !needle) { free(haystack); free(needle); return 0; }
    int64_t r = (strstr(haystack + (size_t)start_val, needle) != NULL) ? 1 : 0;
    free(haystack); free(needle);
    return r;
}

int64_t __find_from(int64_t s_val, int64_t needle_val, int64_t start_val) {
    char* s = brief_str_to_c(s_val);
    char* needle = brief_str_to_c(needle_val);
    if (!s || !needle) { free(s); free(needle); return -1; }
    char* found = strstr(s + (size_t)start_val, needle);
    int64_t r = found ? (int64_t)(found - s) : -1;
    free(s); free(needle);
    return r;
}

int64_t __int_to_str(int64_t n) {
    char buf[64];
    int len = snprintf(buf, sizeof(buf), "%lld", (long long)n);
    return buf_to_brief(buf, (int64_t)len);
}

int64_t __splitn(int64_t s_val, int64_t delim_val, int64_t n_val) {
    (void)n_val;
    char* s = brief_str_to_c(s_val);
    char* delim = brief_str_to_c(delim_val);
    if (!s || !delim) { free(s); free(delim); return 0; }
    // Count tokens
    int64_t count = 1;
    for (char* p = s; *p; p++) {
        if (strncmp(p, delim, strlen(delim)) == 0) { count++; p += strlen(delim) - 1; }
    }
    // Allocate list header: [data_ptr, length, elem0, elem1, ...]
    int64_t* list = malloc(sizeof(int64_t) * (2 + count));
    list[0] = (int64_t)(list + 2);  // data pointer
    list[1] = count;
    int64_t idx = 0;
    char* tok = strtok(s, delim);
    while (tok && idx < count) {
        list[2 + idx] = cstr_to_brief(tok);
        idx++;
        tok = strtok(NULL, delim);
    }
    int64_t result = (int64_t)list;
    free(s); free(delim);
    return result;
}

/* ── Process ──────────────────────────────────────────────────────── */

int64_t __spawn_with_output(int64_t cmd_val, int64_t args_val) {
    (void)args_val;
    char* cmd = brief_str_to_c(cmd_val);
    if (!cmd) return 0;
    FILE* fp = popen(cmd, "r");
    free(cmd);
    if (!fp) return 0;
    char buf[65536];
    size_t n = fread(buf, 1, sizeof(buf) - 1, fp);
    int status = pclose(fp);
    if (n > 0) {
        buf[n] = '\0';
        while (n > 0 && (buf[n-1] == '\n' || buf[n-1] == '\r')) n--;
        buf[n] = '\0';
        return cstr_to_brief(buf);
    }
    return status == 0 ? cstr_to_brief("") : 0;
}

/* ── Officina-local frgn (JSON, substring) ────────────────────────── */

int64_t substring(int64_t s_val) {
    char* s = brief_str_to_c(s_val);
    if (!s) return 0;
    int64_t len = (int64_t)strlen(s);
    // List header: [data_ptr, length, char0, char1, ...]
    // Each char is stored as int64_t with the byte in the low 8 bits
    int64_t* list = malloc(sizeof(int64_t) * (size_t)(2 + len));
    list[0] = (int64_t)(list + 2);
    list[1] = len;
    for (int64_t i = 0; i < len; i++) {
        list[2 + i] = (int64_t)(unsigned char)s[i];
    }
    free(s);
    return (int64_t)list;
}

int64_t json_parse(int64_t s_val) {
    // Minimal JSON object parser — returns list of (key, value) pairs
    char* s = brief_str_to_c(s_val);
    if (!s) return 0;
    return (int64_t)s; // placeholder — full JSON parser needed
}

int64_t json_is_array(int64_t v_val) {
    (void)v_val;
    return 0;
}

int64_t json_length(int64_t v_val) {
    (void)v_val;
    return 0;
}

int64_t json_get(int64_t v_val, int64_t key_val) {
    (void)v_val;
    (void)key_val;
    return 0;
}

int64_t json_get_by_index(int64_t v_val, int64_t i_val) {
    (void)v_val;
    (void)i_val;
    return 0;
}

/* ===================================================================
 * 7. Brief String Helpers — convert between Brief string format and C
 *
 * Brief string format:
 *   int64_t header[2 + len]
 *   header[0] = data pointer (address of header[2])
 *   header[1] = length (int64_t)
 *   header[2..2+len] = character data (one int64_t per char)
 *
 * An int64_t parameter is ptrtoint(header) — a pointer to header[0].
 * =================================================================== */

/* Convert a Brief string pointer to a null-terminated C string. Caller frees. */
static char* brief_str_to_c(int64_t bstr) {
    int64_t* h = (int64_t*)bstr;
    if (!h) return NULL;
    int64_t len = h[1];
    if (len <= 0) return NULL;
    char* s = malloc((size_t)len + 1);
    if (!s) return NULL;
    for (int64_t i = 0; i < len; i++) {
        s[i] = (char)(h[i + 2] & 0xFF);
    }
    s[len] = '\0';
    return s;
}

/* Create a Brief string from a C null-terminated string. Returns ptrtoint.
   The caller receives ownership of the heap-allocated header. */
static int64_t cstr_to_brief(const char* s) {
    if (!s) return 0;
    size_t len = strlen(s);
    if (len == 0) return 0;
    int64_t* h = malloc((len + 2) * sizeof(int64_t));
    if (!h) return 0;
    h[0] = (int64_t)(h + 2);
    h[1] = (int64_t)len;
    for (size_t i = 0; i < len; i++) {
        h[i + 2] = (int64_t)((unsigned char)s[i]);
    }
    return (int64_t)h;
}

/* Create a Brief string from a buffer with known length. Returns ptrtoint. */
static int64_t buf_to_brief(const char* buf, int64_t len) {
    if (!buf || len <= 0) return 0;
    int64_t* h = malloc(((size_t)len + 2) * sizeof(int64_t));
    if (!h) return 0;
    h[0] = (int64_t)(h + 2);
    h[1] = len;
    for (int64_t i = 0; i < len; i++) {
        h[i + 2] = (int64_t)((unsigned char)buf[i]);
    }
    return (int64_t)h;
}

/* Constructor wrapper — ensures init runs even without the generated main() */
__attribute__((constructor))
static void brief_rt_ctor(void) {
    __rt_init();
}