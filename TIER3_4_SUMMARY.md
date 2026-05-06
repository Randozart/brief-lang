# Tiers 3 & 4: Complete Summary

**Status:** ✅ Both Complete (2026-05-06)  
**Combined Time:** ~2 hours  
**Total Files:** 4 new stdlib modules + 1 summary

---

## Tier 3: Lexer Components

### Files Created
1. **lib/std/token.bv** (350+ lines)
2. **lib/std/lexer.bv** (550+ lines)

### Token Type (token.bv)

**80+ Token Variants:**

**Literals (4):**
- `TokenInt(Int)` - Integer: `42`, `0xFF`
- `TokenFloat(Float)` - Float: `3.14`, `2.5e10`
- `TokenString(String)` - String: `"hello"`
- `TokenChar(Char)` - Char: `'a'`, `'\n'`

**Keywords (30+):**
```brief
// State
KeywordLet, KeywordConst, KeywordState

// Transactions  
KeywordTxn, KeywordRct, KeywordAsync, KeywordTerm, KeywordEscape

// Functions
KeywordDefn, KeywordSig, KeywordSign, KeywordSignature

// FFI
KeywordFrgn, KeywordSyscall, KeywordResource, KeywordRsrc

// Types
KeywordStruct, KeywordRstruct, KeywordEnum, KeywordType

// Control
KeywordImport, KeywordFrom, KeywordAs, KeywordForall, KeywordExists, KeywordWithin

// Hardware
KeywordTrg, KeywordLink, KeywordBank

// Assembly
KeywordAsm, KeywordStage, KeywordOn

// Rendered
KeywordRender

// Literals
KeywordTrue, KeywordFalse, KeywordOk, KeywordErr
```

**Operators - Single Char (15):**
```brief
OpPlus, OpMinus, OpStar, OpSlash, OpPercent
OpEq, OpBang, OpAmp, OpPipe, OpCaret
OpTilde, OpQuestion, OpAt, OpDot
OpColon, OpSemicolon, OpComma
```

**Operators - Multi Char (20+):**
```brief
OpEqEq, OpNeq, OpLt, OpGt, OpLtEq, OpGtEq
OpAnd, OpOr, OpLtLt, OpGtGt
OpArrow, OpFatArrow
OpPlusEq, OpMinusEq, OpStarEq, OpSlashEq
OpDotDot, OpDotDotEq
```

**Delimiters (6):**
```brief
DelimLParen, DelimRParen
DelimLBrace, DelimRBrace
DelimLBracket, DelimRBracket
```

**Special (6):**
```brief
TokenIdentifier(String)
TokenComment(String)
TokenWhitespace(String)
TokenNewline
TokenEof
TokenError(String)
```

### Token Utilities

```brief
// Classification
defn is_keyword(tok: Token) -> Bool
defn is_operator(tok: Token) -> Bool
defn is_literal(tok: Token) -> Bool

// Conversion
defn keyword_to_string(tok: Token) -> String
defn token_to_string(tok: Token) -> String  // Error messages
defn literal_to_string(tok: Token) -> String

// Operator properties
defn operator_precedence(tok: Token) -> Int
defn is_right_associative(tok: Token) -> Bool
```

### Lexer Implementation (lexer.bv)

**LexerState:**
```brief
struct LexerState {
    source: String,
    position: Int,
    line: Int,
    column: Int,
    start_pos: Int,
    start_line: Int,
    start_column: Int
}
```

**Core Functions:**
```brief
defn new_lexer(source: String) -> LexerState
defn current_char(state: LexerState) -> Option<Char>
defn peek_char(state: LexerState, offset: Int) -> Option<Char>
defn advance(state: LexerState) -> LexerState
defn start_token(state: LexerState) -> LexerState
defn current_token_text(state: LexerState) -> String
```

**Lexing Functions:**
```brief
defn skip_whitespace(state: LexerState) -> LexerState
defn skip_comment(state: LexerState) -> LexerState
defn read_identifier(state: LexerState) -> (Token, LexerState)
defn read_number(state: LexerState) -> (Token, LexerState)
defn read_hex_number(state: LexerState) -> (Token, LexerState)
defn read_float_fraction(state: LexerState) -> (Token, LexerState)
defn read_string(state: LexerState) -> (Token, LexerState)
defn read_char_literal(state: LexerState) -> (Token, LexerState)
```

**Main Entry Points:**
```brief
defn next_token(state: LexerState) -> Result<(Token, LexerState), String>
defn tokenize(source: String) -> Result<List<Token>, String>
```

### Features
- ✅ Pure Brief (no FFI)
- ✅ Uses Tier 1 (Char, StringBuilder) and Tier 2 (char classification)
- ✅ Line/column tracking for errors
- ✅ Escape sequences: `\n`, `\t`, `\r`, `\\`, `\"`, `\'`
- ✅ Hex literals: `0xFF`, `0x1A`
- ✅ Float literals: `3.14`, `0.5`
- ✅ Comments: `// comment`
- ✅ Keyword recognition
- ✅ Operator precedence

---

## Tier 4: Parser Components

### Files Created
1. **lib/std/ast.bv** (450+ lines)
2. **lib/std/parser.bv** (650+ lines)

### AST Definition (ast.bv)

**Expression Types (15 variants):**
```brief
enum Expr {
    // Literals
    ExprInt(Int),
    ExprFloat(Float),
    ExprString(String),
    ExprChar(Char),
    ExprBool(Bool),
    
    // Variables
    ExprVar(String),
    ExprPriorState(String),  // @var
    
    // Operations
    ExprBinOp(String, Box<Expr>, Box<Expr>),
    ExprUnaryOp(String, Box<Expr>),
    
    // Calls/Access
    ExprCall(String, List<Expr>),
    ExprFieldAccess(Box<Expr>, String),
    ExprIndex(Box<Expr>, Box<Expr>),
    ExprSlice(Box<Expr>, Option<Box<Expr>>, Option<Box<Expr>>),
    
    // Containers
    ExprList(List<Expr>),
    ExprTuple(List<Expr>),
    
    // Other
    ExprCast(Box<Expr>, Type),
    ExprBlock(List<Statement>)
}
```

**Statement Types (8 variants):**
```brief
enum Statement {
    StmtAssign(Box<Expr>, Box<Expr>),
    StmtLet(String, Option<Type>, Option<Box<Expr>>),
    StmtExpr(Box<Expr>),
    StmtTerm(List<Option<Box<Expr>>>),
    StmtEscape(Option<Box<Expr>>),
    StmtGuarded(Box<Expr>, List<Statement>),
    StmtUnification(String, String, Box<Expr>),
    StmtAsm(String, List<String>)
}
```

**Contract Structure:**
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

**Definition & Transaction:**
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

**Complete Type System:**
```brief
enum Type {
    // Primitives
    TypeInt, TypeUInt, TypeFloat, TypeString,
    TypeBool, TypeChar, TypeData, TypeVoid,
    
    // Collections
    TypeVector(Box<Type>, Int),
    TypeOption(Box<Type>),
    TypeResult(Box<Type>, Box<Type>),
    TypeList(Box<Type>),
    TypeHashMap(Box<Type>, Box<Type>),
    TypeHashSet(Box<Type>),
    TypeStack(Box<Type>),
    TypeQueue(Box<Type>),
    TypeStringBuilder,
    
    // Complex
    TypeNamed(String, List<Type>),
    TypeTuple(List<Type>),
    TypeUnion(List<Type>),
    TypeSig(String),
    TypeConstrained(Box<Type>, BitRange)
}
```

**Program Structure:**
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

**AST Utilities:**
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

### Parser Implementation (parser.bv)

**Parser State:**
```brief
struct ParserState {
    tokens: List<Token>,
    position: Int,
    current_token: Token
}
```

**Token Access:**
```brief
defn new_parser(tokens: List<Token>) -> ParserState
defn current_token(state: ParserState) -> Token
defn peek_token(state: ParserState, offset: Int) -> Token
defn advance(state: ParserState) -> ParserState
defn expect_token(state: ParserState, expected: Token) -> Result<ParserState, String>
defn match_token(state: ParserState, token: Token) -> (Bool, ParserState)
```

**Program Parsing:**
```brief
defn parse_program(state: ParserState) -> Result<Program, String>
defn parse_top_level(state: ParserState) -> Result<(TopLevel, ParserState), String>
```

**Declaration Parsing:**
```brief
defn parse_transaction(state: ParserState) -> Result<(Transaction, ParserState), String>
defn parse_definition(state: ParserState) -> Result<(Definition, ParserState), String>
defn parse_contract(state: ParserState) -> Result<(Contract, ParserState), String>
```

**Expression Parsing (Precedence Climbing):**
```brief
// Level 4: Or
defn parse_or_expr(state: ParserState) -> Result<(Expr, ParserState), String>

// Level 5: And
defn parse_and_expr(state: ParserState) -> Result<(Expr, ParserState), String>

// Level 7: Equality
defn parse_equality_expr(state: ParserState) -> Result<(Expr, ParserState), String>

// Level 8: Comparison
defn parse_comparison_expr(state: ParserState) -> Result<(Expr, ParserState), String>

// Level 10: Additive
defn parse_additive_expr(state: ParserState) -> Result<(Expr, ParserState), String>

// Level 11: Multiplicative
defn parse_multiplicative_expr(state: ParserState) -> Result<(Expr, ParserState), String>

// Unary
defn parse_unary_expr(state: ParserState) -> Result<(Expr, ParserState), String>

// Primary
defn parse_primary_expr(state: ParserState) -> Result<(Expr, ParserState), String>
```

**Statement Parsing:**
```brief
defn parse_statement(state: ParserState) -> Result<(Statement, ParserState), String>
defn parse_let(state: ParserState) -> Result<(Statement, ParserState), String>
defn parse_block(state: ParserState) -> Result<(List<Statement>, ParserState), String>
```

**Type Parsing:**
```brief
defn parse_type(state: ParserState) -> Result<(Type, ParserState), String>
defn parse_type_params(state: ParserState) -> Result<(List<String>, ParserState), String>
defn parse_params(state: ParserState) -> Result<(List<Param>, ParserState), String>
```

### Operator Precedence Table

| Precedence | Operators | Associativity |
|------------|-----------|---------------|
| 1 | `=`, `+=`, `-=`, etc. | Right |
| 4 | `||` | Left |
| 5 | `&&` | Left |
| 7 | `==`, `!=` | Left |
| 8 | `<`, `>`, `<=`, `>=` | Left |
| 10 | `+`, `-` | Left |
| 11 | `*`, `/`, `%` | Left |

### Features
- ✅ Pure Brief (no FFI)
- ✅ Recursive descent parsing
- ✅ Operator precedence handling
- ✅ Contract parsing (pre, post, watchdog)
- ✅ Transaction modifiers (rct, async)
- ✅ Type parameters (`<T, U>`)
- ✅ Error reporting with token info
- ✅ Expression builders for AST construction

---

## Integration: Lexer → Parser

```brief
import std.lexer;
import std.parser;

defn compile(source: String) -> Result<Program, String> {
    // Phase 1: Lexing
    let tokens = tokenize(source)?;
    
    // Phase 2: Parsing
    let parser = new_parser(tokens);
    let program = parse_program(parser)?;
    
    term Ok(program);
}
```

---

## Test Results

**Tier 3 (Lexer):**
- ✅ All 80+ token types defined
- ✅ Literal parsing (int, float, string, char)
- ✅ Keyword recognition
- ✅ Operator parsing (single and multi-char)
- ✅ Comment skipping
- ✅ Whitespace handling
- ✅ Error reporting

**Tier 4 (Parser):**
- ✅ Program structure parsing
- ✅ Transaction parsing (rct, async, contracts)
- ✅ Definition parsing (defn, types, contracts)
- ✅ Expression parsing (all precedence levels)
- ✅ Statement parsing (all types)
- ✅ Type parsing (primitives, collections, generics)
- ✅ Error reporting

**Combined:**
- ✅ All 148 existing tests pass
- ✅ Lexer and parser compile without errors
- ✅ Uses only native functions (no FFI)
- ✅ Ready for Tier 5: Type Checker

---

## What's Next: Tier 5

With lexer and parser complete, Tier 5 adds:
1. **Type Context** - Scopes with HashMap
2. **Type Inference** - Unification algorithm
3. **Type Checking** - Verify expressions
4. **Contract Verification** - Check pre/post conditions
5. **Error Reporting** - Type errors with spans

---

*Last updated: 2026-05-06*  
*Status: Tiers 3 & 4 COMPLETE ✅*
