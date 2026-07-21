# `#fuzz` Pragmas — Compile-Time Inline Tests

**Date:** 2026-06-30
**Status:** Implemented

## Motivation

Contracts (`[pre][post]`) are mandatory and general — they describe all
possible states. `#fuzz` is an optional, specific assertion: "I, the
programmer, have concrete inputs and I know the expected output. Prove me
right." It catches logic errors where the contract is technically satisfied
but the implementation is wrong.

This is especially important for `inop` blocks, where the BILD body is opaque
to the Brief-level proof engine. A fuzz case with a BILD simulator can
concretely verify that the BILD instructions compute what the programmer
expects — no fallback expression required.

## Syntax

```brief
#fuzz(param = expr, param = expr, ...) -> expected_expr
```

Each binding is `param = expr` — named parameters set to deterministic
expressions. The expected output comes after `->`. Multiple `#fuzz` lines
before an item define multiple test cases.

### Examples

```brief
// defn — uses interpreter
#fuzz(x = 0, y = 0) -> 0
#fuzz(x = 1, y = 2) -> 3
#fuzz(x = -1, y = 5) -> 4
defn add_threshold(x: Int, y: Int) -> Int {
    let sum: Int = x + y;
    [sum >= 10] { term sum * 2; };
    term sum;
};

// txn (callable) — uses interpreter
#fuzz(value = 5) -> 15
txn accumulate(value: Int) [Count < 10][Count == 10] -> Int {
    &Sum = Sum + value;
    &Count = Count + 1;
    term Sum;
};

// inop — uses BILD simulator (no fallback needed)
#fuzz(a = 10, b = 2) -> 5
#fuzz(a = 0, b = 1) -> 0
inop! safe_div(a: Int, b: Int) -> Int [b != 0][result >= 0] (%state) {
    %res = sdiv i64 %a, %b;
    term %res;
};

// cell — skipped with a warning (state setup too complex)
#fuzz(x = 5) -> 25
cell! Squarer(x: Int) -> Int {
    result: Int = 0;
    txn compute() { &result = x * x; term result; };
};
```

## Top-level variable bindings

If a function references a top-level `let` or `const`, that must also be bound
in the fuzz:

```brief
let threshold: Int = 10;

#fuzz(x = 5, threshold = 3) -> 8    // overrides threshold locally
#fuzz(x = 5, threshold = 20) -> 15  // different path
defn capped_add(x: Int) -> Int {
    let raw: Int = x + threshold;
    [raw > 20] { term 20; };
    term raw;
};
```

## AST Changes (`src/ast.rs`)

### New struct

```rust
#[derive(Debug, Clone)]
pub struct FuzzCase {
    pub bindings: Vec<(String, Expr)>,  // param_name = expr
    pub expected: Expr,                  // expected result expression
    pub span: Option<Span>,
}
```

### New TopLevel variant

```rust
pub enum TopLevel {
    // ...existing variants...
    /// `#fuzz(...) -> ...` pragma — wraps an item with inline test cases.
    /// Verified at compile time via interpreter (defn/txn) or BILD simulator (inop).
    /// Skipped if the inner item is a cell (state setup too complex).
    Fuzzed {
        item: Box<TopLevel>,
        cases: Vec<FuzzCase>,
    },
}
```

Wrapping order when both `#fuzz` and `#test` are present:

```
TopLevel::Test { item: Box<TopLevel::Fuzzed { item: Box<TopLevel::Definition>, ... }>, ... }
```

## Parser Changes (`src/parser.rs`)

### New method: `parse_fuzz_case()`

Parses: `#fuzz(ident = expr, ...) -> expr`

Lookahead: `Token::Hash` followed by `Identifier("fuzz")` followed by `LParen`.

```rust
fn parse_fuzz_case(&mut self) -> Result<FuzzCase, SyntaxError> {
    // consume `#fuzz`
    self.advance(); // Hash
    let name = self.expect_identifier()?; // "fuzz"
    // consume `(`
    self.expect(Token::LParen)?;
    // parse bindings: ident = expr, ...
    let mut bindings = Vec::new();
    loop {
        let ident = self.expect_identifier()?;
        self.expect(Token::Eq)?;
        let expr = self.parse_expression()?;
        bindings.push((ident, expr));
        if let Some(Ok(Token::RParen)) = self.current_token() {
            self.advance();
            break;
        }
        self.expect(Token::Comma)?;
    }
    // parse `->`
    self.expect(Token::Arrow)?;
    // parse expected expression
    let expected = self.parse_expression()?;
    // expect `;` terminator
    self.expect(Token::Semicolon)?;
    Ok(FuzzCase { bindings, expected, span: ... })
}
```

### Modified: `parse_top_level()`

Between modifier parsing and the main `match cur_tok`, add a loop to collect
consecutive `#fuzz` lines:

```rust
// Collect fuzz cases before the item
let mut fuzz_cases: Vec<FuzzCase> = Vec::new();
while self.lookahead_is_fuzz() {
    fuzz_cases.push(self.parse_fuzz_case()?);
}
```

After parsing the item, wrap if any fuzz cases:

```rust
let mut result = match cur_tok { /* ... existing match ... */ };
if !fuzz_cases.is_empty() {
    result = result.map(|item| TopLevel::Fuzzed {
        item: Box::new(item),
        cases: fuzz_cases,
    });
}
Ok(wrap_test(result?, &test_groups))
```

### Lookahead helper

```rust
fn lookahead_is_fuzz(&mut self) -> bool {
    matches!(self.current_token(), Some(Ok(Token::Hash)))
        && self.peek_identifier().as_deref() == Some("fuzz")
}
```

## Pass-Through: Every Stage Unwraps `TopLevel::Fuzzed`

Wherever `TopLevel::Test` is unwrapped, `TopLevel::Fuzzed` must be too:

| File | Where | What to add |
|------|-------|-------------|
| `src/typechecker.rs` | Pass 1 (line ~629) | Match `Fuzzed { item, .. }` → recurse into inner |
| `src/typechecker.rs` | Pass 2 (line ~729) | Match `Fuzzed { item, .. }` → typecheck inner |
| `src/interpreter.rs` | Load (line ~1379) | Match `Fuzzed { item, .. }` → register inner defns |
| `src/interpreter.rs` | Loop (line ~1421) | Match `Fuzzed { item, .. }` → unwrap and process |
| `src/proof_engine.rs` | Build maps (line ~1697) | Match `Fuzzed { item, .. }` → unwrap for defn/txn maps |

This follows the exact same pattern as `TopLevel::Test`.

## Fuzz Checker Architecture (`src/fuzz_checker/`)

Two submodules:

### `src/fuzz_checker/bild_sim.rs` — BILD Interpreter

A lightweight register-machine simulator that executes BILD bodies with
concrete `Value` types.

```rust
pub struct BildSimulator;

impl BildSimulator {
    /// Execute a BILD body with concrete register bindings.
    /// Returns the terminator value(s), or an error if execution fails.
    pub fn execute(
        body: &[String],                    // BILD instructions (one per element, semicolons included)
        params: &[(String, Type)],          // formal parameter names and types
        bindings: &HashMap<String, Value>,  // concrete argument bindings
        has_state: bool,                    // whether `%state` pointer is available
        state_values: &HashMap<String, Value>, // state field values (for load/store)
    ) -> Result<Vec<Value>, String>
}
```

#### Instruction handling

| Category | Instructions | Semantics |
|----------|-------------|-----------|
| Arithmetic | `add`, `sub`, `mul`, `sdiv`, `udiv`, `srem`, `urem` | `Value::Int` → `Value::Int` |
| Float | `fadd`, `fsub`, `fmul`, `fdiv` | `Value::Float` / `Value::Float64` → same type |
| Bitwise | `and`, `or`, `xor`, `shl`, `lshr`, `ashr` | `Value::Int` → `Value::Int` |
| Compare | `icmp eq/ne/slt/sle/sgt/sge/ult/ule/ugt/uge` | `Value::Int` → `Value::Bool` |
| Select | `select i1 %cond, type %a, type %b` | Pick one of two |
| Cast | `trunc`, `zext`, `sext`, `fptrunc`, `fpext`, `fptosi`, `sitofp`, `uitofp` | Type conversion |
| Memory | `load`, `store` | Simulated via `state_values` map + GEP index |
| GEP | `getelementptr inbounds %State, ptr %state, i32 0, i32 N` | Compute field index N |
| Terminator | `term %reg` / `term %r1, %r2` / `term` | Return value(s) |
| Opaque | `call`, `asm`, `atomicrmw`, `cmpxchg`, `extractvalue`, `inttoptr`, `alloca` | Return `Value::Unknown` placeholder |

**Register file**: `HashMap<String, Value>`. Pre-populated with param bindings
(`%param_name`). `%state` is stored as `Value::Ptr` if `has_state` is true.

**GEP simulation**: `getelementptr inbounds %State, ptr %state, i32 0, i32 N`
→ the final index `N` selects which state field. Loads/stores from a separate
`state_values: HashMap<usize, Value>` map keyed by field index. This avoids
needing an actual memory model.

**Opaque op fallback**: When an instruction cannot be simulated (call, asm,
etc.), the simulator returns `Value::Unknown` for that register and continues.
If the final result depends on an unknown register, the fuzz case is reported
as "unverifiable — opaque instructions in critical path."

#### Tokenizer

Simpler than LLVM's — each BILD instruction line is split by whitespace:

```
%res = sdiv i64 %a, %b;
→ ["%res", "=", "sdiv", "i64", "%a", "%b"]
```

Type tokens (`i64`, `float`, `ptr`, etc.) are recognized and consumed but
only used for result type determination. The concrete LLVM type is not
critical for simulation — values are already typed at the Brief level.

### `src/fuzz_checker/mod.rs` — Orchestrator

```rust
use crate::ast::*;
use crate::interpreter::{Interpreter, Value, RuntimeError};
use crate::errors::FuzzError;

pub fn check_fuzz_cases(
    program: &Program,
    interpreter: &mut Interpreter,
) -> Vec<FuzzError>
```

Iterates `program.items`, finds all `TopLevel::Fuzzed`, dispatches by inner
item type:

#### For `Definition` and `Transaction` (callable)

1. Look up the item's parameter list from the AST node
2. For each `FuzzCase`:
   a. Evaluate each binding `expr` → `Value` via interpreter
   b. Reconstruct the argument list in parameter order, looking up each param
      name in the bindings. Error if any param is missing.
   c. Evaluate `expected` → `Value` via interpreter
   d. Call `interpreter.call_defn(name, &args)` or
      `interpreter.call_txn(name, &args)`
   e. Compare actual result vs expected using `Value == Value`
   f. On mismatch, emit `FuzzError::Mismatch`

#### For `InopDeclaration`

1. Evaluate binding expressions → `HashMap<String, Value>`
2. Check precondition: for each binding, evaluate the precondition expression
   with those values. If precondition `false`, the fuzz is invalid — emit
   `FuzzError::InvalidInput`
3. Execute BILD body via `BildSimulator::execute()`:
   - Pass param names/types, bindings, `has_state_access`, and state values
   - If `has_state_access`, any state fields referenced by fuzz bindings
     (prefixed with `state.`) are loaded into the state values map
4. Compare BILD simulator result vs expected
5. Check postcondition with the result value
6. On any mismatch, emit `FuzzError::Mismatch`

#### For `CellDef`

Skip with `FuzzError::Skipped` (warning, not error). State initialization
for cells is complex (transactions run inside the cell's state space) and is
deferred.

## Error Types (`src/errors.rs`)

New `FuzzError` enum (separate from `ProofError` for clarity):

```rust
#[derive(Debug, Clone)]
pub enum FuzzError {
    /// Expected output does not match actual
    Mismatch {
        function: String,
        case_index: usize,
        inputs: String,
        expected: String,
        actual: String,
        span: Span,
    },
    /// Fuzz inputs violate the item's precondition
    InvalidInput {
        function: String,
        case_index: usize,
        detail: String,
        span: Span,
    },
    /// BILD simulation encountered an unrecoverable opaque instruction
    Unverifiable {
        function: String,
        case_index: usize,
        detail: String,
        span: Span,
    },
    /// A required parameter was not bound in the fuzz case
    MissingBinding {
        function: String,
        case_index: usize,
        param: String,
        span: Span,
    },
    /// Cell fuzzing is not yet supported
    Skipped {
        function: String,
        reason: String,
        span: Span,
    },
}

impl fmt::Display for FuzzError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FuzzError::Mismatch { function, case_index, inputs, expected, actual, .. } => {
                write!(f, "fuzz case {} of '{}': expected {}, got {}{}",
                    case_index, function, expected, actual,
                    if inputs.is_empty() { String::new() } else { format!(" (inputs: {})", inputs) })
            }
            FuzzError::InvalidInput { function, case_index, detail, .. } => {
                write!(f, "fuzz case {} of '{}': precondition not satisfied: {}",
                    case_index, function, detail)
            }
            // ...
        }
    }
}
```

## Pipeline Integration (`src/main.rs`)

### In `run_check()` — after proof engine, before "All proofs verified"

```rust
if verbose { println!("[FuzzChecker] Running fuzz verification..."); }
if program.has_fuzz_cases() {
    let mut interpreter = interpreter::Interpreter::new()
        .with_program(&program);
    let fuzz_errors = fuzz_checker::check_fuzz_cases(&program, &mut interpreter);
    if !fuzz_errors.is_empty() {
        eprintln!("{}", format_fuzz_errors(&fuzz_errors, file_path_str));
        let has_real_errors = fuzz_errors.iter().any(|e| !matches!(e, FuzzError::Skipped { .. }));
        if has_real_errors { return Err("Fuzz errors".into()); }
    }
}
```

### New subcommand: `"fuzz"` / `"fz"`

```
brief fuzz <file.bv>
```

Stands alone — only runs: parse → resolve → desugar → typecheck → fuzz check.
Skips proof engine and codegen.

```
$ brief fuzz examples/my_program.bv
✓ safe_div case 0: a=10, b=2 → 5
✓ safe_div case 1: a=0, b=1 → 0
✗ add_threshold case 2: x=-1, y=5 → 4 (got 6)
  at examples/my_program.bv:12
```

Exit 0 if all cases pass, 1 if any fail.

## Tests

### Parser tests (`src/parser.rs`)

| Test | Input | Expected |
|------|-------|----------|
| Single fuzz case | `#fuzz(x = 5) -> 25 defn foo(x: Int) -> Int { term x * x; };` | `TopLevel::Fuzzed` with 1 case |
| Multiple fuzz cases | `#fuzz(x = 1) -> 1 #fuzz(x = 2) -> 4 defn ...` | 2 cases in vector |
| Named bindings out of order | `#fuzz(b = 2, a = 1) -> 3 defn add(a, b) ...` | Bindings stored as-is |
| Fuzz + Test | `#test("g") #fuzz(x = 1) -> 2 defn ...` | `Test { Fuzzed { Definition } }` |
| Fuzz on txn | `#fuzz(v = 10) -> 20 txn add(v: Int) ...` | Fuzzed wrapping Transaction |
| Fuzz on inop | `#fuzz(a = 5, b = 3) -> 1 inop! ...` | Fuzzed wrapping InopDeclaration |
| Reject bare expression | `#fuzz(x > 1) -> 2` | Parse error |

### BILD simulator tests (`src/fuzz_checker/bild_sim.rs`)

| Test | BILD body | Inputs | Expected |
|------|-----------|--------|----------|
| Add | `%r = add i64 %a, %b; term %r;` | a=3, b=4 | 7 |
| Sub | `%r = sub i64 %a, %b; term %r;` | a=10, b=3 | 7 |
| Mul | `%r = mul i64 %a, %b; term %r;` | a=6, b=7 | 42 |
| SDiv | `%r = sdiv i64 %a, %b; term %r;` | a=10, b=3 | 3 |
| ICMP eq | `%r = icmp eq i64 %a, %b; term %r;` | a=5, b=5 | true |
| ICMP slt | `%r = icmp slt i64 %a, %b; term %r;` | a=3, b=5 | true |
| Select | `%c = icmp slt i64 %a, %b; %r = select i1 %c, i64 %a, i64 %b; term %r;` | a=3, b=5 | 3 |
| Multi-step | `%t1 = add i64 %a, %b; %t2 = mul i64 %t1, %c; term %t2;` | a=1, b=2, c=3 | 9 |
| ZExt | `%r = zext i8 %a to i64; term %r;` | a=255 (Bool) | 255 |
| Float add | `%r = fadd float %a, %b; term %r;` | a=1.5, b=2.5 | 4.0 |
| Load state | GEP + load pattern | state.x=42 | 42 |
| Store state | GEP + store pattern | state.x=0, store 7 | 7 (after store) |
| Opaque fallthrough | call instr in body | any | `Value::Unknown` or error |
| Void term | `term;` (no value) | any | `Value::Unit` |

### Fuzz checker tests (`src/fuzz_checker/mod.rs`)

| Test | Setup | Expected |
|------|-------|----------|
| Defn passes | `#fuzz(x = 5) -> 25 defn sq(x) { term x*x; }` | No errors |
| Defn fails | `#fuzz(x = 5) -> 26 defn sq(x) { term x*x; }` | `FuzzError::Mismatch` |
| Out-of-order bindings | `#fuzz(b = 1, a = 2) -> 3 defn add(a, b) { term a+b; }` | No errors |
| Top-level let | `let t = 10; #fuzz(x = 5, t = 3) -> 8 defn f(x) { term x + t; }` | No errors |
| Missing binding | `#fuzz(x = 5) -> 6 defn add(x, y) { term x+y; }` | `FuzzError::MissingBinding` for `y` |
| Multi-return defn | `#fuzz(x = 3) -> (9, 27) defn f(x) ...` | No errors |
| Txn callable | `#fuzz(v = 5) -> 15 txn ...` | No errors |
| Inop with BILD sim | `#fuzz(a=10, b=2) -> 5 inop! div ... { %r = sdiv i64 %a, %b; term %r; }` | No errors |
| Inop contract fail | `#fuzz(a=10, b=0) -> 5 inop! div ... [b != 0]` | `FuzzError::InvalidInput` |
| Inop postcondition fail | `#fuzz(a=10, b=2) -> 3 inop! div ... [b != 0][result >= 0]` | `FuzzError::Mismatch` |
| Cell skipped | `#fuzz(x = 5) -> 25 cell! Sqr ...` | `FuzzError::Skipped` warning |

### Integration tests

| Test | Command | Expected |
|------|---------|----------|
| All fuzz pass | `brief fuzz passing.bv` | Exit 0, all `✓` |
| Some fuzz fail | `brief fuzz failing.bv` | Exit 1, errors printed |
| Mixed fuzz and skip | `brief fuzz mixed.bv` | Exit 1, skip warnings |
| Fuzz in check | `brief check passing.bv` | Exit 0 |
| Fuzz in check | `brief check failing.bv` | Exit 1 |
| Fuzz in compile | `brief build failing.bv` | Exit 1, no binary |

## Files to Create/Modify

| File | Action | Change summary |
|------|--------|----------------|
| `src/ast.rs` | Modify | Add `FuzzCase` struct, `TopLevel::Fuzzed` variant |
| `src/parser.rs` | Modify | Add `parse_fuzz_case()`, `lookahead_is_fuzz()`, modify `parse_top_level()` |
| `src/fuzz_checker/mod.rs` | **Create** | Orchestrator: iterate `TopLevel::Fuzzed`, dispatch to interpreter or BILD sim |
| `src/fuzz_checker/bild_sim.rs` | **Create** | BILD simulator: register machine, execute BILD instructions on `Value` types |
| `src/errors.rs` | Modify | Add `FuzzError` enum with Display impl |
| `src/lib.rs` | Modify | Add `pub mod fuzz_checker` |
| `src/typechecker.rs` | Modify | Pass 1 + Pass 2: unwrap `TopLevel::Fuzzed` like `TopLevel::Test` |
| `src/interpreter.rs` | Modify | Load + loop: unwrap `TopLevel::Fuzzed` like `TopLevel::Test` |
| `src/proof_engine.rs` | Modify | Build maps: unwrap `TopLevel::Fuzzed` for defn/txn lookups |
| `src/main.rs` | Modify | Add `"fuzz"` subcommand, hook fuzz checker into `run_check()` and compile flows |

## Open Questions

1. **Stateful inops with `(%state)`**: Should fuzz bindings allow setting initial
   state by naming fields (e.g., `state.count = 5`)? For now, state fields start
   at zero/default unless bound — the syntax could be extended later.

2. **Inops with opaque critical paths**: When the result depends on a `call` or
   `asm` instruction that the BILD simulator returns `Value::Unknown` for,
   should the fuzz case be an error or a warning? Proposal: warning-level error
   that does not block compilation.

3. **Reactive txns**: `node` has no callable interface (no params, no return).
   Fuzz on reactive txns should be a compile error ("cannot fuzz reactive
   transaction") unless we add a way to simulate a tick cycle.
