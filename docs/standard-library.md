# Brief Standard Library

> Extracted from the README (2026-07-31).
### Core Types
- **Char** - Unicode codepoints
- **HashMap<K,V>** - O(1) lookup
- **HashSet<T>** - O(1) membership
- **StringBuilder** - O(n) string building
- **Stack<T>** - LIFO structure
- **Queue<T>** - FIFO structure
- **Result/Option** - Error handling with combinators

### String Processing
- Character classification (`is_whitespace`, `is_digit`, `is_alpha`)
- Case conversion (`to_upper`, `to_lower`, `capitalize`)
- String manipulation (`trim`, `reverse`, `split`, `join`)
- 95% native functions (no FFI)

### Collections
- `List<T>` - Dynamic arrays
- Vector operations
- Sorting, filtering, mapping

### IO & Process
- File I/O (`read_file`, `write_file`, `file_exists`)
- Path operations (`join_path`, `split_path`, `file_extension`)
- Process spawning (`spawn`, `spawn_with_output`)
- Environment access (`env_var`, `current_dir`)

### OS Prelude (auto-imported, 20 modules)
The `std/os/` modules replace 127 former compiler intrinsics:
- **fs.bv** — open, close, read, write, lseek, pread, pwrite, stat, ftruncate, fsync, dup, fcntl
- **net.bv** — socket, bind, listen, accept, connect, send, recv, setsockopt, getaddrinfo
- **dir.bv** — mkdir, rmdir, unlink, rename, symlink, readlink, getcwd, readdir, chmod, chown, access
- **thread.bv** — thread_create, thread_join, mutex_lock, mutex_unlock, condvar_wait, condvar_signal
- **atomic.bv** — atomic_load, atomic_store, atomic_cas, atomic_xchg, atomic_add, fence, futex
- **mem.bv** — mmap, munmap, mprotect, brk, mlock
- **process.bv** — spawn, getpid, getppid, exit, abort, sleep
- **signal.bv** — sigaction, sigprocmask, kill, signal_fd, timerfd_create
- **time.bv** — clock_gettime, nanosleep, time
- **ipc.bv** — pipe, shm_open, shm_unlink, sem_open, sem_wait, sem_post
- **io.bv** — print, println, readln, get_env, set_env
- **tty.bv** — tty_raw_mode, tty_size, tty_read_key, ioctl, isatty
- **user.bv** — getuid, geteuid, getgid, getegid
- **sched.bv** — sched_yield, getpriority, setpriority
- **resource.bv** — getrlimit, setrlimit
- **sysinfo.bv** — uname, hostname, realpath, pagesize, cpu_count
- **dynlib.bv** — dlopen, dlsym, dlclose
- **debug.bv** — backtrace, halt, abort
- **temp.bv** — mkstemp, mkdtemp
- **ring.bv** — ring_push, ring_pop
- **rand.bv** — getrandom, errno

### Iterators
- `map`, `filter`, `fold`
- `take`, `skip`, `zip`, `chain`
- `sum`, `product`, `min`, `max`
- `find`, `any`, `all`

**Total:** 300+ native functions across 15+ modules + 20 auto-imported OS modules
