# Brief Compiler - CS Optimizations

**Last Updated:** 2026-05-06  
**Status:** Active document - updated with each tier

---

## Overview

This document tracks algorithmic optimizations and smart design decisions that give Brief favorable complexity characteristics.

---

## Tier 1: Core Data Types

### HashMap<K,V> - O(1) Operations

**Optimization:** Hash-based lookup instead of linear search

```brief
// BEFORE (if using List): O(n) lookup
defn lookup_linear(list: List<(String, Type)>, name: String) -> Option<Type> {
    let i: Int = 0;
    [i < list.len()] {
        [list[i].0 == name] {
            term Some(list[i].1);
        };
        &i = i + 1;
    };
    term None;
}

// AFTER (using HashMap): O(1) lookup
defn lookup_map(map: HashMap<String, Type>, name: String) -> Option<Type> {
    term map.get(name);  // O(1) average
}
```

**Impact:** Symbol table lookups in type checker: O(n) → O(1)

---

### HashSet<T> - O(1) Membership

**Optimization:** Hash-based set instead of linear search

```brief
// BEFORE: O(n) contains check
defn contains_linear(list: List<String>, item: String) -> Bool {
    let i: Int = 0;
    [i < list.len()] {
        [list[i] == item] {
            term true;
        };
        &i = i + 1;
    };
    term false;
}

// AFTER: O(1) contains check
defn contains_set(set: HashSet<String>, item: String) -> Bool {
    term set.contains(item);  // O(1) average
}
```

**Impact:** Deadlock detection, dependency tracking: O(n²) → O(n)

---

### StringBuilder - O(1) Append

**Optimization:** Amortized O(1) append vs O(n) string concatenation

```brief
// BEFORE: O(n²) for building string of length n
let s = "";
let i: Int = 0;
[i < n] {
    s = s + "x";  // O(n) - creates new string each time
    &i = i + 1;
};
// Total: O(1 + 2 + 3 + ... + n) = O(n²)

// AFTER: O(n) for building string of length n
let sb = new_builder();
let i: Int = 0;
[i < n] {
    sb = sb.append_char('x');  // O(1) amortized
    &i = i + 1;
};
let s = sb.to_string();  // O(n)
// Total: O(n)
```

**Impact:** Lexer token building, error message formatting: O(n²) → O(n)

---

### Stack/Queue - O(1) Push/Pop

**Optimization:** Native Vec/VecDeque operations

```brief
// Stack: O(1) push/pop
defn stack_push<T>(stack: Stack<T>, item: T) -> Stack<T> {
    term stack.push(item);  // Vec push_back: O(1)
}

// Queue: O(1) enqueue/dequeue
defn queue_enqueue<T>(queue: Queue<T>, item: T) -> Queue<T> {
    term queue.push_back(item);  // VecDeque: O(1)
}
```

**Impact:** Parser state management, BFS/DFS: O(n) per op → O(1) per op

---

## Tier 2: String & Text Processing

### Character Classification - O(1)

**Optimization:** Direct codepoint comparison vs table lookup

```brief
// O(1) - single comparison
defn is_digit(c: Char) -> Bool {
    term c >= '0' && c <= '9';  // 2 comparisons
}

// Alternative O(1) but slower
defn is_digit_slow(c: Char) -> Bool {
    term c == '0' || c == '1' || ... || c == '9';  // 10 comparisons
}
```

**Impact:** Every character check in lexer: 1-2 cycles vs 10+ cycles

---

### Case Conversion - O(1) Arithmetic

**Optimization:** ASCII offset arithmetic vs table lookup

```brief
// O(1) - single arithmetic operation
defn to_upper(c: Char) -> Char {
    [is_lower(c)] {
        term int_to_char(char_to_int(c) - 32);  // ASCII offset
    };
    term c;
}
```

**Impact:** Keyword matching, identifier comparison: O(1) with no memory access

---

## Tier 3: Lexer Components

### Single-Pass Lexing - O(n)

**Optimization:** Single pass over source, no backtracking

```brief
defn tokenize(source: String) -> List<Token> {
    let tokens = [];
    let i: Int = 0;
    [i < source.len()] {
        // Classify current character - O(1)
        // Read token - O(token_length)
        // Advance position - O(1)
        &i = i + token_length;
    };
    term tokens;
}
// Total: O(n) where n = source length
```

**Impact:** Full tokenization in linear time

---

### Keyword Recognition - O(1) per Keyword

**Optimization:** Direct string comparison after identifier read

```brief
defn read_identifier(source: String, pos: Int) -> (Token, Int) {
    // Read identifier: O(k) where k = identifier length
    let (text, new_pos) = read_chars(source, pos);
    
    // Check keywords: O(1) per keyword (30 keywords = 30 comparisons max)
    [text == "let"] { term (KeywordLet, new_pos); };
    [text == "txn"] { term (KeywordTxn, new_pos); };
    // ... 28 more
    term (TokenIdentifier(text), new_pos);
}
```

**Future Optimization:** Perfect hashing for O(1) total keyword lookup
**Current:** O(k * m) where k = identifier length, m = number of keywords

---

### Operator Precedence - O(1) Lookup

**Optimization:** Direct token → precedence mapping

```brief
defn operator_precedence(tok: Token) -> Int {
    unification tok(OpStar) = 11;
    unification tok(OpSlash) = 11;
    unification tok(OpPlus) = 10;
    unification tok(OpMinus) = 10;
    // ... etc
    term 0;
}
// O(1) - single pattern match
```

**Impact:** Parser precedence handling: O(1) per operator

---

## Tier 4: Parser Components

### Recursive Descent - O(n)

**Optimization:** Single pass, no backtracking (LL(1) grammar)

```brief
defn parse_expression(tokens: List<Token>) -> Expr {
    // Each token examined once: O(n)
    // Each production rule: O(1)
    // Total: O(n)
}
```

**Impact:** Full parsing in linear time

---

### Operator Precedence Parsing - O(n)

**Optimization:** Precedence climbing instead of shunting-yard

```brief
// No intermediate data structures
// No operator stack
// Direct recursive calls based on precedence
defn parse_expr(precedence: Int) -> Expr {
    let left = parse_primary();
    
    [current_precedence() >= precedence] {
        let op = current_operator();
        advance();
        let right = parse_expr(precedence + 1);
        left = make_binop(op, left, right);
    };
    
    term left;
}
```

**Impact:** Expression parsing: O(n) with minimal allocations

---

### AST Construction - O(n)

**Optimization:** Direct AST building during parse (no intermediate representation)

```brief
// Single pass: source → tokens → AST
// No parse tree → AST transformation needed
defn compile(source: String) -> AST {
    let tokens = tokenize(source);  // O(n)
    let ast = parse(tokens);  // O(n)
    term ast;  // Total: O(n)
}
```

**Impact:** Memory usage: O(n) total, not O(2n) or O(3n)

---

## Tier 5: Type Checker

### Lexical Scoping - O(d) Lookup

**Optimization:** Stack of HashMaps, search from innermost scope

```brief
defn lookup_type(ctx: TypeContext, name: String) -> Option<Type> {
    // Search from innermost to outermost
    let i: Int = ctx.current_scope;
    [i >= 0] {
        [ctx.scopes[i].contains_key(name)] {
            term ctx.scopes[i].get(name);  // O(1) HashMap lookup
        };
        &i = i - 1;
    };
    term None;
}
// O(d) where d = scope depth (typically 1-5)
```

**Impact:** Variable lookup: O(d) instead of O(n) for flat namespace

---

### Type Unification - O(n · α(n))

**Optimization:** Union-Find with path compression (nearly O(1))

```brief
defn unify(t1: Type, t2: Type, subst: Substitution) -> Substitution {
    // Apply substitutions: O(α(n)) with path compression
    // Bind variables: O(1) HashMap insert
    // Occurs check: O(n) worst case
    // Total: O(n · α(n)) where α = inverse Ackermann function
}
```

**Impact:** Type inference: nearly linear instead of quadratic

---

### Two-Pass Checking - O(n) Total

**Optimization:** Separate declaration collection from checking

```brief
// Pass 1: Collect all declarations - O(n)
for item in program.items {
    add_to_context(item);  // O(1) HashMap insert
}

// Pass 2: Check all bodies - O(n)
for item in program.items {
    check_item(item);  // O(item_size)
}

// Total: O(n) instead of O(n²) for naive forward references
```

**Impact:** Forward references resolved in O(1), not O(n) per reference

---

## Tier 6: Proof Engine

### Symbolic Evaluation - O(n)

**Optimization:** Single pass over expression tree

```brief
defn eval_symbolic(expr: Expr, state: SymbolicState) -> SymbolicValue {
    // Visit each node once: O(n)
    // Each node: O(1) pattern match + HashMap lookup
    // Total: O(n)
}
```

---

### Constant Folding - O(n) with Optimization

**Optimization:** Simplify during evaluation, not as separate pass

```brief
defn eval_and_simplify(expr: Expr, state: SymbolicState) -> SymbolicValue {
    unification expr(ExprBinOp("+", left, right)) = {
        let left_val = eval_and_simplify(*left, state);
        let right_val = eval_and_simplify(*right, state);
        
        // Fold constants immediately
        unification (left_val, right_val) = (SymInt(l), SymInt(r)) = {
            term SymInt(l + r);  // O(1)
        };
        
        term SymBinaryOp("+", Box::new(left_val), Box::new(right_val));
    };
}
```

**Impact:** No separate simplification pass needed

---

### Path Exploration - O(2^b) with Pruning

**Optimization:** BFS with feasibility checking

```brief
defn explore_paths(stmts: List<Statement>, state: SymbolicState) -> List<ExecutionPath> {
    let queue = [(stmts, state, [])];
    let paths = [];
    
    [queue.len() > 0] {
        let (remaining, state, constraints) = queue[0];
        queue = queue.drop(1);
        
        // Prune infeasible paths early
        [!constraints_satisfiable(constraints)] {
            continue;  // Don't explore this path
        };
        
        // ... process statement
    };
    
    term paths;
}
// O(2^b) worst case, but pruning reduces average case significantly
```

**Impact:** Infeasible paths pruned early, reducing exploration

---

### Mutual Exclusion - O(n · m)

**Optimization:** Collect writes once, compare sets

```brief
defn check_mutual_exclusion(txn1, txn2) -> ConflictResult {
    let writes1 = collect_writes(txn1.body);  // O(n)
    let writes2 = collect_writes(txn2.body);  // O(m)
    
    // Compare using HashSet: O(n + m)
    for var in writes1 {
        [writes2.contains(var)] {
            term ConflictResult { has_conflict: true, ... };
        };
    };
    
    term ConflictResult { has_conflict: false, ... };
}
// Total: O(n + m) instead of O(n · m) for nested iteration
```

---

### Deadlock Detection - O(V + E)

**Optimization:** DFS-based cycle detection

```brief
defn detect_deadlock(txns: List<Transaction>) -> DeadlockResult {
    // Build dependency graph: O(V + E)
    let deps = build_graph(txns);
    
    // DFS cycle detection: O(V + E)
    for txn in txns {
        [!visited.contains(txn)] {
            let (has_cycle, _) = dfs_cycle(txn, deps, ...);
            [has_cycle] {
                term DeadlockResult { has_deadlock: true, ... };
            };
        };
    };
    
    term DeadlockResult { has_deadlock: false, ... };
}
// Total: O(V + E) where V = transactions, E = dependencies
```

**Impact:** Linear in graph size, not exponential

---

## Tier 7: Code Generation (Planned)

### Register Allocation - O(n)

**Planned Optimization:** Linear scan instead of graph coloring

```brief
// Graph coloring: O(n²) or worse
// Linear scan: O(n)
defn allocate_registers(instrs: List<Instruction>) -> List<Instruction> {
    let live_intervals = compute_live_intervals(instrs);  // O(n)
    let registers = [];
    
    // Sort by start position: O(n log n)
    live_intervals = sort(live_intervals);
    
    // Linear scan: O(n)
    for interval in live_intervals {
        expire_old_intervals(interval.start);
        [free_regs.len() > 0] {
            allocate_free_reg(interval);
        };
        [free_regs.len() == 0] {
            spill_at_interval(interval);
        };
    };
    
    term instrs_with_regs;
}
// Total: O(n log n) instead of O(n²)
```

---

### Instruction Selection - O(n)

**Planned Optimization:** Tree pattern matching with maximal munch

```brief
defn select_instructions(expr: Expr) -> List<Instruction> {
    // Maximal munch: greedily match largest pattern
    // O(n) - single pass over expression tree
    unification expr(ExprBinOp("+", left, right)) = {
        // Check for specialized patterns
        unification (*left, *right) = (ExprInt(_), ExprInt(_)) = {
            term [emit_add_imm(left, right)];  // Specialized: ADDI
        };
        term [emit_add_reg(left, right)];  // General: ADD
    };
}
```

---

### Peephole Optimization - O(n)

**Planned Optimization:** Single pass with sliding window

```brief
defn peephole_optimize(instrs: List<Instruction>) -> List<Instruction> {
    let result = [];
    let i: Int = 0;
    
    [i < instrs.len()] {
        // Check 2-instruction patterns
        [i + 1 < instrs.len()] {
            [is_redundant_load_store(instrs[i], instrs[i+1])] {
                // Eliminate redundant load/store
                &i = i + 2;
                continue;
            };
            [is_constant_fold(instrs[i], instrs[i+1])] {
                // Fold constants
                result = result.append(fold(instrs[i], instrs[i+1]));
                &i = i + 2;
                continue;
            };
        };
        
        result = result.append(instrs[i]);
        &i = i + 1;
    };
    
    term result;
}
// Total: O(n) with window size k = O(1)
```

---

## Summary Table

| Tier | Component | Optimization | Before | After |
|------|-----------|--------------|--------|-------|
| 1 | HashMap | Hash lookup | O(n) | O(1) |
| 1 | HashSet | Hash membership | O(n) | O(1) |
| 1 | StringBuilder | Amortized append | O(n²) | O(n) |
| 2 | Char classification | Direct comparison | O(n) table | O(1) |
| 2 | Case conversion | ASCII arithmetic | O(1) table | O(1) math |
| 3 | Lexing | Single pass | O(n²) | O(n) |
| 3 | Keywords | Direct comparison | O(k·m) hash | O(k·m) direct |
| 4 | Parsing | Recursive descent | O(n²) | O(n) |
| 4 | Precedence | Climbing algorithm | O(n²) stack | O(n) recursive |
| 5 | Scoping | Stack of HashMaps | O(n) flat | O(d) scoped |
| 5 | Unification | Union-Find | O(n²) | O(n·α(n)) |
| 5 | Two-pass | Declaration first | O(n²) | O(n) |
| 6 | Symbolic eval | Single pass | O(n²) | O(n) |
| 6 | Path exploration | BFS pruning | O(2^n) | O(2^b) pruned |
| 6 | Deadlock detection | DFS cycle | O(n!) | O(V+E) |
| 7 | Register alloc | Linear scan | O(n²) | O(n log n) |
| 7 | Instruction select | Maximal munch | O(n·p) | O(n) |
| 7 | Peephole | Sliding window | O(n²) | O(n) |

---

## Future Optimizations

### Tier 7+ Planned

1. **SSA Form** - O(n) for many optimizations
2. **Common Subexpression Elimination** - O(n) with value numbering
3. **Loop Invariant Code Motion** - O(n) with dominator trees
4. **Inline Expansion** - O(n) with call graph analysis

### Tier 8+ Research

1. **Profile-Guided Optimization** - Runtime feedback
2. **Whole-Program Optimization** - Link-time optimization
3. **Parallel Code Generation** - Multi-core backend

---

*This document is updated with each tier completion.*
