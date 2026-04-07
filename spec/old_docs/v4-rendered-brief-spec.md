# Rendered Brief (.rbv) — Specification v1.0

**Version:** 1.0  
**Date:** 2026-04-04  
**Status:** Authoritative Reference  

---

## 1. Overview

Rendered Brief (`.rbv`) is a single-file reactive UI format where Brief logic and HTML/CSS views coexist. Brief is the reactive state engine; HTML and CSS are declarative projections of that state.

### Design Philosophy

- **Brief owns all state.** The view is a passive mirror.
- **No component tree.** Each `.rbv` is a complete page. No props drilling, no context, no hierarchy.
- **No virtual DOM.** Bindings map directly to DOM nodes via pre-computed references.
- **Compile-time proofs extend to the view.** Directive references are validated against Brief state at compile time.

---

## 2. File Format

### 2.1 Structure

An `.rbv` file contains three sections:

```html
<script type="brief">
  // Brief code: imports, state, transactions, definitions
</script>

<view>
  <!-- HTML with Brief directives -->
</view>

<style>
  /* CSS styles (optional) */
</style>
```

### 2.2 Rules

| Rule | Enforcement |
|------|-------------|
| Exactly one `<script type="brief">` block | Compile error if missing or duplicate |
| Exactly one `<view>` block | Compile error if missing or duplicate |
| `<style>` block | Optional, 0 or 1 |
| `<script>` must precede `<view>` | Compile error if reversed |
| `<style>` must follow `<view>` | Optional placement |
| No embedded `<script>` or `<style>` inside `<view>` | Compile error |

### 2.3 Example

```html
<script type="brief">
  let count: Int = 0;
  let message: String = "Hello";

  txn increment [true][count == @count + 1] {
    &count = count + 1;
    term;
  };

  rct txn auto_greet [count == 5][message == "You hit 5!"] {
    &message = "You hit 5!";
    term;
  };
</script>

<view>
  <div class="app">
    <p b-text="message">Default</p>
    <span b-text="count">0</span>
    <button b-trigger="increment">+1</button>
    <p b-show="count >= 3">Almost there!</p>
  </div>
</view>

<style>
  .app { font-family: sans-serif; }
  button { padding: 0.5rem 1rem; }
</style>
```

---

## 3. Brief Section

### 3.1 Allowed Constructs

The `<script type="brief">` block accepts standard Brief syntax:

- `import` statements
- `let` state declarations
- `const` constant declarations
- `sig` signature declarations
- `defn` function definitions
- `txn` transactions
- `rct txn` reactive transactions
- `struct` struct definitions

### 3.2 State as Signals

All `let` declarations at the top level become reactive signals. The compiler generates a signal store from these declarations.

```brief
let username: String = "";
let age: Int = 0;
let active: Bool = true;
let items: List<Int> = [1, 2, 3];
```

### 3.3 Generic Types

Brief supports the `List<T>` generic type for homogeneous collections:

```brief
let numbers: List<Int> = [1, 2, 3, 4, 5];
let names: List<String> = ["Alice", "Bob", "Charlie"];
```

**List Operations:**
- `[1, 2, 3]` - List literal
- `list[0]` - Index access (0-based)
- `list.len()` - Get length

### 3.4 Reactive Transactions

Transactions marked with `rct` self-fire when their preconditions are satisfied:

```brief
rct txn update_display [count > 10][true] {
  // This fires whenever count > 10
  term;
};
```

Transactions without `rct` are passive and must be triggered explicitly via `b-trigger`.

---

## 4. View Section

### 4.1 Directives

Directives are HTML attributes prefixed with `b-`. They bind DOM elements to Brief state.

#### `b-text`

Binds text content to a state variable or expression.

```html
<span b-text="count">0</span>
<p b-text="'Hello, ' + user.name"></p>
```

#### `b-show`

Conditionally displays an element when expression is true.

```html
<div b-show="isLoggedIn">Welcome!</div>
<div b-show="count > 0">Items: <span b-text="count"></span></div>
```

#### `b-hide`

Conditionally hides an element when expression is true.

```html
<div b-hide="isLoading">Content</div>
```

#### `b-trigger`

Binds a DOM event to a transaction invocation.

```html
<button b-trigger="increment">+</button>
<form b-trigger:submit="login">Submit</form>
<input b-trigger:input="onSearch" />
```

**Syntax variants:**
- `b-trigger="txn_name"` — defaults to `click` event
- `b-trigger:event="txn_name"` — specify event type

#### `b-class`

Conditionally applies CSS classes.

```html
<div b-class="isActive: 'active', isDisabled: 'disabled'"></div>
```

**Syntax:** `"stateVar: 'className', stateVar2: 'className2'"`

The class is applied when the state variable is truthy.

#### `b-attr`

Conditionally sets HTML attributes.

```html
<button b-attr="disabled: isSubmitting">Submit</button>
<img b-attr="src: imageUrl, alt: imageAlt" />
```

**Syntax:** `"attrName: stateVar, attrName2: stateVar2"`

If the state value is `null` or `false`, the attribute is removed.

#### `b-each`

Iterates over a List signal, rendering a template for each item.

**Syntax:** `b-each:itemName="listSignal"`

```html
<div b-each:item="items">
    <span b-text="item">Default</span>
</div>
```

The directive:
- `b-each:item` - declares `item` as the iteration variable
- `="items"` - specifies which List signal to iterate over

Each item in the list is rendered using the element's inner HTML as a template.

### 4.2 Directive Expression Grammar

Expressions in directive values are a subset of Brief expressions:

```
expr       ::= bool_expr | string_expr | ident
bool_expr  ::= ident ('&&' | '||' | '!=' | '==' | '<' | '>' | '<=' | '>=') (ident | literal)
string_expr ::= ident | string_literal ('+' string_expr)?
ident      ::= identifier
literal    ::= string_literal | int_literal | float_literal | bool_literal
```

### 4.3 HTML Parsing

HTML is parsed as a fragment (not a full document). The parser:
- Recognizes standard HTML elements
- Extracts `b-*` attributes as directives
- Assigns stable element IDs for binding resolution
- Preserves structure for DOM reconstruction

### 4.4 SVG Support

SVG elements are valid within `<view>`. Directives work on SVG elements the same as HTML elements.

```html
<view>
  <svg>
    <circle b-attr="fill: circleColor" />
    <text b-text="label">Label</text>
  </svg>
</view>
```

---

## 5. Style Section

### 5.1 CSS Location

CSS can be placed inline within `<style>` or handled externally:

```html
<!-- Inline -->
<style>
  .button { background: blue; }
</style>

<!-- External (handled by build tool) -->
<link rel="stylesheet" href="theme.css" />
```

### 5.2 Scoping

CSS is global by default. Build tools may offer scoped CSS via hash-based class names.

---

## 6. Compilation Pipeline

### 6.1 Phases

```
.rbv file
    │
    ▼
┌─────────────────────────────┐
│  Extraction                  │
│  - Split blocks              │
│  - Validate structure        │
└─────────────┬───────────────┘
              │
              ▼
┌─────────────────────────────┐
│  Brief Compilation          │
│  - Parse Brief AST          │
│  - Type check               │
│  - Prove transactions       │
│  - Generate WASM            │
└─────────────┬───────────────┘
              │
              ▼
┌─────────────────────────────┐
│  View Compilation           │
│  - Parse HTML AST           │
│  - Extract directives       │
│  - Validate against signals │
│  - Build binding table      │
└─────────────┬───────────────┘
              │
              ▼
┌─────────────────────────────┐
│  JS Glue Generation         │
│  - Event bridge             │
│  - Signal bridge            │
│  - Hydration                │
└─────────────┬───────────────┘
              │
              ▼
┌─────────────────────────────┐
│  Output                      │
│  - .wasm                    │
│  - .js                      │
│  - .css (if extracted)      │
└─────────────────────────────┘
```

### 6.2 Extraction

```rust
fn extract_blocks(source: &str) -> Result<(BriefSource, ViewSource, Option<CssSource>), Error> {
    // Find <script type="brief">...</script>
    // Find <view>...</view>
    // Find <style>...</style> (optional)
    // Validate order and uniqueness
}
```

### 6.3 Brief Compilation

The Brief section is compiled using the existing Brief compiler infrastructure:
- Lexer
- Parser
- Type checker
- Proof engine (contract verification, reachability)
- WASM code generation

**WASM exports:**
| Export | Signature | Purpose |
|--------|-----------|---------|
| `init` | `() -> void` | Initialize state and scheduler |
| `invoke_txn` | `(name_ptr: i32, name_len: i32) -> i32` | Call a transaction by name |
| `get_signal` | `(id: i32, buf_ptr: i32) -> i32` | Read signal value |
| `set_signal` | `(id: i32, ptr: i32, len: i32) -> i32` | Write signal value |
| `poll_dispatch` | `() -> i32` | Get pending DOM instructions |

### 6.4 View Compilation

```rust
struct ViewCompiler {
    signals: HashMap<String, SignalId>,
    transactions: HashMap<String, TxnId>,
}

struct Binding {
    element_id: ElementId,
    directive: Directive,
    signal_ids: Vec<SignalId>,
}

enum Directive {
    Text { signal_id: SignalId },
    Show { expr_id: ExprId },
    Hide { expr_id: ExprId },
    Trigger { event: String, txn_name: String },
    Class { pairs: Vec<(String, SignalId)> },
    Attr { name: String, signal_id: SignalId },
}
```

### 6.5 JS Glue

The JS glue is generated, not handwritten. It contains zero business logic.

**Structure:**
```javascript
(function() {
    'use strict';
    
    // === CONFIGURATION (generated) ===
    const ELEMENT_MAP = { /* el_id -> query selector */ };
    const SIGNAL_TABLE = { /* name -> id */ };
    const TRANSACTION_TABLE = { /* name -> id */ };
    
    // === Wasm instance ===
    let wasm = null;
    
    // === Init ===
    async function init(wasmUrl) {
        wasm = await WebAssembly.instantiateStreaming(fetch(wasmUrl));
        wasm.instance.exports.init();
        hydrate();
        startPollLoop();
    }
    
    // === Event Bridge ===
    function attachListeners() {
        // For each b-trigger binding:
        //   element.addEventListener(event, () => wasm.invoke_txn(txn_name))
    }
    
    // === Signal Bridge ===
    function startPollLoop() {
        function poll() {
            const instructions = wasm.poll_dispatch();
            if (instructions) {
                applyInstructions(instructions);
            }
            requestAnimationFrame(poll);
        }
        requestAnimationFrame(poll);
    }
    
    // === Apply DOM Updates ===
    function applyInstructions(queue) {
        // For each instruction: apply to DOM
        // show/hide: toggle hidden attribute
        // text: set textContent
        // class: toggle classList
        // attr: setAttribute
    }
    
    window.rbv = { init };
})();
```

---

## 7. Dispatch Protocol

### 7.1 Signal Flow

```
Brief state changes (via txn)
        │
        ▼
WASM marks dirty signals
        │
        ▼
WASM evaluates affected directives
        │
        ▼
WASM queues dispatch instructions
        │
        ▼
JS poll loop (requestAnimationFrame)
        │
        ▼
JS drains queue, applies to DOM
```

### 7.2 Instruction Set

| Op | Fields | DOM Effect |
|----|--------|------------|
| `show` | `{ el: id, visible: bool }` | `el.toggleAttribute('hidden', !visible)` |
| `text` | `{ el: id, value: string }` | `el.textContent = value` |
| `class_add` | `{ el: id, class: name }` | `el.classList.add(name)` |
| `class_remove` | `{ el: id, class: name }` | `el.classList.remove(name)` |
| `attr_set` | `{ el: id, attr: name, value: string }` | `el.setAttribute(name, value)` |
| `attr_remove` | `{ el: id, attr: name }` | `el.removeAttribute(name)` |

---

## 8. Error Codes

### 8.1 Extraction Errors

| Code | Message |
|------|---------|
| RBV001 | Missing `<script type="brief">` block |
| RBV002 | Missing `<view>` block |
| RBV003 | Duplicate `<script type="brief">` block |
| RBV004 | Duplicate `<view>` block |
| RBV005 | `<script>` block must precede `<view>` |

### 8.2 Validation Errors

| Code | Message |
|------|---------|
| RBV010 | Unknown state variable '{name}' in directive |
| RBV011 | Unknown transaction '{name}' in b-trigger |
| RBV012 | Type mismatch: cannot bind {type} to directive |
| RBV013 | b-trigger on non-interactive element |

### 8.3 Brief Errors

All Brief compilation errors pass through with RBV prefix.

---

## 9. Build Output

### 9.1 File Structure

```
dist/
├── component.wasm    # Compiled Brief → WASM
├── component.js     # Generated JS glue
└── component.css    # Extracted CSS (if style block exists)
```

### 9.2 CLI

```bash
# Compile single file
briefc compile component.rbv --out dist/

# Watch mode
briefc watch src/ --out dist/
```

---

## 10. Future Work (Post v1)

- Component composition (shared templates/state)
- Server-side rendering
- Hot module reload
- DevTools integration
- Interrolog integration for multi-page state
- Scoped CSS

---

## Appendix A: Full Example

```html
<script type="brief">
  let username: String = "";
  let password: String = "";
  let error: String? = null;
  let isLoading: Bool = false;
  let attempts: Int = 0;

  txn login [username.len() > 0 && password.len() >= 8][isLoading == false] {
    &isLoading = true;
    &error = null;
    &attempts = attempts + 1;
    term;
  };

  rct txn show_error [error != null][true] {
    term;
  };
</script>

<view>
  <div class="login-form">
    <h1>Sign In</h1>
    
    <div class="field">
      <label>Username</label>
      <input type="text" b-bind="username" b-attr="disabled: isLoading" />
    </div>
    
    <div class="field">
      <label>Password</label>
      <input type="password" b-bind="password" b-attr="disabled: isLoading" />
    </div>
    
    <p b-show="error != null" class="error" b-text="error"></p>
    
    <p b-show="attempts > 3" class="warning">
      Too many attempts. Please wait.
    </p>
    
    <button b-trigger="login" b-hide="isLoading">Sign In</button>
    <button b-show="isLoading" disabled>Loading...</button>
  </div>
</view>

<style>
  .login-form { max-width: 400px; margin: 2rem auto; }
  .field { margin-bottom: 1rem; }
  .error { color: red; }
  .warning { color: orange; }
  input { width: 100%; padding: 0.5rem; }
  button { width: 100%; padding: 0.75rem; }
</style>
```
