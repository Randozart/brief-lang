# Webstack TypeScript Emitter + WASM FFI — Implementation Plan

**Date**: 2026-06-19
**Status**: ❌ Superseded 2026-07-26

> **⚠️ SUPERSEDED 2026-07-26:** This plan is replaced by the WASM-first
> webstack v2 architecture. The TypeScript emitter (`WebstackGenerator`) is
> deprecated; the new pipeline uses `LlvmBackend(wasm32)` + `GlueWebGenerator`
> to produce `.wasm` + `dom-shim.mjs`.
>
> See:
> - `docs/architecture/features/rendered-brief-wasm.md` — current spec
> - `docs/plans/2026-07-26-rendered-brief-webstack-v2.md` — current plan
>
> Phase A (TS emitter) was partially implemented — the TS emitter in
> `src/backend/webstack.rs` exists and is the current code path. Phase B
> (`(wasm) import` syntax) was never fully implemented; the new architecture
> makes WASM the default compilation target instead of a special import form.

## Architecture

Brief's web target has two compilation paths that coexist in the same build:

```
            ┌─ .rbv → TypeScript emitter → .ts + .html  (reactive UI, signal wiring)
brief build ┤
            └─ (wasm) import → LLVM wasm32 → .wasm       (compute sidecar)
```

The TypeScript emitter replaces the current Rust/wasm-bindgen codegen. The `(wasm)`
import syntax allows `.rbv` files to embed compute-heavy `.bv` sub-programs compiled
to WASM. Intrinsics are handled natively by each target — no JS shims needed for
standard intrinsics. Custom `frgn from "c"` / `from "rust"` declarations in a
`.bv` that's being compiled to WASM are:

| `frgn from` | WASM target action |
|---|---|
| `"c"` (known intrinsic: `__print_int`, etc.) | Compile `brief_rt.c` to wasm32 bitcode, link |
| `"c"` (unknown function) | Compile-time error: *"no WASM implementation"* |
| `"javascript"` / `"ts"` | Compile-time error: *"JS/TS FFI cannot be used in WASM-compiled modules"* |
| `"rust"` | Compile-time error: *"Rust FFI not available in WASM target"* |
| Omitted (internal symbol) | Resolved from linked targets (e.g., stdlib) |

## Phase 0 — `.rbv` Format: Brief as Default

### Goal
Flip the `.rbv` file format so Brief code is the default content, not wrapped in `<script>` tags. CSS imports via `import "file.css"` already works — document and keep.

### 0.1 — Current Format (deprecated)

```html
<script type="brief">
let count: Int = 0;
txn inc [true][@count + 1 == count] { &count = count + 1; term; };
</script>

<view>
<span b-text="count">0</span>
</view>

<style>
span { color: red; }
</style>
```

### 0.2 — New Format

```rbv
let count: Int = 0;
txn inc [true][@count + 1 == count] { &count = count + 1; term; };

<view>
<span b-text="count">0</span>
</view>

<style>
span { color: red; }
</style>
```

Everything outside `<view>...</view>` and `<style>...</style>` is Brief code.

### 0.3 — CSS Imports (already works)

Within the Brief section:

```rbv
import "styles.css";

let count: Int = 0;
```

The existing pipeline (`run_rbv()` in `main.rs`) already:
1. Parses `import "styles.css"` as `TopLevel::Stylesheet(css_content)`
2. Extracts CSS content from the program
3. Combines it with inline `<style>` content
4. Writes `{name}.css` and links it in the generated HTML

### 0.4 — Changes

| File | Change | Lines |
|---|---|---|
| `src/rbv.rs` | `RbvFile::parse()` — remove `<script>` extraction, treat everything outside `<view>`/`<style>` as Brief | ~20 |
| `src/rbv.rs` | Update `RbvError` — remove `MissingScript`, add `MissingView` (only error if no `<view>`) | ~5 |
| `src/rbv.rs` | Update test | ~10 |

### 0.5 — `<script>` Backward Compatibility

For the transition period, `RbvFile::parse()` can detect the old format:
- If `<script>` tag is found, use old extraction logic
- Otherwise, use new Brief-as-default logic

This allows existing `.rbv` files to compile unchanged while new files use the cleaner syntax. Remove the old path after 2 releases.

### 0.6 — Tests

| Test | What it verifies |
|---|---|
| `test_parse_rbv_new_format` | Brief as default, `<view>` extracted, `<style>` optional |
| `test_parse_rbv_old_format` | `<script>`-wrapped format still works |
| `test_parse_rbv_no_view` | Error when `<view>` is missing |
| `test_parse_rbv_css_import` | `import "styles.css"` results in `Stylesheet` TopLevel |

---

## Phase A — TypeScript Emitter

### Goal
`brief build ui.rbv` emits valid `.ts` + `.html` — no wasm-bindgen dependency.

### A.1 — Signal Storage (webstack.rs)

Replace `Vec<JsValue>` with typed vectors:

| Brief type | TS storage | JS boundary |
|---|---|---|
| `Int` | `Float64Array` | `get_x(): number` |
| `Bool` | `Float64Array` (0/1) | `get_x(): boolean` |
| `Float` | `Float64Array` | `get_x(): number` |
| `String` | `string[]` | `get_x(): string` |
| `List<T>` | `T[][]` | `get_x(): T[]` |
| `Struct` / `Vector` | `any[]` | `get_x(): any` |

Generated TS:
```typescript
class State {
  num_signals: Float64Array = new Float64Array(N);
  str_signals: string[] = new Array(N).fill("");
  arr_signals: any[] = new Array(N);
  dirty_signals: boolean[] = new Array(N).fill(false);
  // ... signal_graph, dirty_transactions, etc.
}
```

### A.2 — Expression Codegen (expr_to_js_value → expr_to_ts)

Every Expr variant currently emits `JsValue::from(...as_f64()...)`.
Change to emit native TS arithmetic:

| Expr | Current (Rust) | New (TypeScript) |
|---|---|---|
| `Add(a, b)` | `JsValue::from(a.as_f64() + b.as_f64())` | `a + b` |
| `Sub(a, b)` | `JsValue::from(a.as_f64() - b.as_f64())` | `a - b` |
| `Mul(a, b)` | `JsValue::from(a.as_f64() * b.as_f64())` | `a * b` |
| `Div(a, b)` | `JsValue::from(a.as_f64() / b.as_f64())` | `Math.trunc(a / b)` (Int) or `a / b` (Float) |
| `Eq(a, b)` | `JsValue::from(a.as_f64() == b.as_f64())` | `a === b` |
| `Lt(a, b)` | `JsValue::from(a.as_f64() < b.as_f64())` | `a < b` |
| `And(a, b)` | `JsValue::from(a.as_f64() != 0 && b.as_f64() != 0)` | `a && b` |
| `Not(inner)` | `JsValue::from(inner.as_f64() == 0)` | `!inner` |
| `Identifier(x)` | `self.signals[id].clone()` | `this.num_signals[id]` / `this.str_signals[id]` |
| `PriorState(x)` | `prior_{x}.clone()` | `prior_{x}` |

Type dispatch: when both operands are `Int`, emit `Math.trunc()` for division.
When either is `Float`, emit `/` natively. Bool uses `!0` / `0` conventions.

### A.3 — Statement Codegen (statement_to_rust → statement_to_ts)

| Statement | Current (Rust) | New (TypeScript) |
|---|---|---|
| `Assignment { lhs = OwnedRef(x), expr }` | `self.signals[id] = val.into()` | `this.num_signals[id] = val` |
| `Let { name, expr }` | `let {name} = {expr_code};` | `let {name}: type = {expr_code};` |
| `Term { .. }` | `return;` | `return;` |
| `TermBang { .. }` | `return;` + barrier | `return;` + barrier |
| `Await { expr }` | `let __await_result = await {expr};` | `let __await_result = await {expr};` |
| `SyncBlock { body }` | sequential stmts | sequential stmts |
| `Foreach { list, body }` | `for __item in {list}` | `for (const __item of {list})` |

### A.4 — Reactive Transaction Codegen (generate_transaction)

Same algorithm, TS syntax:

```typescript
invoke_txn(params: type, ...): void {
  // Precondition
  if (!({pre_code})) return;
  // Save prior state
  const prior_num = this.num_signals.slice();
  const prior_str = this.str_signals.slice();
  const prior_arr = this.arr_signals.slice();
  // Body
  {statements}
  // Barrier (async await promises)
  {promise_barrier}
  // Postcondition
  if (!({post_code})) {
    this.num_signals = prior_num;
    this.str_signals = prior_str;
    this.arr_signals = prior_arr;
    return;
  }
}
```

### A.5 — Getters/Setters

```typescript
// Generated for each signal based on SignalType:
get counter(): number { return this.num_signals[counter_id]; }
set counter(val: number) {
  this.num_signals[counter_id] = val;
  this.dirty_signals[counter_id] = true;
}
```

No wasm-bindgen annotations needed — these are plain TS getters/setters.

### A.6 — poll_dispatch

Same contract as current: iterate `dirty_signals`, emit JSON `{op, el, value}`.
Only change: read from typed arrays instead of `Vec<JsValue>`.

### A.7 — `frgn from "javascript"` Handler

When `frgn foo(x: Int) -> Bool from "javascript"` is encountered in `.rbv`:
- Emit a TS function with the Brief name as a JS builtin call
- If the name matches a known JS global (`alert`, `console.log`, `fetch`, etc.),
  emit it directly. Otherwise emit `${name}(${args})` as a call expression.

Known mapping (stdlib):
| `frgn name` | TS emit |
|---|---|
| `__print_int#` | `console.log(n)` |
| `__put_char#` | `process.stdout.write(String.fromCharCode(c))` |
| `__read_file#` | `await fetch(path).then(r => r.text())` |
| `__exit#` | `process.exit(0)` / `throw null` |
| `__get_env_int#` | `parseInt(process.env[name] ?? "0", 10)` |

### A.8 — View Compiler Integration

Unchanged. The view compiler (`src/view_compiler/`) already produces target-agnostic
binding directives (`b-text`, `b-show`, `b-class`, `b-each`). The TS emitter
generates the DOM update glue from the same binding list — just different syntax.

### A.9 — Tests

Archive existing `test_webstack_*` tests alongside the old codegen.
Add new `test_ts_*` tests for the TS emitter:

| Test | What it verifies |
|---|---|
| `test_ts_signal_storage` | Float64Array/string[] layout matches signal_map |
| `test_ts_arithmetic_native` | `x + 1` emits `a + b`, not `JsValue::from(...)` |
| `test_ts_int_division_trunc` | `3 / 2` emits `Math.trunc(3 / 2)` |
| `test_ts_reactive_contract` | Precondition → body → rollback emits correctly |
| `test_ts_frgn_javascript` | `frgn from "javascript"` emits inline function |

### A.10 — Archive

| File | Action |
|---|---|
| `src/backend/webstack.rs` — old Rust codegen functions | Extract to `archive/backend/webstack_rust_codegen.rs` |
| `src/backend/webstack.rs` — rewrite in place with TS emitter | ~800 new lines |

The archived functions: `generate_rust_code()`, `generate_transaction()`,
`statement_to_rust()`, `expr_to_js_value()`, all test helpers that reference them,
and the `SignalType` enum (if not reused by TS path).

### A.11 — Migration Path

1. Extract `generate_rust_code()`, `statement_to_rust()`, `expr_to_js_value()`,
   and all their transitive callees from `src/backend/webstack.rs` into
   `archive/backend/webstack_rust_codegen.rs`
2. Remove the above from `src/backend/webstack.rs`
3. Implement TS emitter (`generate_ts_code()`, `statement_to_ts()`, `expr_to_ts()`)
   in the now-clean `src/backend/webstack.rs`
4. `run_build` for `.rbv` calls the TS path
5. Add new tests

---

## Phase B — `(wasm) import` Syntax

### Goal
`.rbv` files can import `.bv` sub-programs compiled to WASM.

### B.1 — Parser (parser.rs, ~30 lines)

New import prefix syntax:

```brief
(wasm) import physics from "physics.bv";
```

Parse rule in `parse_import()`:
```rust
if let Some(Ok(Token::LParen)) = self.peek() {
    self.advance();
    let target = self.parse_identifier()?;  // "wasm"
    self.expect(Token::RParen)?;
    // Continue with normal import parsing
}
```

Store target in a new `ImportTarget` enum:
```rust
enum ImportTarget { Default, Wasm }
```

### B.2 — Import Resolver (import_resolver.rs, ~80 lines)

When a `.rbv` has an import annotated with `(wasm)`:
1. Parse the `.bv` file to extract exported `defn`/`txn` signatures (names + parameter/return types)
2. Queue the `.bv` for wasm32 LLVM compilation
3. Store mapping: `import_name → (wasm_blob: Vec<u8>, exports: Vec<ExportSig>)`

### B.3 — LLVM wasm32 Target (main.rs, ~50 lines)

In `run_llvm_compile`, when target is `wasm32`:
```rust
if target == "wasm32" {
    let ll_path = out_dir.join(format!("{}.ll", stem));
    let wasm_path = out_dir.join(format!("{}.wasm", stem));

    // Emit LLVM IR as usual
    // llc -march=wasm32 -filetype=obj
    // wasm-ld --no-entry --allow-undefined
    // brief_rt.c compiled to wasm32 bitcode, linked in
}
```

Also compile `brief_rt.c` to wasm32 bitcode to resolve standard intrinsics.

### B.4 — WASM Embedding (webstack.rs, ~100 lines)

In the TS emitter, for each `(wasm)` import:

```typescript
// At module init:
const wasmBytes = Uint8Array.from(atob(`{base64}`), c => c.charCodeAt(0));
const wasmModule = (await WebAssembly.instantiate(wasmBytes, {})).instance;
const physics = wasmModule.exports;

// For each exported function, generate a typed wrapper:
function physics_simulate(p: number, n: number): number {
  return physics.simulate(p, n) as number;
}
```

### B.5 — `frgn` Cross-Compilation Rules

Applied at compile time when a `.bv` is targeted for WASM:

| Declaration | Action |
|---|---|
| `frgn name from "c"` where name is known intrinsic | Resolved via `brief_rt.wasm` |
| `frgn name from "c"` where name is unknown | **Error**: *"no WASM implementation for {name}"* |
| `frgn name from "javascript"` | **Error**: *"JS FFI not available in WASM-compiled modules"* |
| `frgn name from "rust"` / other | **Error**: *"FFI target not available in WASM"* |
| `import "link/..."` | Resolved if target linked; error if not |
| Instrinsic call (`print_int#`, etc.) | Handled natively (WASI or compiled-in) |

### B.6 — Tests

| Test | What it verifies |
|---|---|
| `test_parse_wasm_import` | `(wasm) import x from "y"` parses correctly |
| `test_wasm_sidecar_emit` | TS emitter produces correct instantiate code |
| `test_llvm_wasm32_target` | `-target wasm32` produces `.wasm` output |

---

## Implementation Order

```
Phase 0:
Step 0a: Rewrite RbvFile::parse() — Brief as default, backward compat
Step 0b: Update tests + commit
────────────────────────────────────────
Phase A:
Step 1:  Archive old Rust codegen → archive/backend/webstack_rust_codegen.rs
Step 2:  A.1 — Typed signal storage in TS emitter
Step 3:  A.3 — Statement codegen (statement_to_ts)
Step 4:  A.2 — Expression codegen (expr_to_ts)
Step 5:  A.4 — Transaction codegen
Step 6:  A.5-A.6 — Getters/setters + poll_dispatch
Step 7:  A.7 — frgn from "javascript" handler
Step 8:  A.9 — New tests + commit
Step 9:  Wire run_build to TS emitter
────────────────────────────────────────
Phase B:
Step 10: B.1 — Parser (wasm) import
Step 11: B.2 — Import resolver
Step 12: B.3 — LLVM wasm32 target
Step 13: B.4 — WASM embedding in TS emitter
Step 14: B.6 — Tests
```

Each step is individually testable and commitable.
