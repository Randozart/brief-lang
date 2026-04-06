# Brief Compiler Parser Bugs - Visual Flowcharts

## Bug 1: Nested Block Elements - Algorithm Comparison

### BROKEN Current Implementation

```
Input: <div> <div>child</div> </div>

Parser State:
  pos = 0, start = 0
  tag_name = "div"
  close_tag = "</div>"

Scanning Loop:
  pos=0:  "< d i v >" ← skip to opening >
  pos=5:  " < d i v > c h i l d <"
  pos=25: " / d i v >" ← FOUND FIRST </div> !!!
  
  Return: <div> <div>child</div>   ✗ INCOMPLETE!
  
  Remaining: </div>  ← Parser encounters this as ERROR
  
Error: Unexpected token in rstruct: Ok(LtSlash)
```

### FIXED Implementation with Depth Tracking

```
Input: <div> <div>child</div> </div>

Parser State:
  pos = 0, start = 0
  tag_name = "div"
  close_tag = "</div>"
  open_tag = "<div"
  depth = 1

Scanning Loop:
  pos=5:  Found "<" → Check for opening/closing tags
         Match: "<div>" (opening tag) → depth++  [depth = 2]
  
  pos=25: Found "<" → Check for opening/closing tags
         Match: "</div>" (closing tag) → depth--  [depth = 1]
  
  pos=31: Found "<" → Check for opening/closing tags
         Match: "</div>" (closing tag) → depth--  [depth = 0] ← DONE!
  
  Return: <div> <div>child</div> </div>  ✓ COMPLETE!

No errors! Parser continues correctly.
```

---

## Bug 2: Unicode/Emoji - Character Boundary Issues

### BROKEN Current Implementation

```
Source: "// Comment emoji 🛍\n<div>..."
        0123456789...24..27..28

UTF-8 Encoding of 🛍:
  Emoji bytes: [F0 9F 9B 8D]  (4 bytes)
  Byte positions: 24, 25, 26, 27

Current Code (Line 639):
  while pos < len && self.source.chars().nth(pos) != Some('>') {
       ↑ chars().nth(pos) treats pos as CHAR COUNT
       pos += 1;  ← But pos is BYTE offset!
  }

Byte Loop:
  pos=0: 'b' (1 byte) ✓ ASCII
  pos=1: 'y' (1 byte) ✓ ASCII
  ...
  pos=23: ' ' (1 byte) ✓ ASCII
  pos=24: 🛍 byte 0 [F0] (START of emoji)
  pos=25: 🛍 byte 1 [9F] (MIDDLE - NOT A BOUNDARY!) ✗
  pos=26: 🛍 byte 2 [9B] (MIDDLE - NOT A BOUNDARY!) ✗
  pos=27: 🛍 byte 3 [8D] (MIDDLE - NOT A BOUNDARY!) ✗

When trying to slice at pos=26:
  self.source[26..]  ← This is in MIDDLE of emoji!
  
  Rust panics: "byte index 26 is not a char boundary"
             "it is inside '🛍' (bytes 24..28)"
```

### FIXED Implementation with UTF-8 Awareness

```
Source: "// Comment emoji 🛍\n<div>..."
        0123456789...24..27..28

UTF-8 Character Iteration:
  Emoji length: char.len_utf8() = 4 bytes

Fixed Code:
  let mut byte_pos = start;
  for ch in self.source[start..].chars() {
    if ch == '>' { break; }
    byte_pos += ch.len_utf8();  ← Properly advance by char width
  }

Byte Loop (Fixed):
  pos=0:  'b' (1 byte) → byte_pos += 1 = 1
  pos=1:  'y' (1 byte) → byte_pos += 1 = 2
  ...
  pos=23: ' ' (1 byte) → byte_pos += 1 = 24
  pos=24: '🛍' (4 bytes) → byte_pos += 4 = 28  ✓ SKIPS ENTIRE EMOJI!
  pos=28: '\n' (1 byte) → byte_pos += 1 = 29
  ...

When slicing at pos=28:
  self.source[28..]  ← This is RIGHT AFTER emoji (valid boundary)
  
  ✓ No panic! Slicing is safe.
```

### UTF-8 Boundary Detection Helper

```
Valid UTF-8 Byte Boundaries:
  - Position 0 (start)
  - Position len (end)
  - Position where byte has high bits != 0b10xxxxxx

Byte Patterns:
  0b0xxxxxxx (0x00-0x7F) - ASCII, always boundary
  0b10xxxxxx (0x80-0xBF) - CONTINUATION byte, never boundary
  0b11xxxxxx (0xC0-0xFF) - START of multi-byte, always boundary

Example with emoji 🛍 [F0 9F 9B 8D]:
  Byte 0: F0 = 11110000 → Boundary ✓ (start of 4-byte sequence)
  Byte 1: 9F = 10011111 → NOT boundary ✗ (continuation)
  Byte 2: 9B = 10011011 → NOT boundary ✗ (continuation)
  Byte 3: 8D = 10001101 → NOT boundary ✗ (continuation)
  Byte 4: 0A = 00001010 → Boundary ✓ (next char starts)

is_char_boundary() function:
  fn is_char_boundary(&self, pos: usize) -> bool {
    if pos == 0 || pos == len { return true; }
    let byte = self.source.as_bytes()[pos];
    (byte & 0b11000000) != 0b10000000  ← Not continuation byte?
  }
```

---

## Bug 3: WASM Method Name Transformation

### BROKEN Current Implementation

```
Brief Source:
  rstruct Counter {
    txn Counter.increment [true][@count + 1 == count] { ... };
    <button b-trigger:click="increment">+</button>
  }

Rust Code Generation (Line 338-341):
  txn.name = "Counter.increment"
  txn.name.replace(".", "_") = "Counter_increment"
  
  Generated: pub fn invoke_Counter_increment(&mut self) { ... }
  
  Exported by wasm-bindgen as:
    wasm_pkg.State.invoke_Counter_increment

HTML/Binding Parsing:
  Directive::Trigger { event: "click", txn: "increment" }
  
  Note: txn is just "increment", NOT "Counter.increment"
  (depends on how bindings are extracted from HTML)

JavaScript Generation (Line 779):
  for (const [elId, config] of Object.entries(TRIGGER_MAP)) {
    el.addEventListener(config.event, () => {
      wasm[`invoke_${config.txn}`]();  ← Tries to call...
    });
  }
  
  Substituting: wasm[`invoke_increment`]();
  
  ✗ ERROR! Method "invoke_increment" doesn't exist!
  
  Available methods: ["invoke_Counter_increment", ...]
  
  Runtime Error:
    TypeError: wasm.invoke_increment is not a function
```

### FIXED Implementation with Consistent Naming

```
Brief Source:
  rstruct Counter {
    txn Counter.increment [true][@count + 1 == count] { ... };
    <button b-trigger:click="increment">+</button>
  }

Rust Code Generation (unchanged):
  Generated: pub fn invoke_Counter_increment(&mut self) { ... }
  
  Exported as: wasm_pkg.State.invoke_Counter_increment

JavaScript Generation (FIXED):

Step 1: Build transaction name mapping:
  const TRANSACTION_METHODS = {
    "Counter.increment": "invoke_Counter_increment",
    "reset": "invoke_reset",
    ...
  };

Step 2: Build trigger map with correct method names:
  const TRIGGER_MAP = {
    'btnIncrement': {
      event: 'click',
      txn: 'increment',              ← short name in binding
      method: 'invoke_Counter_increment'  ← full method name
    },
    ...
  };

Step 3: Event listener uses full method name:
  for (const [elId, config] of Object.entries(TRIGGER_MAP)) {
    el.addEventListener(config.event, () => {
      wasm[config.method]();  ← Uses "invoke_Counter_increment"
    });
  }
  
  Substituting: wasm['invoke_Counter_increment']();
  
  ✓ SUCCESS! Method exists and is called correctly!

Data Flow:
  Brief Source
      ↓
  Parser extracts: txn="increment" (from b-trigger:click)
      ↓
  Binding stores: txn: "increment"
      ↓
  WASM Gen looks up: txn_map["Counter.increment"] → "invoke_Counter_increment"
      ↓
  JS stores: method: "invoke_Counter_increment"
      ↓
  Runtime calls: wasm[config.method]() ✓
```

---

## Comparison: Bug Fix Complexity

### Bug 1: Nested HTML (Medium Complexity)

```
Effort Scale: ████████░ (8/10)

Changes Needed:
  1. Add 'depth' variable (1 line)
  2. Implement tag pattern matching (20 lines)
  3. Increment/decrement depth logic (15 lines)
  4. Handle self-closing tags (10 lines)
  5. Testing (50 lines)
  
Total: ~100 lines of code changes

Testing Effort:
  - Need comprehensive test cases
  - Multiple nesting levels to verify
  - Edge cases (self-closing, comments)
  - Integration with existing parser

Estimated Time: 1-2 hours
```

### Bug 2: Unicode Handling (Low Complexity)

```
Effort Scale: ██░░░░░░░ (2/10)

Changes Needed:
  1. Replace chars().nth() calls (5 lines)
  2. Add is_char_boundary() helper (5 lines)
  3. Use char.len_utf8() (5 lines)
  4. Testing (30 lines)
  
Total: ~45 lines of code changes

Testing Effort:
  - Straightforward: emoji works or it doesn't
  - Multiple character types to test
  - Simple validation

Estimated Time: 30 minutes
```

### Bug 3: WASM Method Names (Very Low Complexity)

```
Effort Scale: ███░░░░░░░ (3/10)

Changes Needed:
  1. Add transaction name mapping loop (10 lines)
  2. Store method name in binding map (2 lines)
  3. Update event listener code (3 lines)
  4. Testing (20 lines)
  
Total: ~35 lines of code changes

Testing Effort:
  - Verify generated JavaScript contains correct names
  - Test with struct.method transaction names
  - Verify click handlers work

Estimated Time: 15 minutes
```

---

## Implementation Order Recommendation

```
Priority Matrix:

         Impact
         High    ┌─────────────┐
                 │   Bug #1    │
                 │  Nested     │
                 │   HTML      │
                 │             │
         Medium  │   Bug #2    │
                 │  Unicode    │
                 │             │
         Low     │   Bug #3    │
                 │   WASM      │
                 └─────────────┘
                 Low    Medium   High
                      Complexity

Suggested Order (by ROI):
  1. Bug #3 (WASM) → 15 min fix, unblocks all interactive apps
  2. Bug #2 (Unicode) → 30 min fix, improves usability
  3. Bug #1 (HTML) → 2 hour fix, enables 80% of real UIs

Total Fix Time: ~2.5 hours for all three critical bugs
```

