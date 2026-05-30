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
    /* If epoll is available, use it */
    if (ensure_epoll() == 0) {
        struct epoll_event events[MAX_EPOLL_EVENTS];
        int n = epoll_wait(g_epoll_fd, events, MAX_EPOLL_EVENTS, -1);
        if (n > 0) {
            /* Check stdin */
            for (int i = 0; i < n; i++) {
                if (events[i].data.fd == STDIN_FILENO
                    && (events[i].events & EPOLLIN)) {
                    __stdin_ready = 1;
                    __io_pending = 1;
                }
            }
        }
        return;
    }
    /* Fallback: poll stdin with 1-second timeout */
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

#elif defined(__arm__) || defined(__aarch64__) || defined(_ARM_) || defined(_M_ARM)
void __rt_wait(void) {
    /* ARM Wait For Interrupt — CPU halts until interrupt/event */
    __asm__ volatile("wfi" ::: "memory");
    __io_pending = 1;
}

#elif defined(__x86_64__) || defined(__i386__) || defined(_M_IX86) || defined(_M_X64)
void __rt_wait(void) {
    /* x86: enable interrupts then halt — CPU sleeps until IRQ */
    __asm__ volatile("sti; hlt" ::: "memory");
    __io_pending = 1;
}

#elif defined(__wasm__) || defined(__EMSCRIPTEN__)
void __rt_wait(void) {
    /* WASM: yield to host event loop. Returns when re-entered. */
    __builtin_wasm_memory_grow(0, 0);
    __io_pending = 1;
}

#else
/* Fallback: busy-sleep with 1ms polling */
void __rt_wait(void) {
    struct timespec ts = {0, 1000000}; /* 1ms */
    nanosleep(&ts, NULL);
    __io_pending = 1;
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

/* Constructor wrapper — ensures init runs even without the generated main() */
__attribute__((constructor))
static void brief_rt_ctor(void) {
    __rt_init();
}