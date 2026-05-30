# Brief Compiler — Comprehensive Corrective Plan

> Generated: 2026-05-29T14:30Z
> Covers: 6 backend bugs, pragma normalization (`#!`/`#io`/`#wake`), IO registry, documentation, stdlib migration, and future design insights.

---

## Phase 0 — The 6 Backend Bugs (Correctness Hazards)

Fix these before any new features. Each causes either data corruption, undefined behavior, or silent wrong-codegen.

| # | Bug | File:Line | Fix |
|---|-----|-----------|-----|
| **B1** | **Unicode truncation** — `c as u8` drops high bytes of non-ASCII chars | `llvm.rs:11` | Iterate `s.bytes()` instead of `s.chars()`, emit `\\xx` per byte |
| **B2** | **Float type bypass on locals** — `is_float_expr` only checks global `field_index_map`, not `let_bindings` + `register_types`. `Expr::Float` doesn't register its type. | `llvm.rs:1382` + `760` | (a) `Expr::Float`: insert into `register_types`. (b) `is_float_expr` identifier branch: check `let_bindings` → `register_types`, then fall back to `field_index_map` |
| **B3** | **Non-reactive txns skipped in `build_write_masks`** — `if !t.is_reactive { continue; }` causes write-mask of 0, enabling data races in parallel dispatch | `llvm.rs:1193` | Remove the `is_reactive` guard; build write masks for ALL transactions |
| **B4** | **Unification discriminant hardcoded to `0`** — switch always matches variant `0`. Payload-bearing variants (`Some`, `Ok`) use discriminant `1`. | `llvm.rs:735` | Destructure `name` from `Statement::Unification { name, pattern, expr }`. Set `target = if name == "None" \|\| name == "Err" { 0 } else { 1 }`. |
| **B5** | **False non-negative range** — `dlo` defaults to `0` when `lo == i64::MIN`, lying to LLVM that the variable can never be negative | `llvm.rs:539` | `let dlo = if lo > i64::MIN { lo } else { i64::MIN };` |
| **B6** | **Overly permissive `nuw nsw`** — `binop_has_bounds` returns `true` if variables are in `range_bounds`, even if combined ranges overflow `i64` | `llvm.rs:1363` + `1420` | **Recommended**: Remove manual `nuw nsw` emission entirely. LLVM's `ScalarEvolution` infers it from `!range` metadata more accurately. |

### B6 design rationale

You already emit `!range` metadata on all bounded loads (via `dlo`/`hi` bounds from `emit_transaction`). LLVM's `ScalarEvolution` pass can prove `nuw nsw` from range metadata more accurately than an ad-hoc `binop_has_bounds` check. Removing the manual emission is:
- **Safer**: no risk of UB from false positives
- **Simpler**: delete ~10 lines of code
- **No performance loss**: LLVM re-adds the flags where provably safe

---

## Phase 1 — Pragma Syntax Normalization

### 1a — `#!dispatch(parallel)` as file-level directive

**Goal**: Drop the redundant `pragma` keyword. `#!` already signals "file-level directive."

**Current state**: `#pragma dispatch(parallel)` (item-level) and `#!pragma dispatch(parallel)]` (file-level, with ugly `]`) both work. `#!` at file level is currently a parse error.

**Lexer**: No changes. `HashBang` token exists at `lexer.rs:277`.

**Parser changes** (`parser.rs`):

1. `parse_program()` — add `HashBang` to file-level attribute detection:
```rust
if matches!(current_token, HashBangBracket | PragmaBang | HashBang) {
    file_attrs = self.parse_attributes()?;
}
```

2. `parse_attributes()` switch — add `HashBang` arm:
```rust
Some(Ok(Token::HashBang)) => {
    self.advance(); // consume #!
    is_pragma = true;
    is_file_level = true;
}
```

**Valid syntaxes**:
- `#!dispatch(parallel)` — preferred file-level form
- `#pragma dispatch(parallel)` — item-level, backward compat
- `#!pragma dispatch(parallel)]` — file-level, backward compat

---

### 1b — `#wake` modifier on triggers

**Goal**: Mark a `@ link` trigger as wake-capable (can be used with blocking waits).

**AST change** (`ast.rs`):
```rust
pub struct TriggerDeclaration {
    pub name: String,
    pub ty: Type,
    pub address: LinkRef,
    pub bit_range: Option<BitRange>,
    pub stages: Vec<String>,
    pub condition: Option<Expr>,
    pub is_wake: bool,           // NEW
    pub span: Option<Span>,
}
```

**Parser change** (`parse_trigger()`): After address, stages, condition — before `;`:
```rust
// Check for #wake modifier
if let Some(Ok(Token::Hash)) = self.current_token() {
    self.advance();
    if let Some(Ok(Token::Identifier(n))) = self.current_token() {
        if n == "wake" {
            self.advance();
            is_wake = true;
        } else {
            error("Expected 'wake' after '#'");
        }
    }
}
```

**Semantics**:
- `trg x: Bool @ link __x #wake;` — explicit wake-capable
- `trg x: Bool @ 0x4000 #wake;` — **error**: redundant, MMIO is natively wake-capable
- `#io` always implies `#wake` automatically
- Polling remains the default reactor mode; `#wake` is a flag the runtime CAN use

**Backend**: No codegen change yet. Future: emit `@llvm.wake_triggers` metadata array for the C runtime.

---

### 1c — `#io` file-level OS trigger declarations

**Goal**: Declare an OS-linked trigger without knowing C runtime symbol names. `#io` is a compiler magic word — it maps a platform-agnostic concept name to the appropriate runtime symbol per target.

**Two forms**:

```brief
#io sigint;                              // Implicit: creates trg sigint: Bool
#io sigint -> trg my_sigint: Bool;        // Explicit: user chooses name, validates type
#io timer(1hz);                           // Parametrized concept
#io timer(1hz) -> trg t: Int;             // Parametrized + explicit
```

**New file**: `src/io_registry.rs` — single lookup table:

```rust
pub struct IoConcept {
    pub concept: &'static str,   // "sigint", "timer(1hz)"
    pub symbol: &'static str,    // "__io_sigint", "__io_timer_1hz"
    pub ty: Type,
    pub has_param: bool,
    pub description: &'static str,
}

pub fn io_lookup(concept: &str) -> Option<&'static IoConcept>;
pub fn list_concepts() -> String;
```

**Initial concepts** (~10, cross-platform):

| Concept | Type | Symbol | Description |
|---------|------|--------|-------------|
| `sigint` | Bool | `__io_sigint` | SIGINT interrupt (Ctrl+C) |
| `sigterm` | Bool | `__io_sigterm` | SIGTERM termination signal |
| `sighup` | Bool | `__io_sighup` | SIGHUP hangup signal |
| `stdin_ready` | Bool | `__io_stdin_ready` | Stdin has data available |
| `stdin_line` | String | `__io_stdin_buffer` | Current stdin line buffer |
| `timer(1hz)` | Int | `__io_timer_1hz` | 1-second timer tick |
| `timer(100hz)` | Int | `__io_timer_100hz` | 10ms timer tick |
| `io_pending` | Bool | `__io_pending` | Generic IO pending flag |
| `mouse_click` | Bool | `__io_mouse_click` | Mouse button click |
| `key_press` | Char | `__io_key_press` | Keyboard key press |

**Parser** (`parse_io_declaration()`):
1. Parse concept name + optional `(param)`
2. Look up in registry — error with available list if not found
3. If `-> trg name: Type;` form: validate type matches registry, use user's name
4. If `;` form (implicit): name = concept, type from registry
5. Construct `TriggerDeclaration { address: LinkRef::Linked(symbol), is_wake: true }`
6. Error on duplicate `#io` concept

**Parser integration** (`parse_program()`):
```rust
while let Some(Ok(Token::Hash)) = self.current_token() {
    self.advance();
    if self.current_token() == Some(Ok(Token::Identifier("io"))) {
        self.advance();
        io_decls.push(self.parse_io_declaration()?);
    } else { break; }
}
items.splice(0..0, io_decls);  // Prepend IO decls before other top-level items
```

**Edge cases**:
- `#io` inside transaction body: caught by statement-level `#` handler → error. Correct.
- Duplicate `#io sigint;` → error.
- `#io unknown_concept;` → error listing `sigint`, `sigterm`, `stdin_ready`, etc.
- `#io sigint -> trg x: String;` → error: registry says Bool, you wrote String.

**Philosophy**: `#io` is explicitly "compiler magic" — allowed because the `#` prefix openly declares it as a compiler instruction. This differs from the "no magic words" rule which applies to language semantics (no implicit `is_digit` built-ins, no auto-imported `None`).

---

### 1d — Stdlib migration

`lib/std/system.bv` migrates from:
```brief
trg sigint: Bool @ link __sigint_flag;
trg stdin_ready: Bool @ link __stdin_ready;
trg clock_tick_1hz: Int @ link __timer_1hz;
```
to:
```brief
#io sigint;
#io stdin_ready;
#io timer(1hz) -> trg clock_tick_1hz: Int;
```

Old `trg @ link` syntax continues to work forever. The stdlib uses the newer, cleaner form.

---

## Phase 2 — Pragma Documentation

**New file**: `learn-brief/12-pragmas.md`

Exhaustive, authoritative reference of ALL compiler pragmas. Required preamble:

> *"These directives are Brief's complete set of compiler-intrinsic behavior. Everything else — imports, FFI, transactions, contracts — is standard library or user code. No hidden magic. If it doesn't appear on this page, the compiler doesn't know about it by name."*

Contents:
- Table of all directives (scope, purpose, example)
- Canonical IO Concepts table (every `#io` name, type, description, runtime symbol)
- Explanation of why `#io`/`#wake` exist: OS environments are complex; Embedded/Rendered Brief triggers are natively wake-capable
- Migration guide: `trg @ link` → `#io`

---

## Phase 3 — Runtime Blocking Wait (Future PR)

Not implemented in this session. Architecture decision record:

The reactor currently busy-loops:
```llvm
tick:
  call void @reactor_tick()
  br label %tick
```

With `#wake` triggers declared, it can become:
```c
while (1) {
    epoll_wait(epoll_fd, events, MAX_EVENTS, -1);  // or signalfd, kqueue, WFI
    reactor_tick();
}
```

This requires:
1. Backend emits `@llvm.wake_triggers` metadata listing wake-capable symbols
2. C runtime reads metadata at init, sets up signalfd/epoll/kqueue fds
3. `main()` calls `__rt_wait()` instead of busy-looping

Design decision: the pragma changes (Phases 0-2) establish the *declaration* of wake intent. The runtime *optimization* is a separate PR that changes only `brief_rt.c` and the codegen of `main()`.

---

## Phase 4 — Future Design Insights

These are design conclusions reached during this session, not implementation tasks.

### 4a — Contract-Driven Optimization Feedback

The compiler should emit human-readable remarks when state variables are unbounded:

```
remark[performance]: State variable 'index' is unbounded below in 'Counter.update'
  --> src/counter.bv:12:24
   |
12 |     txn Counter.update [index < 64] {
   |                         ^^^^^^^^^ 'index' is constrained above, but has no lower bound.
   |
   = help: Because 'index' can theoretically go below 0, the compiler must emit runtime
           underflow guards and boundary checks for any list or array indexing.
           
   = suggestion: Constrain the lower bound with `[index >= 0 && index < 64]`
                 to unlock direct CPU register mapping and branchless SIMD vectorization.
```

This reverses the traditional safety/performance trade-off: proving safety IS what makes code fast. Writing stricter contracts unlocks better codegen.

### 4b — Big O / Complexity Analysis

Brief's constraints (acyclic calls, SMT-bounded loops, no pointer aliasing) make it uniquely suited for static complexity analysis. Pragmas for asserting or querying complexity:

```brief
#assert_linear           // Compiler errors if analysis exceeds O(N)
#limit_cycles 1000       // Budget worst-case execution time
#address_space shared    // Route memory to hardware tier
```

Implementation approach (pragmatic, not provably complete):
1. Nesting depth counter: 1 level = O(N), 2 levels = O(N²)
2. Hardcoded cost models for stdlib primitives
3. Symbolic loop bounds from SMT solver
4. WCET comparison against `#limit_cycles` budget

### 4c — WCET (Worst-Case Execution Time) Budgeting

Critical for real-time / embedded / hardware targets where a missed tick deadline = physical failure. The compiler can:
1. Track nesting depth of runtime-dependent loops
2. Hardcode cost models for each stdlib primitive
3. Compare symbolic loop bounds against annotated `#limit_cycles` budget
4. Error if WCET exceeds budget

### 4d — `#` as the universal "compiler instruction" prefix

Every directive that changes how the compiler processes code without changing the program's mathematical behavior uses `#`:

| Directive | Category |
|-----------|----------|
| `#!dispatch(parallel)` | Reactor mode |
| `#io sigint` | OS linkage |
| `#wake` | Trigger semantics |
| `#assert_linear` | Complexity audit |
| `#limit_cycles 1000` | WCET budget |
| `#address_space shared` | Memory tier |

The `#` prefix is the visual and syntactic signal: "this is not application logic, this is a compiler instruction." It's Brief's single, universal escape hatch — and it's documented exhaustively so users always know exactly what the compiler knows.

---

## Test Plan

| Test | Verifies |
|------|----------|
| B1: Non-ASCII string literal | `"héllo"` produces correct LLVM IR bytes |
| B2: Local float binding | `let x = 1.5; &y = x + 2.0;` emits `fadd float` |
| B3: Parallel dispatch with non-reactive txn | Write mask includes non-reactive writes |
| B4: `uni Some(x) = expr;` | Switch targets discriminant `1`, extracts payload |
| B5: `[x < 100]` without lower bound | `!range` metadata uses `i64::MIN` as lower bound |
| B6: `[x >= 0, x < 10]` + `[y < 10]` — `x + y` | No `nuw nsw` emitted |
| `#!dispatch(parallel)` at top of file | Reactor uses parallel codegen |
| `trg x: Bool @ link __x #wake;` | Parse succeeds, `is_wake = true` |
| `#io sigint;` | Creates `trg sigint: Bool @ link __io_sigint` with `is_wake` |
| `#io sigint -> trg mysig: Bool;` | Explicit name, type validation |
| `#io timer(1hz);` | Parametrized concept |
| `#io nonexistent;` | Error listing available concepts |
| `#io sigint; #io sigint;` | Duplicate error |
| `trg x: Bool @ 0x4000 #wake;` | Error |
| 271 existing tests | No regressions |

---

## Files Changed (Complete List)

| File | Change | Phase |
|------|--------|-------|
| `src/backend/llvm.rs` | B1-B6 bug fixes | 0 |
| `src/ast.rs` | `is_wake: bool` on `TriggerDeclaration` | 1b |
| `src/parser.rs` | `#!dispatch`, `#wake`, `#io` parsing | 1a-1c |
| `src/io_registry.rs` | **New file** | 1c |
| `lib/std/system.bv` | Migrate to `#io` | 3 |
| `learn-brief/12-pragmas.md` | **New file** | 2 |
| `learn-brief/11-triggers.md` | Update with `#io`/`#wake` reference | 2 |
