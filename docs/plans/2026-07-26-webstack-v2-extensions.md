# Webstack v2 Extension Plan: Protocols, Stdlib, SSR, HMR

**Date:** 2026-07-26
**Status:** Planning phase — not yet implemented
**Depends on:** `docs/plans/2026-07-26-rendered-brief-webstack-v2.md` (Phases 1-7 complete)

## Overview

Four extensions to the webstack v2 architecture, ordered from quickest to largest:

| # | Item | Est. | Depends on |
|---|------|------|------------|
| 1 | `config/protocols.toml` — `#Web` entry | 30 min | Nothing |
| 2 | `lib/std/web/` — Browser API wrappers | 2 hrs | #1 (optional) |
| 3 | SSR — Native Server-Side Rendering | 4 hrs | #2 (for API wrappers) |
| 4 | HMR — Hot Module Replacement | 8 hrs | #3 (shares instantiation) |

---

## 1. `config/protocols.toml` — Explicit `#Web` Entry

### Problem

`ProtocolConfig::resolve()` at `src/target.rs:79` rejects any protocol that is not `"#System"`:

```rust
if protocol != "#System" {
    return Err(format!("'{}' is not a valid protocol hashword. \
             #System is the only supported protocol", protocol));
}
```

The `frgn_dispatch.rs` handler for `#Web` catches it before this point (added in Phase 6 `from #Web` wiring), so this error path is never reached for `#Web`. But it's a lie in the error message — `#Web` IS a valid protocol now.

### Solution

Add a `wasm32-wasi` entry for `#Web` to `config/protocols.toml`:

```toml
[wasm32-wasi]
"#System" = "wasi_snapshot_preview1"
"#Web" = "wasm_runtime"
```

Update `ProtocolConfig::resolve()` to accept both `#System` and `#Web`:

```rust
pub fn resolve(&self, target_triple: &str, protocol: &str) -> Result<Option<&str>, String> {
    if protocol != "#System" && protocol != "#Web" {
        return Err(format!(
            "'{}' is not a valid protocol hashword. \
             #System and #Web are the supported protocols",
            protocol
        ));
    }
    // ... existing target lookup ...
}
```

The `"wasm_runtime"` library name is a sentinel — the resolver returns it as `Some("wasm_runtime")`, and the GLUE bridge dispatch checks for this to route through the web target. The LLVM linker never sees `-lwasm_runtime` because `frgn_dispatch.rs` converts `Bridge { language: "web" }` before the linker stage.

### Files Changed

| File | Change |
|------|--------|
| `config/protocols.toml` | Add `"#Web" = "wasm_runtime"` under `[wasm32-wasi]` |
| `src/target.rs` | `resolve()` accepts `#Web` alongside `#System` |

### Test

- Unit: `ProtocolConfig::resolve("wasm32-wasi", "#Web")` returns `Ok(Some("wasm_runtime"))`
- Unit: `ProtocolConfig::resolve("x86_64-linux", "#Web")` returns error (not available on non-WASM targets)

---

## 2. `lib/std/web/` — Browser API Wrappers

### Problem

Users currently write raw `frgn from #Web` declarations for common browser APIs:

```brief
frgn performance_now() -> Float from #Web;
frgn console_log(msg: String) from #Web;
```

These are boilerplate. The GLUE `[web]` target generates type-correct stubs, but the Brief-side declarations should live in stdlib so users can `import "web/console.bv"` and get them.

### Solution

Create `lib/std/web/` with `.bv` files wrapping common `frgn from #Web` declarations:

```
lib/std/web/
  console.bv       # log(msg), warn(msg), error(msg)
  time.bv          # now() -> Float  (performance.now)
  dom.bv           # set_text(elem, text)
  canvas.bv        # get_canvas(id) -> CanvasContext, present_frame(ctx)
  fetch.bv         # fetch(url) -> String
```

Each file is 3-5 lines of Brief:

```brief
// lib/std/web/time.bv
frgn performance_now() -> Float as now from #Web;
```

```brief
// lib/std/web/console.bv
frgn console_log(msg: String) as log from #Web;
frgn console_warn(msg: String) as warn from #Web;
frgn console_error(msg: String) as error from #Web;
```

```brief
// lib/std/web/dom.bv
frgn set_text(elem: Element, text: String) from #Web;
frgn get_element_by_id(id: String) -> Element from #Web;
```

```brief
// lib/std/web/canvas.bv
frgn get_canvas(id: String) -> CanvasContext from #Web;
frgn present_frame(ctx: CanvasContext) from #Web;
```

```brief
// lib/std/web/fetch.bv
frgn fetch_url(url: String) -> String as fetch from #Web;
```

### No compiler changes needed

The `#Web` protocol and GLUE `[web]` target already handle these. These `.bv` files are pure stdlib — they follow the "stdlib is the extension mechanism" rule (Golden Rule 13).

---

## 3. SSR — Native Server-Side Rendering

### Problem

The current pipeline emits a static `app.html` with the raw `<view>` template and a `<script>` tag that instantiates WASM on the client. For SEO and first-load performance, the server should pre-render the initial state into the HTML.

### Approach

SSR runs **at compile time**, not on a live server. The compiler has all the information it needs after typecheck + codegen:

1. **State declarations** — `let count: Int = 0;` has initial values at compile time
2. **View bindings** — the view compiler's `Vec<Binding>` maps element IDs to state fields
3. **Initial values** — state declarations carry default initializers

The SSR pass is a **contract-fulfillment proof**: it proves that the initial state satisfies the view's rendering contracts (every `b-text="count"` has a known initial value), and emits the pre-rendered HTML.

### Why Not a Runtime Server

A WASM runtime server would need to:
- Instantiate the WASM module
- Call `state_layout()` to read initial values
- Render the template

This is architecturally clean but adds a runtime dependency (`wasmtime` or the Brief interpreter). The compile-time approach is simpler and zero-cost — if the initial state is fully determined at compile time (no `GetEnvInt#` or `frgn from #Web` in initializers), the SSR is a pure function of the source.

For cases with runtime-determined initial state (e.g., `let count: Int = frgn get_initial_count() from #Web`), SSR falls through to the runtime path — deferred to a future enhancement.

### Implementation

#### SS1: Compile-time SSR pass

New module `src/ssr.rs`:

```rust
pub struct SsrOutput {
    pub pre_rendered_html: String,
    pub boot_script: String,    // JS that bootstraps WASM on the client
}

pub fn render_ssr(
    view_html: &str,
    bindings: &[Binding],
    state_initializers: &HashMap<String, Expr>,
) -> SsrOutput {
    // For each b-text binding, look up the initial value
    // Replace placeholder text in the HTML with the initial value
    // For b-show, evaluate the initial condition (remove or keep)
    // Returns the pre-rendered HTML + boot script
}
```

The output `app.html` is a **fully rendered page** containing:
- The SSR'd HTML with state values baked in
- A `<script>` block that instantiates the WASM module (for client-side interactivity)
- The WASM module takes over from the pre-rendered state (no flash of wrong content)

#### SS2: `--ssr` CLI flag

```bash
briefc build app.rbv --backend webstack --ssr
```

When `--ssr` is set, the compiler runs `render_ssr()` before writing `app.html`. The boot script is a minimal `<script>` that:
1. Reads the SSR'd state from a `<script type="application/json" id="ssr-state">` tag
2. Passes it to the WASM module on instantiation
3. Re-binds DOM elements (the existing `_loadStateLayout` and `_applyFlush`)
4. Client-side interactivity takes over from the pre-rendered state

#### SS3: WASM init with pre-rendered state

The WASM module exports an `init_ssr(state_ptr: i32, state_len: i32)` function. When SSR data is present, the boot script calls this instead of starting from zero. The state values are already in the DOM — WASM just validates them and starts responding to user events.

### Files Changed

| File | Change |
|------|--------|
| `src/ssr.rs` | New module — `SsrOutput`, `render_ssr()` function |
| `src/compile.rs` | Add `--ssr` flag to `BuildOptions`, run SSR pass for webstack |
| `src/backend/llvm/mod.rs` | Add `init_ssr` export for pre-rendered state |
| `docs/architecture/features/rendered-brief-wasm.md` | Update spec with SSR section |

### Test

- Unit: `render_ssr()` with `b-text="count"` and `let count: Int = 5` produces HTML containing `5`
- Unit: `render_ssr()` with `b-show="visible"` and `let visible: Bool = false` produces HTML with `display: none`
- Integration: `.rbv` compiled with `--ssr` produces `app.html` with SSR'd content and boot script

---

## 4. HMR — Hot Module Replacement

### Problem

During development, changing a `.rbv` file currently requires a full rebuild and browser refresh. HMR would hot-swap the WASM module without losing application state.

### Approach

HMR uses the **generation counter** (exported as `generation` global in Phase 4) as a "state version" signal:

1. **Dev server** watches the file system for `.rbv` changes
2. On change, recompiles the WASM module
3. Notifies the browser (via WebSocket or the `dev-shim.js` polling `fetch`)
4. The JS shim:
   a. Reads the current `generation` value from the old WASM module
   b. Fetches the new `.wasm` binary
   c. Creates a new `WasmDomRuntime` instance
   d. Copies the handle table state to the new instance
   e. Re-binds each DOM element by element ID (element IDs don't change)
   f. Transfers the generation counter to the new module
   g. Unloads the old WASM module

### Prerequisites

- SSR (this plan's item 3) — the "state serialization" mechanism is shared with HMR
- A dev server — a small file watcher + HTTP server (can be a Brief tool or a thin shell)

### Implementation

#### HM1: `src/glue/web_generator.rs` — Add HMR methods to `WasmDomRuntime`

```javascript
class WasmDomRuntime {
  async hotReload(wasmBytes) {
    const oldState = this._serializeState();
    const newRuntime = new WasmDomRuntime(wasmBytes);
    newRuntime._handles = this._handles.slice();
    newRuntime._deserializeState(oldState);
    this._instance = newRuntime._instance;
    this._memory = newRuntime._memory;
    this._loadStateLayout();
  }

  _serializeState() {
    const mem = new DataView(this._memory.buffer);
    const state = {};
    for (const binding of this._bindingTable) {
      if (binding) {
        state[binding.handle] = mem.getUint32(binding.offset, true);
      }
    }
    return state;
  }

  _deserializeState(state) {
    const mem = new DataView(this._memory.buffer);
    for (const [handle, val] of Object.entries(state)) {
      mem.setUint32(this._bindingTable[Number(handle)].offset, val, true);
    }
  }
}
```

#### HM2: Dev server

A small tool (`briefc dev`) that:
1. Starts an HTTP server serving the build directory
2. Watches for `.rbv` changes via `inotify` or polling
3. On change: recompiles with `briefc build` and emits a WebSocket message
4. The `dev-shim.mjs` listens for the WebSocket message and calls `hotReload`

#### HM3: Dev shim variant

A separate `dev-shim.mjs` that extends `dom-shim.mjs` with the WebSocket listener.

```html
<script type="module" src="dev-shim.mjs"></script>
```

### Files Changed

| File | Change |
|------|--------|
| `src/glue/web_generator.rs` | Add `_serializeState`, `_deserializeState`, `hotReload` methods to the generated JS template |
| `src/compile.rs` | Add `--dev` flag, emit `dev-shim.mjs` variant |
| `tools/briefc-dev/` | New dev server (file watcher + HTTP + WebSocket) |

### Test

- Manual: edit an `.rbv`, save, verify the browser updates without full reload
- Unit: `_serializeState` / `_deserializeState` roundtrip preserves all state values

---

## Documentation

### Inline Rationale Comments

- `src/target.rs` (resolve fn): `// 2026-07-26: #Web is a recognized protocol for wasm32-wasi targets. Routes through GLUE wasm_runtime bridge. No linker flag needed.`
- `src/ssr.rs` header: `// 2026-07-26: SSR pass. Proves initial state satisfies view contracts at compile time. Emits pre-rendered HTML + boot script. No runtime dependency.`

### Architecture Docs

| Document | Update |
|----------|--------|
| `docs/architecture/features/rendered-brief-wasm.md` | Add SSR section under "Outputs", add HMR section under "Future" |
| `docs/plans/2026-07-26-webstack-v2-extensions.md` | (this plan) — definitive reference |

---

## Regression Guard

1. `cargo test --lib` before every commit
2. `cargo build` — no warnings
3. Check that existing `.rbv` examples still compile without `--ssr` (backward compat)
4. Verify `ProtocolConfig::resolve("#System")` still works for existing targets
5. Check that `lib/std/` imports still work (new files must not break existing stdlib)
6. Verify the `dom-shim.mjs` template still generates valid JavaScript without dev features
