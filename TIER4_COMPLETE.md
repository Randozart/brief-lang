# Tier 4: Parser Components - COMPLETE

**Status:** ✅ Complete (2026-05-06)  
**Implementation Time:** ~1 hour  
**Files:** 2 new stdlib modules

---

## Overview

Tier 4 implements the complete AST definition and recursive descent parser for Brief in pure Brief.

**Components:**
1. **AST Definition** (`ast.bv`) - Complete AST type hierarchy
2. **Parser Implementation** (`parser.bv`) - Recursive descent parser

---

## 4.1 AST Definition (ast.bv)

**File:** `lib/std/ast.bv`

### Expression Types

**Literals:**
- `ExprInt(Int)` - Integer literals
- `ExprFloat(Float)` - Float literals
- `ExprString(String)` - String literals
- `ExprChar(Char)` - Char literals
- `ExprBool(Bool)` - Boolean literals

**Variables:**
- `ExprVar(String)` - Variable references
- `ExprPriorState(String)` - Prior state (@var)

**Operations:**
- `ExprBinOp(String, Box<Expr>, Box<Expr>)` - Binary operations
- `ExprUnaryOp(String, Box<Expr>)` - Unary operations

**Calls and Access:**
- `ExprCall(String, List<Expr>)` - Function calls
- `ExprFieldAccess(Box<Expr>, String)` - Field access
- `ExprIndex(Box<Expr>, Box<Expr>)` - Array indexing
- `ExprSlice(Box<Expr>, Option<Box<Expr>>, Option<Box<Expr>>)` - Slicing

**Containers:**
- `ExprList(List<Expr>)` - List literals
- `ExprTuple(List<Expr>)` - Tuple literals

**Other:**
- `ExprCast(Box<Expr>, Type)` - Type casts
- `ExprBlock(List<Statement>)` - Statement blocks

### Statement Types

- `StmtAssign(Box<Expr>, Box<Expr>)` - Assignment
- `StmtLet(String, Option<Type>, Option<Box<Expr>>)` - Let binding
- `StmtExpr(Box<Expr>)` - Expression statement
- `StmtTerm(List<Option<Box<Expr>>>)` - Return statement
- `StmtEscape(Option<Box<Expr>>)` - Escape/rollback
- `StmtGuarded(Box<Expr>, List<Statement>)` - Guarded statement
- `StmtUnification(String, String, Box<Expr>)` - Pattern unification
- `StmtAsm(String, List<String>)` - Inline assembly

### Contract Structure

```brief
struct Contract {
    precondition: Expr,
    postcondition: Expr,
    watchdog: Option<Watchdog>
}

struct Watchdog {
    condition: Expr,
    is_required: Bool  // true = !, false = ?
}
```

### Definition and Transaction

```brief
struct Definition {
    name: String,
    type_params: List<String>,
    params: List<Param>,
    output_type: Option<Type>,
    contract: Contract,
    body: List<Statement>
}

struct Transaction {
    name: String,
    is_async: Bool,
    is_reactive: Bool,
    type_params: List<String>,
    params: List<Param>,
    contract: Contract,
    body: List<Statement>
}
```

### Type System

Complete type enum with all Tier 1 types:
- Primitives: Int, UInt, Float, String, Bool, Char, Data, Void
- Collections: Vector, Option, Result, List, HashMap, HashSet, Stack, Queue
- Special: StringBuilder, Named, Tuple, Union, Sig, Constrained

### Program Structure

```brief
enum TopLevel {
    TopDefn(Definition),
    TopTxn(Transaction),
    TopSig(Signature),
    TopStruct(StructDefinition),
    TopRStruct(RStructDefinition),
    TopEnum(EnumDefinition),
    TopImport(Import),
    TopConst(Constant),
    TopState(StateDecl),
    TopRender(String, ViewBody)
}

struct Program {
    items: List<TopLevel>
}
```

### AST Utilities

```brief
defn expr_precedence(expr: Expr) -> Int
defn is_literal_expr(expr: Expr) -> Bool
defn infer_expr_type(expr: Expr) -> Option<Type>

// Expression builders
defn make_binop(op: String, left: Expr, right: Expr) -> Expr
defn make_unaryop(op: String, operand: Expr) -> Expr
defn make_var(name: String) -> Expr
defn make_int(n: Int) -> Expr
defn make_float(f: Float) -> Expr
defn make_bool(b: Bool) -> Expr
defn make_string(s: String) -> Expr
defn make_char(c: Char) -> Expr
```

---

## 4.2 Parser Implementation (parser.bv)

**File:** `lib/std/parser.bv`

### Parser State

```brief
struct ParserState {
    tokens: List<Token>,
    position: Int,
    current_token: Token
}
```

### Parser Construction

```brief
defn new_parser(tokens: List<Token>) -> ParserState
```

### Token Access

```brief
defn current_token(state: ParserState) -> Token
defn peek_token(state: ParserState, offset: Int) -> Token
defn advance(state: ParserState) -> ParserState
defn expect_token(state: ParserState, expected: Token) -> Result<ParserState, String>
defn match_token(state: ParserState, token: Token) -> (Bool, ParserState)
```

### Program Parsing

```brief
defn parse_program(state: ParserState) -> Result<Program, String>
defn parse_top_level(state: ParserState) -> Result<(TopLevel, ParserState), String>
```

**Parses:**
- `let` declarations
- `const` declarations
- `txn` transactions
- `rct txn` reactive transactions
- `defn` functions
- `sig` signatures
- `struct` definitions
- `enum` definitions
- `import` statements

### Transaction Parsing

```brief
defn parse_transaction(state: ParserState) -> Result<(Transaction, ParserState), String>
```

**Handles:**
- `rct` keyword (reactive)
- `async` keyword
- `txn` keyword
- Type parameters `<T, U>`
- Parameters `(x: Int, y: String)`
- Contracts `[pre][post]`
- Optional watchdog `?[timeout]` or `![timeout]`
- Body `{ ... }` or `;`

### Definition Parsing

```brief
defn parse_definition(state: ParserState) -> Result<(Definition, ParserState), String>
```

**Handles:**
- `defn` keyword
- Type parameters
- Parameters
- Return type `-> Type`
- Contracts
- Body

### Contract Parsing

```brief
defn parse_contract(state: ParserState) -> Result<(Contract, ParserState), String>
```

**Parses:**
- Precondition `[expression]`
- Postcondition `[expression]`
- Optional watchdog `?[condition]` or `![condition]`

### Expression Parsing

**Operator Precedence (lowest to highest):**
1. `||` (Or) - precedence 4
2. `&&` (And) - precedence 5
3. `==`, `!=` (Equality) - precedence 7
4. `<`, `>`, `<=`, `>=` (Comparison) - precedence 8
5. `+`, `-` (Additive) - precedence 10
6. `*`, `/`, `%` (Multiplicative) - precedence 11

**Implementation:**
```brief
defn parse_expression(state: ParserState) -> Result<(Expr, ParserState), String>
defn parse_or_expr(state: ParserState) -> Result<(Expr, ParserState), String>
defn parse_and_expr(state: ParserState) -> Result<(Expr, ParserState), String>
defn parse_equality_expr(state: ParserState) -> Result<(Expr, ParserState), String>
defn parse_comparison_expr(state: ParserState) -> Result<(Expr, ParserState), String>
defn parse_additive_expr(state: ParserState) -> Result<(Expr, ParserState), String>
defn parse_multiplicative_expr(state: ParserState) -> Result<(Expr, ParserState), String>
defn parse_unary_expr(state: ParserState) -> Result<(Expr, ParserState), String>
defn parse_primary_expr(state: ParserState) -> Result<(Expr, ParserState), String>
```

**Primary expressions:**
- Literals (int, float, string, char, bool)
- Variables
- Prior state (@var)
- Function calls
- Parenthesized expressions
- List literals

### Statement Parsing

```brief
defn parse_statement(state: ParserState) -> Result<(Statement, ParserState), String>
```

**Parses:**
- Let bindings: `let x: Int = 42;`
- Assignments: `x = y + 1;`
- Term: `term;` or `term expr;`
- Escape: `escape;` or `escape expr;`
- Guarded: `[condition] { ... }` or `[condition] stmt;`
- Blocks: `{ stmt1; stmt2; }`
- Expression statements: `expr;`

### Type Parsing

```brief
defn parse_type(state: ParserState) -> Result<(Type, ParserState), String>
defn parse_type_params(state: ParserState) -> Result<(List<String>, ParserState), String>
defn parse_params(state: ParserState) -> Result<(List<Param>, ParserState), String>
```

---

## Usage Examples

### Parse Complete Program

```brief
import std.lexer;
import std.parser;

let source = "
    let x: Int = 42;
    
    txn increment() [x < 100][x == @x + 1] {
        &x = x + 1;
        term;
    };
";

let tokens = tokenize(source)?;
let mut parser = new_parser(tokens);
let program = parse_program(parser)?;

// program.items contains the AST
```

### Parse Single Expression

```brief
let mut parser = new_parser(tokenize("x + y * 2")?);
let (expr, _) = parse_expression(parser)?;

// expr = ExprBinOp("+", 
//          ExprVar("x"),
//          ExprBinOp("*", ExprVar("y"), ExprInt(2)))
```

### Error Handling

```brief
let result = parse_program(new_parser(tokenize("invalid syntax")?));

[result.is_err()] {
    let error = result.unwrap_err();
    println("Parse error: " + error);
};
```

---

## Implementation Details

### Recursive Descent

The parser uses recursive descent with one function per precedence level:

```brief
defn parse_expression(state: ParserState) -> Result<(Expr, ParserState), String> {
    parse_or_expr(state)
}

defn parse_or_expr(state: ParserState) -> Result<(Expr, ParserState), String> {
    let (mut left, new_state) = parse_and_expr(state)?;
    &state = new_state;
    
    [current_token(state) == OpOr] {
        &state = advance(state);
        let (right, new_state) = parse_or_expr(state)?;
        &state = new_state;
        &left = make_binop("||", left, right);
    };
    
    term (left, state);
}
```

### Error Recovery

Currently uses immediate error reporting. Future enhancement: panic mode recovery.

```brief
defn expect_token(state: ParserState, expected: Token) -> Result<ParserState, String> {
    [current_token(state) == expected] {
        term Ok(advance(state));
    };
    term Err("Expected " + token_to_string(expected) + 
             " but got " + token_to_string(current_token(state)));
}
```

---

## Performance Characteristics

| Operation | Complexity | Notes |
|-----------|------------|-------|
| **parse_program** | O(n) | n = number of tokens |
| **parse_expression** | O(n) | n = expression length |
| **parse_statement** | O(n) | n = statement length |
| **expect_token** | O(1) | Single token check |

---

## Testing

All parser features tested:
- ✅ Program structure
- ✅ Transaction parsing (reactive, async)
- ✅ Definition parsing
- ✅ Contract parsing (pre, post, watchdog)
- ✅ Expression parsing (all precedence levels)
- ✅ Statement parsing (all types)
- ✅ Type parsing
- ✅ Error reporting

---

## Integration

### With Lexer

```brief
defn compile(source: String) -> Result<Program, String> {
    let tokens = tokenize(source)?;
    let parser = new_parser(tokens);
    parse_program(parser)
}
```

### With Type Checker (Tier 5)

```brief
defn typecheck(program: Program) -> Result<TypedProgram, TypeError> {
    let mut ctx = new_type_context();
    
    for item in program.items {
        unification item(TopDefn(defn)) = {
            typecheck_definition(defn, ctx)?;
        };
        unification item(TopTxn(txn)) = {
            typecheck_transaction(txn, ctx)?;
        };
        // ... etc
    };
    
    term Ok(program);
}
```

---

## Next Steps

With Tier 4 complete, the parser is ready. Next is **Tier 5: Type Checker**:

1. Type context with scopes (using HashMap)
2. Type inference for expressions
3. Contract verification
4. Error reporting with spans

---

*Last updated: 2026-05-06*  
*Status: Tier 4 COMPLETE ✅*
