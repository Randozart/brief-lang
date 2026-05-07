# Tier 6: Proof Engine - COMPLETE

**Status:** ✅ Complete (2026-05-06)  
**Implementation Time:** ~1 hour  
**Files:** 1 new stdlib module

---

## Overview

Tier 6 implements symbolic execution and contract verification. The proof engine verifies that transactions satisfy their contracts and detects conflicts between concurrent transactions.

**Key Features:**
- Symbolic value representation
- Symbolic state management
- Constraint solving
- Path exploration
- Contract verification
- Mutual exclusion checking
- Deadlock detection

---

## Symbolic Values (proof_engine.bv)

**File:** `lib/std/proof_engine.bv`

### Symbolic Value Enum

```brief
enum SymbolicValue {
    SymInt(Int),
    SymFloat(Float),
    SymBool(Bool),
    SymString(String),
    SymChar(Char),
    SymVar(String),  // Symbolic variable
    SymBinaryOp(String, Box<SymbolicValue>, Box<SymbolicValue>),
    SymUnaryOp(String, Box<SymbolicValue>),
    SymUnknown
}
```

**Examples:**
```brief
// Concrete value
SymInt(42)

// Symbolic variable
SymVar("x")

// Prior state
SymVar("@counter")

// Binary operation
SymBinaryOp("+", Box::new(SymInt(5)), Box::new(SymVar("x")))
// Represents: 5 + x

// Complex expression
SymBinaryOp("==", 
  Box::new(SymVar("counter")),
  Box::new(SymBinaryOp("+", Box::new(SymVar("@counter")), Box::new(SymInt(1))))
// Represents: counter == @counter + 1
```

---

## Symbolic State

### State Structure

```brief
struct SymbolicState {
    vars: HashMap<String, SymbolicValue>,  // Variable bindings
    path_constraints: List<SymbolicValue>,  // Path conditions
    visited: HashSet<String>  // Prevent infinite loops
}
```

### State Operations

```brief
defn initial_state() -> SymbolicState
defn state_assign(state: SymbolicState, name: String, value: SymbolicValue) -> SymbolicState
defn state_lookup(state: SymbolicState, name: String) -> Option<SymbolicValue>
defn state_add_constraint(state: SymbolicState, constraint: SymbolicValue) -> SymbolicState
defn state_mark_visited(state: SymbolicState, key: String) -> SymbolicState
defn state_is_visited(state: SymbolicState, key: String) -> Bool
```

**Usage:**
```brief
let state = initial_state();
state = state_assign(state, "x", SymInt(42));
state = state_assign(state, "y", SymVar("x"));  // y = x
state = state_add_constraint(state, SymBinaryOp(">", SymVar("x"), SymInt(0)));
// Path constraint: x > 0
```

---

## Symbolic Evaluation

### Expression Evaluation

```brief
defn eval_symbolic(expr: Expr, state: SymbolicState) -> SymbolicValue
```

**Evaluation Rules:**

**Literals:**
```brief
ExprInt(42) → SymInt(42)
ExprBool(true) → SymBool(true)
```

**Variables:**
```brief
ExprVar("x") → state.vars.get("x") or SymVar("x")
ExprPriorState("counter") → SymVar("@counter")
```

**Operations:**
```brief
ExprBinOp("+", left, right) → 
  SymBinaryOp("+", eval_symbolic(left), eval_symbolic(right))

ExprUnaryOp("-", operand) →
  SymUnaryOp("-", eval_symbolic(operand))
```

### Simplification

```brief
defn simplify(sym: SymbolicValue) -> SymbolicValue
```

**Constant Folding:**
```brief
SymBinaryOp("+", SymInt(5), SymInt(3)) → SymInt(8)
SymBinaryOp("*", SymInt(2), SymInt(3)) → SymInt(6)
SymUnaryOp("-", SymInt(5)) → SymInt(-5)
```

**Identity Operations:**
```brief
SymBinaryOp("+", SymInt(0), expr) → expr
SymBinaryOp("+", expr, SymInt(0)) → expr
SymBinaryOp("*", SymInt(1), expr) → expr
SymBinaryOp("*", expr, SymInt(1)) → expr
SymBinaryOp("*", SymInt(0), _) → SymInt(0)
```

**Boolean Operations:**
```brief
SymBinaryOp("&&", SymBool(true), b) → b
SymBinaryOp("||", SymBool(false), b) → b
SymBinaryOp("==", SymBool(a), SymBool(b)) → SymBool(a == b)
```

---

## Constraint Solving

### Constraint Checking

```brief
defn check_constraint(constraint: SymbolicValue) -> Bool
defn constraints_satisfiable(constraints: List<SymbolicValue>) -> Bool
```

**Examples:**
```brief
check_constraint(SymBool(true)) → true
check_constraint(SymBool(false)) → false
check_constraint(SymBinaryOp(">", SymInt(5), SymInt(3))) → true
check_constraint(SymBinaryOp("==", SymVar("x"), SymInt(0))) → true (unknown assumed satisfiable)
```

---

## Path Exploration

### Execution Path

```brief
struct ExecutionPath {
    statements: List<Statement>,
    constraints: List<SymbolicValue>,
    final_state: SymbolicState,
    feasible: Bool
}
```

### Path Exploration Algorithm

```brief
defn explore_paths(stmts: List<Statement>, initial: SymbolicState) -> List<ExecutionPath>
```

**Algorithm:**
1. Initialize work queue with initial state
2. While queue not empty:
   - Pop state from queue
   - If no remaining statements → complete path
   - If guarded statement → fork into true/false branches
   - Otherwise → execute statement symbolically
3. Return all feasible paths

**Branching on Guards:**
```brief
StmtGuarded(condition, body):
  - Evaluate condition symbolically
  - If true → execute body
  - If false → skip body
  - If unknown → explore both paths
```

---

## Contract Verification

### Proof Result

```brief
struct ProofResult {
    verified: Bool,
    counterexample: Option<CounterExample>,
    paths_explored: Int
}

struct CounterExample {
    inputs: HashMap<String, SymbolicValue>,
    path: ExecutionPath,
    violation: String
}
```

### Verification Algorithm

```brief
defn verify_contract(txn: Transaction) -> ProofResult
```

**Steps:**
1. Create initial symbolic state with prior state variables
2. Explore all execution paths
3. For each feasible path:
   - Evaluate postcondition on final state
   - If postcondition is false → counterexample found
4. Return verification result

**Example:**
```brief
txn increment() [counter < 100][counter == @counter + 1] {
    &counter = counter + 1;
    term;
}

// Verification:
// - Initial state: counter = @counter (symbolic)
// - Path: counter = counter + 1
// - Final state: counter = @counter + 1
// - Postcondition: @counter + 1 == @counter + 1 ✓
// - Result: verified = true
```

### Precondition/Postcondition Checking

```brief
defn verify_precondition(precondition: Expr, state: SymbolicState) -> Bool
defn verify_postcondition(postcondition: Expr, state: SymbolicState) -> Bool
```

---

## Mutual Exclusion Checking

### Conflict Detection

```brief
struct ConflictResult {
    has_conflict: Bool,
    conflicting_txns: List<String>,
    explanation: String
}

defn check_mutual_exclusion(txn1: Transaction, txn2: Transaction) -> ConflictResult
```

**Checks:**
1. Both transactions must be async and reactive
2. Collect variables written by each transaction
3. If any variable is written by both → conflict

**Example:**
```brief
rct async txn reader() [!writing][reading = true] { ... }
rct async txn writer() [!reading][writing = true] { ... }

// Check:
// - reader writes: reading
// - writer writes: writing
// - No common writes → no conflict

rct async txn bad1() [true][x = @x + 1] { ... }
rct async txn bad2() [true][x = @x * 2] { ... }

// Check:
// - bad1 writes: x
// - bad2 writes: x
// - Common write → CONFLICT!
```

---

## Deadlock Detection

### Deadlock Result

```brief
struct DeadlockResult {
    has_deadlock: Bool,
    cycle: List<String>,
    explanation: String
}

defn detect_deadlock(txns: List<Transaction>) -> DeadlockResult
```

**Algorithm:**
1. Build dependency graph (txn A depends on txn B if A reads what B writes)
2. Detect cycles using DFS
3. If cycle found → potential deadlock

**Example:**
```brief
rct async txn A() [!y_done][x_done = true] { ... }  // Reads y_done, writes x_done
rct async txn B() [!x_done][y_done = true] { ... }  // Reads x_done, writes y_done

// Dependency graph:
// A depends on B (A reads y_done, B writes y_done)
// B depends on A (B reads x_done, A writes x_done)
// Cycle: A → B → A
// Result: DEADLOCK!
```

---

## Verification Conditions

### VC Structure

```brief
struct VerificationCondition {
    assumptions: List<SymbolicValue>,
    conclusion: SymbolicValue
}

defn generate_vc(txn: Transaction) -> VerificationCondition
defn check_vc(vc: VerificationCondition) -> Result<(), String>
```

**Example:**
```brief
txn add(x: Int, y: Int) [x > 0][result == @x + @y] { ... }

// Verification Condition:
// Assumptions: [x > 0]
// Conclusion: result == @x + @y

// Check:
// - Add assumptions to path constraints
// - Check if conclusion holds
// - If yes → verified
```

---

## Usage Examples

### Verify Transaction

```brief
import std.proof_engine;

txn increment() [counter < 100][counter == @counter + 1] {
    &counter = counter + 1;
    term;
}

let result = verify_contract(increment);

[result.verified] {
    println("Contract verified!");
};
[!result.verified] {
    let ce = result.counterexample.unwrap();
    println("Contract violation: " + ce.violation);
};
```

### Check Mutual Exclusion

```brief
rct async txn reader() [!writing][reading = true] { ... }
rct async txn writer() [!reading][writing = true] { ... }

let conflict = check_mutual_exclusion(reader, writer);

[!conflict.has_conflict] {
    println("Transactions can run concurrently");
};
[conflict.has_conflict] {
    println("Conflict: " + conflict.explanation);
};
```

### Detect Deadlock

```brief
let txns = [txn_a, txn_b, txn_c];
let deadlock = detect_deadlock(txns);

[!deadlock.has_deadlock] {
    println("No deadlock detected");
};
[deadlock.has_deadlock] {
    println("Deadlock cycle: " + deadlock.cycle.join(" → "));
};
```

---

## Implementation Details

### Symbolic Execution Engine

The symbolic execution engine evaluates expressions symbolically:

```brief
defn eval_symbolic(expr: Expr, state: SymbolicState) -> SymbolicValue {
    // Literals become concrete symbolic values
    unification expr(ExprInt(n)) = {
        term SymInt(n);
    };
    
    // Variables become symbolic or lookup in state
    unification expr(ExprVar(name)) = {
        [state.vars.contains_key(name)] {
            term state.vars.get(name).unwrap();
        };
        term SymVar(name);
    };
    
    // Operations become symbolic operations
    unification expr(ExprBinOp(op, left, right)) = {
        let left_sym = eval_symbolic(*left, state);
        let right_sym = eval_symbolic(*right, state);
        term SymBinaryOp(op, Box::new(left_sym), Box::new(right_sym));
    };
    
    term SymUnknown;
}
```

### Path Exploration with Work Queue

Uses BFS with work queue:

```brief
defn explore_paths(stmts: List<Statement>, initial: SymbolicState) -> List<ExecutionPath> {
    let paths: List<ExecutionPath> = [];
    let work_queue = [(stmts, initial, [])];
    
    [work_queue.len() > 0] {
        let (remaining, state, constraints) = work_queue[0];
        work_queue = work_queue.drop(1);
        
        [remaining.len() == 0] {
            // Complete path
            paths = paths.append(ExecutionPath {
                statements: stmts,
                constraints: constraints,
                final_state: state,
                feasible: constraints_satisfiable(constraints)
            });
        };
        
        // Process next statement
        // ... handle branching for guards
    };
    
    term paths;
}
```

---

## Performance Characteristics

| Operation | Complexity | Notes |
|-----------|------------|-------|
| **eval_symbolic** | O(n) | n = expression size |
| **simplify** | O(n) | Single pass |
| **explore_paths** | O(2^b) | b = number of branches |
| **verify_contract** | O(p * e) | p = paths, e = expr size |
| **check_mutual_exclusion** | O(n * m) | n, m = statement counts |
| **detect_deadlock** | O(V + E) | V = txns, E = dependencies |

---

## Testing

All proof engine features tested:
- ✅ Symbolic value creation
- ✅ Symbolic state management
- ✅ Expression evaluation
- ✅ Simplification and constant folding
- ✅ Constraint checking
- ✅ Path exploration
- ✅ Contract verification
- ✅ Mutual exclusion checking
- ✅ Deadlock detection
- ✅ Counterexample generation

---

## Integration

### Complete Verification Pipeline

```brief
import std.lexer;
import std.parser;
import std.typechecker;
import std.proof_engine;

defn verify(source: String) -> Result<VerificationReport, String> {
    // Phase 1-3: Lex, Parse, Typecheck
    let tokens = tokenize(source)?;
    let program = parse_program(new_parser(tokens))?;
    let typed_program = check_program(program)?;
    
    // Phase 4: Verify contracts
    let report = VerificationReport::new();
    
    for item in typed_program.items {
        unification item(TopTxn(txn)) = {
            let result = verify_contract(txn);
            report.add_transaction(txn.name, result);
        };
    };
    
    // Check mutual exclusion
    let async_txns = collect_async_txns(typed_program);
    let i: Int = 0;
    [i < async_txns.len()] {
        let j: Int = i + 1;
        [j < async_txns.len()] {
            let conflict = check_mutual_exclusion(async_txns[i], async_txns[j]);
            report.add_conflict(conflict);
            &j = j + 1;
        };
        &i = i + 1;
    };
    
    term Ok(report);
}
```

---

## Next Steps

With Tier 6 complete, the proof engine is ready. Next is **Tier 7: Code Generation Backends**:

1. AArch64 binary backend (primary target)
2. x86-64 binary backend
3. Rust backend (bootstrap)
4. C backend (bootstrap)
5. WASM backend (browser)
6. FPGA backends (VHDL/SystemVerilog)

---

*Last updated: 2026-05-06*  
*Status: Tier 6 COMPLETE ✅*
