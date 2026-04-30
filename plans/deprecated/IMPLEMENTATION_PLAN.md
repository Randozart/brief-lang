# Brief Compiler - rstruct Implementation Fixes

## Overview

This document details the fixes required to make rstruct components work correctly in Brief. The issues were discovered during testing of the landing page demo.

## Issues Identified

### 1. b-each Container Selector Bug

**Location**: `brief-compiler/src/view_compiler.rs` line 191

**Problem**: The code uses `.nth(1)` to find the container element, which gets the grandparent element instead of the direct parent.

**Current (WRONG)**:
```rust
let container_id = if let Some((_, parent_pos)) = element_stack.iter().rev().nth(1) {
```

**Fixed**:
```rust
let container_id = if let Some((_, parent_pos)) = element_stack.iter().rev().nth(0) {
```

**Impact**: The b-each directive was rendering into the wrong element (h2 instead of ul).

---

### 2. rstruct Trigger Mapping Bug

**Location**: `brief-compiler/src/wasm_gen.rs` lines 801-806

**Problem**: When user writes `b-trigger:click="tick"` inside an rstruct, the JS trigger map maps to `invoke_tick` but only `invoke_Counter_tick` exists (the alias isn't being used correctly).

**Current code**:
```rust
let invoke_method = format!("invoke_{}", txn.name.replace(".", "_"));
```

**Fixed code**:
```rust
let short_name = if txn.name.contains('.') { 
    txn.name.split('.').last().unwrap_or(&txn.name) 
} else { &txn.name };
let invoke_method = format!("invoke_{}", short_name);
```

**Impact**: Buttons inside rstruct components were not working.

---

### 3. rstruct Field Initialization

**Philosophy**: ALL values must be initialized. No hidden values.

**Exception**: Implicit declaration via contracts still works (desugarer auto-generates state from contract variables).

#### 3a. AST Changes

**File**: `brief-compiler/src/ast.rs`

Add `default` field to `StructField`:

```rust
pub struct StructField {
    pub name: String,
    pub ty: Type,
    pub default: Option<Expr>,  // NEW - required initializer
}
```

#### 3b. Parser Changes

**File**: `brief-compiler/src/parser.rs`

- Accept `let count: Int = 5;` syntax in rstruct
- Error if field has no initializer: "rstruct field '{name}' must have initial value"
- Support optional `let` keyword before field declaration

#### 3c. WASM Generation

**File**: `brief-compiler/src/wasm_gen.rs`

- Generate signals from rstruct field defaults
- Include rstruct fields in the signal map

---

### 4. Transaction Naming Shortcut

**Location**: `brief-compiler/src/parser.rs` (rstruct parsing)

**Problem**: Users must write `txn Counter.tick [...]` but should be able to write just `txn tick [...]`

**Fix**: Inside rstruct, allow `txn tick [...]` syntax and auto-expand to `rstructname.tick`

---

## Updated Syntax Examples

### Old (broken):
```brief
rstruct Counter {
    count: Int;  // ERROR - no initializer
    
    txn Counter.tick [count > 0][count == @count - 1] {
        &count = count - 1;
        term;
    };
    
    <div>
        <span b-text="count">0</span>
        <button b-trigger:click="tick">-</button>
    </div>
}
```

### New (correct):
```brief
rstruct Counter {
    let count: Int = 0;  // REQUIRED - must have default
    
    txn tick [count > 0][count == @count - 1] {  // "tick" auto-expands to "Counter.tick"
        &count = count - 1;
        term;
    };
    
    <div>
        <span b-text="count">0</span>
        <button b-trigger:click="tick">-</button>
    </div>
}
```

---

## Testing

After implementation, test with:

```bash
cd /home/randozart/Desktop/Projects/codicil/landing-page
rm -rf landing-build
/home/randozart/Desktop/Projects/brief-compiler/target/release/brief-compiler rbv landing.rbv
```

Expected results:
- Counter buttons work (+ and -)
- List demo shows all 3 items (Contracts, Reactivity, Type Safety)
- rstruct component displays count correctly

---

## 5. Reactive Transactions in WASM

**Location**: `brief-compiler/src/wasm_gen.rs`

### 5.1 Reactive Transaction Tracking

Added fields to `WasmGenerator`:
```rust
reactive_txns: Vec<Transaction>,
reactive_dependency_map: HashMap<String, Vec<usize>>,
```

### 5.2 Dependency Extraction

Added `extract_dependencies()` and `extract_identifiers()` methods to extract signal dependencies from precondition expressions.

### 5.3 Reactive Execution Loop

Added reactive transaction execution in `poll_dispatch()`:
```rust
if !self.reactive_txns.is_empty() {
    // Run up to 1000 iterations
    for _ in 0..1000 {
        // Execute each reactive transaction
        // Break if no signals changed
    }
}
```

### Usage
```brief
rstruct Counter {
    let count: Int = 5;
    rct txn tick [count > 0][count == @count - 1] {
        &count = count - 1;
        term;
    };
};
```

---

## 6. --no-cache CLI Flag

**Location**: `brief-compiler/src/main.rs`

Added `--no-cache` flag to both `run` and `rbv` commands to clear build cache before compiling.

**Usage**:
```bash
brief run landing.rbv --no-cache
brief rbv landing.rbv --no-cache
```

---

## 7. Removed Emoji from CLI

Removed checkmark emoji from success messages in `main.rs`:
- `✓ All checks passed` → `All checks passed`
- `✓ Project '{}' created successfully` → `Project '{}' created successfully`
- `✓ RBV compiled successfully` → `RBV compiled successfully`

Note: wasm-pack still outputs emoji (🎯🌀⚡✨📦) - that's external to the compiler.

---

## 8. Property Access in Directives

**Location**: `brief-compiler/src/wasm_gen.rs` - `render_each` function

### Implementation

Extended b-each rendering to support property access like `item.name`:

```rust
// Detect pattern: b-text="item.property"
// Access property on JS object using js_sys::Object::from(item).get(prop_name)
```

### Usage

```brief
struct Item {
    let name: String = "";
    let color: String = "#fff";
};

let items: List<Item> = [
    Item { name: "Contracts" },
    Item { name: "Reactivity" },
    Item { name: "Type Safety" }
];

// In template:
<ul b-each:item="items">
    <li b-text="item.name">Item</li>
</ul>
```

### How It Works

1. During render_each, detect `b-text="item.property">` patterns
2. Convert item to JS object: `js_sys::Object::from(item)`
3. Access property: `.get("property")`
4. Convert to string and escape for HTML

### Limitations

- Currently works for initial render only (not reactive)
- Single-level property access (item.name works, item.address.city does not)
- Requires items to be JS objects with properties

---

## Summary

Recent changes implement:

1. **@Hz timing** - Reactive transactions now respect file-level reactor speed (default 10Hz). Add `@60` to transaction for custom speed.

2. **Property access** - Uses `js_sys::Reflect::get()` for property access in b-each templates.

3. **Struct parsing** - Added `let` keyword support in struct definitions.