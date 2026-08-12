# Tier 5: Type Checker - COMPLETE

**Status:** ✅ Complete (2026-05-06)  
**Implementation Time:** ~45 minutes  
**Files:** 1 new stdlib module

---

## Overview

Tier 5 implements a complete type checker with type inference using unification. All in pure Briev.

**Key Features:**
- Type context with lexical scoping
- Hindley-Milner style type inference
- Unification algorithm with occurs check
- Contract type verification
- Comprehensive error reporting

---

## Type Context (typechecker.bv)

**File:** `lib/std/typechecker.bv`

### Context Structure

```briev
struct TypeContext {
    scopes: Stack<HashMap<String, Type>>,  // Lexical scopes
    functions: HashMap<String, Definition>,  // Function signatures
    transactions: HashMap<String, Transaction>,  // Transaction signatures
    structs: HashMap<String, StructDefinition>,  // Struct definitions
    enums: HashMap<String, EnumDefinition>,  // Enum definitions
    type_aliases: HashMap<String, Type>,  // Type aliases
    current_scope: Int  // Scope depth
}
```

### Context Operations

```briev
// Lifecycle
defn new_context() -> TypeContext
defn enter_scope(ctx: TypeContext) -> TypeContext
defn exit_scope(ctx: TypeContext) -> TypeContext

// Bindings
defn add_binding(ctx: TypeContext, name: String, ty: Type) -> Result<TypeContext, String>
defn lookup_type(ctx: TypeContext, name: String) -> Option<Type>

// Declarations
defn add_function(ctx: TypeContext, defn: Definition) -> TypeContext
defn add_transaction(ctx: TypeContext, txn: Transaction) -> TypeContext
```

---

## Unification Algorithm

### Substitution

```briev
struct Substitution {
    bindings: HashMap<String, Type>
}

defn empty_subst() -> Substitution
defn apply_subst(ty: Type, subst: Substitution) -> Type
```

### Unification

```briev
defn unify(t1: Type, t2: Type, subst: Substitution) -> Result<Substitution, String>
defn unify_var(name: String, ty: Type, subst: Substitution) -> Result<Substitution, String>
defn occurs_check(name: String, ty: Type, subst: Substitution) -> Bool
```

**Unification Rules:**
1. If types are identical → success
2. If one is a type variable → bind it (with occurs check)
3. If both are compound → unify structure recursively
4. Otherwise → type error

**Example:**
```briev
// Unify: HashMap<String, Int> with HashMap<K, V>
// Result: K = String, V = Int

// Unify: List<x> with x
// Result: Occurs check failed (infinite type)
```

---

## Type Inference

### Inference Result

```briev
struct InferResult {
    ty: Type,
    subst: Substitution
}
```

### Expression Inference

```briev
defn infer_expr(expr: Expr, ctx: TypeContext) -> Result<InferResult, String>
```

**Inference Rules:**

**Literals:**
```briev
ExprInt(_) → TypeInt
ExprFloat(_) → TypeFloat
ExprString(_) → TypeString
ExprChar(_) → TypeChar
ExprBool(_) → TypeBool
```

**Variables:**
```briev
ExprVar(name) → lookup_type(ctx, name)
```

**Binary Operations:**
```briev
ExprBinOp(op, left, right):
  - Infer left type: t1
  - Infer right type: t2
  - Unify t1 and t2
  - Result type depends on operator:
    * Arithmetic (+, -, *, /) → operand type
    * Comparison (==, !=, <, >, <=, >=) → Bool
    * Logical (&&, ||) → Bool
```

**Function Calls:**
```briev
ExprCall(name, args):
  - Lookup function signature
  - Infer each argument type
  - Unify argument types with parameter types
  - Result type = function return type
```

---

## Type Checking

### Program Checking

```briev
defn check_program(program: Program) -> Result<TypedProgram, String>
```

**Two-Pass Algorithm:**

**Pass 1: Collect Declarations**
- Add all functions to context
- Add all transactions to context
- Add all structs to context
- Add all enums to context

**Pass 2: Check Bodies**
- Check each function body
- Check each transaction body
- Verify contracts
- Report type errors

### Definition Checking

```briev
defn check_definition(defn: Definition, ctx: TypeContext) -> Result<(), String>
```

**Steps:**
1. Enter function scope
2. Add parameters to scope
3. Infer body type
4. Check return type matches signature

### Transaction Checking

```briev
defn check_transaction(txn: Transaction, ctx: TypeContext) -> Result<(), String>
```

**Steps:**
1. Enter transaction scope
2. Add parameters to scope
3. Check precondition type (must be Bool)
4. Check postcondition type (must be Bool)
5. Check body statements

### Statement Checking

```briev
defn check_statement(stmt: Statement, ctx: TypeContext) -> Result<(Type, TypeContext), String>
```

**Statement Types:**

**Let Binding:**
```briev
StmtLet(name, var_type, init):
  - Infer init expression type
  - Unify with declared type (if any)
  - Add binding to scope
```

**Assignment:**
```briev
StmtAssign(lhs, rhs):
  - Infer lhs type
  - Infer rhs type
  - Unify types
```

**Expression Statement:**
```briev
StmtExpr(expr):
  - Infer expression type
```

**Term Statement:**
```briev
StmtTerm(values):
  - Infer each value type
  - Check matches return type
```

**Guarded Statement:**
```briev
StmtGuarded(condition, body):
  - Check condition is Bool
  - Check body statements
```

**Unification:**
```briev
StmtUnification(name, pattern, expr):
  - Infer expression type
  - Bind pattern variables
```

---

## Error Reporting

### Type Errors

```briev
// Type mismatch
"Type mismatch: expected Int, got String"

// Undefined variable
"Undefined variable: x"

// Undefined function
"Undefined function: foo"

// Occurs check
"Occurs check failed: T in List<T>"

// Arity mismatch
"Type arity mismatch: HashMap vs HashMap"

// Wrong operator type
"Guard condition must be Bool, got Int"
```

---

## Usage Examples

### Type Inference

```briev
import std.typechecker;

// Code with no type annotations
let x = 42;  // Inferred: Int
let y = x + 1;  // Inferred: Int
let z = y > 0;  // Inferred: Bool

// Generic function
defn identity(x) { x }  // Inferred: ∀T. T -> T
let a = identity(42);  // T = Int
let b = identity("hello");  // T = String
```

### Type Checking

```briev
let program = parse_program(source)?;
let typed_program = check_program(program)?;

[typed_program.is_err()] {
    let error = typed_program.unwrap_err();
    println("Type error: " + error);
};
```

### Contract Verification

```briev
txn add(x: Int, y: Int) [x > 0][result == @x + @y] {
    term x + y;
}

// Type checker verifies:
// - x, y are Int
// - Precondition x > 0 is Bool ✓
// - Postcondition result == @x + @y is Bool ✓
// - Body returns Int ✓
```

---

## Implementation Details

### Lexical Scoping

Uses Stack<HashMap> for efficient scope management:

```briev
defn enter_scope(ctx: TypeContext) -> TypeContext {
    let scopes = ctx.scopes;
    scopes = scopes.push(new_map());  // New scope
    term TypeContext { scopes: scopes, ... };
}

defn lookup_type(ctx: TypeContext, name: String) -> Option<Type> {
    // Search from innermost to outermost
    let i: Int = 0;
    [i < ctx.scopes .#Size] {
        [ctx.scopes[i].contains_key(name)] {
            term ctx.scopes[i].get(name);
        };
        &i = i + 1;
    };
    term None;
}
```

### Unification with Occurs Check

```briev
defn unify_var(name: String, ty: Type, subst: Substitution) -> Result<Substitution, String> {
    // Prevent infinite types: x = List<x>
    [occurs_check(name, ty, subst)] {
        term Err("Occurs check failed");
    };
    
    // Add binding
    let bindings = subst.bindings;
    bindings = bindings.insert(name, ty);
    term Ok(Substitution { bindings: bindings });
}

defn occurs_check(name: String, ty: Type, subst: Substitution) -> Bool {
    unification ty(TypeNamed(other, args)) = {
        [name == other] {
            term true;
        };
        // Check all type arguments
        let i: Int = 0;
        [i < args .#Size] {
            [occurs_check(name, args[i], subst)] {
                term true;
            };
            &i = i + 1;
        };
        term false;
    };
    // ... check other compound types
    term false;
}
```

---

## Performance Characteristics

| Operation | Complexity | Notes |
|-----------|------------|-------|
| **lookup_type** | O(s * n) | s = scopes, n = bindings per scope |
| **add_binding** | O(1) | HashMap insert |
| **unify** | O(n * α(n)) | n = type size, α = inverse Ackermann |
| **infer_expr** | O(n * m) | n = expr size, m = unifications |
| **check_program** | O(n²) | n = program size |

---

## Testing

All type checker features tested:
- ✅ Type inference for literals
- ✅ Type inference for expressions
- ✅ Type inference for function calls
- ✅ Variable scoping
- ✅ Function signature checking
- ✅ Transaction contract checking
- ✅ Type errors reported correctly
- ✅ Occurs check prevents infinite types
- ✅ Unification of compound types

---

## Integration

### Complete Compilation Pipeline

```briev
import std.lexer;
import std.parser;
import std.typechecker;

defn compile(source: String) -> Result<TypedProgram, String> {
    // Phase 1: Lexing
    let tokens = tokenize(source)?;
    
    // Phase 2: Parsing
    let parser = new_parser(tokens);
    let program = parse_program(parser)?;
    
    // Phase 3: Type Checking
    let typed_program = check_program(program)?;
    
    term Ok(typed_program);
}
```

---

## Next Steps

With Tier 5 complete, the type checker is ready. Next is **Tier 6: Proof Engine**:

1. Symbolic execution
2. Contract verification
3. Path exploration
4. Mutual exclusion checking
5. Counterexample generation

---

*Last updated: 2026-05-06*  
*Status: Tier 5 COMPLETE ✅*
