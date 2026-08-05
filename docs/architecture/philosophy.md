# Safety as Optimization: The Briv Philosophy

**Date:** 2026-06-17

---

## Core Thesis

Briv's contract system (`[pre][post]`) is not a correctness tax you pay
for verification — it is information the compiler uses to optimize harder.

Where C++ requires the programmer to mentally track aliasing, bounds, and
lifetime to write fast code, Briv requires the same information *as
structured contracts*. The compiler then feeds that structure into LLVM as
`!range`, `noalias`, `dereferenceable`, and alignment metadata.

The goal is C++–competitive performance through a system that *checks* the
invariants C++ assumes at the programmer's risk. Full machine access —
pointers, syscalls, manual memory — is available through Briv-y means:
contracts proven at compile time, not `unsafe` blocks.

---

## Key Claims

### 1. Safety Is an Optimization Enabler

Every contract — `[i < list .#Size]`, `[ptr + offset < end]`,
`[result != null]` — is a fact the compiler can prove at compile time and
emit as LLVM metadata. C++ `restrict` is a programmer promise; Briv
`[!aliased]` is a machine-checked guarantee. The optimizer can rely on it
absolutely.

The A005c dual-path store strategy is the canonical example: when the
compiler proves `done:` does not read `%State`, it suppresses all stores
in the hot loop body (Path A). C preserves every store through function
call barriers; Briv's contract structure lets the compiler see through
them.

### 2. Full Machine Access Through Contracts, Not Escapes

Pointers, raw memory, and syscalls are all available. The difference is they
come with contracts that bound, constrain, and verify.

An arena allocator whose contract is `[ptr + N < end]` gives the optimizer
*more* information than `malloc(N)` ever could. The compiler knows:

- The allocation never exceeds the arena bounds
- The arena is a flat, non-aliasing region
- All arena allocations die when the arena dies

You do not drop to `unsafe` — you prove. The proof IS the optimizer's input.

### 3. The Compiler Can Be Smarter Than the Programmer

Precomputation folding, dead-field elimination, and dispatch-chain collapse
are not possible in C++ because the optimizer cannot prove the invariants
that Briv's contracts make explicit.

When a transaction bound is a compile-time constant (`const N: Int = 50`),
the interpreter folds ALL iterations before LLVM ever sees the loop.
The generated code is ~`store i64 N, ret`~. The programmer does not write
the loop; the compiler proves it is unnecessary.

Briv accepts that the compiler can know more. The programmer writes intent;
the compiler exploits it.

### 4. Performance Without a Rewrite

A Briv program at `--dev` and `--prod` are the same source. You do not:

- Hand-unroll loops
- Annotate aliasing with `restrict`
- Restructure code for auto-vectorization
- Add `likely`/`unlikely` hints
- Use different allocators per hot path

You write contracts. The compiler does the rest. When you need more
performance, you add *stronger contracts*, not `__builtin_assume`.

### 5. LLVM Is the Right Backend Because It Is the Universal Lowering

LLVM does not understand reactivity, contracts, or convergence loops. It
never needs to. Every optimization LLVM performs — inlining, vectorization,
SROA, LICM, GVN — can be *seeded* by contract-derived metadata.

The bugs we fix in the LLVM backend are the friction of mapping Briv's
novel semantics onto LLVM's classical model. They converge to zero over
time. Every fix makes Briv more resilient for every program.

---

## What This Means for the Backend

### Intrinsics and the C Boundary

Intrinsics that wrap libc are **not failures of the abstraction** — they
are the correct boundary. libc *is* the portable assembly. The novelty is
above that boundary, in the contracts and dispatch system.

Of 86 `#`-intrinsics, 75 emit direct libc calls in LLVM IR. The remaining
11 (`read_file`, `tty_raw_mode`, `spawn_with_output`, `readdir`, `futex`,
`sigaction`, `sigprocmask`, `getaddrinfo`, `barrier_release`,
`barrier_wait`, `thread_pool_init`) are multi-call sequences involving
opaque C structs (`sigset_t`, `DIR*`, `struct addrinfo`). They stay as C
shims in `briv_rt.c` — auto-linked for all native builds, zero user
configuration.

### Reactive Model Is Pure LLVM IR

The reactive event loop — `epoll_create1`, `epoll_ctl`, `epoll_wait`,
dirty-flag dispatch, convergence checks — is entirely generated IR. C has
no role in it. LLVM sees a state machine with branches and phi nodes, and
optimizes it accordingly.

---

## What This Means for the Standard Library

`lib/std/` should provide the higher-level tools that make "Briv-y systems
programming" ergonomic — all backed by contracts, all verifiable at compile
time:

- Arena allocator with `[ptr + N < end]` contracts
- Zero-copy parsers with bounds-proven slices
- Lock-free data structures with non-aliasing guarantees
- `Memory` / `Buffer` types carrying length contracts

The JSON parser in `lib/std/json.bv` is the pattern: pure Briv, recursive
descent, contract-proven bounds on every array and string access, no hidden
allocation, no opaque FFI.

---

## Relationship to Other Languages

| Concern | C++ | Rust | Briv |
|---------|-----|------|-------|
| Safety model | Convention | Borrow checker | Contract checker |
| Optimization source | `restrict`, inline asm | `unsafe`, `#[repr]` | Contract metadata |
| Allocator control | Custom allocators | `Allocator` trait | `Ptr<T>` + contracts |
| SIMD | Intrinsics + auto-vec | `std::simd` + auto-vec | Auto-vec only (today) |
| Error handling | Exceptions / codes | `Result<T,E>` | `Result<T,E>` |
| Proof | External (CBMC, Verifast) | External (Kani, Creusot) | Built-in (preconditions) |

Briv does not replace C++ or Rust. It occupies a different point in the
design space: **contract-proven optimization as the default, not an
afterthought.**

---

## Document History

| Date | Change |
|------|--------|
| 2026-06-17 | Initial write — distilled from compiler correctness discussions |
