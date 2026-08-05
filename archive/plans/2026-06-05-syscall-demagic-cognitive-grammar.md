# Plan: Syscall Architecture, De-Magic, and Cognitive Grammar Update

**Date:** 2026-06-05T22:30:00+01:00
**Status:** Proposed → In Progress

---

## Symbolic Design Philosophy: What the Symbols Mean

Briv's symbols are not arbitrary ASCII choices. Each symbol's **visual shape** maps to a **cognitive metaphor**, which maps to a **systems meaning**. All uses of a given symbol share that core metaphor.

### The Shape-to-Meaning Mapping

| Symbol | Visual Shape | Cognitive Metaphor | Systems Meaning | All Uses Share |
|--------|-------------|-------------------|----------------|----------------|
| **`;`** | A dot with a tail falling away | A hard stop, a reset | Universal statement termination. The parser syncs here. | Every statement MUST end here. Error recovery resets to `;`. |
| **`.`** | A single pinpoint | Puncturing, reaching into | Struct field access / UFCS | `subject.member` — you reach into a thing (struct field or function namespace). |
| **`->`** | An arrow pointing right | Forward motion, transformation | Dataflow / State transition | Something becomes something else. `input -> output`, `term -> action`. |
| **`<-`** | An arrow pointing left | Backward motion, extraction | Mutation / Discard | Something comes out of something. `x <- &list` (extract), `<- &list` (discard). |
| **`:`** | Two stacked dots | Identity, equivalence | Static type / definition | "This IS that." `x: Int` (x IS Int). The colon equates name to type. |
| **`:>`** | Colon combined with right-arrow | Identity that projects outward | Compile-time metadata extraction | "The compiler's knowledge ABOUT this projects outward." `list :> Size` — the compiler's knowledge of list length. |
| **`[]`** | Brackets that enclose | Containment, boundary | Constraints, bounds, guards | Everything inside `[]` is bounded. `[x > 0]`, `list[0..5]`, `Vector<Int, 10>`. |
| **`{}`** | Curly braces that hug | Grouping, bundling | Code block / organizational unit | Statements bundled together. Struct bodies, transaction bodies. |
| **`()`** | Parentheses that cup | Holding, containing | Parameter / argument enclosure | Arguments are "held" inside `()`. Function args, enum constructors, pragma config. |
| **`@`** | The at-sign — a loop with an 'a' | Position, location, anchor | Spatial / Temporal / Dimensional / Chronological anchor | "At this location." `@ 0x4000` (address), `@100Hz` (frequency), `@12` (dimension), `@counter` (prior state). |
| **`&`** | Ampersand — ligature of "et" (and) | Connection, conjunction | Mutation marker | "Connect this variable to mutation." `&x = x + 1` — the `&` links the name to the mutable location. Required for all mutation. |
| **`!`** | A vertical line with a dot | An exclamation, a warning | Control flow anomaly / boundary | "Pay attention — something unusual here." `frgn!`, `syscall!` (fire-and-forget), `term!` (program exit), `trg!` (async). The `!` warns the reader. |
| **`~`** | A wavy line | Oscillation, flipping | Boolean toggle | "Flip back and forth." `[~/ready]` = toggle ready from false to true. Like a waveform. |
| **`?`** | A hook | A question, a check | Watchdog / timeout | "Is this still OK?" `?[5000ms]` — hook that checks a condition. |
| **`_`** | A small horizontal line | A gap, a placeholder | Ignored / unused value | "Something goes here but we don't care what." Pattern matching wildcard. |
| **`$`** | The dollar sign (S with a bar) | Value, currency, template | Reserved metaprogramming / generics | "This is a compile-time special value." Template parameters, macro substitutions. Not in active use yet. |

### The Principle: Syntactic Radical Honesty

The core design rule: **If an operation has distinct physical, temporal, or compiler-level behavior under the hood, its visual representation must explicitly reflect that boundary.** This is why:

- `x + 1` and `&x = x + 1` look different (addition vs mutation)
- `frgn sqrt(x)` and `syscall! @ SYS_EXIT(0)` look different (call vs kernel transition)
- `io.println("hello")` and `term!` look different (normal call vs program exit)
- `list :> Size` and `len(list)` deliver the same value but the `:>` visually says "I am extracting compiler-held metadata"

No hidden transformations. No magic behind the curtain. Every boundary-crossing operation uses a different visual symbol.

---

## Phase A: Fix Parser `parse_frgn_binding` Syscall Bug

**Files:** `src/parser.rs:930-940`, `src/backend/c.rs:609`

### Problem
`parse_frgn_binding()` only matches `Token::Frgn` / `Token::FrgnBang`. When the lexer emits `Token::Syscall` or `Token::SyscallBang` (which it does correctly), the function falls through to the error: `"Expected 'frgn' or 'frgn!'"`.

### Fix (2 new match arms)
```rust
Some(Ok(Token::Syscall)) => { self.advance(); FfiKind::Syscall }
Some(Ok(Token::SyscallBang)) => { self.advance(); FfiKind::SyscallBang }
```

### Also fix: void return handling for `SyscallBang`
At `parser.rs:992`, the code checks `ffi_kind == FfiKind::FrgnBang` to skip return type parsing. Add `|| ffi_kind == FfiKind::SyscallBang` so that `syscall!` declarations don't require a `-> Result<Int, Error>` clause.

### Also fix: C backend at `src/backend/c.rs:609`
Currently only checks `FfiKind::FrgnBang` — add `FfiKind::SyscallBang`.

### Verification
```bash
cargo test --lib   # existing parser tests should pass
```

**Risk:** Low

---

## Phase B: Remove Hardcoded `len` Magic from Interpreter

**Files:** `src/interpreter.rs:1272-1280`, `:1388-1390`, `:1433-1435`, `:1494-1496`

### Problem
The interpreter has Rust string-match built-ins that bypass the standard library:

```rust
// Line 1272 — RUNS BEFORE user definitions
if fn_name == "len" && arg_values.len() == 1 {
    match &arg_values[0] {
        Value::String(s) => return Ok(Value::Int(s.len() as i64)),
        Value::List(l) => return Ok(Value::Int(l.len() as i64)),
        _ => {}
    }
}
```

Same pattern repeats for HashMap (line 1388, 1494) and HashSet (line 1433). This violates the No-Magic principle — `len` should be implemented in stdlib via `:> Size`, which already works.

The comment says "must run before user definitions to prevent infinite recursion" — but the stdlib already defines `defn len(s: String)` using `term s :> Size` (`lib/std/string.bv:53-55`) and `defn len(list: List<Int>)` using `term list :> Size` (`lib/std/collections.bv:10-12`). The interpreter's Projection handler at `:1917` already evaluates `:> Size` for all collection types. No recursion possible — `:> Size` is an AST projection node, not a function call.

### Fix
Remove all 4 hardcoded `fn_name == "len"` blocks.

Also consider removing the other hardcoded method blocks for HashMap/HashSet (`insert`, `get`, `contains_key`, `remove`, `is_empty`, `keys`, `values` at lines 1380-1540) — these follow the same magic pattern and could be moved to stdlib.

Also remove `is_ok`/`is_err`/`unwrap`/`unwrap_err` at line 1265.

### Verification
```bash
cargo test --lib   # all tests using len() must pass via stdlib
```

**Risk:** Medium — existing tests may import `len` incorrectly. Need to verify stdlib imports.

---

## Phase C: Syscall Target Tables in `.dbvs` Files

**Files:** `targets/x86_64.dbvs`, `targets/aarch64.dbvs`, `src/ffi/registry.rs`

### Design Per Conversation
Syscall numbers are **NOT** hardcoded in Rust. They are defined declaratively in each target's `.dbvs` specification.

### Implementation

#### C1. Extend `.dbvs` parser (if needed)
The existing `src/dbriv/parser.rs` must support schema-valued fields (nested structs in schemas).

#### C2. Add `SyscallMap` schema to target files

`targets/x86_64.dbvs`:
```dbvs
schema SyscallMap {
    SYS_READ: Int = 0;
    SYS_WRITE: Int = 1;
    SYS_OPEN: Int = 2;
    SYS_CLOSE: Int = 3;
    SYS_MMAP: Int = 9;
    SYS_MUNMAP: Int = 11;
    SYS_BRK: Int = 12;
    SYS_IOCTL: Int = 16;
    SYS_NANOSLEEP: Int = 35;
    SYS_GETPID: Int = 39;
    SYS_EXIT: Int = 60;
    SYS_FUTEX: Int = 202;
};
```

`targets/aarch64.dbvs`:
```dbvs
schema SyscallMap {
    SYS_READ: Int = 63;
    SYS_WRITE: Int = 64;
    SYS_OPEN: Int = 56;
    SYS_CLOSE: Int = 57;
    SYS_MMAP: Int = 222;
    SYS_MUNMAP: Int = 215;
    SYS_EXIT: Int = 93;
};
```

#### C3. Replace TOML syscall loading in `ffi/registry.rs`
Replace the unused `load_syscall_bindings()` with `.dbvs`-based resolution. Change `get_syscall_number()` to read from loaded `.dbvs` data.

**Risk:** Low-medium (depends on `.dbvs` parser flexibility)

---

## Phase D: Syscall Codegen in LLVM Backend

**Files:** `src/backend/llvm.rs`

### Design
Thread `ffi_kind` from `ForeignSignature` into the `Expr::Call` codegen. For `Syscall`/`SyscallBang`:

**x86_64:** Emit inline asm
```llvm
%res = call i64 asm sideeffect "syscall", "={rax},{rax},{rdi},{rsi},{rdx},{r10},{r8},{r9}"(i64 %num, i64 %arg1, ...)
```

**aarch64:** Emit inline asm with `svc #0`, syscall number in `x8`.
```llvm
%res = call i64 asm sideeffect "svc #0", "={x0},{x8},{x0},{x1},{x2},{x3},{x4},{x5}"(i64 %num, i64 %arg1, ...)
```

### Implementation Steps
1. Thread `ffi_kind` from signature to `Expr::Call` emittter
2. Add syscall number resolution (call into Phase C infrastructure)
3. Generate inline asm blocks for each target
4. Handle return type wrapping (Result for `Syscall`, void for `SyscallBang`)

**Risk:** Medium (inline asm syntax varies by target)

---

## Phase E: Update "The Briv Mindset" Document

**File:** `learn-briv/00a-base-design.md`

### Changes
- Add Symbolic Design Philosophy section at the top
- `.` Accessor: Clarify UFCS desugaring. `list.len()` → `len(list)` → stdlib `term list :> Size`.
- `->` / `<-`: Full directional dataflow section (push, pop, discard, swan song)
- `;`: Add Universal Statement Termination
- `$`: Add Reserved Metaprogramming Space
- `<- discard`: Explicit discard for syscall results
- `!`: Add `term!` (program exit)
- Update "The Feel Summary" with missing symbols

**Risk:** Low (documentation only)

---

## Dependency Graph

```
Phase A (parser) ──→ Phase D (codegen)
Phase B (de-magic)
Phase C (.dbvs) ──→ Phase D (codegen)
Phase E (doc) ──→ depends on B
```

A, B, C can be parallel. D needs A+C. E needs B.

---

## File Change Summary

| File | Phase | Change |
|------|-------|--------|
| `src/parser.rs:930-940` | A | Add `Syscall`/`SyscallBang` match arms |
| `src/parser.rs:992` | A | Add `SyscallBang` to void-return check |
| `src/backend/c.rs:609` | A | Add `SyscallBang` to void-handling arm |
| `src/interpreter.rs:1272-1280` | B | Remove hardcoded `len` for String/List |
| `src/interpreter.rs:1388-1390` | B | Remove hardcoded `len` for HashMap |
| `src/interpreter.rs:1433-1435` | B | Remove hardcoded `len` for HashSet |
| `src/interpreter.rs:1494-1496` | B | Remove hardcoded `len` for HashMap (2nd) |
| `src/interpreter.rs:1265-1270` | B | Consider: remove `is_ok`/`is_err`/`unwrap` magic |
| `targets/x86_64.dbvs` | C | Add `SyscallMap` schema |
| `targets/aarch64.dbvs` | C | Add `SyscallMap` schema |
| `src/ffi/registry.rs:45-98` | C | Replace TOML syscall loading with `.dbvs` |
| `src/backend/llvm.rs` | D | Add syscall inline asm for x86_64 + aarch64 |
| `learn-briv/00a-base-design.md` | E | Full cognitive grammar update |

## Anti-Patterns to Avoid

1. **No syscall number hardcoding in Rust** — use `.dbvs` files (Phase C)
2. **No keeping `len` magic as "temporary"** — `:> Size` already works in both interpreter and LLVM backend
3. **No new AST nodes** — reuse existing `Expr::Call` + `FfiKind` (already wired)
4. **No weakening existing optimization paths** — additive match arms only
5. **No new compiler intrinsics** — `:> Size` handles `len`, `:> Ptr` handles pointer access