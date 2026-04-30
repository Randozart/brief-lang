# Brief Compiler - Parser Bugs Quick Reference

## Quick Diagnosis

### Bug 1: Nested Divs in `.rbv` Files
**Symptom:** `Unexpected token in rstruct: Ok(LtSlash)` error  
**Root Cause:** `scan_html_block()` finds first `</div>` without tracking nesting depth  
**Location:** `src/parser.rs:636-687`

```rust
// BROKEN: Greedy first-match algorithm
while pos < self.source.len() {
    if self.source[pos..].starts_with(&close_tag) {  // Stops at FIRST </div>
        return Ok((self.source[start..pos].to_string(), pos));
    }
    pos += 1;
}
```

**Example that breaks:**
```html
<div>
  <div>child</div>    <!-- Parser stops here at first </div> -->
</div>               <!-- ERROR: stray </div> becomes parser error -->
```

**Fix:** Implement depth tracking counter for open/close tags

---

### Bug 2: Unicode/Emoji in Comments
**Symptom:** Panic - `byte index X is not a char boundary; it is inside 'emoji'`  
**Root Cause:** Mixed byte and character indexing on UTF-8 strings  
**Location:** `src/parser.rs:639, 675, 680`

```rust
// BROKEN: Mixing byte and character iteration
while pos < self.source.len() && self.source.chars().nth(pos) != Some('>') {
    // ↑ chars().nth() treats pos as char count
    pos += 1;  // But pos is actually a byte offset
}

// Later, byte-based slicing fails on multi-byte boundaries
if self.source[pos..].starts_with(&close_tag) {
    // ↑ If pos is in middle of emoji, this panics
}
```

**Example that breaks:**
```rust
# Shopping Cart 🛍️
rstruct App {
  <div>content</div>
}
// panic: byte index 1369 is not a char boundary
```

**Fix:** Use byte-aware iteration with proper UTF-8 boundary checking

---

### Bug 3: WASM Method Names Mismatch
**Symptom:** Runtime error - `wasm.invoke_increment is not a function`  
**Root Cause:** Method name transformation mismatch between Rust and JavaScript  
**Location:** `src/wasm_gen.rs:337-390` (Rust gen), `src/wasm_gen.rs:779` (JS gen)

```rust
// BROKEN: Name transformation inconsistency
// Line 338-341 (Rust generation):
let method_name = format!(
    "pub fn invoke_{}(&mut self) {{",
    txn.name.replace(".", "_")  // "Counter.increment" → "invoke_Counter_increment"
);

// Line 779 (JavaScript generation):
output.push_str("wasm[`invoke_${config.txn}`]();");
// config.txn might be just "increment" (incomplete name) → "invoke_increment" (doesn't exist!)
```

**Example that breaks:**
```brief
txn Counter.increment [true][@count + 1 == count] {
  &count = count + 1;
  term;
}
```

Generates Rust method: `invoke_Counter_increment`  
But JS tries to call: `invoke_increment` ← ERROR!

**Fix:** Ensure method name transformation is consistent in both Rust and JavaScript generation

---

## Quick Fix Checklist

### Bug 1 Fix (Nested HTML)
- [ ] Add `depth` counter to `scan_html_block()`
- [ ] Increment depth on `<tagname` (opening tag)
- [ ] Decrement depth on `</tagname>` (closing tag)
- [ ] Return only when `depth == 0`
- [ ] Handle self-closing tags (`<br>`, `<hr>`)
- [ ] Test with nested divs at multiple levels

### Bug 2 Fix (Unicode)
- [ ] Use `as_bytes()` for byte-level operations
- [ ] Implement UTF-8 boundary checking helper
- [ ] Use `char.len_utf8()` when incrementing position
- [ ] Alternative: Use character iterator instead of byte iteration
- [ ] Test with emoji, Chinese chars, combining marks

### Bug 3 Fix (WASM)
- [ ] Add transaction name mapping in JS glue code
- [ ] Transform `"Counter.increment"` → `"invoke_Counter_increment"` in binding map
- [ ] Update `attachListeners()` to use correct method name
- [ ] Add validation that binding transaction names exist
- [ ] Test with struct.method transaction names

---

## Bug Severity & Impact

| Bug | Severity | Impact | Users Blocked | Fix Time |
|-----|----------|--------|----------------|----------|
| Nested HTML | HIGH | Can't use nested divs, breaks 80% of real UIs | Most | 1-2 hrs |
| Unicode | MEDIUM | Emoji in comments causes panic | Some | 30 min |
| WASM Names | CRITICAL | Runtime errors on click handlers | All | 15 min |

---

## Testing Before/After

### Before Fix: These all fail

```brief
# Test 1: Nested divs
rstruct Container {
  <div class="outer">
    <div class="inner">content</div>
  </div>
}
# ERROR: Unexpected token in rstruct: Ok(LtSlash)

# Test 2: Emoji in comment
# Shop UI with 🛍️ icon
rstruct Shop {
  <div>Shopping Cart</div>
}
# ERROR: byte index not a char boundary

# Test 3: Transaction with dot
txn Counter.increment [true][@count + 1 == count] {
  &count = count + 1;
  term;
}
<button b-trigger:click="increment">+</button>
# Runtime Error: invoke_increment is not a function
```

### After Fix: All work

```brief
# Test 1: Nested divs
rstruct Container {
  <div class="outer">
    <div class="inner">content</div>
  </div>
}
# ✓ Compiles successfully

# Test 2: Emoji in comment
# Shop UI with 🛍️ icon
rstruct Shop {
  <div>Shopping Cart</div>
}
# ✓ Parses correctly

# Test 3: Transaction with dot
txn Counter.increment [true][@count + 1 == count] {
  &count = count + 1;
  term;
}
<button b-trigger:click="increment">+</button>
# ✓ Click handler calls correct method
```

---

## Files to Modify

1. **Bug 1 + 2:** `/src/parser.rs`
   - Function: `scan_html_block()` (lines 636-687)
   - Add: UTF-8 aware iteration
   - Add: Depth tracking

2. **Bug 3:** `/src/wasm_gen.rs`
   - Function: `generate_js_glue()` (lines 705-845)
   - Location: Transaction mapping (around line 728-779)
   - Add: Method name transformation map
   - Modify: Event listener attachment logic

---

## Git Commit Messages (After Fix)

```
fix: implement depth tracking for nested HTML elements in rstruct

- Modify scan_html_block() to properly count opening/closing tags
- Fix issue where nested <div> tags cause parser to stop at first </div>
- Add support for self-closing tags (<br>, <hr>, <img>)
- Fixes: Unexpected token in rstruct: Ok(LtSlash) errors

fix: use UTF-8 aware indexing in HTML parser

- Replace mixed byte/character indexing with consistent UTF-8 byte handling
- Add is_char_boundary() helper for safe string slicing
- Fix panic on emoji and multi-byte UTF-8 characters
- Fixes: byte index not a char boundary errors

fix: ensure consistent WASM method name transformation in JS glue code

- Add transaction method name mapping in generated JavaScript
- Transform struct.method names to invoke_Struct_method format
- Validate binding transaction names exist at generation time
- Fixes: method not found errors in button click handlers
```

