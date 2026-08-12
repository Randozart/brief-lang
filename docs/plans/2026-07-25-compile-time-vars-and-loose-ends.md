# Phase 9 — Compile-Time Variables, Runtime Intrinsics, and Remaining Loose Ends
## 2026-07-25

## Overview

Ten remaining items across four categories: compile-time infrastructure (`$let`/`$const`,
runtime intrinsics, `defn`/`txn` at compile time, `const` bridge), intrinsic migration
(`$` → `#`), stdlib cleanup (`bytes` removal, Rust benchmark), and protocol bridge
(SharedArrayBuffer).

## 1. Six `$` → `#` Intrinsics

### Motivation

`StrSplit$`, `SysQuery$`, `EnvGet$`, `TimeNow$`, `HttpFetch$`, and `ShellCmd$` are
general-purpose functions that happen to be exposed only at compile time. They don't
touch the AST, compiler state, or pipeline. Making them `#` intrinsics allows regular
`defn`/`txn` bodies to call them at runtime.

### Implementation

Each gets a handler in `execute_intrinsic()` in `src/interpreter/intrinsics.rs`.
The `$` versions in `eval.rs` become thin wrappers that delegate to the same logic.

| `$` intrinsic | `#` intrinsic | Implementation | Crate dep? |
|--------------|---------------|----------------|------------|
| `StrSplit$` | `StrSplit#` | `s.split(pat).map(str).collect::<Vec<_>>()` | No |
| `SysQuery$` | `SysQuery#` | `/proc/cpuinfo`, `uname`, `/proc/meminfo` reads | No |
| `EnvGet$` | `EnvGet#` | `std::env::var("KEY")` | No |
| `TimeNow$` | `TimeNow#` | `SystemTime::now().duration_since(UNIX_EPOCH)` | No |
| `HttpFetch$` | `HttpFetch#` | HTTP GET via `ureq` crate | `ureq` |
| `ShellCmd$` | `ShellCmd#` | `std::process::Command::new(cmd).output()` | No |

**Files changed:**
- `src/interpreter/intrinsics.rs` — add 6 new match arms in `execute_intrinsic()`
- `src/macros/eval.rs` — each `$` handler can stay as-is (the code is the same),
  or optionally delegate to the `#` version via the interpreter
- `src/compile.rs` — `ShellCmd#` and `HttpFetch#` need `--allow-run`/`--allow-net`
  flags forwarded for the sandbox

### Signature Convention

`#` intrinsics follow the same PascalCase + `#` suffix as existing intrinsics:
`StrSplit#`, `SysQuery#`, `EnvGet#`, `TimeNow#`, `HttpFetch#`, `ShellCmd#`.
The interpreter resolves them via the same `execute_intrinsic()` dispatch.

### Sandbox at Runtime

`ShellCmd#` and `HttpFetch#` at runtime have NO sandbox by default (the user
controls the runtime). At compile time, the existing `Sandbox` with `--allow-run`
and `--allow-net` applies. The runtime has no such restriction — if the user's
code calls `ShellCmd#`, it executes.

---

## 2. `$let` / `$const` — Compile-Time Variables

### Motivation

Stage blocks need to pass data between each other and to runtime `const`
declarations. Currently, `$defn` functions can compute values but there's no
persistent shared state between stages. `$let`/`$const` fill this gap.

### Syntax

```briev
// Top-level only:
$let counter = 0;
$const arch = SysQuery$("cpu.arch");

$(Parsed @ highest) {
    counter = counter + 1;
    EmitInfo$("counter: " + counter);
};

$(Normalized @ highest) {
    when arch == "arm64" {
        EmitInfo$("ARM-specific generation");
    };
};

// Bridge to runtime:
const BRIDGE_COUNT = counter;
const TARGET_ARCH = arch;

trg memory_space: Bool @ BRIDGE_COUNT;
```

**Rules:**
- `$let` — mutable, can be reassigned from stage blocks
- `$const` — immutable, error on reassignment
- Both disappear after compilation (not present in codegen output)
- The `$` prefix is part of the **declaration only**. Inside stage blocks,
  the bare name (without `$`) is used. The stage evaluator resolves bare
  names by checking: local scope → stage block params → `comptime_vars`.
  There's no ambiguity because `$let`/`$const` values are the only
  compile-time-persistent bindings that survive across stages.
- Can be referenced in `const X = name;` declarations (resolved at compile time)
- Can be referenced in `trg` declarations with `@ name` syntax
- NOT accessible inside regular `defn`/`txn` bodies (those run at runtime)

**Naming convention example:**
```briev
$let counter = 0;                      // declaration: $ prefix
$const arch = SysQuery$("cpu.arch");   // declaration: $ prefix

$(Parsed @ highest) {
    counter = counter + 1;             // usage: no $ prefix
    let local_x = 5;                   // local let — shadows comptime_vars
};

$(Normalized @ highest) {
    when arch == "arm64" {             // usage: no $ prefix
        EmitInfo$("ARM64 target");
    };
};
```

**Bare-name resolution order in stage block evaluator:**

```rust
// In eval_nav_chain, Expr::Identifier handler:
// 1. Check local scope (block-level let, function params)
if let Some(val) = scope.get(name) { return Ok(val.clone()); }
// 2. Check comptime_vars (from $let/$const declarations)
if let Some((val, _)) = pm.as_ref().and_then(|p| p.comptime_vars.get(name)) {
    return Ok(val.clone());
}
// 3. Error — undefined
Err(format!("undefined identifier '{}'", name))
```

### AST

```rust
pub enum TopLevel {
    // ... existing ...
    /// 2026-07-25: $let name = expr; — compile-time mutable variable.
    CompileTimeLet(String, Expr),
    /// 2026-07-25: $const name = expr; — compile-time immutable constant.
    CompileTimeConst(String, Expr),
}
```

### Parser

In `parse_top_level`, when `$let` or `$const` is encountered:

```rust
if self.check_identifier("$let") {
    return self.parse_compile_time_let();
}
if self.check_identifier("$const") {
    return self.parse_compile_time_const();
}
```

Parser reads: `$let name = expr;` → `TopLevel::CompileTimeLet(name, expr)`.
`$let` is lexed as `Token::Identifier("$let")` (same rule as `$defn`).

### Evaluation — Storage

`PluginManager` gains:

```rust
pub struct PluginManager {
    // ... existing ...
    /// 2026-07-25: Compile-time variables ($let/$const). Persist across stages.
    pub comptime_vars: HashMap<String, (NavValue, bool)>,  // (value, is_const)
}
```

### Evaluation — Access

Inside `evaluate_stage_block`, when an `Expr::Identifier(name)` is encountered
and it's not in the local scope, check `pm.comptime_vars[name]`:

```rust
// In eval_nav_chain, Expr::Identifier handler:
if let Some((val, _)) = pm.as_ref().and_then(|p| p.comptime_vars.get(name)) {
    return Ok(val.clone());
}
```

### Evaluation — Mutation

`Statement::Assign(target, value)` with a `$let` target writes to
`pm.comptime_vars[target].0`. `$let` targets are recognized by being in the
comptime_vars map. `$const` targets reject assignment with an error.

### Extraction

`extract_inline_stage_blocks` in `src/plugin/loader.rs` also extracts
`TopLevel::CompileTimeLet` and `TopLevel::CompileTimeConst` items from the
program into `PluginManager.comptime_vars`. They are evaluated immediately
at extraction time so their values are available to all subsequent stage blocks.

After extraction, the items are REMOVED from the program (same as `$defn`/`$txn`).
They do not reach codegen.

### Bridge to Runtime — `const` Initialization

When a `const X = comptime_name;` declaration references a `$let`/`$const` name,
the const initializer is resolved at compile time before type checking:

```rust
// In resolve.rs or typechecker, during const resolution:
if let Expr::Identifier(name) = &const_def.expr {
    if let Some((val, _)) = pm.comptime_vars.get(name) {
        // Replace with the literal value
        const_def.expr = nav_value_to_expr(val);
    }
}
```

This bakes the compile-time value into the runtime binary.

---

## 3. Regular `defn`/`txn` at Compile Time

### Motivation

Library functions like `std/math.bv:sqrt` should be callable from stage blocks
without needing a `$defn` duplicate. Any regular `defn` whose body can be
interpreted at compile time should be available.

### Implementation

In `eval_nav_chain`, after the `fn_registry` lookup for `$defn`/`$txn` fails:

```rust
// In Expr::Call handler, after fn_registry lookup:
// NEW: Search program for a regular defn or txn.
if let Some(defn) = find_defn_in_program(program, name) {
    return execute_defn_at_compile_time(defn, args, ...);
}
if let Some(txn) = find_txn_in_program(program, name) {
    return execute_txn_at_compile_time(txn, args, ...);
}
```

### `execute_defn_at_compile_time`

```rust
fn execute_defn_at_compile_time(
    defn: &Definition,
    args: &[Expr],
    program, universe, stage, scope, sandbox, pm,
) -> Result<NavValue> {
    // 1. Evaluate arg expressions
    let arg_values: Vec<NavValue> = args.iter()
        .map(|a| eval_nav_chain(a, ...))
        .collect::<Result<_, _>>()?;

    // 2. Create fresh scope with param bindings
    let mut fn_scope = Scope::new();
    for (i, (name, _ty)) in defn.parameters.iter().enumerate() {
        fn_scope.insert(name.clone(), arg_values[i].clone());
    }

    // 3. Evaluate body (same path as $defn)
    let result = evaluate_stage_block(&defn.body, program, universe,
        stage, &mut fn_scope, sandbox, pm)?;
    // (returns Option<NavValue> — None if no term reached)

    result.ok_or_else(|| "defn reached end without term".into())
}
```

### `execute_txn_at_compile_time`

```rust
fn execute_txn_at_compile_time(
    txn: &Transaction,
    args: &[Expr],
    program, universe, stage, scope, sandbox, pm,
) -> Result<NavValue> {
    // 1. Evaluate args
    let arg_values: Vec<NavValue> = args.iter()
        .map(|a| eval_nav_chain(a, ...))
        .collect()?;

    // 2. Initialize scope with params
    let mut txn_scope = Scope::new();
    // ... bind params ...

    // 3. Check precondition (contract.pre_condition)
    let pre_ok = eval_nav_chain(&txn.contract.pre_condition, ...)?;
    if !nav_is_truthy(&pre_ok) {
        return Err("txn precondition failed at compile time".into());
    }

    // 4. Evaluate body in convergent loop (max 1000 iterations, like $txn)
    for _ in 0..1000 {
        let result = evaluate_stage_block(&txn.body, ...)?;
        let post_ok = eval_nav_chain(&txn.contract.post_condition, ...)?;
        if nav_is_truthy(&post_ok) {
            return Ok(result.unwrap_or(NavValue::Void));
        }
    }
    Err("txn did not converge within 1000 iterations at compile time".into())
}
```

### Simulated Heap for `txn`

The interpreter's `Heap` is used as the simulated compile-time heap. When a `txn`
body allocates or mutates `%State` fields, they go into this heap. The heap is
fresh per `txn` execution (or shared, depending on semantics — see open question).

**Open question:** Is the compile-time heap shared across `txn` invocations or
fresh each time? Shared allows cumulative state mutation across calls; fresh
gives deterministic results per call. The `$let`/`$const` mechanism already
provides persistent state between stages. My recommendation: **fresh heap per `txn`
call** — let `$let` handle persistent state.

---

## 4. `const` Bridge and `trg @` Binding

### `const` from Compile-Time Value

```briev
$let exports = Tag$("export").Count$();
const EXPORT_COUNT = exports;
```

The `const` initializer is resolved at compile time. The `exports` identifier
is looked up in `pm.comptime_vars` and replaced with the literal value before
the type checker runs.

### `trg @` from Compile-Time Constant

```briev
const KNOWN_REGISTER = 42;

trg memory_space: Bool @ KNOWN_REGISTER;
```

The `@ name` syntax in `trg` declarations references a compile-time-known
constant. The resolver replaces `@ KNOWN_REGISTER` with `@ 42` before codegen.

**AST change:**

```rust
pub enum TriggerBinding {
    // ... existing ...
    /// 2026-07-25: @ identifier — compile-time-resolved register.
    CmpTimeRef(String),
}
```

The t r g parser reads `@ name` → `TriggerBinding::CompTimeRef(name)`. The
resolver replaces it with the actual register number before codegen.

---

## 5. Extraction and Pipeline Integration

`extract_inline_stage_blocks` in `src/plugin/loader.rs` is extended to handle
`TopLevel::CompileTimeLet` and `TopLevel::CompileTimeConst`:

```rust
// After extracting $defn/$txn:
for (i, item) in program.iter().enumerate() {
    match item {
        TopLevel::CompileTimeLet(name, expr) => {
            let val = evaluate_stage_block(&[Statement::Expression(expr.clone())],
                program, universe, &StageKind::Parsed, &mut Scope::new(), sandbox, &mgr)?;
            mgr.comptime_vars.insert(name.clone(), (val.unwrap_or(NavValue::Void), false));
            indices.push(i);
        }
        TopLevel::CompileTimeConst(name, expr) => {
            // ... same, but with is_const=true ...
        }
        _ => {}
    }
}
// Remove extracted items from program (reverse order)
for i in indices.into_iter().rev() { program.remove(i); }
```

---

## 6. Rust Benchmark (`gen_rust`)

### Motivation

`gen_rust.bv` generates `extern "C"` declarations for Briev exports. The Tier 1
claim (~1ns, same as C) has never been benchmarked.

### Implementation

1. Build the `.so` from `benchmarks/metropolitan/bench_add.bv --shared`
2. Create a Rust benchmarking program that links to the `.so`:

```rust
// bench_rust.rs
extern "C" {
    fn add(a: i64, b: i64) -> i64;
}

fn main() {
    // Warmup + verify correctness
    assert_eq!(unsafe { add(3, 4) }, 7);
    // Benchmark: 100000 iterations, measure total time
    let start = std::time::Instant::now();
    for _ in 0..100000 {
        unsafe { add(3, 4) };
    }
    let elapsed = start.elapsed();
    println!("Rust extern C: {}ns/call", elapsed.as_nanos() / 100000);
}
```

3. Compile and run:
```bash
rustc -O bench_rust.rs -L out -l bench_add -o bench_rust
LD_LIBRARY_PATH=out ./bench_rust
```

**Expected result:** ~1ns per call (same as C baseline), confirming Tier 1.

**Files added:**
- `benchmarks/metropolitan/bench_rust.rs`
- `benchmarks/metropolitan/Makefile` target for Rust
- Generated `bridge.rs` is the reference for the extern "C" block

---

## 7. `bytes` Removal

### Motivation

`bytes <~ N` has been superseded by `maxbits <~ N*8`. The `bytes` key is still
present in ~15 `.bv` metadata declarations. These need to be migrated.

### Sites

| File | Line | Current | Target |
|------|------|---------|--------|
| `lib/std/from-bits.bv:22` | `bytes <~ 8` | `maxbits <~ 64` |
| `lib/std/from-bits.bv:54` | `bytes <~ 4` | `maxbits <~ 32` |
| `lib/std/from-bits.bv:75` | `bytes <~ 16` | `maxbits <~ 128` |
| `lib/std/from-bits.bv:91` | `bytes <~ 24` | `maxbits <~ 192` |
| `lib/std/from-bits.bv:122` | `bytes <~ 24` | `maxbits <~ 192` |
| `lib/std/from-bits.bv:175` | `bytes <~ 64` | `maxbits <~ 512` |
| `lib/std/from-bits.bv:193` | `bytes <~ 4` | `maxbits <~ 32` |
| `lib/std/from-bits.bv:217` | `bytes <~ 1` | `maxbits <~ 8` |
| `lib/std/from-bits.bv:235` | `bytes <~ 8` | `maxbits <~ 64` |
| `lib/std/types/float.bv:12` | `bytes <~ 2` | `maxbits <~ 16` |
| `lib/std/types/float.bv:23` | `bytes <~ 16` | `maxbits <~ 128` |
| `lib/glue/rust/types.bv:6` | `bytes <~ 8` | `maxbits <~ 64` |
| `lib/glue/rust/types.bv:13` | `bytes <~ 8` | `maxbits <~ 64` |
| `lib/glue/python/types.bv:6` | `bytes <~ 8` | `maxbits <~ 64` |
| `lib/glue/python/types.bv:13` | `bytes <~ 16` | `maxbits <~ 128` |

**No compiler changes needed.** `bytes` is metadata parsed by the generic
`~>` property system. Changing the key name is a find-and-replace in `.bv` files.

---

## 8. SharedArrayBuffer Protocol Bridge

### Motivation

The protocol bridge (`gen_protocol`) uses subprocess communication at ~5ms per
call. Spawn overhead dominates. A persistent worker process with mmap'd shared
memory drops this to ~5µs — a 1000× improvement.

### Architecture

```
Parent (Python/JS/shell)          Worker (persistent process)
│                                      │
│  mmap (MAP_SHARED)                   │
│  ┌────────────────────────┐          │
│  │ req_flag: 0 │ resp_flag: 0 │      │  Worker loop:
│  │ args: [3, 4]             │      │    wait for req_flag
│  │ result: 0                │      │    read args
│  └────────────────────────┘          │    execute fn
│                                      │    write result
│  req_flag = 1               ───────  │    set resp_flag
│  (spin on resp_flag)                │
│  read result                ───────  │
│  resp_flag = 0                      │
```

### Implementation

**Worker — Pure Briev with `frgn` for POSIX calls.**

The shim is a Briev program using `frgn` for `shm_open`/`mmap`/`ftruncate`
and a `node` for the event loop. No C compiler needed — just `briev build --shared`.

```briev
// proto_shm.bv — auto-generated by gen_shm.bv
frgn shm_open(name: String, oflag: Int, mode: Int) -> Int as frgn_shm_open from "c" fallback -1;
frgn ftruncate(fd: Int, length: Int) -> Int as frgn_ftruncate from "c" fallback -1;
frgn mmap(addr: Int, length: Int, prot: Int, flags: Int, fd: Int, offset: Int) -> Ptr<Int> as frgn_mmap from "c" fallback 0;
frgn munmap(addr: Int, length: Int) -> Int as frgn_munmap from "c" fallback -1;
frgn shm_unlink(name: String) -> Int as frgn_shm_unlink from "c" fallback -1;

const SHM_SIZE = 64;
const PROT_RW = 3;
const MAP_SHARED = 1;
const O_RDWR = 2;
const O_CREAT = 64;

const shm_fd = frgn_shm_open("/briev_bridge", O_RDWR | O_CREAT, 0o666);
frgn_ftruncate(shm_fd, SHM_SIZE);
const shm = frgn_mmap(0, SHM_SIZE, PROT_RW, MAP_SHARED, shm_fd, 0);

export defn add(a: Int, b: Int) -> Int { term a + b; };

node bridge_loop [shm != 0][true] {
    let flags = Ptr<Int>(shm).read();
    when flags == 1 {                    // req_flag = requested
        let fn_idx = Ptr<Int>(shm + 8).read();
        let a = Ptr<Int>(shm + 16).read();
        let b = Ptr<Int>(shm + 24).read();
        let result = match fn_idx {
            0 => { term add(a, b); };
            _ => { term 0; };
        };
        Ptr<Int>(shm + 32).write(result);  // write result
        Ptr<Int>(shm + 4).write(1);         // resp_flag = ready
        Ptr<Int>(shm).write(0);             // req_flag = done
    };
    when flags == 0xFF {                   // shutdown
        frgn_munmap(shm, SHM_SIZE);
        frgn_shm_unlink("/briev_bridge");
    };
};
```

**Client wrapper** (generated by `gen_shm.bv`):

Python/JS/Shell client that:
1. Creates the shared memory segment via `shm_open`
2. Spawns the worker process (the compiled Briev binary)
3. Writes args to shared memory via mmap
4. Flips `req_flag`
5. Spins on `resp_flag`
6. Reads result, clears `resp_flag`

**Files changed:**
- `lib/ffi/gen_shm.bv` — new generator (produces Briev shim + client wrappers)
- `benchmarks/protocol/` — new benchmark directory

**Expected latency:** ~5µs per call (vs ~5ms for subprocess protocol).
1000× improvement from eliminating fork+exec per call. No C toolchain needed.

---

## 9. Implementation Order

| Step | What | Files | Dependencies |
|------|------|-------|-------------|
| 1 | `#` intrinsics (6 in execute_intrinsic) | `src/interpreter/intrinsics.rs` | None |
| 2 | `$let`/`$const` AST + parser | `src/ast/top.rs`, `src/parser/definitions.rs`, `src/lexer.rs` | None |
| 3 | `$let`/`$const` evaluation + PluginManager | `src/plugin/mod.rs`, `src/macros/eval.rs`, `src/plugin/loader.rs` | Step 2 |
| 4 | `defn`/`txn` at compile time | `src/macros/eval.rs` | None |
| 5 | `const` bridge + `trg @` | `src/parser/definitions.rs`, `src/resolver.rs` | Step 3 |
| 6 | `bytes` removal | `.bv` files (15+ sites) | None |
| 7 | Rust benchmark | `benchmarks/metropolitan/bench_rust.rs`, `Makefile` | None |
| 8 | SharedArrayBuffer protocol bridge | `lib/ffi/gen_shm.bv`, `benchmarks/protocol/` | None |

Steps 1, 6, 7, and 8 are independent and can proceed in parallel.
Steps 2+3+4+5 are a dependency chain and must be done in order.

---

## 10. Risk Mitigation

| Risk | Mitigation |
|------|------------|
| `$let`/`$const` conflicts with existing `let`/`const` at stage-eval scope | `comptime_vars` checked AFTER local scope, so local `let x` shadows `$let x` |
| `defn` at compile time loops forever | Use existing `optimize_budget` limit (max expression evaluations) |
| SharedArrayBuffer has platform-specific path | Use `shm_open` (POSIX), fallback to file-backed mmap, fallback to subprocess |
| `trg @` with non-integer `const` | Type-check the `const` target — must be `Int` or resolve to integer literal |
| `bytes` used in runtime FRGN C struct size computation | `bytes` in metadata is read by the normalizer and LLVM backend via `type_size`. Changing the key from `bytes` to `maxbits` in metadata doesn't affect `ResolvedType.bytes` — the normalizer sets that from the TypeDef's slot layout or the primordial table, not from the metadata key name. `bytes` in metadata was always a hint, not the source of truth. |
