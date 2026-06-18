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
#include <unistd.h>
#include <fcntl.h>
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
volatile char      __tty_read_key  __attribute__((section("brief_trg")));
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
 * 1.5 I/O helpers (std/io.bv)
 * =================================================================== */
int64_t __print(const char* msg) {
    if (msg) fputs(msg, stdout);
    fflush(stdout);
    return 1;
}
int64_t __print_int(int64_t n) {
    char buf[32];
    int len = snprintf(buf, sizeof(buf), "%ld", n);
    fwrite(buf, 1, len, stdout);
    fflush(stdout);
    return 1;
}
void __exit(int64_t code) {
    exit((int)code);
}

/* ===================================================================
 * 1.8a String concatenation helper
 *
 * Used by the LLVM backend's emit_binop for String + String.
 * Allocates a new buffer containing a + b.
 * =================================================================== */

/* ===================================================================
 * 1.9 Built-in trigger source wrappers
 *
 * These are called by the LLVM backend for @stdin#, @timer#(hz),
 * and @signal#(name) trigger declarations.  Each returns int32_t:
 *   stdin  — the byte read (0 if none available)
 *   timer  — number of timer ticks (0 on error)
 *   signal — signal number that fired (0 on error)
 * =================================================================== */

/* ── Cast helpers ─────────────────────────────────────────────── */

/* Convert a 32-bit character code to a 1-character string. */
char* __chr_to_str(int32_t c) {
    static char buf[2] = {0, 0};
    buf[0] = (char)(c & 0xFF);
    return buf;
}

#if defined(__linux__)
#include <sys/timerfd.h>
#include <sys/signalfd.h>

/* Create a timerfd at N Hz. Returns fd, or -1 on error. */
int32_t __trg_timerfd_open(int64_t hz) {
    if (hz <= 0) return -1;
    int fd = timerfd_create(CLOCK_MONOTONIC, TFD_NONBLOCK);
    if (fd < 0) return -1;
    long ns_per_tick = 1000000000L / hz;
    struct itimerspec spec = {
        .it_interval = { .tv_sec = ns_per_tick / 1000000000L,
                         .tv_nsec = ns_per_tick % 1000000000L },
        .it_value    = { .tv_sec = ns_per_tick / 1000000000L,
                         .tv_nsec = ns_per_tick % 1000000000L },
    };
    if (timerfd_settime(fd, 0, &spec, NULL) < 0) { close(fd); return -1; }
    return fd;
}

/* Read timerfd expiration count. Returns 1 if timer fired, 0 otherwise. */
int32_t __trg_timerfd_read(int32_t fd) {
    uint64_t expirations = 0;
    ssize_t n = read(fd, &expirations, sizeof(expirations));
    return (n > 0 && expirations > 0) ? 1 : 0;
}

/* Open a signalfd for the given signal name. Returns fd, or -1 on error. */
int32_t __trg_signalfd_open(const char* name) {
    int sig = 0;
    if      (strcmp(name, "SIGINT")   == 0) sig = SIGINT;
    else if (strcmp(name, "SIGTERM")  == 0) sig = SIGTERM;
    else if (strcmp(name, "SIGHUP")   == 0) sig = SIGHUP;
    else if (strcmp(name, "SIGUSR1")  == 0) sig = SIGUSR1;
    else if (strcmp(name, "SIGUSR2")  == 0) sig = SIGUSR2;
    else return -1;
    sigset_t mask;
    sigemptyset(&mask);
    sigaddset(&mask, sig);
    sigprocmask(SIG_BLOCK, &mask, NULL);
    int fd = signalfd(-1, &mask, SFD_NONBLOCK);
    return fd;
}

/* Read signalfd event. Returns 1 if signal fired, 0 otherwise. */
int32_t __trg_signalfd_read(int32_t fd) {
    struct signalfd_siginfo info;
    ssize_t n = read(fd, &info, sizeof(info));
    return (n > 0) ? 1 : 0;
}

#else
/* Non-Linux: timerfd/signalfd stubs (return error sentinel) */
int32_t __trg_timerfd_open(int64_t hz) { (void)hz; return -1; }
int32_t __trg_timerfd_read(int32_t fd) { (void)fd; return 0; }
int32_t __trg_signalfd_open(const char* name) { (void)name; return -1; }
int32_t __trg_signalfd_read(int32_t fd) { (void)fd; return 0; }
#endif

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
    setvbuf(stderr, NULL, _IOFBF, 65536);  /* buffer FFI stderr output (e.g. fasta __putchar) */

    /* Set stdin to non-blocking mode for @stdin# trigger */
    int stdin_flags = fcntl(STDIN_FILENO, F_GETFL, 0);
    fcntl(STDIN_FILENO, F_SETFL, stdin_flags | O_NONBLOCK);

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

// Forward declaration for brief_str_to_c (defined in section 7)
char* brief_str_to_c(int64_t bstr);

/* Write data (Brief string) to path (Brief string). Returns 1 (true) on success, 0 (false) on failure. */
int64_t brief_write_file(int64_t path_bstr, int64_t data_bstr) {
    char* c_path = brief_str_to_c(path_bstr);
    if (!c_path) return 0;
    
    int64_t* dh = (int64_t*)data_bstr;
    if (!dh) { free(c_path); return 0; }
    int64_t data_len = dh[1];
    char* data = (char*)(dh + 2);
    
    FILE* fp = fopen(c_path, "w");
    free(c_path);
    if (!fp) return 0;
    
    fwrite(data, 1, (size_t)data_len, fp);
    fclose(fp);
    return 1;
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
char*   brief_str_to_c(int64_t bstr);
static int64_t cstr_to_brief(const char* s);
static int64_t buf_to_brief(const char* buf, int64_t len);

/* ── Reactor wait ─────────────────────────────────────────────────── */

void __rt_wait(void) {
    __io_pending = 1;
}

void __rt_poll(void) {
    __io_pending = 1;
}

/* ── Terminal ─────────────────────────────────────────────────────── */

int64_t tty_raw_mode(int64_t enable) {
    int64_t r = *((volatile int64_t*)&enable); // read arg to prevent elimination
    return r ? 1 : 0; // placeholder — real termios requires platform includes
}

int64_t tty_size(void) {
    return (int64_t)(80 * 10000 + 24); // placeholder — real ioctl
}

// tty_read_key is now a volatile char global (see @ link globals above),
// set by __rt_wait()/__rt_poll() when stdin data is available.

/* ===================================================================
 * Phase A: #-intrinsic implementations (intrinsics.md)
 *
 * These are called by the LLVM backend's emit_expr for name#() calls.
 * All take and return int64_t (i64 in LLVM IR). String parameters
 * follow the Brief string format (brief_str_to_c / cstr_to_brief).
 * =================================================================== */

#include <termios.h>
#include <sys/ioctl.h>

int64_t brief_tty_raw_mode(int64_t enable) {
    static struct termios orig_termios;
    static int saved = 0;
    if (enable) {
        struct termios raw;
        if (tcgetattr(STDIN_FILENO, &orig_termios) != 0) return 0;
        raw = orig_termios;
        cfmakeraw(&raw);
        if (tcsetattr(STDIN_FILENO, TCSANOW, &raw) != 0) return 0;
        saved = 1;
        return 1;
    } else {
        if (!saved) return 1;
        if (tcsetattr(STDIN_FILENO, TCSANOW, &orig_termios) != 0) return 0;
        saved = 0;
        return 1;
    }
}

int64_t brief_tty_size(void) {
    struct winsize ws;
    if (ioctl(STDOUT_FILENO, TIOCGWINSZ, &ws) == 0 && ws.ws_col > 0) {
        return (int64_t)(ws.ws_col) * 10000 + ws.ws_row;
    }
    return (int64_t)(80 * 10000 + 24);
}

int64_t brief_tty_read_key(void) {
    unsigned char ch = 0;
    ssize_t n = read(STDIN_FILENO, &ch, 1);
    if (n > 0) return (int64_t)ch;
    return -1;
}

int64_t brief_ioctl(int64_t fd, int64_t request, int64_t arg) {
    return (int64_t)ioctl((int)fd, (unsigned long)request, (void*)(uintptr_t)arg);
}

int64_t brief_isatty(int64_t fd) {
    return (int64_t)isatty((int)fd);
}

int64_t brief_spawn_with_output(int64_t cmd_bstr) {
    char* c_cmd = brief_str_to_c(cmd_bstr);
    if (!c_cmd) return 0;

    FILE* fp = popen(c_cmd, "r");
    free(c_cmd);

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

int64_t brief_spawn(int64_t cmd_bstr) {
    char* c_cmd = brief_str_to_c(cmd_bstr);
    if (!c_cmd) return -1;

    int status = system(c_cmd);
    free(c_cmd);

    if (status < 0) return -1;
    return (int64_t)WEXITSTATUS(status);
}

/* ── Stdlib __ functions ──────────────────────────────────────────── */

int64_t __trim_left(const char* s) {
    if (!s) return 0;
    const char* start = s;
    while (*start && (unsigned char)*start <= 32) start++;
    return cstr_to_brief(start);
}

int64_t __trim_right(const char* s) {
    if (!s) return 0;
    size_t len = strlen(s);
    const char* end = s + len;
    while (end > s && (unsigned char)*(end - 1) <= 32) end--;
    // Copy since we need to null-terminate
    char buf[65536];
    size_t new_len = (size_t)(end - s);
    if (new_len >= sizeof(buf)) new_len = sizeof(buf) - 1;
    memcpy(buf, s, new_len);
    buf[new_len] = '\0';
    return cstr_to_brief(buf);
}

int64_t __to_lower(const char* s) {
    if (!s) return 0;
    size_t len = strlen(s);
    if (len > 65535) len = 65535;
    char buf[65536];
    for (size_t i = 0; i < len; i++) {
        buf[i] = (char)tolower((unsigned char)s[i]);
    }
    buf[len] = '\0';
    return cstr_to_brief(buf);
}

int64_t __contains_at(const char* haystack, const char* needle, int64_t start) {
    if (!haystack || !needle) return 0;
    const char* pos = strstr(haystack + (size_t)start, needle);
    return pos ? 1 : 0;
}

int64_t __find_from(const char* s, const char* needle, int64_t start) {
    if (!s || !needle) return -1;
    const char* found = strstr(s + (size_t)start, needle);
    return found ? (int64_t)(found - s) : -1;
}

int64_t __int_to_str(int64_t n) {
    char buf[64];
    int len = snprintf(buf, sizeof(buf), "%lld", (long long)n);
    int64_t result = buf_to_brief(buf, (int64_t)len);
    fprintf(stderr, "DEBUG __int_to_str(%lld) = 0x%llx\n", (long long)n, (unsigned long long)result);
    return result;
}

int64_t __splitn(const char* s, const char* delim, int64_t n_val) {
    (void)n_val;
    if (!s || !delim) return 0;
    size_t slen = strlen(s);
    if (slen > 65500) slen = 65500;
    char tmp[65536];
    memcpy(tmp, s, slen);
    tmp[slen] = '\0';
    size_t dlen = strlen(delim);
    // Count tokens
    int64_t count = 1;
    for (char* p = tmp; *p; p++) {
        if (strncmp(p, delim, dlen) == 0) { count++; p += dlen - 1; }
    }
    // Allocate list header: [data_ptr, length, elem0, elem1, ...]
    int64_t* list = malloc(sizeof(int64_t) * (2 + count));
    list[0] = (int64_t)(list + 2);
    list[1] = count;
    int64_t idx = 0;
    char* tok = strtok(tmp, delim);
    while (tok && idx < count) {
        list[2 + idx] = cstr_to_brief(tok);
        idx++;
        tok = strtok(NULL, delim);
    }
    return (int64_t)list;
}

/* ── Process ──────────────────────────────────────────────────────── */

int64_t __spawn_with_output(const char* cmd, int64_t args_val) {
    (void)args_val;
    if (!cmd) return 0;
    FILE* fp = popen(cmd, "r");
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

/* ===================================================================
 * Phase B: Raw File I/O intrinsics (intrinsics.md D2)
 *
 * Wraps POSIX fd operations. All return int64_t (-1 on error except
 * read/write/pread/pwrite which return bytes transferred, also -1 on
 * error). Path parameters follow Brief string format.
 * =================================================================== */

#include <fcntl.h>
#include <unistd.h>
#include <sys/stat.h>

int64_t brief_open(int64_t path_bstr, int64_t flags, int64_t mode) {
    char* c_path = brief_str_to_c(path_bstr);
    if (!c_path) return -1;
    int fd = open(c_path, (int)flags, (mode_t)mode);
    free(c_path);
    return (int64_t)fd;
}

int64_t brief_close(int64_t fd) {
    int ret = close((int)fd);
    return (int64_t)ret;
}

int64_t brief_read(int64_t fd, int64_t buf, int64_t count) {
    ssize_t n = read((int)fd, (void*)(uintptr_t)buf, (size_t)count);
    return (int64_t)n;
}

int64_t brief_write(int64_t fd, int64_t buf, int64_t count) {
    ssize_t n = write((int)fd, (void*)(uintptr_t)buf, (size_t)count);
    return (int64_t)n;
}

int64_t brief_lseek(int64_t fd, int64_t offset, int64_t whence) {
    off_t off = lseek((int)fd, (off_t)offset, (int)whence);
    return (int64_t)off;
}

int64_t brief_pread(int64_t fd, int64_t buf, int64_t count, int64_t offset) {
    ssize_t n = pread((int)fd, (void*)(uintptr_t)buf, (size_t)count, (off_t)offset);
    return (int64_t)n;
}

int64_t brief_pwrite(int64_t fd, int64_t buf, int64_t count, int64_t offset) {
    ssize_t n = pwrite((int)fd, (void*)(uintptr_t)buf, (size_t)count, (off_t)offset);
    return (int64_t)n;
}

int64_t brief_stat(int64_t path_bstr) {
    char* c_path = brief_str_to_c(path_bstr);
    if (!c_path) return -1;
    struct stat st;
    int ret = stat(c_path, &st);
    free(c_path);
    return (int64_t)ret;
}

int64_t brief_fstat(int64_t fd) {
    struct stat st;
    int ret = fstat((int)fd, &st);
    return (int64_t)ret;
}

int64_t brief_truncate(int64_t path_bstr, int64_t len) {
    char* c_path = brief_str_to_c(path_bstr);
    if (!c_path) return -1;
    int ret = truncate(c_path, (off_t)len);
    free(c_path);
    return (int64_t)ret;
}

int64_t brief_ftruncate(int64_t fd, int64_t len) {
    int ret = ftruncate((int)fd, (off_t)len);
    return (int64_t)ret;
}

int64_t brief_fsync(int64_t fd) {
    int ret = fsync((int)fd);
    return (int64_t)ret;
}

int64_t brief_dup(int64_t fd) {
    int newfd = dup((int)fd);
    return (int64_t)newfd;
}

int64_t brief_dup2(int64_t old, int64_t newfd) {
    int ret = dup2((int)old, (int)newfd);
    return (int64_t)ret;
}

int64_t brief_fcntl(int64_t fd, int64_t cmd, int64_t arg) {
    int ret = fcntl((int)fd, (int)cmd, (long)arg);
    return (int64_t)ret;
}

/* ===================================================================
 * Phase C: Filesystem intrinsics (intrinsics.md D3)
 *
 * Wraps POSIX filesystem operations. Path parameters follow Brief string
 * format. readlink/getcwd return Brief string (0 on error). readdir
 * returns Brief List<string> (0 on error).
 * =================================================================== */

#include <dirent.h>

int64_t brief_mkdir(int64_t path_bstr, int64_t mode) {
    char* c_path = brief_str_to_c(path_bstr);
    if (!c_path) return -1;
    int ret = mkdir(c_path, (mode_t)mode);
    free(c_path);
    return (int64_t)ret;
}

int64_t brief_rmdir(int64_t path_bstr) {
    char* c_path = brief_str_to_c(path_bstr);
    if (!c_path) return -1;
    int ret = rmdir(c_path);
    free(c_path);
    return (int64_t)ret;
}

int64_t brief_unlink(int64_t path_bstr) {
    char* c_path = brief_str_to_c(path_bstr);
    if (!c_path) return -1;
    int ret = unlink(c_path);
    free(c_path);
    return (int64_t)ret;
}

int64_t brief_rename(int64_t old_bstr, int64_t new_bstr) {
    char* c_old = brief_str_to_c(old_bstr);
    if (!c_old) return -1;
    char* c_new = brief_str_to_c(new_bstr);
    if (!c_new) { free(c_old); return -1; }
    int ret = rename(c_old, c_new);
    free(c_old); free(c_new);
    return (int64_t)ret;
}

int64_t brief_symlink(int64_t target_bstr, int64_t link_bstr) {
    char* c_target = brief_str_to_c(target_bstr);
    if (!c_target) return -1;
    char* c_link = brief_str_to_c(link_bstr);
    if (!c_link) { free(c_target); return -1; }
    int ret = symlink(c_target, c_link);
    free(c_target); free(c_link);
    return (int64_t)ret;
}

int64_t brief_readlink(int64_t path_bstr) {
    char* c_path = brief_str_to_c(path_bstr);
    if (!c_path) return 0;
    char buf[4096];
    ssize_t n = readlink(c_path, buf, sizeof(buf) - 1);
    free(c_path);
    if (n < 0) return 0;
    buf[n] = '\0';
    return cstr_to_brief(buf);
}

int64_t brief_link(int64_t old_bstr, int64_t new_bstr) {
    char* c_old = brief_str_to_c(old_bstr);
    if (!c_old) return -1;
    char* c_new = brief_str_to_c(new_bstr);
    if (!c_new) { free(c_old); return -1; }
    int ret = link(c_old, c_new);
    free(c_old); free(c_new);
    return (int64_t)ret;
}

int64_t brief_getcwd(void) {
    char buf[4096];
    if (!getcwd(buf, sizeof(buf))) return 0;
    return cstr_to_brief(buf);
}

int64_t brief_chdir(int64_t path_bstr) {
    char* c_path = brief_str_to_c(path_bstr);
    if (!c_path) return -1;
    int ret = chdir(c_path);
    free(c_path);
    return (int64_t)ret;
}

int64_t brief_readdir(int64_t path_bstr) {
    char* c_path = brief_str_to_c(path_bstr);
    if (!c_path) return 0;
    DIR* dir = opendir(c_path);
    free(c_path);
    if (!dir) return 0;

    // Count entries
    struct dirent* entry;
    int count = 0;
    rewinddir(dir);
    while ((entry = readdir(dir)) != NULL) count++;
    rewinddir(dir);

    // Allocate Brief list header: [data_ptr, size, str1, str2, ...]
    int64_t* list = malloc(sizeof(int64_t) * (size_t)(2 + count));
    if (!list) { closedir(dir); return 0; }
    list[0] = (int64_t)(list + 2);
    list[1] = count;

    int i = 0;
    while ((entry = readdir(dir)) != NULL && i < count) {
        list[2 + i] = cstr_to_brief(entry->d_name);
        i++;
    }
    closedir(dir);
    return (int64_t)list;
}

int64_t brief_chmod(int64_t path_bstr, int64_t mode) {
    char* c_path = brief_str_to_c(path_bstr);
    if (!c_path) return -1;
    int ret = chmod(c_path, (mode_t)mode);
    free(c_path);
    return (int64_t)ret;
}

int64_t brief_chown(int64_t path_bstr, int64_t uid, int64_t gid) {
    char* c_path = brief_str_to_c(path_bstr);
    if (!c_path) return -1;
    int ret = chown(c_path, (uid_t)uid, (gid_t)gid);
    free(c_path);
    return (int64_t)ret;
}

int64_t brief_umask(int64_t mask) {
    mode_t old = umask((mode_t)mask);
    return (int64_t)old;
}

int64_t brief_access(int64_t path_bstr, int64_t mode) {
    char* c_path = brief_str_to_c(path_bstr);
    if (!c_path) return -1;
    int ret = access(c_path, (int)mode);
    free(c_path);
    return (int64_t)ret;
}

/* ===================================================================
 * Phase D: Memory + Synchronization intrinsics (intrinsics.md D1 + D9)
 *
 * Memory operations (mmap, munmap, mprotect, brk, mlock) are Shim
 * category — libc wrappers. futex is also Shim — syscall via libc.
 * Atomic operations (load/store/cas/xchg/add/fence) are Native category
 * — emitted as LLVM atomic IR, no C implementation needed.
 * =================================================================== */

#include <sys/mman.h>
#include <sys/syscall.h>
#include <unistd.h>

int64_t brief_mmap(int64_t addr, int64_t length, int64_t prot, int64_t flags, int64_t fd, int64_t offset) {
    void* ret = mmap((void*)(uintptr_t)addr, (size_t)length, (int)prot, (int)flags, (int)fd, (off_t)offset);
    if (ret == MAP_FAILED) return -1;
    return (int64_t)(uintptr_t)ret;
}

int64_t brief_munmap(int64_t addr, int64_t length) {
    return (int64_t)munmap((void*)(uintptr_t)addr, (size_t)length);
}

int64_t brief_mprotect(int64_t addr, int64_t length, int64_t prot) {
    return (int64_t)mprotect((void*)(uintptr_t)addr, (size_t)length, (int)prot);
}

int64_t brief_brk(int64_t addr) {
    // brk() is not on all systems; use sbrk(0) for query, brk() for set
    if (addr == 0) {
        void* cur = sbrk(0);
        return (int64_t)(uintptr_t)cur;
    }
    int ret = brk((void*)(uintptr_t)addr);
    return (int64_t)ret;
}

int64_t brief_mlock(int64_t addr, int64_t length) {
    return (int64_t)mlock((void*)(uintptr_t)addr, (size_t)length);
}

int64_t brief_futex(int64_t uaddr, int64_t op, int64_t val, int64_t timeout, int64_t uaddr2, int64_t val3) {
    (void)uaddr; (void)op; (void)val; (void)timeout; (void)uaddr2; (void)val3;
    // Futex is architecture-dependent; stub returns -1 (unsupported)
    return -1;
}

/* ===================================================================
 * Phase E: IPC intrinsics (intrinsics.md D11)
 *
 * Shared memory (shm_open/shm_unlink) and POSIX semaphores
 * (sem_open/sem_wait/sem_post). Pipe wraps pipe(2) syscall.
 * =================================================================== */

#include <semaphore.h>
#include <sys/mman.h>

int64_t brief_pipe(int64_t fds) {
    return (int64_t)pipe((int*)(uintptr_t)fds);
}

int64_t brief_shm_open(int64_t name_bstr, int64_t flags, int64_t mode) {
    char* c_name = brief_str_to_c(name_bstr);
    if (!c_name) return -1;
    int fd = shm_open(c_name, (int)flags, (mode_t)mode);
    free(c_name);
    return (int64_t)fd;
}

int64_t brief_shm_unlink(int64_t name_bstr) {
    char* c_name = brief_str_to_c(name_bstr);
    if (!c_name) return -1;
    int ret = shm_unlink(c_name);
    free(c_name);
    return (int64_t)ret;
}

int64_t brief_sem_open(int64_t name_bstr, int64_t flags, int64_t mode, int64_t value) {
    char* c_name = brief_str_to_c(name_bstr);
    if (!c_name) return -1;
    sem_t* sem = sem_open(c_name, (int)flags, (mode_t)mode, (unsigned)value);
    free(c_name);
    return (int64_t)(uintptr_t)sem;
}

int64_t brief_sem_wait(int64_t sem) {
    return (int64_t)sem_wait((sem_t*)(uintptr_t)sem);
}

int64_t brief_sem_post(int64_t sem) {
    return (int64_t)sem_post((sem_t*)(uintptr_t)sem);
}

/* ===================================================================
 * Phase F: Signals intrinsics (intrinsics.md D8)
 *
 * Signal handling (sigaction, sigprocmask, kill) and Linux-specific
 * signalfd/timerfd_create for reactive trigger sources.
 * =================================================================== */

#include <signal.h>
#include <sys/signalfd.h>
#include <sys/timerfd.h>

int64_t brief_sigaction(int64_t signum, int64_t handler) {
    struct sigaction sa;
    struct sigaction old;
    sa.sa_handler = (void(*)(int))(uintptr_t)handler;
    sigemptyset(&sa.sa_mask);
    sa.sa_flags = 0;
    return (int64_t)sigaction((int)signum, &sa, &old);
}

int64_t brief_sigprocmask(int64_t how, int64_t mask) {
    sigset_t set;
    (void)mask;
    sigemptyset(&set);
    return (int64_t)sigprocmask((int)how, &set, NULL);
}

int64_t brief_kill(int64_t pid, int64_t sig) {
    return (int64_t)kill((pid_t)pid, (int)sig);
}

int64_t brief_signalfd(int64_t mask) {
    sigset_t set;
    sigemptyset(&set);
    (void)mask;
    return (int64_t)signalfd(-1, &set, SFD_NONBLOCK);
}

int64_t brief_timerfd_create(int64_t hz) {
    int fd = timerfd_create(CLOCK_MONOTONIC, TFD_NONBLOCK);
    if (fd < 0) return -1;
    if (hz > 0) {
        long nsec = 1000000000L / hz;
        struct itimerspec spec;
        spec.it_interval.tv_sec = 0;
        spec.it_interval.tv_nsec = nsec;
        spec.it_value.tv_sec = 0;
        spec.it_value.tv_nsec = nsec;
        timerfd_settime(fd, 0, &spec, NULL);
    }
    return (int64_t)fd;
}

/* ===================================================================
 * Phase G: Networking intrinsics (intrinsics.md D10)
 *
 * POSIX socket API: socket, bind, listen, accept, connect, send, recv,
 * sendto, recvfrom, setsockopt, getsockopt, shutdown, getaddrinfo.
 * =================================================================== */

#include <sys/socket.h>
#include <netdb.h>

int64_t brief_socket(int64_t domain, int64_t type, int64_t protocol) {
    return (int64_t)socket((int)domain, (int)type, (int)protocol);
}

int64_t brief_bind(int64_t fd, int64_t addr, int64_t addrlen) {
    return (int64_t)bind((int)fd, (const struct sockaddr*)(uintptr_t)addr, (socklen_t)addrlen);
}

int64_t brief_listen(int64_t fd, int64_t backlog) {
    return (int64_t)listen((int)fd, (int)backlog);
}

int64_t brief_accept(int64_t fd, int64_t addr, int64_t addrlen) {
    socklen_t len = (socklen_t)addrlen;
    return (int64_t)accept((int)fd, (struct sockaddr*)(uintptr_t)addr, &len);
}

int64_t brief_connect(int64_t fd, int64_t addr, int64_t addrlen) {
    return (int64_t)connect((int)fd, (const struct sockaddr*)(uintptr_t)addr, (socklen_t)addrlen);
}

int64_t brief_send(int64_t fd, int64_t buf, int64_t len, int64_t flags) {
    return (int64_t)send((int)fd, (const void*)(uintptr_t)buf, (size_t)len, (int)flags);
}

int64_t brief_recv(int64_t fd, int64_t buf, int64_t len, int64_t flags) {
    return (int64_t)recv((int)fd, (void*)(uintptr_t)buf, (size_t)len, (int)flags);
}

int64_t brief_sendto(int64_t fd, int64_t buf, int64_t len, int64_t flags, int64_t dest_addr, int64_t addrlen) {
    return (int64_t)sendto((int)fd, (const void*)(uintptr_t)buf, (size_t)len, (int)flags,
                           (const struct sockaddr*)(uintptr_t)dest_addr, (socklen_t)addrlen);
}

int64_t brief_recvfrom(int64_t fd, int64_t buf, int64_t len, int64_t flags, int64_t src_addr, int64_t addrlen) {
    socklen_t slen = (socklen_t)addrlen;
    return (int64_t)recvfrom((int)fd, (void*)(uintptr_t)buf, (size_t)len, (int)flags,
                             (struct sockaddr*)(uintptr_t)src_addr, &slen);
}

int64_t brief_setsockopt(int64_t fd, int64_t level, int64_t opt, int64_t val, int64_t len) {
    return (int64_t)setsockopt((int)fd, (int)level, (int)opt, (const void*)(uintptr_t)val, (socklen_t)len);
}

int64_t brief_getsockopt(int64_t fd, int64_t level, int64_t opt, int64_t val, int64_t len) {
    socklen_t slen = (socklen_t)len;
    return (int64_t)getsockopt((int)fd, (int)level, (int)opt, (void*)(uintptr_t)val, &slen);
}

int64_t brief_shutdown(int64_t fd, int64_t how) {
    return (int64_t)shutdown((int)fd, (int)how);
}

int64_t brief_getaddrinfo(int64_t node, int64_t service) {
    struct addrinfo hints;
    struct addrinfo *result;
    memset(&hints, 0, sizeof(hints));
    hints.ai_family = AF_UNSPEC;
    hints.ai_socktype = SOCK_STREAM;
    int ret = getaddrinfo((const char*)(uintptr_t)node, (const char*)(uintptr_t)service, &hints, &result);
    if (ret == 0 && result) freeaddrinfo(result);
    return (int64_t)ret;
}

/* ===================================================================
 * Phase H: Everything Else (intrinsics.md D6, D7)
 *
 * Environment variables, process info, and timing.
 * =================================================================== */

#include <unistd.h>
#include <time.h>

int64_t brief_getenv(int64_t name) {
    const char* s = getenv((const char*)(uintptr_t)name);
    if (!s) return 0;
    return (int64_t)s;
}

int64_t brief_setenv(int64_t name, int64_t value) {
    return (int64_t)setenv((const char*)(uintptr_t)name, (const char*)(uintptr_t)value, 1);
}

int64_t brief_unsetenv(int64_t name) {
    return (int64_t)unsetenv((const char*)(uintptr_t)name);
}

int64_t brief_getpid(void) {
    return (int64_t)getpid();
}

int64_t brief_getppid(void) {
    return (int64_t)getppid();
}

int64_t brief_clock_gettime(int64_t clock_id) {
    struct timespec ts;
    if (clock_gettime((clockid_t)clock_id, &ts) == 0) {
        return ts.tv_sec * 1000000000L + ts.tv_nsec;
    }
    return 0;
}

int64_t brief_nanosleep(int64_t ns) {
    struct timespec req;
    struct timespec rem;
    req.tv_sec = ns / 1000000000L;
    req.tv_nsec = ns % 1000000000L;
    return (int64_t)nanosleep(&req, &rem);
}

/* ── Officina-local frgn (substring only) ────────────────────────── */

int64_t substring(const char* s) {
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
    return (int64_t)list;
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
char* brief_str_to_c(int64_t bstr) {
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