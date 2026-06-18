# Backend Completion: Officina Validation + Memory Leak Fix

**Date:** 2026-06-17
**Session:** Functional Validation & Correctness

## Work Item 1: Smoke Test Officina

Run officina binary with piped input to verify it:
- Boots without crash
- Renders the terminal UI (top bar, prompt)
- Processes keystrokes (type commands, navigate history)
- Exits cleanly on Ctrl+C (from the `#!exit` pragma)

## Work Item 2: Fix `<-` Push Memory Leak

### Problem
Every `&list <- value` allocates a new buffer via `malloc` and copies
all old elements + the new element. The old buffer is never freed. Over
time this leaks memory linearly with the number of pushes.

### Fix Approaches

**Approach A: `free(old_ptr)` after malloc+copy**
- Declare `free(i8*)` in LLVM IR
- After copying old elements to the new buffer, call `free` on the old.
- Simple, no refactoring needed

**Approach B: `realloc` instead of malloc+copy**
- Declare `realloc(i8*, i64)` in LLVM IR
- Extend the existing buffer in-place when possible
- Avoids the copy entirely when realloc extends in place
- Changes the pointer (realloc may move memory)

**Approach C: Copy-on-write with refcount**
- Add a refcount to the list header
- On push, if refcount > 1, allocate new; otherwise, realloc in place
- Requires refcount field in list struct — changes ABI

I'll implement **Approach B (realloc)** — it's the cleanest:
- No leak
- No extra copy when realloc extends in place
- Single allocation for the common append pattern
- Minimal code change (replace malloc + memcpy with realloc)

## Verification
1. `cargo test --lib` — all tests pass
2. Officina boots and accepts piped input
3. `<-` push on a large list doesn't leak (valgrind clean)
