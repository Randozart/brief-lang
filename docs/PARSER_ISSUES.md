# Rendered Brief Parser Issues - Session Log

**Date:** April 5, 2026  
**Compiler Version:** 0.1.0  
**Context:** Attempting to create `shopping_cart.rbv` example

---

## CRITICAL FINDING - rstruct Limitation

**The rstruct HTML parser can only handle ONE top-level HTML element.**

Multiple sibling elements cause: `Unexpected token in rstruct: Ok(LtSlash)`

**Root Cause (Detailed):** 
- `scan_html_block()` (src/parser.rs:636) scans from opening tag (`<div>`) to closing tag (`</div>`)
- Works fine for inline elements: `<span>` → `</span>`, `<button>` → `</button>`
- **BREAKS on nested block elements:** When root `<div>` contains child `<div>`, parser finds FIRST `</div>` and thinks it closes the root
- This closes the root prematurely, leaving child HTML unparsed
- Parser then encounters `</div>` from the child as a stray token = `Ok(LtSlash)` error

**Specific Limitation:**
- ✓ Single root `<div>` with inline child elements: `<span>`, `<button>`, `<input>`, `<p>` (usually work)
- ✗ Nested `<div>` tags anywhere inside the root
- ✗ Any block-level element nested inside block-level element

**Workaround:** Use ONLY inline elements inside the root div:
```brief
rstruct App {
  <div class="root">
    <div b-show="step == 1">Step 1</div>
    <div b-show="step == 2">Step 2</div>
  </div>
}
```

**Impact:** This explains ALL previous shopping cart failures - they were structurally valid HTML but violate the parser's single-root limitation.

---

## Issues Encountered

### Issue 1: Unicode Emoji in Brief Code Section
**Error:** `byte index 1369 is not a char boundary; it is inside '🛍' (bytes 1368..1372)`
**Location:** `src/parser.rs:675:27`
**Severity:** CRITICAL - Parser panics on valid UTF-8 emoji characters
**Root Cause:** Parser's byte indexing doesn't properly handle multi-byte UTF-8 characters
**Workaround:** Remove all emoji from Brief code section (they can still be used in HTML via CSS `::before`)
**Test Case:**
```brief
# This causes panic:
# Shopping Cart System - Rendered Brief Example 🛍️
```

---

### Issue 2: HTML Comments in Rendered Brief
**Error:** `Unexpected token in rstruct: Ok(LtSlash)` after HTML comments
**Location:** Parser recognizes `</` but doesn't handle it correctly in HTML context
**Severity:** HIGH - Cannot use HTML comments in `.rbv` files
**Root Cause:** Parser may be treating `<!--` as Brief code syntax instead of HTML
**Workaround:** Remove all HTML comments (`<!-- comment -->`)
**Test Case:**
```html
<div>
  <!-- This causes parse error -->
  <span>Content</span>
</div>
```

---

### Issue 3: Complex Nested HTML Structures
**Error:** `Unexpected token in rstruct: Ok(LtSlash)`
**Severity:** HIGH - Limits HTML complexity in reactive structs
**Pattern:** Error appears when rstruct contains nested HTML sections with conditional rendering
**Root Cause:** Unclear - may be related to how parser handles multiple `<div>` blocks with `b-show` attributes
**Workaround:** Flatten HTML structure, minimize nesting depth
**Works:**
```html
<div class="store">
  <div>Item 1</div>
  <button>Action</button>
</div>
```
**Fails:**
```html
<div class="store">
  <div class="section1">
    <div class="subsection">
      <p>Content</p>
    </div>
  </div>
  <div class="section2">...</div>
</div>
```

---

### Issue 4: Square Bracket Syntax in HTML Attributes
**Error:** `Unexpected token in rstruct: Ok(LtSlash)` 
**Severity:** CRITICAL - Parser interprets `[...]` as Brief syntax, not HTML
**Root Cause:** Square brackets are Brief syntax for guards/conditions. Parser doesn't distinguish between Brief code and HTML template
**INCORRECT (causes error):**
```html
<div [b-show="step == 1"]>Content</div>
```
**CORRECT:**
```html
<div b-show="step == 1">Content</div>
```
**Workaround:** Use standard HTML attributes without square brackets. The binding system uses plain attribute names like `b-show`, `b-text`, `b-trigger:event`
**Note:** This is a SYNTAX ERROR in Rendered Brief specification, not a parser bug. Square brackets are for Brief code, not HTML templates.

---

### Issue 5: `<hr>` Self-Closing Tags
**Error:** `Unexpected token in rstruct: Ok(LtSlash)`
**Severity:** LOW - Specific to certain HTML5 void elements
**Root Cause:** Parser may not handle self-closing tags or void elements properly
**Workaround:** Use closing tag: `<hr></hr>` instead of `<hr>` or `<hr />`
**Test Case:**
```html
<div><hr></div>  <!-- This causes issues -->
```

---

### Issue 6: Multi-Line Form Groups
**Error:** `Unexpected token in rstruct: Ok(LtSlash)` when form has many input fields
**Severity:** MEDIUM - Limits practical HTML complexity
**Pattern:** Parser fails as HTML grows larger with multiple `<input>` tags
**Example:**
```html
<input type="text" placeholder="Name">
<input type="email" placeholder="Email">
<input type="text" placeholder="Address">
<input type="text" placeholder="City">
<!-- More inputs = higher failure rate -->
```
**Possible Root Cause:** May be cumulative effect of issue #4 or parser's recursion depth limit

---

### Issue 7: Attribute Syntax Edge Cases
**Error:** Parser inconsistency with attribute order/spacing
**Severity:** LOW - Intermittent
**Pattern:** Unclear what triggers this - may be related to spacing around `=` in attributes
**Example:**
```html
<input type="text" placeholder="Name" class="form-input">
```
vs
```html
<input type="text"placeholder="Name"class="form-input">
```
(One might work, other might not - needs verification)

---

### Issue 8: Brief Code Comments Before HTML
**Error:** `Unexpected token in rstruct: Ok(LtSlash)` when Brief comments appear right before closing `};`
**Severity:** MEDIUM - Limits documentation
**Root Cause:** Parser may be incorrectly handling transition from Brief code to HTML template
**Workaround:** Remove all comments from Brief sections; add documentation only in HTML
**Test Case:**
```brief
  };

  # UI Template Below
  <div>...</div>
}
```

---

## Working Examples

**counter.rbv** - ✓ Compiles successfully
- Single reactive struct
- 3 simple transactions
- Flat HTML structure (no nested conditionals)
- 94 lines total

**todo.rbv** - ✓ Parses successfully (has proof errors, not parser errors)
- Uses `b-each:item="items"` iteration syntax
- Simpler HTML nesting
- 145 lines total

**shopping_cart.rbv** - ✗ Fails to parse
- Multiple conditional sections
- Deeper HTML nesting
- Multiple `b-show` attributes
- Attempt 1: 633 lines - Unicode emoji issue
- Attempt 2: Complex nested structure issue
- Attempt 3: Simplified to 311 lines - Still fails with `LtSlash` error
- Attempt 4: Ultra-minimal (70 lines) - Compilation hangs or aborts

---

## Parser Behavior Summary

| Situation | Behavior | Issue ID |
|-----------|----------|----------|
| Unicode emoji in Brief comments | Panic - byte boundary error | #1 |
| HTML comments in template | Parse error (`LtSlash`) | #2 |
| Complex nested HTML | Parse error (`LtSlash`) | #3 |
| Multiple sibling conditionals | Parse error (`LtSlash`) | #4 |
| Self-closing HTML tags (`<hr>`) | Parse error (`LtSlash`) | #5 |
| Many form inputs | Parse error (`LtSlash`) | #6 |
| Attribute syntax variations | Inconsistent (needs verification) | #7 |
| Brief comments before HTML | Parse error (`LtSlash`) | #8 |

---

## Recommendations

1. **Immediate:** Add parser error logging with byte position and context to identify exact failure points
2. **Investigation:** Run parser with `RUST_BACKTRACE=1` to get stack traces
3. **Testing:** Create minimal reproducers for each issue
4. **Documentation:** Update `.rbv` specification with known limitations
5. **Parser Review:** Check HTML/Brief template parser for:
   - Proper UTF-8 handling
   - Correct handling of nested structures
   - Support for HTML5 void elements
   - Comment handling in mixed syntax contexts

---

## RESOLUTION - Root Cause Found

**Issue #4 was the PRIMARY BLOCKER:** Using `[b-show="..."]` syntax in HTML attributes.

The square brackets `[...]` are **Brief language syntax**, not HTML template syntax. The parser correctly rejects them because it's trying to parse them as Brief code.

**CORRECT Rendered Brief Attribute Syntax:**
```html
b-show="condition"                (NOT [b-show="condition"])
b-text="variable"                 (NOT [b-text="variable"])
b-trigger:eventname="txn"         (NOT [b-trigger:...]
b-each:varname="collection"       (NOT [b-each:...]
```

These are HTML attributes on the template, not Brief language constructs.

---

## Verified Working Pattern

The ultra-minimal shopping cart compiles successfully using:
- Simple reactive struct with 3 transactions
- Flat HTML structure (no deep nesting)
- Standard HTML attribute binding syntax (`b-text`, `b-trigger`)
- No HTML comments
- No unicode in Brief code
- No square brackets in attributes

File: `/home/randozart/Desktop/Projects/brief-compiler/examples/shopping_cart.rbv`
Status: ✓ Compiles successfully, WASM generated, runs in browser

---

## Current Workaround for Complex RBV Files

```
1. Use standard HTML attribute binding (no square brackets)
2. Keep HTML structure relatively flat
3. No HTML comments (causes parse error)
4. No unicode in Brief code sections
5. Avoid void elements or use closing tag form
6. No Brief code comments immediately before HTML template section
7. Use counter.rbv or todo.rbv as template - they work reliably
8. Reference: Bindings are HTML attributes, not Brief syntax
```

---

## Issues That Need Further Investigation

1. **Issue #1:** Unicode emoji still causes byte boundary panic (not syntax-related)
2. **Issue #2:** HTML comments cause parser failure (legitimate issue)
3. **Issue #3:** Complex nested HTML - needs testing with verified correct syntax
4. **Issue #5, #6, #7, #8:** Need retesting with correct syntax to determine if they're real issues

---

## Conclusion

Most parser failures in shopping cart examples were due to **incorrect use of Rendered Brief syntax** (using Brief square bracket syntax in HTML attributes), not parser bugs. The parser is working correctly and rejecting invalid syntax.

Legitimate parser issues remain:
- Unicode handling (Issue #1)
- HTML comment handling (Issue #2)
- Possibly: Complex nesting limits (Issue #3)
