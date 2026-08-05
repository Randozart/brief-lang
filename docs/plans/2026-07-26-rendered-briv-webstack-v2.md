# Plan: Rendered Briv Webstack v2 — WASM-First Architecture

**Date:** 2026-07-26
**Status:** Implementation plan
**Spec:** `docs/architecture/features/rendered-briv-wasm.md`

## Baseline

Before any changes, record the current benchmark results for the webstack:

```bash
cargo build --release
bash benchmarks/build_and_bench.sh --runtime
```

The existing webstack backend emits TypeScript. These numbers serve as the baseline for comparison once the WASM-first pipeline is operational.

**Current webstack backend stats:**
- `src/backend/webstack.rs`: 1350 lines (TS emitter)
- `src/backend/webstack_normalizer.rs`: 64 lines
- `src/view_compiler.rs`: 1015 lines
- No WASM generation occurs in the webstack path
- GLUE system has no `[web]` target

## Scope

This plan covers 7 phases. Each phase is a logical commit boundary. After each phase, `cargo test --lib` must pass.

All work happens in a **dedicated git worktree** at `../briv-compiler-wasm-webstack/` to avoid interfering with other agents working on the core compiler.

### Worktree Setup

```bash
cd /home/randozart/Desktop/Projects/briv-compiler
git worktree add -b wasm-webstack-v2 ../briv-compiler-wasm-webstack main
cd ../briv-compiler-wasm-webstack
cargo build  # verify the worktree compiles
```

---

## Phase 1: Wire `render` Keyword in Parser

**Goal:** The `render struct` and `render obj` syntax produces `TopLevel::RenderBlock` AST nodes.

### Preconditions

- `Token::Render` already exists in `src/lexer.rs:133`
- `TopLevel::RenderBlock(RenderBlock)` already exists in `src/ast/top.rs:66`
- `RenderBlock` struct already exists at `src/ast/top.rs:1007`
- The old `rstruct` keyword (`Token::Rstruct`, `TopLevel::RStruct`) exists but is deprecated

### Work

1. **Add `render` to `parse_top_level()` in `src/parser/definitions.rs`:**

   After the `Some(Token::Enum)` arm (line 39), add:
   ```rust
   Some(Token::Render) => {
       if self.check_identifier("struct") {
           self.parse_render_struct()
       } else if self.check_identifier("obj") {
           self.parse_render_obj()
       } else {
           self.error_at_current("expected 'struct' or 'obj' after 'render'")
       }
   }
   ```

2. **Add `parse_render_struct()` method:**

   ```rust
   fn parse_render_struct(&mut self) -> Result<TopLevel, SyntaxError> {
       // consume "struct" keyword
       self.advance();
       let name = self.expect_identifier()?;
       // expect '{' then HTML content then '}'
       // For now, the HTML is a single token range — the view compiler
       // will parse it. We store the raw HTML string.
       let view_html = self.parse_raw_block_content()?;
       Ok(TopLevel::RenderBlock(RenderBlock {
           struct_name: name,
           view_html,
           span: self.current_span(),
       }))
   }
   ```

3. **Add `parse_render_obj()` method:** Same signature as `parse_render_struct()` — identical AST representation. `render obj` may carry additional metadata in the future (method contracts, lifecycle hooks), but for Phase 1 it desugars the same way.

4. **Emit deprecation warning for `rstruct`:** When `Token::Rstruct` is encountered, print a warning and parse as `RStruct` (existing path), then convert to `RenderBlock` in the normalizer.

### Tests

- `cargo test --lib` — existing parser tests pass
- Add parser test: `test_parse_render_struct()` — feed `render struct Foo { <div>b-text="x"</div> }` and verify `TopLevel::RenderBlock`
- Add parser test: `test_parse_render_obj()` — same for `render obj`
- Add parser test: `test_parse_render_rejects_bare_render()` — `render foo` produces error

### Files Changed

| File | Change |
|------|--------|
| `src/parser/definitions.rs` | Add `render` arms in `parse_top_level()`, add `parse_render_struct()` and `parse_render_obj()` methods |
| `src/parser/helpers.rs` | Add `parse_raw_block_content()` helper (extract HTML content between `{` `}`) |

---

## Phase 2: Add GLUE `[web]` Target

**Goal:** The GLUE registry knows about the `web` target protocol mappings.

### Preconditions

- GLUE config loads from `lib/glue.toml` via `src/glue/config.rs`
- No web target exists yet

### Work

1. **Add `[web]` section to `lib/glue.toml`:**

   ```toml
   [web]
   types_module = "glue/web/types.bv"
   extension = "mjs"
   bridge_kind = "wasm_runtime"
   calling_convention = "wasm_import"
   
   [web.protocols]
   "#Int" = { native = "number", wasm_abi = "i32" }
   "#Float" = { native = "number", wasm_abi = "f64" }
   "#Bool" = { native = "boolean", wasm_abi = "i32" }
   "#String" = { native = "string", wasm_abi = "i32" }
   "#Element" = { native = "Element", wasm_abi = "i32" }
   ```

2. **Add `wasm_abi` field to `ProtocolEntry` in `src/glue/config.rs`:**

   ```rust
   pub struct ProtocolEntry {
       pub native: String,
       pub c_abi: Option<String>,      // existing
       pub wasm_abi: Option<String>,   // new — for web target
   }
   ```

   This field is optional — backends that don't target WASM ignore it.

3. **Add `glue/web/types.bv`:**

   ```briv
   // Web protocol type definitions
   // Maps Briv types to web runtime representations
   
   type Element: #System {
       // Opaque handle — i32 in WASM, HTMLElement in JS
   };
   
   type CanvasContext: #System {
       // Opaque handle — WebGL/WebGPU context
   };
   ```

### Tests

- `cargo test --lib` — GLUE config parse tests pass
- Add test: `test_glue_web_target_loads()` — verify `lib/glue.toml` parses with `[web]` section
- Add test: `test_web_protocol_mappings()` — verify `"#Int".wasm_abi == Some("i32")`

### Files Changed

| File | Change |
|------|--------|
| `lib/glue.toml` | Add `[web]` section |
| `src/glue/config.rs` | Add `wasm_abi` field to `ProtocolEntry` |
| `lib/glue/web/types.bv` | New file — web protocol type definitions |

---

## Phase 3: GlueWebGenerator — JS Shim Generator

**Goal:** A new `GlueWebGenerator` module emits the JS runtime shim (`dom-shim.mjs`) from the view compiler's bindings and the WASM module's state layout.

### Work

1. **Create `src/glue/web_generator.rs`:**

   Public struct `GlueWebGenerator` with:
   ```rust
   pub struct GlueWebGenerator {
       wasm_module: Vec<u8>,                          // compiled .wasm bytes
       bindings: Vec<Binding>,                        // from ViewCompiler
       state_layout: StateLayout,                     // from compile-time analysis
       protocol_mappings: HashMap<String, ProtocolEntry>, // from GLUE config
   }
   
   pub struct GlueWebOutput {
       pub dom_shim: String,   // dom-shim.mjs content
       pub dts: String,        // .d.ts declarations
   }
   
   impl GlueWebGenerator {
       pub fn generate(&self) -> Result<GlueWebOutput, String>;
   }
   ```

2. **`generate()` emits `dom-shim.mjs` with:**

   ```javascript
   // Auto-generated by Briv/GLUE Web Generator
   
   export class WasmDomRuntime {
     constructor(wasmBytes, importObject = {}) {
       this._handles = [null];       // handle table: handle → DOM object
       this._bindingTable = [];       // handle → { applyFn, elementId }
       this._pendingFlush = null;
       this._init(wasmBytes, importObject);
     }
   
     async _init(wasmBytes, importObject) {
       const wasm = await WebAssembly.instantiate(wasmBytes, {
         env: {
           __web_flush_state: (updatesPtr, count) => {
             this._applyFlush(updatesPtr, count);
           },
           // ... frgn from #Web imports mapped to handle table ops ...
         }
       });
       this._instance = wasm.instance;
       this._memory = wasm.instance.exports.memory;
       this._readStateLayout(wasm.instance.exports.state_layout());
       this._buildBindingTable(wasm.instance.exports.state_layout());
       // ... per-binding watcher setup ...
     }
   
     _applyFlush(updatesPtr, count) {
       const mem = new DataView(this._memory.buffer);
       let offset = updatesPtr;
       for (let i = 0; i < count; i++) {
         const handle = mem.getUint32(offset, true); offset += 4;
         const valPtr = mem.getUint32(offset, true); offset += 4;
         const valLen = mem.getUint32(offset, true); offset += 4;
         const binding = this._bindingTable[handle];
         if (binding) {
           const value = valLen > 0
             ? new TextDecoder().decode(new Uint8Array(this._memory.buffer, valPtr, valLen))
             : null;
           binding.applyFn(value);
         }
       }
     }
   }
   ```

3. **`generate()` emits `app.d.ts`:**

   ```typescript
   export interface WasmDomRuntime {
     // Per-transaction methods exposed from the WASM module
     increment(): void;
     reset(): void;
     // ...
   }
   
   export function createApp(): Promise<WasmDomRuntime>;
   ```

### Tests

- `cargo test --lib` — new module compiles
- Add unit test: create a mock `StateLayout` and `Vec<Binding>`, call `generate()`, verify the output `.mjs` contains the expected binding table entries and flush handler
- Add unit test: verify the `.d.ts` output contains export declarations for each transaction

### Files Changed

| File | Change |
|------|--------|
| `src/glue/web_generator.rs` | New file — `GlueWebGenerator` |
| `src/glue/mod.rs` | Add `pub mod web_generator;` |

---

## Phase 4: Emit `__web_flush_state` in LLVM Backend

**Goal:** When compiling for the webstack (`.rbv` or `--backend webstack`), the LLVM backend emits the `__web_flush_state` import call at each `term` statement, and exports the `state_layout` function and `generation` global.

### Preconditions

- `LlvmBackend` already supports `wasm32-unknown-wasi` target triple (lines 1088-1091 in `src/backend/llvm/mod.rs`)
- The backend already has `is_wasm()` in `CompilerContext` (line 168-169 in `context.rs`)
- The `Term` statement handler already exists in `emit_stmt.rs`

### Work

1. **Add `emit_web_flush_state()` method to `LLVMBuilder` in `src/backend/llvm/builder.rs`:**

   ```rust
   /// Emit the __web_flush_state import call.
   /// Called at `term;` statements when compiling for webstack.
   /// Collects all state fields modified by this transaction and writes
   /// their current offsets+values to the flush buffer, then calls
   /// __web_flush_state(flush_buf_ptr, field_count).
   /// 2026-07-26: Phase 4 — WASM webstack DOM flush.
   pub fn emit_web_flush_state(&mut self, out: &mut String, indent: &str) {
       // Only emit if compiling for webstack (is_wasm() && webstack_enabled)
       if !self.ctx.is_wasm() || !self.webstack_enabled {
           return;
       }
       // ... emit store operations to flush buffer at known offset ...
       // ... emit call to __web_flush_state import ...
   }
   ```

2. **Add `webstack_enabled` field to `CompilerContext`:**

   ```rust
   /// 2026-07-26: Phase 4 — Set when compiling for webstack (.rbv or --backend webstack).
   /// Enables __web_flush_state emission at term statements.
   pub webstack_enabled: bool,
   ```

3. **Add `with_webstack()` builder method to `LlvmBackend`:**

   ```rust
   pub fn with_webstack(mut self, enabled: bool) -> Self {
       self.ctx.webstack_enabled = enabled;
       self
   }
   ```

4. **Modify `emit_toplevel.rs`** to emit the `state_layout` export and `generation` global when `webstack_enabled`:

   ```rust
   if ctx.webstack_enabled {
       // Export generation counter
       writeln!(out, "@__web_generation = global i32 0")?;
       // Export state layout table
       // ... emit struct with field offsets ...
       // Export the state_layout function
       writeln!(out, "define i32 @state_layout() {{")?;
       writeln!(out, "  ret i32 ptrtoint ({}* @__web_layout to i32)", layout_ty)?;
       writeln!(out, "}}")?;
   }
   ```

5. **Modify `emit_stmt.rs` `Term` arm** to call `emit_web_flush_state`:

   ```rust
   Statement::Term(swan_song) | Statement::TermBang(swan_song) => {
       // 2026-07-26: Phase 4 — webstack flush at term
       builder.emit_web_flush_state(out, indent);
       // ... existing term emission ...
   }
   ```

6. **Modify `src/compile.rs` `codegen()` function** (line 789) to wire `with_webstack` for the webstack backend:

   ```rust
   BackendKind::Webstack => {
       let mut b = LlvmBackend::new()
           .with_webstack(true)
           .with_int_bits(32)  // WASM: 32-bit ints
           .with_target_triple("wasm32-unknown-wasi")
           // ... other config ...
       output = b.generate(items, None);
       ".wasm"  // or ".ll" for inspection
   }
   ```

### Tests

- `cargo test --lib` — existing LLVM backend tests pass
- Add IR snapshot test: compile a minimal `.rbv` with `--backend webstack`, verify the emitted `.ll` contains `__web_flush_state` call and `state_layout` export
- Add IR snapshot test: verify `__web_flush_state` is NOT emitted when `webstack_enabled` is false

### Files Changed

| File | Change |
|------|--------|
| `src/backend/llvm/context.rs` | Add `webstack_enabled: bool` field |
| `src/backend/llvm/builder.rs` | Add `emit_web_flush_state()` method |
| `src/backend/llvm/mod.rs` | Add `with_webstack()` builder method |
| `src/backend/llvm/emit_toplevel.rs` | Emit `state_layout` export, `generation` global |
| `src/backend/llvm/emit_stmt.rs` | Call `emit_web_flush_state` at `Term` |
| `src/compile.rs` | Wire `with_webstack(true)` for `BackendKind::Webstack` |

---

## Phase 5: `.bv` → Webstack Path (Logic-Only WASM)

**Goal:** A `.bv` file can be compiled to a logic-only WASM module via `--backend webstack`.

### Preconditions

- Phase 4 wires `BackendKind::Webstack` to `LlvmBackend` with WASM target
- No view compiler or `__web_flush_state` needed for `.bv`

### Work

1. **Modify `src/compile.rs` to handle `.bv` + `BackendKind::Webstack`:**

   ```rust
   BackendKind::Webstack => {
       let has_view = !bindings.is_empty(); // bindings from ViewCompiler
       let mut b = LlvmBackend::new()
           .with_webstack(has_view)   // only emit flush calls if there's a view
           .with_int_bits(32)
           .with_target_triple("wasm32-unknown-wasi");
       // ... config ...
       output = b.generate(items, None);
       ".wasm"
   }
   ```

2. **Update `config/targets.toml` to allow `.bv` → webstack:**

   No change needed — the `--backend` CLI flag already overrides the extension mapping. But add a documentation note that `.bv` files support `--backend webstack`.

3. **Update `GlueWebGenerator` to produce minimal metropipe-shim for `.bv`:**

   When there are no view bindings, the shim contains only the `frgn from #Web` import stubs — no DOM binding table, no `__web_flush_state`.

### Tests

- `cargo test --lib` — existing tests pass
- Add integration test: compile `examples/hello.bv` with `--backend webstack`, verify `hello.wasm` is produced
- Add integration test: verify the WASM module exports a `memory` and the functions declared in the source

### Files Changed

| File | Change |
|------|--------|
| `src/compile.rs` | Handle `has_view` flag for `BackendKind::Webstack` |
| `src/glue/web_generator.rs` | Handle empty bindings case |

---

## Phase 6: Type-Driven Import Stubs + CSS Passthrough

**Goal:** `frgn from #Web` produces real JS import stubs driven by type
signatures (not function names). `<style>` blocks from `.rbv` are emitted
as `app.css`. `render struct`/`render obj` own the DOM — no `frgn` needed
for element creation.

### Why Name Matching Is Wrong

A match arm for `"create_element"` solves today's benchmark but fails for
a user's `custom_fetch(url: String) -> Element from #Web`. The type
signature already carries all information the GLUE system needs:

| Briv type | wasm_abi | marshal in | marshal out |
|------------|----------|------------|-------------|
| `String` | `i32` | `_readString(ptr)` | `_writeString(str)` |
| `Int` | `i32` | raw `val` | raw `val` |
| `Float` | `f64` | raw `val` | raw `val` |
| `Bool` | `i32` | `val !== 0` | `val ? 1 : 0` |
| `Element` | `i32` | `_handles[handle]` | `_handles.push(obj) - 1` |
| `CanvasContext` | `i32` | `_handles[handle]` | `_handles.push(obj) - 1` |

The generator iterates each parameter and return type, looks up the protocol
mapping in the GLUE config, and emits the appropriate marshal code. No
`match` on function names.

### 6a — Type-Driven `generate_imports()`

For each `frgn from #Web` declaration:

```javascript
_buildImports() {
  return {
    __web_flush_state: (updatesPtr, count) => { this._applyFlush(...); },
    // type-driven — Example: frgn get_canvas(id: String) -> CanvasContext from #Web
    get_canvas: (strPtr0) => {
      const id = this._readString(strPtr0);
      const canvas = document.getElementById(id);
      if (!canvas) return 0;
      const ctx = canvas.getContext('webgl2');
      return this._handles.push(ctx) - 1;
    },
    // type-driven — Example: frgn console_log(msg: String) from #Web
    console_log: (strPtr0) => {
      console.log(this._readString(strPtr0));
    },
  };
}
```

Algorithm:
1. For each `ForeignBinding` with `FromSpec::Protocol("#Web")`:
   a. Briv name → JS function name (the `as` clause or foreign symbol)
   b. For each input param: look up protocol mapping by type, emit marshal-in code
   c. For return type: look up protocol mapping, emit marshal-out code
   d. If `Element` or `CanvasContext` return: emit `_handles.push(...) - 1`
2. The function name is an identifier — never matched against a string list.

### 6b — `render struct`/`render obj` HTML DOM Ownership

The `<html>` template inside `render struct`/`render obj` is processed by
the view compiler into `Vec<Binding>`. These bindings are the **sole source**
of DOM structure. No `frgn create_element` exists — the compiler generates
the DOM creation code from the template at compile time, and the JS shim's
`_loadStateLayout()` reads WASM state at known offsets to update element
content/attributes.

The `render struct`/`render obj` pattern:
```briv
render struct Counter {
    <div class="counter">
        <span b-text="count">0</span>
        <button b-trigger:click="increment">+</button>
    </div>
};
```
Desugars to:
- View compiler produces `Vec<Binding>`: `[Text { count → #counter-span }, Trigger { click → increment }]`
- GlueWebGenerator's binding table maps each binding to a WASM state field offset
- At `__web_flush_state`, the JS shim applies the new text content or fires the transaction

Legitimate use cases for `frgn from #Web`:
- `frgn get_canvas(id: String) -> CanvasContext from #Web` — browser canvas API
- `frgn present_frame(ctx: CanvasContext) from #Web` — GPU presentation
- `frgn console_log(msg: String) from #Web` — debugging
- `frgn fetch(url: String) -> String from #Web` — HTTP requests

### 6c — CSS Passthrough

The `<style>` block (extracted by `RbvFile::parse()`) is written to `app.css`
alongside the compiled `.wasm` and `.mjs`. A companion `index.html` links it:

```html
<!DOCTYPE html>
<html>
<head>
  <link rel="stylesheet" href="app.css">
  <script type="module" src="dom-shim.mjs"></script>
</head>
<body>
  <script type="module">
    import { createApp } from './dom-shim.mjs';
    const wasm = await fetch('app.wasm');
    const app = await createApp(new Uint8Array(await wasm.arrayBuffer()));
  </script>
</body>
</html>
```

### Tests

- Unit: `GlueWebGenerator` with `frgn (String) -> Element from #Web` produces
  JS stub with `_readString()` call and `_handles.push()` return
- Unit: `GlueWebGenerator` with `frgn (Element, String) from #Web` produces
  JS stub with `_handles[handle]` lookup and `_readString()` call
- Unit: `GlueWebGenerator` with `frgn (Int, Float, Bool) from #Web` produces
  JS stub with raw value parameters
- Unit: No function name strings appear in the compiler

### Files Changed

| File | Change |
|------|--------|
| `src/glue/web_generator.rs` | Rewrite `generate_imports()` — type-driven dispatch using protocol mappings. Add `frgn_to_js_stub()` helper. Tests for each protocol mapping. |
| `src/compile.rs` | Filter `frgn from #Web` declarations, pass to `GlueWebGenerator`. Write `app.css` from style content. Write `index.html` stub. |
| `lib/glue.toml` | Add `[web.templates]` entry for `index.html` |

---

## Phase 7: Migration and Cleanup

**Goal:** Migrate existing examples, deprecate old webstack code, remove TS emitter path.

### Work

1. **Migrate `.rbv` examples** from old syntax to `render struct`:
   - `examples/rstruct-demo.rbv` → `render struct`
   - `examples/counter.rbv` → `render struct`
   - `examples/shopping_cart.rbv` → `render struct`
   - Update `docs/architecture/features/rstruct.md` to point to the new syntax

2. **Add deprecation notice to `WebstackGenerator`:**

   ```rust
   // 2026-07-26: Deprecated — use LlvmBackend(wasm32) + GlueWebGenerator instead.
   // The TS emitter is kept for migration period. New code should not use this path.
   ```

3. **Update `docs/architecture/backend-strategy.md`** — replace the "Webstack Backend" section with a pointer to the new spec.

4. **Clean up dead code paths** in `WebstackGenerator`:
   - Remove `generate_arm_rust_code()` (dead ARM backend)
   - Remove `statement_to_rust()` / `expr_to_rust()` (dead ARM backend)
   - Remove `CodeTarget::Arm` and `CodeTarget::Fpga` variants
   - Remove unused `ffi_ts_impl`, `ffi_ts_setups`, `wasm_modules` fields

### Tests

- `cargo test --lib` — all tests pass after cleanup
- End-to-end test: compile an `.rbv` with the new pipeline, load on a headless WASM runtime, verify DOM mutations
- Verify `git diff` shows no unintended changes

### Files Changed

| File | Change |
|------|--------|
| `examples/*.rbv` | Migrate to `render struct` syntax |
| `src/backend/webstack.rs` | Add deprecation notice, remove dead code |
| `docs/architecture/backend-strategy.md` | Update webstack section |
| `docs/architecture/features/rstruct.md` | Point to new syntax |

---

## Documentation

### Inline Rationale Comments

Every new/modified code site must carry:

```
// 2026-07-26: Phase N — <short description>
// <what problem it solves, what pattern it targets>
```

Specific sites:

- `parser/definitions.rs`: `// 2026-07-26: Phase 1 — render struct/obj keyword. Attaches view HTML to a struct/obj type for webstack rendering. Replaces old rstruct syntax.`
- `glue/web_generator.rs` header: `// 2026-07-26: Phase 3 — GLUE Web Generator. Produces dom-shim.mjs and .d.ts from view bindings + WASM state layout. Reads WASM linear memory at known offsets — no JS watchers, no Proxy, no polling.`
- `backend/llvm/builder.rs` (`emit_web_flush_state`): `// 2026-07-26: Phase 4 — Webstack DOM flush at term. Emits __web_flush_state import call with pointers to state field values in linear memory. One FFI call per transaction, zero JS in the idle state.`
- `compile.rs` (webstack codegen dispatch): `// 2026-07-26: Phase 4 — Webstack backend uses LlvmBackend(wasm32) + with_webstack(). No more TS emitter. The old WebstackGenerator path is deprecated.`

### Architecture Docs

| Document | Update |
|----------|--------|
| `docs/architecture/features/rendered-briv-wasm.md` | (this spec) — definitive reference |
| `docs/architecture/backend-strategy.md` | Update webstack section, point to new spec |
| `docs/architecture/features/rstruct.md` | Add deprecation notice, point to `render struct` |

---

## Regression Guard Checklist

Before every commit:

1. `cargo test --lib` — all tests pass
2. `cargo build` — no warnings
3. **Inspect every match arm** in modified files — no silent regressions
4. Verify the old `WebstackGenerator` path still compiles (deprecated, not deleted)
5. Check `config/targets.toml` references are consistent
6. Update inline comments
7. Document any gotchas in `BUGS.md`

---

## Future Considerations

### Post-Phase-7

- **SSR (Server-Side Rendering):** The `state_layout` export allows a Node.js runtime to pre-render the DOM on the server by reading initial state and emitting HTML. No browser needed.
- **Hot Module Replacement:** The generation counter in WASM memory enables HMR — the JS shim detects a new WASM module instantiation, re-reads `state_layout`, and patches the binding table without a page reload.
- **Shared Nothing Architecture:** A `.bv` compiled to WASM can be imported as an ES module and called from any JS framework — React, Vue, Svelte — via the `.d.ts` declarations.
