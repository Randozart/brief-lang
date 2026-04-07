# Design Document: Adaptive Reactor Scheduling with @Hz Declarations
**Feature:** Reactor Speed Optimization  
**Date:** 2026-04-05  
**Status:** Ready for Implementation  
**Complexity:** Medium (150-200 lines)  
**Time Estimate:** 3-4 hours

---

## 1. Overview

### 1.1 Problem Statement

Brief's reactor runs continuously to evaluate preconditions, but different files have different needs:

- **UI components** need `@10Hz` (fast enough for smooth interaction)
- **Game logic** needs `@60Hz` (frame-synchronized)
- **Data sync** needs `@1Hz` (occasional polling)
- **Pure libraries** don't need reactor at all

Without optimization, either:
1. Everything runs at slowest speed (wasted CPU)
2. Everything runs at fastest speed (wasted power)
3. Multiple reactors (context switch overhead)

### 1.2 Solution

Implement **single global reactor** with adaptive scheduling:
- One reactor instance across all loaded files
- Each file declares its polling need with `@Hz`
- Global reactor runs at `max(@Hz)` across all files
- Files with slower speeds are intelligently skipped

### 1.3 Benefits

1. **Zero CPU for pure libraries:** No `rct` blocks = no reactor = no cost
2. **Adaptive scheduling:** Reactor auto-tunes to actual requirements
3. **R.rbv optimization:** Passive UI components consume zero polling bandwidth
4. **Composability:** Multiple files work together without explicit coordination
5. **Developer control:** Explicit `@Hz` declarations show intent

---

## 2. Semantics

### 2.1 Syntax

```bnf
program ::= reactor_decl? (definitions | transactions | reactives)*

reactor_decl ::= "reactor" "@" integer "Hz" ";"

transaction ::= txn_decl
txn_decl ::= ("async")? "txn" identifier contract "{" body "}" ("@" integer "Hz")? ";"

rct_transaction ::= "rct" ("async")? "txn" identifier contract "{" body "}" ("@" integer "Hz")? ";"
```

### 2.2 File-Level Reactor Declaration

```brief
reactor @30Hz;
```

**Rules:**
- Optional (default is `@10Hz` if omitted)
- Applies to all `rct` blocks in the file
- Can be overridden by per-rct declarations
- Must appear at top level (not inside functions/structs)

### 2.3 Per-rct Speed Declaration

```brief
rct [condition] txn name [pre][post] { ... } @60Hz;
```

**Rules:**
- Optional (uses file-level default if omitted)
- Overrides file-level reactor declaration
- Must be a positive integer
- Compiler warns if value is unreasonable (`@10000Hz` or higher)

### 2.4 Reactor Inactivity

Files without any `rct` blocks are **reactor-inactive**:

```brief
// Pure library - reactor never activates
let MAX_SIZE: Int = 100;

defn add(a: Int, b: Int) [true][true] -> Int {
  term a + b;
};

txn process [data_ready][processed] {
  // Passive transaction - called explicitly, not reactor-driven
  term;
};
```

**Implication:** Zero reactor overhead for library files.

### 2.5 Global Reactor Adaptation

When multiple files are loaded:

```
File A: reactor @10Hz;     (or default)
File B: reactor @60Hz;
File C: no rct blocks

Global reactor speed = max(10, 60) = @60Hz

Scheduling:
- File A: Check every 6 ticks (60 / 10)
- File B: Check every tick (60 / 60)
- File C: Never participate
```

**Algorithm:**
```
collected_speeds = []
for each loaded file:
    if file has rct blocks:
        collected_speeds.append(file.reactor_speed)

global_speed = max(collected_speeds)

for each file:
    if file.reactor_speed:
        skip_ratio = global_speed / file.reactor_speed
        check_interval = skip_ratio  // Check every N ticks
    else:
        inactive  // Don't check
```

---

## 3. Implementation

### 3.1 Parser Changes

**File:** `src/parser.rs`

Add reactor declaration parsing to `parse_program()`:

```rust
fn parse_program(&mut self) -> Result<Program> {
    let mut reactor_speed: Option<u32> = None;
    let mut items = Vec::new();
    
    // NEW: Check for reactor declaration at start
    if self.peek() == "reactor" {
        self.advance();
        self.consume("@")?;
        let speed_str = self.advance().unwrap_text();
        let speed = speed_str.trim_end_matches("Hz").parse::<u32>()?;
        
        // Validate speed
        if speed == 0 {
            return Err(ParseError::ReactorSpeedMustBePositive);
        }
        if speed >= 10000 {
            self.warnings.push(Warning::AggressiveReactorSpeed(speed));
        }
        
        reactor_speed = Some(speed);
        self.consume(";")?;
    }
    
    // Parse remaining items
    while self.peek() != EOF {
        items.push(self.parse_toplevel()?);
    }
    
    Ok(Program {
        reactor_speed,
        items,
    })
}
```

Update `parse_rct_transaction()` to accept speed:

```rust
fn parse_rct_transaction(&mut self) -> Result<Transaction> {
    // ... existing parsing ...
    
    let mut speed: Option<u32> = None;
    
    // NEW: Check for speed declaration after }
    if self.peek() == "@" {
        self.advance();
        let speed_str = self.advance().unwrap_text();
        let s = speed_str.trim_end_matches("Hz").parse::<u32>()?;
        
        if s == 0 {
            return Err(ParseError::ReactorSpeedMustBePositive);
        }
        if s >= 10000 {
            self.warnings.push(Warning::AggressiveReactorSpeed(s));
        }
        
        speed = Some(s);
    }
    
    self.consume(";")?;
    
    Ok(Transaction {
        // ... existing fields ...
        reactor_speed: speed,
    })
}
```

### 3.2 AST Changes

**File:** `src/ast.rs`

```rust
pub struct Program {
    pub items: Vec<TopLevel>,
    pub reactor_speed: Option<u32>,  // NEW: file-level default
}

pub struct Transaction {
    pub name: String,
    pub contract: Contract,
    pub body: Vec<Statement>,
    pub reactor_speed: Option<u32>,  // NEW: per-rct override
    pub is_reactive: bool,
}
```

### 3.3 Type Checker Changes

**File:** `src/typechecker.rs`

Validate that files with `rct` blocks declare speeds sensibly:

```rust
fn check_program(&mut self, program: &Program) -> Result<(), Error> {
    // Check if file has rct blocks
    let has_rct = program.items.iter().any(|item| {
        matches!(item, TopLevel::RctTransaction(_))
    });
    
    if has_rct && program.reactor_speed.is_none() {
        // File has rct blocks but no explicit reactor speed
        // This is OK - will use default @10Hz
        // But we could warn if developer seems surprised
    }
    
    // Check per-rct speeds
    for item in &program.items {
        if let TopLevel::RctTransaction(txn) = item {
            if let Some(speed) = txn.reactor_speed {
                if speed == 0 {
                    return Err(TypeError::ReactorSpeedMustBePositive);
                }
            }
        }
    }
    
    Ok(())
}
```

### 3.4 Runtime/Interpreter Changes

**File:** `src/interpreter.rs` or new `src/reactor.rs`

```rust
pub struct ReactorScheduler {
    /// Files and their reactor requirements
    files: Vec<FileMetadata>,
    
    /// Global reactor speed (max of all speeds)
    global_speed: u32,
    
    /// Current tick
    current_tick: u64,
}

struct FileMetadata {
    name: String,
    reactor_speed: u32,
    skip_ratio: u32,  // Check every N ticks
}

impl ReactorScheduler {
    pub fn new() -> Self {
        ReactorScheduler {
            files: Vec::new(),
            global_speed: 10,  // Default
            current_tick: 0,
        }
    }
    
    /// Register a new file with its reactor speed
    pub fn register_file(&mut self, name: String, speed: Option<u32>) {
        let speed = speed.unwrap_or(10);  // Default @10Hz
        
        let skip_ratio = if speed > 0 {
            self.global_speed / speed
        } else {
            u32::MAX  // Never check
        };
        
        self.files.push(FileMetadata {
            name,
            reactor_speed: speed,
            skip_ratio,
        });
        
        // Recalculate global speed
        self.recalculate_global_speed();
    }
    
    fn recalculate_global_speed(&mut self) {
        self.global_speed = self.files.iter()
            .map(|f| f.reactor_speed)
            .max()
            .unwrap_or(10);
        
        // Recalculate skip ratios
        for file in &mut self.files {
            file.skip_ratio = self.global_speed / file.reactor_speed;
        }
    }
    
    /// Check if a file should have its preconditions evaluated this tick
    pub fn should_check_file(&self, file_idx: usize) -> bool {
        if file_idx >= self.files.len() {
            return false;
        }
        
        let file = &self.files[file_idx];
        if file.skip_ratio == u32::MAX {
            return false;  // Never check (reactor inactive)
        }
        
        self.current_tick % file.skip_ratio as u64 == 0
    }
    
    /// Advance to next tick
    pub fn tick(&mut self) {
        self.current_tick += 1;
    }
}
```

### 3.5 Integration with Proof Engine

The proof engine should validate that speeds are declared:

```rust
fn verify_program(program: &Program) -> Result<(), Error> {
    let has_rct = program.items.iter().any(|item| {
        matches!(item, TopLevel::RctTransaction(_))
    });
    
    if has_rct && program.reactor_speed.is_none() {
        // OK - will use default @10Hz
        // But could note in output
    }
    
    // Verify each rct transaction
    for item in &program.items {
        if let TopLevel::RctTransaction(txn) = item {
            // Verify contract as usual
            verify_transaction_contract(txn)?;
        }
    }
    
    Ok(())
}
```

---

## 4. Testing Strategy

### 4.1 Unit Tests

**File:** `tests/reactor_tests.rs`

```rust
#[test]
fn test_default_reactor_speed() {
    let code = r#"
        rct [true] txn test [true][true] { term; };
    "#;
    let program = parse(code).unwrap();
    assert_eq!(program.reactor_speed, None);  // Will use default @10Hz
}

#[test]
fn test_explicit_reactor_speed() {
    let code = r#"
        reactor @30Hz;
        rct [true] txn test [true][true] { term; };
    "#;
    let program = parse(code).unwrap();
    assert_eq!(program.reactor_speed, Some(30));
}

#[test]
fn test_per_rct_override() {
    let code = r#"
        reactor @10Hz;
        rct [true] txn slow [true][true] { term; };
        rct [true] txn fast [true][true] { term; } @60Hz;
    "#;
    let program = parse(code).unwrap();
    
    let txns: Vec<_> = program.items.iter()
        .filter_map(|item| if let TopLevel::RctTransaction(t) = item { Some(t) } else { None })
        .collect();
    
    assert_eq!(txns[0].reactor_speed, None);     // Uses default @10Hz
    assert_eq!(txns[1].reactor_speed, Some(60)); // Overrides to @60Hz
}

#[test]
fn test_pure_library_no_reactor() {
    let code = r#"
        defn add(a: Int, b: Int) [true][true] -> Int { term a + b; };
        txn process [true][true] { term; };
    "#;
    let program = parse(code).unwrap();
    
    // No rct blocks = no reactor
    let has_rct = program.items.iter().any(|item| {
        matches!(item, TopLevel::RctTransaction(_))
    });
    assert!(!has_rct);
}

#[test]
fn test_aggressive_speed_warning() {
    let code = r#"
        reactor @50000Hz;
    "#;
    let (program, warnings) = parse_with_warnings(code).unwrap();
    
    assert!(warnings.iter().any(|w| {
        matches!(w, Warning::AggressiveReactorSpeed(_))
    }));
}

#[test]
fn test_scheduler_skip_ratio() {
    let mut scheduler = ReactorScheduler::new();
    scheduler.register_file("file_a".to_string(), Some(10));   // @10Hz
    scheduler.register_file("file_b".to_string(), Some(60));   // @60Hz
    
    assert_eq!(scheduler.global_speed, 60);
    
    // file_a should be checked every 6 ticks (60 / 10)
    let file_a_idx = 0;
    let file_b_idx = 1;
    
    for tick in 0..12 {
        scheduler.current_tick = tick;
        
        if tick % 6 == 0 {
            assert!(scheduler.should_check_file(file_a_idx));
        } else {
            assert!(!scheduler.should_check_file(file_a_idx));
        }
        
        // file_b checked every tick
        assert!(scheduler.should_check_file(file_b_idx));
    }
}
```

### 4.2 Integration Tests

Create `examples/reactor_speeds.bv`:

```brief
// Pure library - no reactor
defn multiply(a: Int, b: Int) [true][true] -> Int {
  term a * b;
};

// Slow polling - UI
reactor @10Hz;

let ui_ready: Bool = false;

rct [true] txn update_ui [pre][post] {
  &ui_ready = true;
  term;
};
```

Create `examples/reactor_adaptive.bv`:

```brief
// Game logic needs fast reactor
reactor @60Hz;

let position: Int = 0;

rct [true] txn move [true][true] {
  &position = position + 1;
  term;
};
```

When both files loaded together:
- Global reactor: @60Hz
- Pure library: Not checked
- Slow UI: Checked every 6 ticks
- Game logic: Checked every tick

### 4.3 Regression Tests

```bash
cargo test --release
# All 8 existing stress tests should pass
# New reactor speed tests should pass
```

---

## 5. Edge Cases

### 5.1 Zero Hz

```brief
reactor @0Hz;  // ERROR: Must be positive
```

✅ **Handled:** Parser rejects with clear error.

### 5.2 Extremely High Hz

```brief
reactor @50000Hz;  // WARNING: Probably unintended
```

✅ **Handled:** Compiler warns "Did you want to set your PC aflame?"

### 5.3 Mixed Speeds with No File-Level Default

```brief
rct [c1] txn t1 [p][q] { term; } @10Hz;
rct [c2] txn t2 [p][q] { term; } @60Hz;
```

✅ **Handled:** Each rct has explicit speed; no conflict.

### 5.4 Per-rct Override Higher Than File Default

```brief
reactor @10Hz;
rct [c] txn test [p][q] { term; } @100Hz;  // OK - overrides upward
```

✅ **Allowed:** Per-rct can override both ways.

### 5.5 File-Level Default Irrelevant for Non-rct

```brief
reactor @100Hz;  // File has no rct blocks
defn pure_fn() [true][true] -> Int { term 42; };
```

✅ **Handled:** Reactor declaration ignored; no reactor activates.

---

## 6. R.rbv Implications

For Rendered Brief components, this enables powerful optimization:

```brief
// mainMenu.rbv - passive UI, doesn't need reactor
<script type="brief">
  let selected: Int = 0;
  
  txn select_item [true][selected > 0] {
    &selected = item_id;
    term;
  };
</script>

// gameLogic.rbv - active gameplay, needs fast reactor
<script type="brief">
  reactor @60Hz;
  
  let position: Int = 0;
  
  rct [input_ready] txn move [pre][post] {
    &position = position + 1;
    term;
  };
</script>

// When served over connection:
// - mainMenu: Zero polling overhead (no reactor)
// - gameLogic: 60Hz polling (necessary for gameplay)
// - Global: Adapts to @60Hz, mainMenu checked every 6 ticks if active
```

---

## 7. Implementation Checklist

- [ ] Update SPEC-v6.1 with reactor speed section (done)
- [ ] Add `reactor_speed` field to Program AST
- [ ] Add `reactor_speed` field to Transaction AST
- [ ] Update parser to handle `reactor @Hz;` declarations
- [ ] Update parser to handle per-rct `@Hz` declarations
- [ ] Add warning system for aggressive speeds
- [ ] Implement ReactorScheduler struct
- [ ] Implement skip_ratio calculation
- [ ] Integrate into interpreter/reactor loop
- [ ] Add validation to type checker
- [ ] Write comprehensive unit tests
- [ ] Create integration test examples
- [ ] Run regression tests
- [ ] Verify all 8 stress tests still pass
- [ ] Commit with message

---

## 8. Success Criteria

- ✅ File-level `reactor @Hz;` parses and stores correctly
- ✅ Per-rct `@Hz` overrides file-level declaration
- ✅ Pure files without `rct` don't activate reactor
- ✅ Default is `@10Hz` if not specified
- ✅ Aggressive speeds (`@10000Hz+`) produce warnings
- ✅ ReactorScheduler calculates skip ratios correctly
- ✅ Multiple files adapt to max speed
- ✅ Files with slower speeds are checked at correct intervals
- ✅ R.rbv components without `rct` have zero overhead
- ✅ All 8 existing stress tests still pass
- ✅ New reactor speed examples compile

---

*End of Design Document: Adaptive Reactor Scheduling*
