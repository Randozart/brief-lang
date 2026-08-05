# Intrinsic Migration: Print/PutChar/GetEnv → Stdlib

**Date:** 2026-07-19
**Status:** Implemented — Phase 1 + Phase 2 complete. 20/24 benchmarks passing.
**Prerequisite:** July 18 master plan (SSO, SVO, Ptr Level 3, allocation strategy) + benchmark stabilization plan — both implemented
**Resumes after:** `docs/plans/2026-07-19-benchmark-stabilization.md` — Phase 4 (benchmark comparison)
**Directives:** AGENTS.md Plan Directives — flat control flow, rationale comments, example updates, doc updates, behavioral tests

---

## Executive Summary

Migrate terminal I/O and environment variable access from compiler intrinsics (`Print#`, `PutChar#`, `GetEnv#`, `GetEnvInt#`) to the standard library. Operations that can be expressed in pure Briv using lower-level intrinsics (`SysCall#`, `Load#`) should not be compiler-known.

This removes 4 intrinsic names from the compiler, replacing them with:
- A `!` plugin-intercept system for dispatch
- Two Front-stage Rust plugins (print/io, env)
- Pure-Briv stdlib implementations using `SysCall#(Write, ...)` and `Load#`-based environ scan

### What stays (genuinely necessary intrinsics — per the embedded criterion)

| Intrinsic | Rationale |
|-----------|-----------|
| `SysCall#` | CPU `syscall` instruction — embedded needs raw kernel access |
| `SysConf#` | POSIX `sysconf()` — platform capabilities query |
| `Load#`/`Store#`/`Copy#`/`Fill#` | Raw memory access — building blocks |
| `Alloc#`/`Malloc#`/`Free#` | Compiler escape analysis intercepts for arena/alloca decisions |
| `AtomicLoad#`/`AtomicStore#`/`AtomicCas#`/etc | Platform CPU barriers — no portable alternative |
| `Sqrt#`/`Sin#`/`Cos#`/`Fabs#`/`Ceil#`/`Floor#`/`Pow#` | LLVM `@llvm.*` intrinsics — no portable pure-Briv impl |
| `AddressOf#` | Compiler knows struct field layout |
| `DlOpen#`/`DlSym#`/`DlClose#` | Dynamic linker — platform-specific |
| GPU intrinsics | `GetGlobalId#`, `WorkgroupSize#`, etc — platform-specific |
| `Get#`/`Insert#` | Collection operations — generic data structure access |
| `Concat#`/`Length#`/`ToInt#`/`ToFloat#`/`ToString#` | String/type conversion — LLVM CodeGen needs type-aware ops |
| `Backtrace#` | Debugging — platform-specific |
| `BitAnd#`/`BitOr#`/`BitXor#`/`Shl#`/`Shr#`/`BitNot#` | Bitwise ops — map directly to LLVM IR instructions |
| `Not#` | Logical not — maps directly to LLVM IR |
| Arith/Compare (`Add#`, `Sub#`, `Mul#`, `Div#`, `Rem#`, `Neg#`, `Abs#`, `Eq#`, `Neq#`, `Lt#`, `Gt#`, `Le#`, `Ge#`) | Mapped through `config/llvm-ops.toml` — type-polymorphic arithmetic, fundamental |

### What moves to stdlib

| Intrinsic | Migrates to | Mechanism |
|-----------|-------------|-----------|
| `Print#(x)` | `lib/std/io.bv`: `print_int`, `print_str`, `print_float`, `print_char` | `SysCall#(Write, 1, ...)` |
| `PutChar#(c)` | `lib/std/io.bv`: `print_char(c)` | `SysCall#(Write, 1, &c, 1, ...)` |
| `GetEnv#(name)` | `lib/std/env.bv`: `getenv(key) -> String` | pure-Briv `Load#`-based environ scan |
| `GetEnvInt#(name)` | `lib/std/env.bv`: `get_env_int(key) -> Int` | `getenv` + `parse_int` in Briv |

---

## Pipeline Architecture Changes

### The `!` Plugin-Intercept System

A new expression variant `Expr::PluginIntercept` carries calls prefixed with `!`:

```
Source:  !PrintLn("hello")
Lexer:   Token::PluginCall + Token::Identifier("PrintLn") + "(" + ...
Parser:  Expr::PluginIntercept { name: "PrintLn", args: [Expr::Quoted("hello")] }
Stage:   Front plugins match on name → rewrite to typed stdlib calls
Check:   After all Front plugins run → if any PluginIntercept remain → compiler error
```

### Plugin Stages (two new Rust plugins)

| Plugin | Stage | File | Purpose |
|--------|-------|------|---------|
| `print` | Front | `src/plugin/print.rs` | `!Print`/`!PrintLn` → dispatch by type to `print_int`/`print_str`/`print_float` |
| `env` | Front | `src/plugin/env.rs` | `!GetEnv`/`!GetEnvInt` → `getenv`/`get_env_int` |

Both are registered in `PluginManager` and enabled by default via the prelude target config.

### Pipeline Flow

```
Source → $(Front) → Lex → Parse → PluginIntercept → Typecheck
                                ↑                    ↓
                     !Print(x)                  print_int(x)
                     !GetEnvInt(n)              get_env_int(n)
```

The plugins run at Front stage so they can use the type checker's `TypeUniverse` to dispatch `!Print(x)` to the right typed function based on `x`'s inferred type.

---

## Phase 1: GetEnv#/GetEnvInt# → Pure-Briv Environ Scan

### How the environ scan works

On POSIX, when a process starts, the kernel places environment variables on the stack as `"KEY=VALUE\0"` strings terminated by a NULL pointer. The `environ` global variable (from libc) points to this array.

```
extern char **environ;  // Provided by libc
environ[0] = "PATH=/usr/bin\0"
environ[1] = "HOME=/home/user\0"
environ[2] = NULL
```

**Mechanism:** A one-line C helper returns the `environ` pointer as an integer. Pure-Briv code then:
1. Calls `__get_environ()` to get the envp array address
2. Loads each entry pointer via `Load#`
3. Compares the prefix against the key
4. Returns the value after `=`, or empty string if not found

### C runtime addition

**File:** `lib/runtime/briv_rt.c`
```c
int64_t __get_environ(void) {
    extern char **environ;
    return (int64_t)(uintptr_t)environ;
}
```

### Stdlib FFI declaration

**File:** `lib/std/ffi/env.bv`
```briv
// 2026-07-19: Returns the environ pointer (char **environ cast to Int).
// Pure-Briv getenv scans this pointer array to find key=value pairs.
frgn __get_environ() -> Int;
```

### Pure-Briv getenv implementation

**File:** `lib/std/env.bv`
```briv
import "std/ffi/env.bv";

defn getenv(key: String) -> String {
    let envp: Int = __get_environ();               // char **environ
    let i: Int = 0;
    txn scan [true][true] -> String {
        let entry_ptr: Int = Load#(envp + i * 8);  // Load# = raw memory read
        [entry_ptr == 0] {
            term "";
        };
        // Compare key prefix — pure-Briv string ops
        let entry: String = ptr_to_string(entry_ptr);
        [entry.starts_with(key) && entry[key.len] == 61 as Byte] {
            term entry.slice(key.len + 1, entry.len - key.len - 1);
        };
        i = i + 1;
        term;
    };
};

defn get_env_int(key: String) -> Int {
    let val: String = getenv(key);
    [val != ""] {
        term parse_int(val);
    };
    term 0;
};
```

> Note: `starts_with`, `slice`, `parse_int` are pure-Briv string utilities available in stdlib. `Load#` is the retained memory-read intrinsic. `ptr_to_string` converts a null-terminated C string pointer to a Briv String — uses `Load#` + byte iteration.

### Compiler files to modify

| File | Change |
|------|--------|
| `src/intrinsic_signatures.rs` | Remove `GetEnv#`, `GetEnvInt#` |
| `src/backend/llvm/intrinsics.rs` | Remove `emit_get_env`, `emit_get_env_int`, dispatch arms |
| `src/backend/llvm/mod.rs` | Remove `getenv`, `strlen`, `atol` declares (check if `atol` used elsewhere — it's not) |
| `src/backend/llvm/normalizer.rs` | Remove from supported set |
| `src/backend/llvm/loop_engine/analysis.rs` | Remove from observable-intrinsic list |
| `src/backend/webstack.rs` | Remove `GetEnv#`, `GetEnvInt#` handlers |
| `src/interpreter/intrinsics.rs` | Remove `GetEnv#`, `GetEnvInt#` handlers |
| `src/proof_engine/mod.rs` | Remove from proof-engine list |
| `src/backend/normalizer.rs` | Remove from standard intrinsics list (if present) |

### Stdlib files to create/modify

| File | Change |
|------|--------|
| `lib/runtime/briv_rt.c` | Add `__get_environ()` |
| `lib/std/ffi/env.bv` | Replace `frgn __get_env_int` with `frgn __get_environ` |
| `lib/std/env.bv` | Full `getenv` + `get_env_int` implementation |
| `lib/std/ffi/env.bv` | Remove old frgn |

### Plugin to create (Rust, Front stage)

| File | Purpose |
|------|---------|
| `src/plugin/env.rs` | Match `!GetEnv(name)` → `getenv(name)`, `!GetEnvInt(name)` → `get_env_int(name)` |

### Benchmark updates

All 23 benchmarks: `GetEnvInt#("BOUND")` → `!GetEnvInt("BOUND")`

`UTF8_ops.bv` — also change `const TOTAL` → `let TOTAL`:
```briv
let TOTAL: Int = !GetEnvInt("BOUND");
let SEED: Int = !GetEnvInt("SEED");
```

---

## Phase 2: Print#/PutChar# → Stdlib via SysCall#(Write, ...)

### !Print/!PrintLn dispatch architecture

The `print` Front plugin receives `!Print(x)` / `!PrintLn(x)` from the parser. Using the type checker's type information for `x`, it replaces the call with one of:

| Inferred type of `x` | Replaced with |
|---|---|
| `Int` | `print_int(x)` |
| `Float` | `print_float(x)` |
| `String` | `print_str(x)` |
| Other | Compiler error: "!Print requires Int, Float, or String argument" |

`!PrintLn(x)` does the same but appends `print_char(10)` for newline.

### Stdlib print implementation

**File:** `lib/std/io.bv`
```briv
defn print_str(s: String) -> Void {
    let _: Int = SysCall#(Write, 1, s.data, s.len, 0, 0, 0);
};

defn print_char(c: Int) -> Void {
    let buf: String = String::from_byte(c);
    let _: Int = SysCall#(Write, 1, buf.data, 1, 0, 0, 0);
};

defn print_int(n: Int) -> Void {
    let s: String = int_to_str(n);
    print_str(s);
};

defn print_float(f: Float) -> Void {
    // Uses SysCall#(Write) with pre-formatted float bytes
    let s: String = float_to_str(f);
    print_str(s);
};
```

**Helper — `int_to_str` in pure Briv:**
```briv
defn int_to_str(n: Int) -> String {
    [n == 0] { term "0"; };
    let neg: Bool = n < 0;
    let abs: Int = if neg { -n } else { n };
    let digits: String = "";
    txn collect [abs > 0][abs == 0] -> String {
        digits = String::from_byte(48 + abs % 10) ++ digits;
        abs = abs / 10;
        term;
    };
    [neg] {
        term "-" ++ digits;
    };
    term digits;
};
```

### Lexer/Parser/AST changes for `!`

**Lexer** (`src/lexer.rs`):
```rust
// New token:
#[token("!")]
PluginCall,
```

**Parser** (`src/parser/expr.rs`):
```rust
// When PluginCall token is followed by Identifier + "(" + ...
// → Expr::PluginIntercept { name: String, args: Vec<Expr> }
```

**AST** — new variant in `Expr`:
```rust
/// 2026-07-19: Plugin-intercept call. `!Name(args)`. Must be resolved
/// by a Front-stage plugin; compiler error if any remain after Front.
PluginIntercept {
    name: String,
    args: Vec<Expr>,
    type_args: Vec<Type>,
},
```

The `type_args` field is populated by the type checker after inferring the argument types, so the plugin can dispatch based on type.

### Compiler files to modify

| File | Change |
|------|--------|
| `src/lexer.rs` | Add `Token::PluginCall` |
| `src/parser/expr.rs` | Parse `!` prefix on calls → `Expr::PluginIntercept` |
| `src/ast/expr.rs` | Add `PluginIntercept` variant |
| `src/intrinsic_signatures.rs` | Remove `Print#`, `PutChar#` |
| `src/backend/llvm/intrinsics.rs` | Remove `emit_print`, `emit_putchar`, dispatch arms |
| `src/backend/llvm/mod.rs` | Remove `printf`, `fputc`, `fflush`, `putchar` declares |
| `src/backend/llvm/normalizer.rs` | Remove from supported set |
| `src/backend/llvm/loop_engine/analysis.rs` | Remove from observable list |
| `src/webstack.rs` | Remove `Print#`, `PutChar#` handlers |
| `src/interpreter/intrinsics.rs` | Remove `Print#`, `PutChar#` handlers |
| `src/proof_engine/mod.rs` | Remove from proof-engine list |
| `src/backend/normalizer.rs` | Remove standard-op entries |

### Plugin to create (Rust, Front stage)

| File | Purpose |
|------|---------|
| `src/plugin/print.rs` | Match `!Print(x)`/`!PrintLn(x)` → dispatch to `print_*` based on type |

### Prelude plugin update

**File:** `plugins/front/prelude.bv`
```briv
$(Front @ highest) {
    InsertLiteralImport$("std/types/bootstrap.bv");
    InsertLiteralImport$("std/os/fs.bv");
    // ... existing imports ...
    InsertLiteralImport$("std/env.bv");    // NEW
    InsertLiteralImport$("std/io.bv");     // NEW
};
```

### Benchmark updates

All 31 benchmarks using `Print#(result)` → `!PrintLn(result)` (most use `term! -> Print#(result)` as swan song).

---

## Architecture Doc Updates

### `docs/architecture/hash-words.md`

Line 50: Change `Print#` example to a retained intrinsic:
```markdown
| `#` suffix on identifiers | Intrinsic marker: `Malloc#`, `SysCall#`, `Sqrt#` |
```

### `docs/architecture/intrinsics-vs-stdlib.md`

Update the Intrinsic Categories table — the "I/O" row changes:
```markdown
| ~~I/O~~ Terminal & Env | — | Moved to stdlib. `!Print`/`!PrintLn` dispatched via Front plugin; `!GetEnv`/`!GetEnvInt` resolved to pure-Briv environ scan. All use `SysCall#(Write, ...)` or `Load#` underneath. |
```

### `docs/architecture/overview.md`

Line 217: `Print#` reference in CIRCT normalizer will naturally become dead code when `Print#` is removed — the normalizer won't encounter it. Update the comment:
```markdown
| `circt/normalizer.rs` | `CirctNormalizer` — attaches `bit_width` |
```

---

## Implementation Order

### Step 1: `!` Plugin-Intercept infrastructure
- Lexer token + parser + AST variant
- Plugin dispatch for `!` calls (if any remain after Front, error)
- Test parse roundtrip

### Step 2: `env` Plugin (Phase 1)
- Create `src/plugin/env.rs`
- Register in PluginManager
- Add `__get_environ()` C helper
- Implement `lib/std/ffi/env.bv` + `lib/std/env.bv`
- Test: `!GetEnvInt("BOUND")` → `get_env_int("BOUND")` → pure-Briv scan

### Step 3: Remove GetEnv#/GetEnvInt# from compiler (Phase 1)
- Signatures, LLVM codegen, webstack, interpreter, normalizer, proof_engine
- Remove declares from mod.rs
- Update all benchmarks
- Build + test

### Step 4: `print` Plugin (Phase 2)
- Create `src/plugin/print.rs`
- Implement `lib/std/io.bv` with `int_to_str`, `print_int`, `print_str`, `print_float`, `print_char`
- Test: `!Print(42)` → `print_int(42)` → `SysCall#(Write, 1, ...)`

### Step 5: Remove Print#/PutChar# from compiler (Phase 2)
- Same files as Step 3
- Remove declares from mod.rs
- Update all benchmarks
- Build + test

### Step 6: Prelude + docs
- Update `plugins/front/prelude.bv`
- Update architecture docs
- Update examples

### Step 7: Verify regression suite
- `cargo test --lib` (all pass)
- Compile all benchmarks
- `cargo build --release`
- Reference: `docs/plans/2026-07-19-benchmark-stabilization.md` — resume benchmark comparison after this migration

---

## Testing Strategy

### Unit tests (behavioral — not literal IR)

| Test | What it asserts |
|------|-----------------|
| `test_plugin_call_parse` | `!Print(42)` parses to `Expr::PluginIntercept { name: "Print", args: [Decimal(42)] }` |
| `test_plugin_call_unresolved_error` | Unhandled `!` call produces compiler error |
| `test_env_plugin` | `!GetEnv("BOUND")` rewrites to `getenv("BOUND")` call |
| `test_print_plugin_int` | `!Print(42)` rewrites to `print_int(42)` |
| `test_print_plugin_str` | `!Print("\"hi\"")` rewrites to `print_str("hi")` |
| `test_print_plugin_float` | `!Print(3.14)` rewrites to `print_float(3.14)` |
| `test_print_plugin_type_error` | `!Print(true)` produces type error |
| `test_getenv_dispatch` | Compiled program with `!GetEnvInt("BOUND")` reads the correct value |
| `test_print_stdout` | Compiled program with `!PrintLn(42)` outputs "42\n" to stdout |

### Integration tests

- Run each benchmark's `.ll` compilation (they use `!Print`/`!PrintLn`/`!GetEnvInt`)
- Compare output of Briv and C programs for `print_loop`, `mandelbrot`, `fannkuch_redux`
- Run `bash benchmarks/build_and_bench.sh --correctness` — all pass

### Regression guards

- All existing 931 tests must pass
- No change to existing optimization match arms (additive only)
- Benchmark `.text` size ratio must not regress (no optimizer regressions)

---

## File Change Summary

### New files (4)

| File | Purpose |
|------|---------|
| `src/plugin/print.rs` | Front plugin: `!Print`/`!PrintLn` dispatch |
| `src/plugin/env.rs` | Front plugin: `!GetEnv`/`!GetEnvInt` rewrite |
| `lib/std/io.bv` | Pure-Briv I/O: `print_int`, `print_str`, `print_float`, `print_char`, `int_to_str` |
| `lib/std/env.bv` | Pure-Briv env: `getenv`, `get_env_int` |

### Modified files (~25)

| File | Phase |
|------|-------|
| `src/lexer.rs` | 2 — add `Token::PluginCall` |
| `src/parser/expr.rs` | 2 — parse `!` prefix |
| `src/ast/expr.rs` | 2 — add `PluginIntercept` variant |
| `src/intrinsic_signatures.rs` | 1+2 — remove 4 entries |
| `src/backend/llvm/intrinsics.rs` | 1+2 — remove 4 dispatch arms + helpers |
| `src/backend/llvm/mod.rs` | 1+2 — remove 7+ declares |
| `src/backend/llvm/normalizer.rs` | 1+2 — remove from supported set |
| `src/backend/llvm/loop_engine/analysis.rs` | 1+2 — remove from observable list |
| `src/backend/webstack.rs` | 1+2 — remove handlers |
| `src/interpreter/intrinsics.rs` | 1+2 — remove handlers |
| `src/proof_engine/mod.rs` | 1+2 — remove from lists |
| `lib/runtime/briv_rt.c` | 1 — add `__get_environ()` |
| `lib/std/ffi/env.bv` | 1 — replace frgn |
| `lib/std/env.bv` | 1 — full implementation |
| `plugins/front/prelude.bv` | 1+2 — add auto-imports |
| `benchmarks/*.bv` (31 files) | 1+2 — `GetEnvInt#` → `!GetEnvInt`, `Print#` → `!PrintLn` |
| `docs/architecture/hash-words.md` | 1+2 — update intrinsic example |
| `docs/architecture/intrinsics-vs-stdlib.md` | 1+2 — update I/O row |
| `docs/architecture/overview.md` | 1+2 — update CIRCT normalizer ref |

### Deleted files (0)

No files deleted — stdlib files replace intrinsic functionality.

---

## Rationale Comments

Every modified code site must carry a `// 2026-07-19: <why>` comment:

- **Lexer/Parser/AST**: `// 2026-07-19: Plugin-intercept call. Must be resolved by a Front-stage plugin.`
- **Intrinsic removal sites**: `// 2026-07-19: Removed — migrated to stdlib. !Print dispatching via plugin.`
- **LLVM declare removals**: `// 2026-07-19: Removed with Print#/PutChar# — no longer needed.`
- **Benchmark changes**: `// 2026-07-19: !GetEnvInt replaces compiler intrinsic GetEnvInt#`
- **C runtime**: `// 2026-07-19: Helper for pure-Briv environ scan. Returns char **environ as Int.`

---

## References

| Document | Link |
|----------|------|
| This plan (active) | `docs/plans/2026-07-19-intrinsic-migration.md` |
| Prev: benchmark stabilization | `docs/plans/2026-07-19-benchmark-stabilization.md` |
| Plugin architecture | `docs/plans/2026-07-15-compiletime-meta-and-plugin-architecture.md` |
| Intrinsics vs stdlib | `docs/architecture/intrinsics-vs-stdlib.md` |
| Hash words | `docs/architecture/hash-words.md` |
| AGENTS.md Golden Rules | `AGENTS.md` — NO MAGIC (Rule 2), INTRINSICS BEFORE FRGN (Rule 3) |

---

## Syntax Decision: `name!()` not `!name()`

The `!` prefix conflicted with the `Not` unary operator (`!expr`). The `name!()` postfix syntax avoids this entirely:

```
!PrintLn("hello")     // REJECTED — ambiguous with Not operator
PrintLn!("hello")     // ACCEPTED — parallels frgn!, Ptr!, term!, syscall!
```

**How it parses:** Lexer sees `PrintLn` → `Identifier("PrintLn")`, then `!` → `Token::Not`, then `(` → `Token::LParen`. The parser's `parse_postfix` checks for `name!(args)` and produces `Expr::PluginIntercept { name: "PrintLn", args: [...] }`.

**No new token needed** — `name!()` reuses `Token::Not` in postfix position (after an expression), which never conflicts with the prefix `Not` operator (`!expr`).
