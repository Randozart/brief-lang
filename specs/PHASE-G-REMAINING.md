# Phase G: Remaining Architecture & Polish

**Date:** 2026-05-29
**Status:** Plan Document
**Version:** 1.0

This document captures everything left after Phases A–F and the event model
integration. Items grouped by type.

---

## 1. Parallel Dispatch (Architectural)

**Problem:** The dispatch chain evaluates transactions sequentially and fires
the first one with a true precondition. This means keyboard + mouse + timer
cannot all fire in the same tick, even if they write to disjoint fields.

**Cost:** UI and game programs must either:
- (a) Co-locate all input handling in one transaction with guard blocks, or
- (b) Accept multi-tick latency for independent event sources

**Solution:** Evaluate ALL preconditions upfront, then fire every transaction
that is (a) true AND (b) non-conflicting with all previously fired transactions
in this tick. The proof engine's `check_mutual_exclusion` and
`find_read_write_conflicts` already compute the write sets.

### LLVM IR Pattern

```llvm
; Evaluate all preconditions upfront
%pr0 = call i1 @pre_txn0(%State* @global_state)
%pr1 = call i1 @pre_txn1(%State* @global_state)
%pr2 = call i1 @pre_txn2(%State* @global_state)

; Fire txn0 if true (no conflict — first always fires)
br i1 %pr0, label %b0, label %ck1
b0: call void @txn0(%State* @global_state)
    br label %ck1

; Fire txn1 if its precondition is true AND it doesn't conflict with txn0
; (txn0's write set is tracked; txn1's write set is checked against it)
ck1:
br i1 %pr1, label %b1c, label %ck2
b1c:
%nc1 = call i1 @conflicts_txn0_txn1(%State* @global_state)
br i1 %nc1, label %b1, label %ck2
b1: call void @txn1(%State* @global_state)
    br label %ck2

; Same for txn2 — checked against writes from both txn0 and txn1
ck2:
br i1 %pr2, label %b2c, label %ck3
b2c:
%nc2 = call i1 @conflicts_txn0and1_txn2(%State* @global_state)
br i1 %nc2, label %b2, label %ck3
b2: call void @txn2(%State* @global_state)
    br label %ck3

ck3:
ret void
```

### Implementation Notes

- **Conflict functions** are generated only when the dispatch contains parallel-eligible
  transactions. They scan the union of write sets from already-fired transactions
  against the next transaction's write set.
- **Fallback to sequential** when transactions are confirmed non-conflicting by the
  proof engine, the conflict check can be elided (constant `true`).
- **User opt-in:** A program that wants parallel dispatch declares it at the top level:

  ```briev
  #pragma dispatch parallel
  ```

  Without the pragma, the current first-true-wins fallthrough chain is used. This
  keeps the default simple while enabling the optimization where it matters.

### Files to Change

| File | Change |
|------|--------|
| `src/backend/llvm.rs` | New `emit_parallel_dispatch()` method, gated by pragma check |
| `src/parser.rs` | Recognize `#pragma dispatch parallel` |
| `src/ast.rs` | Add `DispatchMode` enum to `Program` |
| `specs/EVENT-MODEL.md` | Document parallel dispatch pattern |

---

## 2. Pre-built `briev_rt.o` (Build Tooling)

**Problem:** Users must compile `runtime/briev_rt.c` themselves, which requires
a C compiler and awareness of the runtime dependency.

**Solution 2a (Simple):** A Makefile or shell script in `runtime/` that builds
`briev_rt.o` for the detected host triple:

```bash
cd runtime && make              # Produces build/briev_rt-x86_64-linux-gnu.o
```

```makefile
# runtime/Makefile
TARGET ?= $(shell rustc -vV | grep host | cut -d' ' -f2)
build/$(TARGET).o: briev_rt.c
	mkdir -p build
	cc -c -o $@ $<
```

**Solution 2b (Integrated):** The `briev-compiler` CLI's `llvm` subcommand
accepts a `--link-rt` flag that emits the `.o` path:

```bash
briev-compiler llvm --link-rt program.bv -o program.ll
# Also outputs: link with runtime/briev_rt.c or runtime/build/$(TARGET).o
```

When `--link-rt` is specified and the pre-built `.o` doesn't exist for the
target triple, the compiler prints instructions to build it.

**Solution 2c (Cargo feature):** If the compiler itself is built with
`--features embed-rt`, the `.c` file is compiled at build time using `cc`
crate and embedded as a byte slice, written to disk at runtime.

### Recommended: 2a + 2c

- `runtime/Makefile` for immediate use (any platform, zero new dependencies)
- `embed-rt` Cargo feature for convenience (ships pre-compiled `.o` in the binary)
- `briev-compiler llvm --link-rt` prints the link command regardless

### Files to Change

| File | Change |
|------|--------|
| `runtime/Makefile` | New — builds `.o` for detected target triple |
| `runtime/build/` | Gitignored directory for pre-built artifacts |
| `Cargo.toml` | Add `embed-rt` feature flag |
| `build.rs` | If `embed-rt`, compile `runtime/briev_rt.c` via `cc` crate |
| `src/main.rs` | Add `--link-rt` flag to `llvm` subcommand |
| `README.md` | Document link step in LLVM backend section |

---

## 3. Polish Items

### 3a. Parser: `let` binding type-check on `@ link` globals

Currently `trg name: Type @ link sym;` accepts any `Type`. The `@ link`
runtime only supports `Bool` (i8), `Int` (i64), and `String` (i8\*). A
parser warning or typechecker validation for unsupported trigger types.

### 3b. Backend: `alwaysinline` on dispatch body calls

The fall-through dispatch calls `call void @txn(%State*)`. Adding
`alwaysinline` to each call site (not just the function definition) when the
call graph is acyclic would enable LLVM to inline across the entire chain.

### 3c. Backend: Dead `br` elimination comment

The fallthrough `br label %ck{N+1}` after a body block is dead code when the
body ends in `term` (which it always does). LLVM's `-O3` eliminates it. A
comment in the emitted IR would help debugging:

```llvm
; br label %ck1  ; dead — body always terms, llvm -O3 eliminates
```

### 3d. Stdlib: `io.bv` `__raw_poll` return type

The `__raw_poll` FFI currently returns `Vector<u8>`, but no backend implements
this exact type. Should be `String` for simplicity (raw byte buffer passed as
pointer), or `Data` if binary data is needed. Verify against `frgn` type
marshaling in the backend.

### 3e. Stdlib: `clear_io_ready` defn

The user currently writes `&io.io_ready = false;` manually. A convenience defn:

```briev
defn consume() -> Bool {
    &io_ready = false;
    term true;
};
```

### 3f. Documentation: `trg` tutorial

`learn-briev/11-triggers.md` should be updated to reflect the final event
model: `@ link` bindings, `node [trg]` pattern, no `trg!` inside bodies,
`runtime/briev_rt.c` linking.

---

## Implementation Order

| Priority | Item | Effort | Depends On |
|----------|------|--------|------------|
| P1 | `runtime/Makefile` (2a) | 1 hour | None |
| P1 | Documentation update (3f) | 2 hours | None |
| P2 | `#pragma dispatch parallel` + parallel codegen (1) | 4 hours | None |
| P2 | `--link-rt` flag (2b) | 2 hours | 2a |
| P3 | `embed-rt` Cargo feature (2c) | 2 hours | 2a |
| P3 | `io.bv` return type fix (3d) | 30 min | None |
| P3 | `consume()` defn (3e) | 10 min | None |
| P4 | Trigger type validation (3a) | 1 hour | None |
| P5 | `alwaysinline` on call sites (3b) | 30 min | None |
| P5 | Dead `br` comments (3c) | 15 min | None |