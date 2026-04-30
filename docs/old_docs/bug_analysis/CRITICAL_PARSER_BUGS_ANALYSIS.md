# Brief Compiler - Critical Parser Bugs Technical Analysis

## Bug 1: Nested Block Elements Bug in `scan_html_block()` (Line 636)

### Root Cause Analysis

**Location:** `src/parser.rs:636-687` (`scan_html_block()` function)

The HTML scanning algorithm uses a **naive linear search** to find closing tags without accounting for nesting depth. The exact issue:

```rust
// Line 675: Simple string search without nesting awareness
while pos < self.source.len() {
    if self.source[pos..].starts_with(&close_tag) {
        pos += close_tag.len();
        end_pos = pos;
        return Ok((self.source[start..pos].to_string(), end_pos));
    }
    pos += 1;  // LINE 680: Increments by 1 (byte-wise for ASCII, breaks on UTF-8)
}
```

**The Algorithm's Failure Mode:**

1. Parser encounters opening tag: `<div>` (root element)
2. Extracts tag name: `div`
3. Searches for closing tag: `</div>` by naive string matching
4. **PROBLEM:** If HTML contains nested `<div>` tags, it finds the **first** `</div>` and closes prematurely

**Example showing failure:**

```html
<!-- Input: -->
<div class="root">          <!-- Line 671: close_tag = "</div>" -->
  <div class="child">       <!-- Content between opening and FIRST closing -->
    Inner content
  </div>                     <!-- <-- Parser stops HERE (first </div> found) -->
</div>                       <!-- <-- Still unparsed, becomes "stray token" -->

<!-- What parser extracts -->
<div class="root">
  <div class="child">
    Inner content
  </div>

<!-- Remaining unparsed: -->
</div>
```

The unparsed `</div>` token is then encountered as `Ok(LtSlash)` error in the parser's token stream.

### Why Current Approach Fails

1. **No Nesting Counter:** The algorithm doesn't track opening/closing tag counts
2. **Greedy First Match:** Uses `starts_with()` which always returns the first occurrence
3. **No HTML Semantics:** Doesn't understand that nested tags need proper pairing

### How to Fix It

**Solution: Implement a depth-tracking scanning algorithm**

Replace lines 673-687 with:

```rust
fn scan_html_block(&mut self, start: usize) -> Result<(String, usize), String> {
    let mut pos = start;
    
    // Skip to end of opening tag
    while pos < self.source.len() && self.source.chars().nth(pos) != Some('>') {
        pos += 1;
    }
    
    if pos >= self.source.len() {
        return Err("Unclosed HTML tag in rstruct (no closing >)".to_string());
    }
    
    pos += 1;
    let tag_content = &self.source[start..pos];
    
    // Extract tag name
    let mut tag_name = String::new();
    let after_lt = if tag_content.starts_with("<") {
        &tag_content[1..]
    } else {
        tag_content
    };
    
    if !after_lt.starts_with('/') && !after_lt.starts_with('!') {
        for c in after_lt.chars() {
            if c.is_alphanumeric() || c == '-' {
                tag_name.push(c);
            } else {
                break;
            }
        }
    }
    
    if tag_name.is_empty() {
        return Err("Could not parse HTML tag in rstruct (no tag name)".to_string());
    }
    
    // FIXED: Track nesting depth
    let close_tag = format!("</{}>", tag_name);
    let open_tag_pattern = format!("<{}", tag_name);  // Match <tagname or <tagname>
    let mut depth = 1;
    
    while pos < self.source.len() && depth > 0 {
        // Look for either opening or closing tag
        let remaining = &self.source[pos..];
        
        // Find the next < character
        if let Some(bracket_pos) = remaining.find('<') {
            pos += bracket_pos;
            
            // Check if it's our closing tag
            if remaining[bracket_pos..].starts_with(&close_tag) {
                depth -= 1;
                if depth == 0 {
                    pos += close_tag.len();
                    return Ok((self.source[start..pos].to_string(), pos));
                }
                pos += close_tag.len();
            } 
            // Check if it's our opening tag (but not self-closing or comment)
            else if remaining[bracket_pos..].starts_with(&open_tag_pattern) {
                // Verify it's an opening tag (not </tagname> or <!--)
                let tag_suffix = &remaining[bracket_pos + 1..];
                if !tag_suffix.starts_with('/') && !tag_suffix.starts_with('!') {
                    // Check if tag name boundary exists (whitespace, >, or self-closing /)
                    let after_name = &remaining[bracket_pos + open_tag_pattern.len()..];
                    if after_name.starts_with(|c: char| c.is_whitespace() || c == '>' || c == '/') {
                        // Check if it's self-closing (ends with />)
                        if !remaining[bracket_pos..].contains("/>") 
                            || remaining[bracket_pos..].find('>').unwrap() 
                                < remaining[bracket_pos..].find("/>").unwrap_or(usize::MAX) {
                            depth += 1;
                        }
                    }
                }
                pos += 1;  // Move past the <
            } else {
                pos += 1;
            }
        } else {
            // No more < found, unclosed tag
            return Err(format!(
                "Unclosed HTML tag in rstruct (missing </{}>)",
                tag_name
            ));
        }
    }
    
    Ok((self.source[start..pos].to_string(), pos))
}
```

### Edge Cases and Dependencies to Watch For

1. **Self-closing tags:** `<br>`, `<hr>`, `<img>` should not increment depth
2. **HTML5 void elements:** Don't have closing tags (handle them specially)
3. **HTML Comments:** `<!-- comment -->` should not affect depth tracking
4. **CDATA sections:** `<![CDATA[...]]>` might contain fake tags
5. **Text with angle brackets:** String content like `"value < 5"` can confuse the parser
6. **Multi-byte UTF-8:** Be careful with byte indexing (see Bug 2)

### Testing Recommendations

Add these test cases:

```brief
# Should work:
rstruct Test1 {
  <div>
    <div>nested</div>
  </div>
}

# Should work:
rstruct Test2 {
  <div>
    <div>level 1</div>
    <div>level 1 again</div>
  </div>
}

# Should work:
rstruct Test3 {
  <div>
    <div>
      <div>deeply nested</div>
    </div>
  </div>
}

# Should work:
rstruct Test4 {
  <div>
    <hr>
    <br>
    content
  </div>
}
```

---

## Bug 2: Unicode Emoji Byte Indexing Bug (Line 675)

### Root Cause Analysis

**Location:** `src/parser.rs:639, 675, 680` - Mixed byte and character indexing

The parser uses `chars().nth(pos)` and byte-based slicing inconsistently:

```rust
// Line 639: Character-based indexing
while pos < self.source.len() && self.source.chars().nth(pos) != Some('>') {
    pos += 1;  // Increments position, but assumes 1 byte = 1 char
}

// Line 675: Byte-based slicing
if self.source[pos..].starts_with(&close_tag) {
    // Assumes pos is a valid byte boundary
}
```

**The UTF-8 Problem:**

Emojis and many Unicode characters are multi-byte in UTF-8:

```
Character: '🛍' (shopping bag emoji)
UTF-8 bytes: [0xF0, 0x9F, 0x9B, 0x8D] (4 bytes)

Rust string indexing works on BYTES, not characters:
"count: 1369 is not a char boundary; it is inside '🛍' (bytes 1368..1372)"
                                                      ↓
This means byte position 1369 falls in the middle of the emoji
```

**The Failure Scenario:**

1. Source contains emoji: `# Shopping Cart 🛍️`
2. Parser initializes `pos = start` (some byte position)
3. Line 639 uses `chars().nth(pos)` assuming `pos` is a character count, but it's a byte offset
4. Loop increments `pos` by 1 byte at a time
5. For ASCII before emoji, this works (1 byte per character)
6. When crossing emoji boundary, `pos` lands on multi-byte character middle
7. Line 675: `self.source[pos..]` panics - slicing at invalid byte boundary

**Example:**

```
Source: "// Comment with emoji 🛍\n<div>..."
Byte positions:
0     10    20   24      28-31   32
"// Comment with emoji 🛍\n"
                        ^emoji is bytes 24-27 (4 bytes)

If pos lands at byte 26 (middle of emoji), then:
self.source[26..] -> ERROR: "not a char boundary"
```

### Why Current Approach Fails

1. **Mixing Models:** Uses `chars().nth()` (character indexing) with byte-based slicing
2. **Assumption Violation:** Assumes each character is 1 byte (ASCII-centric)
3. **UTF-8 Ignorance:** Doesn't respect UTF-8 multi-byte boundaries
4. **No Validation:** Doesn't check if byte position is a valid UTF-8 boundary

### How to Fix It

**Solution: Use UTF-8 aware indexing throughout**

Replace lines 636-687 with:

```rust
fn scan_html_block(&mut self, start: usize) -> Result<(String, usize), String> {
    let source_bytes = self.source.as_bytes();
    let mut pos = start;

    // Skip to end of opening tag - use byte indexing properly
    while pos < source_bytes.len() && source_bytes[pos] != b'>' {
        pos += 1;
    }

    if pos >= source_bytes.len() {
        return Err("Unclosed HTML tag in rstruct (no closing >)".to_string());
    }

    pos += 1; // Include the '>'

    let tag_content = &self.source[start..pos];

    // Extract tag name
    let mut tag_name = String::new();
    let after_lt = if tag_content.starts_with("<") {
        &tag_content[1..]
    } else {
        tag_content
    };

    if !after_lt.starts_with('/') && !after_lt.starts_with('!') {
        for c in after_lt.chars() {
            if c.is_alphanumeric() || c == '-' {
                tag_name.push(c);
            } else {
                break;
            }
        }
    }

    if tag_name.is_empty() {
        return Err("Could not parse HTML tag in rstruct (no tag name)".to_string());
    }

    let close_tag = format!("</{}>", tag_name);
    let close_tag_bytes = close_tag.as_bytes();

    while pos < source_bytes.len() {
        // Use byte-level search, then validate it's at a character boundary
        if source_bytes[pos..].starts_with(close_tag_bytes) {
            // Verify we're at a valid UTF-8 boundary before using byte slicing
            if self.is_char_boundary(pos) && self.is_char_boundary(pos + close_tag_bytes.len()) {
                pos += close_tag_bytes.len();
                return Ok((self.source[start..pos].to_string(), pos));
            }
        }
        pos += 1;
    }

    Err(format!(
        "Unclosed HTML tag in rstruct (missing </{}>)",
        tag_name
    ))
}

// Helper function to check if position is a valid UTF-8 boundary
fn is_char_boundary(&self, pos: usize) -> bool {
    if pos > self.source.len() {
        return false;
    }
    if pos == 0 || pos == self.source.len() {
        return true;
    }
    // In Rust, a char boundary is where the high bits aren't 0b10xxxxxx
    // The byte at position `pos` should have leading bits != 0b10
    let byte = self.source.as_bytes()[pos];
    (byte & 0b11000000) != 0b10000000
}
```

Or, **better solution: Use character iteration instead of byte iteration**

```rust
fn scan_html_block(&mut self, start: usize) -> Result<(String, usize), String> {
    // Convert to chars iterator for proper UTF-8 handling
    let chars: Vec<char> = self.source.chars().collect();
    let source_bytes = self.source.as_bytes();
    let mut byte_pos = start;
    let mut char_pos = 0;

    // Find starting position in chars array
    for (i, c) in self.source[start..].chars().enumerate() {
        if byte_pos >= start {
            char_pos = i;
            break;
        }
        byte_pos += c.len_utf8();
    }

    // Scan to end of opening tag
    while byte_pos < source_bytes.len() && source_bytes[byte_pos] != b'>' {
        let ch = chars.get(char_pos).unwrap_or(&' ');
        byte_pos += ch.len_utf8();
        char_pos += 1;
    }

    if byte_pos >= source_bytes.len() {
        return Err("Unclosed HTML tag in rstruct (no closing >)".to_string());
    }

    byte_pos += 1;

    let tag_content = &self.source[start..byte_pos];
    
    // ... rest of logic using byte_pos and char_pos together
}
```

### Edge Cases and Dependencies to Watch For

1. **Emoji with ZWJ (Zero Width Joiner):** `👨‍👩‍👧‍👦` is multiple codepoints
2. **Combining Characters:** Accent marks (é = e + combining acute accent)
3. **RTL (Right-to-Left) text:** Arabic, Hebrew characters
4. **Control Characters:** Special bytes that shouldn't appear in HTML tags
5. **Different UTF-8 lengths:** Characters can be 1-4 bytes
6. **Byte boundary validation:** Must check before any byte-based slicing

### Testing Recommendations

Add these test cases:

```brief
# Test 1: ASCII (should work)
rstruct Test1 {
  <div>content</div>
}

# Test 2: Unicode emoji (currently fails)
# Shopping Cart System with 🛍️
rstruct Test2 {
  <div>🛍️ Items</div>
}

# Test 3: Chinese characters (currently fails)
# 购物车系统
rstruct Test3 {
  <div>购物车：<span>count</span></div>
}

# Test 4: Mixed content (currently fails)
rstruct Test4 {
  <div>🛍️ Shop 购物 €1.99</div>
}
```

---

## Bug 3: WASM Glue Code Method Name Bug (Lines 337-390, 779)

### Root Cause Analysis

**Location:** 
- Transaction method generation: `src/wasm_gen.rs:337-390` (`generate_transaction()`)
- JavaScript glue code: `src/wasm_gen.rs:779` (method invocation)
- Exported names: `src/wasm_gen.rs:254-289` (signal getters/setters)

The bug involves a **mismatch between generated method names and JavaScript invocation patterns**.

**The Issue:**

```rust
// Line 338-341: Generates method name with dot replacement
let method_name = format!(
    "    pub fn invoke_{}(&mut self) {{\n",
    txn.name.replace(".", "_")  // "Counter.increment" -> "invoke_Counter_increment"
);

// But line 779 in JS glue code tries to call:
output.push_str("                wasm[`invoke_${config.txn}`]();\n");
// Where config.txn comes from binding, which might have format issues
```

**The Failure Scenario:**

Given `counter.rbv` with transaction `Counter.increment`:

1. Rust code generates: `pub fn invoke_Counter_increment(&mut self) { ... }`
2. wasm-bindgen exports this as: `wasm_bindgen::invoke_Counter_increment`
3. JavaScript receives: `config.txn = "increment"` (only the part after the dot)
4. JS glue code tries: `wasm["invoke_increment"]()`  ← WRONG, doesn't exist!
5. Runtime error: `TypeError: wasm.invoke_increment is not a function`

### Why Current Approach Fails

Looking at the bindings processing in line 728-737:

```rust
for binding in bindings {
    if let Directive::Trigger { event, txn } = &binding.directive {
        output.push_str(&format!(
            "        '{}': {{ event: '{}', txn: '{}' }},\n",
            binding.element_id, event, txn  // txn is just the method name
        ));
    }
}
```

**The Problem Chain:**

1. **In Rust generation (line 338):**
   - Full method name: `invoke_Counter_increment`
   - But gets exported as-is by wasm-bindgen

2. **In JavaScript generation (line 779):**
   - Tries to call with incomplete name: `invoke_increment`
   - Name doesn't match what was exported

3. **In bindings (lines 730-734):**
   - Transaction name `txn` is passed as-is from the binding
   - Binding only contains the short name or incomplete name
   - No validation that the name exists in the exported WASM interface

### How to Fix It

**Solution 1: Properly propagate full qualified names through bindings**

In `src/wasm_gen.rs`, line 337-342, modify to store both full and short names:

```rust
fn generate_transaction(&self, output: &mut String, txn: &crate::ast::Transaction) {
    let full_method_name = format!("invoke_{}", txn.name.replace(".", "_"));
    let method_signature = format!("    pub fn {}(&mut self) {{\n", full_method_name);
    
    output.push_str(&method_signature);
    // ... rest of function
    
    // Also store this in the struct or return it for JS generation
}
```

Then in `generate_js_glue()` (around line 779), use the full qualified name:

```rust
output.push_str("            el.addEventListener(config.event, () => {\n");
output.push_str("                if (typeof wasm[config.txn] === 'function') {\n");
output.push_str("                    wasm[config.txn]();\n");
output.push_str("                } else {\n");
output.push_str("                    console.error(`Method ${config.txn} not found on WASM State`);\n");
output.push_str("                }\n");
output.push_str("            });\n");
```

**Solution 2: Create a transaction map in JS glue code**

Before line 774, add transaction method mapping:

```rust
output.push_str("    // Transaction method mapping\n");
output.push_str("    const TRANSACTION_METHODS = {\n");

for (txn_name, _) in &self.txn_map {
    let escaped_name = txn_name.replace(".", "_");
    output.push_str(&format!(
        "        '{}': 'invoke_{}',\n",
        txn_name, escaped_name
    ));
}

output.push_str("    };\n\n");

// Then use it:
output.push_str("            el.addEventListener(config.event, () => {\n");
output.push_str("                const methodName = TRANSACTION_METHODS[config.txn] || config.txn;\n");
output.push_str("                wasm[methodName]();\n");
output.push_str("            });\n");
```

**Solution 3: Validate at binding collection time** (Most Robust)

Modify binding validation to ensure referenced transactions exist:

```rust
fn validate_transaction_bindings(
    bindings: &[Binding],
    txn_map: &HashMap<String, usize>,
) -> Result<(), String> {
    for binding in bindings {
        if let Directive::Trigger { txn, .. } = &binding.directive {
            if !txn_map.contains_key(txn) {
                return Err(format!(
                    "Transaction '{}' referenced in binding '{}' not found in program",
                    txn, binding.element_id
                ));
            }
        }
    }
    Ok(())
}
```

### The Actual Fix (Recommended Approach)

Here's the complete fix for `src/wasm_gen.rs`:

**1. Modify `generate()` method to collect transaction names:**

```rust
pub fn generate(
    &mut self,
    program: &Program,
    bindings: &[Binding],
    program_name: &str,
) -> WasmOutput {
    self.collect_signals_and_transactions(program);

    // NEW: Validate bindings reference valid transactions
    for binding in bindings {
        if let Directive::Trigger { txn, .. } = &binding.directive {
            if !self.txn_map.contains_key(txn) {
                eprintln!("Warning: Transaction '{}' not found in program", txn);
            }
        }
    }

    let rust_code = self.generate_rust_code(program, bindings);
    let js_glue = self.generate_js_glue(program_name, bindings);

    WasmOutput {
        rust_code,
        js_glue,
        signal_count: self.signal_counter,
        txn_count: self.txn_counter,
    }
}
```

**2. Modify JavaScript glue code generation to include transaction mapping:**

Around line 728, replace the trigger map generation:

```rust
output.push_str("    const TRIGGER_MAP = {\n");
for binding in bindings {
    if let Directive::Trigger { event, txn } = &binding.directive {
        // Map transaction name to actual exported WASM method name
        let wasm_method = format!("invoke_{}", txn.replace(".", "_"));
        output.push_str(&format!(
            "        '{}': {{ event: '{}', txn: '{}', method: '{}' }},\n",
            binding.element_id, event, txn, wasm_method
        ));
    }
}
output.push_str("    };\n\n");

// Then update the event listener code (around line 778-780):
output.push_str("    function attachListeners() {\n");
output.push_str("        for (const [elId, config] of Object.entries(TRIGGER_MAP)) {\n");
output.push_str("            const el = document.querySelector(ELEMENT_MAP[elId]);\n");
output.push_str("            if (!el) continue;\n");
output.push_str("            el.addEventListener(config.event, () => {\n");
output.push_str("                if (typeof wasm[config.method] === 'function') {\n");
output.push_str("                    wasm[config.method]();\n");
output.push_str("                } else {\n");
output.push_str("                    console.warn(`WASM method ${config.method} not found on State class`);\n");
output.push_str("                }\n");
output.push_str("            });\n");
output.push_str("        }\n");
output.push_str("    }\n\n");
```

### Edge Cases and Dependencies to Watch For

1. **Struct transactions:** `Counter.increment` vs `increment` - need full qualification
2. **Transaction name conflicts:** Same method name in different structs
3. **Special characters in names:** Already handled by `.replace(".", "_")`
4. **Method visibility:** All generated methods are `pub`, so no visibility issues
5. **wasm-bindgen export format:** Must match exactly what wasm-bindgen exports
6. **JavaScript dynamic property access:** `wasm[methodName]` assumes method exists
7. **Export order:** No dependency on export order, just needs name consistency

### Testing Recommendations

Add these test cases in `wasm_gen.rs`:

```rust
#[test]
fn test_struct_transaction_method_names() {
    let mut gen = WasmGenerator::new();
    
    let txn1 = Transaction {
        name: "Counter.increment".to_string(),
        // ... other fields
    };
    
    let txn2 = Transaction {
        name: "reset".to_string(),
        // ... other fields
    };
    
    // Expected exports:
    // invoke_Counter_increment
    // invoke_reset
}

#[test]
fn test_javascript_glue_code_has_correct_method_names() {
    // Generate glue code and verify it contains correct method mappings
    // E.g., should not have "invoke_increment", should have "invoke_Counter_increment"
}

#[test]
fn test_binding_transaction_validation() {
    // Verify binding references valid transaction names
    // Should warn if binding references non-existent transaction
}
```

---

## Summary Table: Bug Fixes Required

| Bug | Issue | Fix Complexity | Priority | Impact |
|-----|-------|-----------------|----------|--------|
| Nested HTML | Parser finds first `</div>` not matching opening | Medium | HIGH | Breaks reactive structs with nested divs |
| Unicode Emoji | Byte indexing panics on multi-byte UTF-8 | Low | MEDIUM | Blocks Unicode in comments/docs |
| WASM Method Names | Method name mismatch between Rust and JS | Low | CRITICAL | Runtime errors in all structs with methods |

---

## Recommended Implementation Order

1. **Bug 3 (WASM)** - Critical, low complexity, quick win
2. **Bug 2 (Unicode)** - Medium priority, low complexity
3. **Bug 1 (Nested HTML)** - High priority, medium complexity, most impactful

---

## Testing Strategy

Create comprehensive test suite:

```rust
#[cfg(test)]
mod parser_html_tests {
    use crate::parser::Parser;

    #[test]
    fn test_nested_div_single_level() { /* ... */ }
    
    #[test]
    fn test_nested_div_multiple_levels() { /* ... */ }
    
    #[test]
    fn test_unicode_emoji_in_html() { /* ... */ }
    
    #[test]
    fn test_unicode_emoji_in_comment() { /* ... */ }
}

#[cfg(test)]
mod wasm_gen_tests {
    use crate::wasm_gen::WasmGenerator;
    
    #[test]
    fn test_transaction_method_name_generation() { /* ... */ }
    
    #[test]
    fn test_js_glue_code_method_mapping() { /* ... */ }
}
```

