# Brief Self-Hosting Implementation Plan

**Goal:** Enable the Brief compiler to be written in Brief itself

**Current Status:** ~10-15% feasible  
**Target:** 100% self-hosting capable  
**Estimated Timeline:** 2-4 weeks with AI assistance

---

## Executive Summary

The Brief compiler currently consists of ~19,452 lines of Rust across 26 modules. To write the compiler in Brief, we need to add:

- **50+ new types** (structs, enums, traits)
- **~470 native function definitions** (`defn`)
- **~20 FFI bindings** (for OS interaction, can be native later)
- **Type system extensions** (traits, constraints, unification)

This plan is organized into **9 tiers** with clear dependencies. Each tier must be completed before the next can begin.

---

## Dependency Graph

```
Tier 1 (Core Types) ──┬──> Tier 2 (String) ──> Tier 3 (Lexer) ──> Tier 4 (Parser)
                      │                                              │
                      ├──> Tier 8 (Infrastructure) ──────────────────┤
                      │                                              ▼
                      └────────────────────────────────────> Tier 5 (Type Checker)
                                                                     │
                                                                     ▼
                                                            Tier 6 (Proof Engine)
                                                                     │
                                                                     ▼
                                                            Tier 7 (Code Gen)
```

---

## Tier 1: Core Data Types

**Status:** ❌ Not started  
**Priority:** CRITICAL - blocks all other tiers  
**Estimated:** 3-4 days

### 1.1 Char Type

**Problem:** Brief has `String` but no single-character type. Cannot iterate strings efficiently.

**Implementation:**
```brief
// New primitive type (requires compiler changes)
let c: Char = 'a';

// Conversion functions
defn char_to_int(c: Char) -> Int
defn int_to_char(n: Int) -> Char
defn char_to_string(c: Char) -> String
defn string_to_char(s: String) -> Option<Char>  // First character

// Comparison
defn char_eq(a: Char, b: Char) -> Bool
defn char_lt(a: Char, b: Char) -> Bool
defn char_le(a: Char, b: Char) -> Bool
```

**Files to modify:**
- `src/ast.rs` - Add `Char` to `Type` enum
- `src/lexer.rs` - Add char literal token
- `src/parser.rs` - Parse char literals
- `src/typechecker.rs` - Type checking for Char
- `lib/std/char.bv` - New stdlib module

**Acceptance criteria:**
- [ ] Can declare `Char` variables
- [ ] Can convert between `Char` and `Int`
- [ ] Char literals work: `'a'`, `'\n'`, `'\u{1F600}'`

---

### 1.2 HashMap<K, V>

**Problem:** Symbol tables, scopes, and lookups require O(1) access. Lists are O(n).

**Implementation:**
```brief
struct HashMap<K, V> {
    // Internal representation (compiler magic or native implementation)
    opaque: Data
}

// Construction
defn new_map<K, V>() -> HashMap<K, V>
defn with_capacity<K, V>(capacity: Int) -> HashMap<K, V>

// Basic operations
defn insert<K, V>(map: HashMap<K, V>, key: K, value: V) -> HashMap<K, V>
defn get<K, V>(map: HashMap<K, V>, key: K) -> Option<V>
defn contains_key<K, V>(map: HashMap<K, V>, key: K) -> Bool
defn remove<K, V>(map: HashMap<K, V>, key: K) -> HashMap<K, V>

// Metadata
defn len<K, V>(map: HashMap<K, V>) -> Int
defn is_empty<K, V>(map: HashMap<K, V>) -> Bool

// Iteration
defn keys<K, V>(map: HashMap<K, V>) -> List<K>
defn values<K, V>(map: HashMap<K, V>) -> List<V>
defn iter<K, V>(map: HashMap<K, V>) -> List<(K, V)>

// Advanced
defn merge<K, V>(a: HashMap<K, V>, b: HashMap<K, V>) -> HashMap<K, V>
defn filter<K, V>(map: HashMap<K, V>, pred: (K, V) -> Bool) -> HashMap<K, V>
```

**Constraint requirements:**
- `K` must implement `Hash` trait
- `K` must implement `Eq` trait

**Files to modify:**
- `lib/std/collections.bv` - Add HashMap definition
- `src/typechecker.rs` - Enforce trait bounds
- `src/backend/*.rs` - Code generation for HashMap

**Acceptance criteria:**
- [ ] Can create empty HashMap
- [ ] Can insert and retrieve values
- [ ] Type checker enforces `Hash + Eq` bounds on K
- [ ] Iteration returns all key-value pairs

---

### 1.3 HashSet<T>

**Problem:** Need O(1) membership testing for visited nodes, dependency tracking.

**Implementation:**
```brief
struct HashSet<T> {
    opaque: Data  // Backed by HashMap<T, ()>
}

defn new_set<T>() -> HashSet<T>
defn insert<T>(set: HashSet<T>, item: T) -> HashSet<T>
defn contains<T>(set: HashSet<T>, item: T) -> Bool
defn remove<T>(set: HashSet<T>, item: T) -> HashSet<T>
defn len<T>(set: HashSet<T>) -> Int
defn is_empty<T>(set: HashSet<T>) -> Bool

// Set operations
defn union<T>(a: HashSet<T>, b: HashSet<T>) -> HashSet<T>
defn intersection<T>(a: HashSet<T>, b: HashSet<T>) -> HashSet<T>
defn difference<T>(a: HashSet<T>, b: HashSet<T>) -> HashSet<T>
defn symmetric_difference<T>(a: HashSet<T>, b: HashSet<T>) -> HashSet<T>

// Iteration
defn iter<T>(set: HashSet<T>) -> List<T>

// Predicates
defn is_subset<T>(a: HashSet<T>, b: HashSet<T>) -> Bool
defn is_superset<T>(a: HashSet<T>, b: HashSet<T>) -> Bool
defn is_disjoint<T>(a: HashSet<T>, b: HashSet<T>) -> Bool
```

**Constraint requirements:**
- `T` must implement `Hash` trait
- `T` must implement `Eq` trait

**Files to modify:**
- `lib/std/collections.bv` - Add HashSet definition

**Acceptance criteria:**
- [ ] All set operations work correctly
- [ ] O(1) contains() performance
- [ ] Type checker enforces trait bounds

---

### 1.4 StringBuilder / Buffer

**Problem:** String concatenation `s = s + "text"` is O(n²). Need efficient building.

**Implementation:**
```brief
struct StringBuilder {
    buffer: List<Char>,
    length: Int
}

// Construction
defn new_builder() -> StringBuilder
defn with_capacity(capacity: Int) -> StringBuilder

// Append operations
defn append_char(builder: StringBuilder, c: Char) -> StringBuilder
defn append_str(builder: StringBuilder, s: String) -> StringBuilder
defn append_int(builder: StringBuilder, n: Int) -> StringBuilder
defn append_bool(builder: StringBuilder, b: Bool) -> StringBuilder
defn append_float(builder: StringBuilder, f: Float) -> StringBuilder

// Conversion
defn to_string(builder: StringBuilder) -> String
defn clear(builder: StringBuilder) -> StringBuilder

// Metadata
defn len(builder: StringBuilder) -> Int
defn is_empty(builder: StringBuilder) -> Bool
```

**Files to modify:**
- `lib/std/string.bv` - Add StringBuilder

**Acceptance criteria:**
- [ ] O(1) append operations
- [ ] Can build strings incrementally
- [ ] to_string() produces correct output

---

### 1.5 Stack<T> and Queue<T>

**Problem:** Parser needs stack for recursive descent. Proof engine needs queue for BFS.

**Implementation:**
```brief
// Stack (LIFO)
struct Stack<T> {
    items: List<T>
}

defn push<T>(stack: Stack<T>, item: T) -> Stack<T>
defn pop<T>(stack: Stack<T>) -> Option<(T, Stack<T>)>
defn peek<T>(stack: Stack<T>) -> Option<T>
defn is_empty<T>(stack: Stack<T>) -> Bool
defn len<T>(stack: Stack<T>) -> Int
defn clear<T>(stack: Stack<T>) -> Stack<T>

// Queue (FIFO)
struct Queue<T> {
    front: List<T>,
    back: List<T>  // Amortized O(1) queue
}

defn enqueue<T>(queue: Queue<T>, item: T) -> Queue<T>
defn dequeue<T>(queue: Queue<T>) -> Option<(T, Queue<T>)>
defn front<T>(queue: Queue<T>) -> Option<T>
defn is_empty<T>(queue: Queue<T>) -> Bool
defn len<T>(queue: Queue<T>) -> Int
```

**Files to modify:**
- `lib/std/collections.bv` - Add Stack and Queue

**Acceptance criteria:**
- [ ] Stack push/pop works in LIFO order
- [ ] Queue enqueue/dequeue works in FIFO order
- [ ] Amortized O(1) operations

---

### 1.6 Result and Option Extensions

**Problem:** Current Result/Option types lack functional methods for chaining.

**Implementation:**
```brief
// Result enhancements
defn map<T, E, U>(result: Result<T, E>, f: T -> U) -> Result<U, E>
defn and_then<T, E, U>(result: Result<T, E>, f: T -> Result<U, E>) -> Result<U, E>
defn or_else<T, E>(result: Result<T, E>, f: E -> Result<T, E>) -> Result<T, E>
defn unwrap_or<T, E>(result: Result<T, E>, default: T) -> T
defn unwrap_or_else<T, E>(result: Result<T, E>, f: E -> T) -> T
defn expect<T, E>(result: Result<T, E>, message: String) -> T

// Option (may already exist, needs methods)
enum Option<T> {
    Some(T),
    None
}

defn map<T, U>(opt: Option<T>, f: T -> U) -> Option<U>
defn and_then<T, U>(opt: Option<T>, f: T -> Option<U>) -> Option<U>
defn unwrap_or<T>(opt: Option<T>, default: T) -> T
defn unwrap_or_else<T>(opt: Option<T>, f: () -> T) -> T
defn is_some<T>(opt: Option<T>) -> Bool
defn is_none<T>(opt: Option<T>) -> Bool
defn filter<T>(opt: Option<T>, pred: T -> Bool) -> Option<T>
```

**Files to modify:**
- `lib/std/result.bv` - New file for Result extensions
- `lib/std/option.bv` - New file for Option type

**Acceptance criteria:**
- [ ] Method chaining works: `result.map(f).and_then(g)`
- [ ] Option type integrates with pattern matching

---

## Tier 2: String & Text Processing

**Status:** ❌ Not started  
**Priority:** CRITICAL - needed for lexer  
**Estimated:** 4-5 days

### 2.1 Character Classification (Move from FFI to Native)

**Current status:** All FFI in `lib/std/string.bv`

**Implementation:**
```brief
defn is_whitespace(c: Char) -> Bool
defn is_digit(c: Char) -> Bool
defn is_hex_digit(c: Char) -> Bool
defn is_oct_digit(c: Char) -> Bool
defn is_alpha(c: Char) -> Bool
defn is_alphanumeric(c: Char) -> Bool
defn is_upper(c: Char) -> Bool
defn is_lower(c: Char) -> Bool
defn is_symbol(c: Char) -> Bool
defn is_punctuation(c: Char) -> Bool
defn is_control(c: Char) -> Bool
defn is_ascii(c: Char) -> Bool
```

**Files to modify:**
- `lib/std/char.bv` - Move from FFI to native

**Acceptance criteria:**
- [ ] All classification functions work without FFI
- [ ] Unicode-aware (not just ASCII)

---

### 2.2 Character Conversion (Move from FFI to Native)

**Implementation:**
```brief
defn to_upper(c: Char) -> Char
defn to_lower(c: Char) -> Char
defn to_title(c: Char) -> Char
defn digit_to_int(c: Char) -> Option<Int>  // '0'-'9' -> 0-9
defn int_to_digit(n: Int) -> Option<Char>  // 0-9 -> '0'-'9'
defn hex_digit_to_int(c: Char) -> Option<Int>  // '0'-'9', 'a'-'f' -> 0-15
defn int_to_hex_digit(n: Int) -> Option<Char>  // 0-15 -> '0'-'9', 'a'-'f'
```

**Files to modify:**
- `lib/std/char.bv`

**Acceptance criteria:**
- [ ] Case conversion works for Unicode
- [ ] Digit conversion handles all bases

---

### 2.3 String Building & Manipulation

**Implementation:**
```brief
defn concat_chars(chars: List<Char>) -> String
defn string_to_chars(s: String) -> List<Char>
defn repeat_char(c: Char, n: Int) -> String
defn pad_left_char(s: String, width: Int, c: Char) -> String
defn pad_right_char(s: String, width: Int, c: Char) -> String
defn trim_char(s: String, c: Char) -> String
defn trim_chars(s: String, chars: List<Char>) -> String
defn replace_char(s: String, old: Char, new: Char) -> String
defn replace_all_char(s: String, old: Char, new: Char) -> String
```

**Files to modify:**
- `lib/std/string.bv`

**Acceptance criteria:**
- [ ] Efficient conversion between String and List<Char>
- [ ] All manipulation functions work without FFI

---

### 2.4 Unicode Support

**Implementation:**
```brief
defn utf8_decode(s: String, index: Int) -> Result<(Char, Int), UnicodeError>
defn utf8_encode(c: Char) -> String
defn utf8_len(s: String) -> Int
defn codepoint_to_int(c: Char) -> Int
defn int_to_codepoint(n: Int) -> Option<Char>
defn is_unicode_scalar(c: Char) -> Bool
defn is_surrogate(c: Char) -> Bool
defn is_valid_codepoint(n: Int) -> Bool
```

**Files to modify:**
- `lib/std/unicode.bv` - New module

**Acceptance criteria:**
- [ ] Can decode UTF-8 strings correctly
- [ ] Handles multi-byte characters
- [ ] Validates codepoint ranges

---

### 2.5 String Formatting

**Implementation:**
```brief
defn format_int(n: Int, base: Int) -> String  // base 2, 8, 10, 16
defn format_uint(n: UInt, base: Int) -> String
defn format_float(f: Float, precision: Int) -> String
defn format_bool(b: Bool) -> String
defn format_char(c: Char) -> String
defn format_hex(n: Int, uppercase: Bool) -> String
defn format_binary(n: Int) -> String
defn format_octal(n: Int) -> String
defn concat_many(strings: List<String>) -> String
defn join_strings(strings: List<String>, sep: String) -> String
```

**Files to modify:**
- `lib/std/string.bv`
- `lib/std/format.bv` - New module

**Acceptance criteria:**
- [ ] Can format all primitive types
- [ ] Supports different number bases
- [ ] Efficient concatenation

---

## Tier 3: Lexer Components

**Status:** ❌ Not started  
**Priority:** HIGH - needed for parser  
**Estimated:** 3-4 days

### 3.1 Token Type Definition

**Implementation:**
```brief
enum Token {
    // Literals
    Identifier(String),
    Integer(Int),
    Float(Float),
    String(String),
    Char(Char),
    
    // Keywords (~80 total)
    KeywordLet,
    KeywordConst,
    KeywordTxn,
    KeywordRct,
    KeywordAsync,
    KeywordDefn,
    KeywordTerm,
    KeywordEscape,
    KeywordImport,
    KeywordFrom,
    KeywordAs,
    KeywordFrgn,
    KeywordSyscall,
    KeywordResource,
    KeywordStruct,
    KeywordRstruct,
    KeywordEnum,
    KeywordRender,
    KeywordTrg,
    KeywordLink,
    KeywordAsm,
    KeywordStage,
    KeywordOn,
    KeywordForall,
    KeywordExists,
    KeywordWithin,
    KeywordBank,
    KeywordSig,
    KeywordSign,
    KeywordSignature,
    KeywordDefinition,
    KeywordTransact,
    KeywordTransaction,
    KeywordTrue,
    KeywordFalse,
    KeywordOk,
    KeywordErr,
    // ... more keywords
    
    // Operators (~30 total)
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Eq,
    EqEq,
    Neq,
    Lt,
    LtEq,
    Gt,
    GtEq,
    And,
    Or,
    Not,
    Ampersand,
    Pipe,
    Caret,
    LtLt,
    GtGt,
    // ... more operators
    
    // Delimiters (~20 total)
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Comma,
    Colon,
    Semicolon,
    Dot,
    Arrow,
    FatArrow,
    At,
    AmpersandMut,
    Question,
    Bang,
    // ... more delimiters
    
    // Special
    Eof,
    Error(String)
}
```

**Files to modify:**
- `lib/std/token.bv` - New module
- `src/ast.rs` - May need Token enum in compiler too

**Acceptance criteria:**
- [ ] All token variants defined
- [ ] Can construct each token type
- [ ] Pattern matching works on Token

---

### 3.2 Lexer Functions

**Implementation:**
```brief
struct LexerState {
    source: String,
    position: Int,
    line: Int,
    column: Int
}

defn new_lexer(source: String) -> LexerState
defn tokenize(source: String) -> List<Token>
defn next_token(state: LexerState) -> Result<(Token, LexerState), LexError>

// Character readers
defn skip_whitespace(state: LexerState) -> LexerState
defn read_identifier(state: LexerState) -> Result<(String, LexerState), LexError>
defn read_number(state: LexerState) -> Result<(Token, LexerState), LexError>
defn read_string(state: LexerState) -> Result<(String, LexerState), LexError>
defn read_char_literal(state: LexerState) -> Result<(Char, LexerState), LexError>
defn read_comment(state: LexerState) -> Result<LexerState, LexError>
defn read_operator(state: LexerState) -> Result<(Token, LexerState), LexError>

// Helpers
defn current_char(state: LexerState) -> Option<Char>
defn peek_char(state: LexerState, offset: Int) -> Option<Char>
defn advance(state: LexerState) -> LexerState
defn keyword_to_token(s: String) -> Token
defn is_operator_char(c: Char) -> Bool
defn is_identifier_start(c: Char) -> Bool
defn is_identifier_continue(c: Char) -> Bool
defn make_error(message: String, state: LexerState) -> LexError
```

**Files to modify:**
- `lib/std/lexer.bv` - New module

**Acceptance criteria:**
- [ ] Can tokenize valid Brief source
- [ ] Produces meaningful errors for invalid input
- [ ] Handles all token types correctly
- [ ] Tracks line/column for error reporting

---

### 3.3 Lexical Error Types

**Implementation:**
```brief
enum LexError {
    UnexpectedChar(Char, Span),
    UnterminatedString(Span),
    UnterminatedChar(Span),
    InvalidNumber(String, Span),
    InvalidEscape(String, Span),
    UnexpectedEof(Span)
}

struct Span {
    file: String,
    line: Int,
    column: Int,
    offset: Int,
    length: Int
}

defn format_lex_error(err: LexError) -> String
defn span_from_state(state: LexerState, length: Int) -> Span
```

**Files to modify:**
- `lib/std/errors.bv` - New module

**Acceptance criteria:**
- [ ] All error variants defined
- [ ] Error messages include source location
- [ ] Can format errors for display

---

## Tier 4: Parser Components

**Status:** ❌ Not started  
**Priority:** HIGH - needed for type checker  
**Estimated:** 5-7 days

### 4.1 AST Struct Definitions

**Implementation:** Define all AST types in Brief:

```brief
// Top-level program structure
struct Program {
    items: List<TopLevel>,
    span: Span
}

enum TopLevel {
    Transaction(Transaction),
    Definition(Definition),
    StateDecl(StateDecl),
    Constant(Constant),
    Import(Import),
    Struct(StructDefinition),
    RStruct(RStructDefinition),
    Enum(EnumDefinition),
    Signature(Signature),
    Resource(ResourceDeclaration),
    RenderBlock(RenderBlock)
}

// Transactions
struct Transaction {
    name: String,
    type_params: List<TypeParam>,
    params: List<Param>,
    contract: Contract,
    body: List<Statement>,
    is_async: Bool,
    is_reactive: Bool,
    span: Span
}

struct Contract {
    pre_condition: Expr,
    post_condition: Expr,
    watchdog: Option<WatchdogSpec>,
    span: Span
}

struct WatchdogSpec {
    condition: Expr,
    is_required: Bool
}

// Functions
struct Definition {
    name: String,
    type_params: List<TypeParam>,
    params: List<Param>,
    outputs: List<Type>,
    contract: Contract,
    body: List<Statement>,
    span: Span
}

// Types
enum Type {
    Int,
    UInt,
    Float,
    Bool,
    String,
    Void,
    Data,
    Char,
    List(Box<Type>),
    Vector(Box<Type>, Int),
    Option(Box<Type>),
    Result(Box<Type>, Box<Type>),
    Tuple(List<Type>),
    Named(String, List<Type>),
    Sig(String),
    Constrained(Box<Type>, BitRange)
}

enum BitRange {
    Bits(Int),
    Range(Int, Int),
    Mask(Int)
}

struct TypeParam {
    name: String,
    bounds: List<TypeBound>
}

enum TypeBound {
    Eq(Type),
    SubTypeOf(Type),
    SuperTypeOf(Type),
    HasTrait(String)
}

// Statements
enum Statement {
    Assignment(Expr, Expr, Option<(Expr, TimeUnit)>),
    Unification(String, String, Expr),
    Guarded(Expr, List<Statement>),
    Term(List<Option<Expr>>),
    Escape(Option<Expr>),
    Expression(Expr),
    Let(String, Option<Type>, Option<Expr>, Option<Int>, Option<BitRange>, Bool),
    InlineAsm(String, List<String>, Span)
}

enum TimeUnit {
    Cycles,
    Nanos,
    Micros,
    Millis,
    Seconds
}

// Expressions
enum Expr {
    Literal(Literal),
    Variable(String),
    BinaryOp(String, Box<Expr>, Box<Expr>),
    UnaryOp(String, Box<Expr>),
    Call(Box<Expr>, List<Expr>),
    FieldAccess(Box<Expr>, String),
    Index(Box<Expr>, Box<Expr>),
    Slice(Box<Expr>, Option<Box<Expr>>, Option<Box<Expr>>),
    Tuple(List<Expr>),
    List(List<Expr>),
    Range(Option<Box<Expr>>, Option<Box<Expr>>),
    Cast(Box<Expr>, Type),
    PriorState(String),
    Block(List<Statement>)
}

enum Literal {
    Int(Int),
    Float(Float),
    Bool(Bool),
    String(String),
    Char(Char)
}

// ... 20+ more struct/enum types
```

**Files to modify:**
- `lib/std/ast.bv` - New module (large)

**Acceptance criteria:**
- [ ] All AST types defined
- [ ] Can construct AST nodes
- [ ] Pattern matching works on all types

---

### 4.2 Parser Functions

**Implementation:**
```brief
struct ParserState {
    tokens: List<Token>,
    position: Int,
    span_tracker: SpanTracker
}

// Main entry points
defn parse_program(tokens: List<Token>) -> Result<Program, ParseError>
defn parse_top_level(state: ParserState) -> Result<(TopLevel, ParserState), ParseError>

// Declaration parsers
defn parse_transaction(state: ParserState) -> Result<(Transaction, ParserState), ParseError>
defn parse_definition(state: ParserState) -> Result<(Definition, ParserState), ParseError>
defn parse_struct(state: ParserState) -> Result<(StructDefinition, ParserState), ParseError>
defn parse_rstruct(state: ParserState) -> Result<(RStructDefinition, ParserState), ParseError>
defn parse_enum(state: ParserState) -> Result<(EnumDefinition, ParserState), ParseError>
defn parse_signature(state: ParserState) -> Result<(Signature, ParserState), ParseError>
defn parse_import(state: ParserState) -> Result<(Import, ParserState), ParseError>
defn parse_constant(state: ParserState) -> Result<(Constant, ParserState), ParseError>
defn parse_state_decl(state: ParserState) -> Result<(StateDecl, ParserState), ParseError>

// Statement parsers
defn parse_statement(state: ParserState) -> Result<(Statement, ParserState), ParseError>
defn parse_let(state: ParserState) -> Result<(Statement, ParserState), ParseError>
defn parse_assignment(state: ParserState) -> Result<(Statement, ParserState), ParseError>
defn parse_guarded(state: ParserState) -> Result<(Statement, ParserState), ParseError>
defn parse_term(state: ParserState) -> Result<(Statement, ParserState), ParseError>
defn parse_escape(state: ParserState) -> Result<(Statement, ParserState), ParseError>

// Expression parsers
defn parse_expression(state: ParserState) -> Result<(Expr, ParserState), ParseError>
defn parse_binary_expr(state: ParserState, precedence: Int) -> Result<(Expr, ParserState), ParseError>
defn parse_unary_expr(state: ParserState) -> Result<(Expr, ParserState), ParseError>
defn parse_primary_expr(state: ParserState) -> Result<(Expr, ParserState), ParseError>
defn parse_call(state: ParserState, func: Expr) -> Result<(Expr, ParserState), ParseError>
defn parse_literal(state: ParserState) -> Result<(Expr, ParserState), ParseError>

// Type parsers
defn parse_type(state: ParserState) -> Result<(Type, ParserState), ParseError>
defn parse_contract(state: ParserState) -> Result<(Contract, ParserState), ParseError>

// Utilities
defn current_token(state: ParserState) -> Token
defn peek_token(state: ParserState, offset: Int) -> Token
defn advance(state: ParserState) -> ParserState
defn expect_token(state: ParserState, expected: Token) -> Result<ParserState, ParseError>
defn skip_token(state: ParserState, token: Token) -> ParserState
defn at_end(state: ParserState) -> Bool
defn make_parse_error(message: String, token: Token, span: Span) -> ParseError
```

**Files to modify:**
- `lib/std/parser.bv` - New module

**Acceptance criteria:**
- [ ] Can parse all valid Brief programs
- [ ] Produces correct AST
- [ ] Error messages include context
- [ ] Handles all syntax variants

---

### 4.3 Parse Error Types

**Implementation:**
```brief
enum ParseError {
    UnexpectedToken(Token, Token, Span),  // got, expected, location
    UnexpectedEof(Span),
    InvalidLiteral(String, Span),
    InvalidPattern(String, Span),
    MismatchedBrackets(Span),
    InvalidContract(Span)
}

defn format_parse_error(err: ParseError, source: String) -> String
defn highlight_span(source: String, span: Span) -> String
```

**Files to modify:**
- `lib/std/errors.bv` - Add ParseError

**Acceptance criteria:**
- [ ] All error variants defined
- [ ] Can format errors with source snippet
- [ ] Points to exact error location

---

## Tier 5: Type Checker

**Status:** ❌ Not started  
**Priority:** CRITICAL - type system extensions required  
**Estimated:** 7-10 days

### 5.1 Trait System (NEW LANGUAGE FEATURE)

**Problem:** Generics syntax exists but trait bounds are not enforced.

**Implementation:**
```brief
// Trait definitions
trait Eq {
    defn eq(self, other: Self) -> Bool;
    defn neq(self, other: Self) -> Bool;
}

trait Ord: Eq {
    defn cmp(self, other: Self) -> Ordering;
    defn lt(self, other: Self) -> Bool;
    defn le(self, other: Self) -> Bool;
    defn gt(self, other: Self) -> Bool;
    defn ge(self, other: Self) -> Bool;
}

trait Hash {
    defn hash(self) -> Int;
}

trait Clone {
    defn clone(self) -> Self;
}

trait Debug {
    defn debug(self) -> String;
}

trait Default {
    defn default() -> Self;
}

// Trait implementations (NEW SYNTAX)
impl Eq for Int {
    defn eq(self, other: Self) -> Bool { self == other }
    defn neq(self, other: Self) -> Bool { self != other }
}

impl Ord for Int {
    defn cmp(self, other: Self) -> Ordering { ... }
    // ... etc
}

impl Hash for Int {
    defn hash(self) -> Int { self }
}

// Generic functions with trait bounds
defn insert<K, V>(map: HashMap<K, V>, key: K, value: V) 
    [K: Hash + Eq]  // Constraint syntax
    -> HashMap<K, V> 
{
    // ...
}

// Where clauses (for complex bounds)
defn process<T, U>(t: T, u: U) -> String 
    [T: Debug, U: Debug, T: Eq, U: Ord]
    where T: Clone, U: Clone
{
    // ...
}
```

**Files to modify:**
- `src/ast.rs` - Add Trait, Impl AST nodes
- `src/parser.rs` - Parse trait definitions
- `src/typechecker.rs` - Trait resolution
- `lib/std/traits.bv` - Define standard traits

**Acceptance criteria:**
- [ ] Can define traits
- [ ] Can implement traits for types
- [ ] Type checker enforces trait bounds
- [ ] Can use trait methods on generic types

---

### 5.2 Type Unification Algorithm

**Implementation:**
```brief
struct Substitution {
    mappings: HashMap<TypeVar, Type>
}

defn empty_subst() -> Substitution
defn apply_subst(type: Type, subst: Substitution) -> Type
defn compose_subst(s1: Substitution, s2: Substitution) -> Substitution

// Unification core
defn unify(t1: Type, t2: Type, subst: Substitution) -> Result<Substitution, TypeError>
defn unify_var(var: TypeVar, type: Type, subst: Substitution) -> Result<Substitution, TypeError>
defn occurs_check(var: TypeVar, type: Type, subst: Substitution) -> Bool

// Type variables
defn fresh_type_var() -> TypeVar
defn instantiate(type_scheme: TypeScheme) -> Type
defn generalize(type: Type, context: TypeContext) -> TypeScheme
```

**Files to modify:**
- `lib/std/unification.bv` - New module

**Acceptance criteria:**
- [ ] Can unify concrete types
- [ ] Can unify type variables
- [ ] Occurs check prevents infinite types
- [ ] Produces most general unifier

---

### 5.3 Type Context and Scopes

**Implementation:**
```brief
struct TypeContext {
    scopes: Stack<HashMap<String, TypeScheme>>,
    traits: HashMap<String, Trait>,
    impls: HashMap<String, List<Impl>>,
    type_params: HashMap<String, List<TypeBound>>
}

defn new_context() -> TypeContext
defn enter_scope(ctx: TypeContext) -> TypeContext
defn exit_scope(ctx: TypeContext) -> TypeContext
defn add_binding(ctx: TypeContext, name: String, ty: TypeScheme) -> TypeContext
defn lookup_type(ctx: TypeContext, name: String) -> Option<TypeScheme>
defn lookup_trait(ctx: TypeContext, name: String) -> Option<Trait>
defn check_trait_impl(ctx: TypeContext, trait_name: String, ty: Type) -> Bool
```

**Files to modify:**
- `lib/std/type_context.bv` - New module

**Acceptance criteria:**
- [ ] Proper scope nesting
- [ ] Shadowing works correctly
- [ ] Trait lookup works

---

### 5.4 Type Checking Functions

**Implementation:**
```brief
// Main entry
defn typecheck_program(program: Program) -> Result<TypedProgram, TypeError>

// Declaration type checking
defn typecheck_transaction(txn: Transaction, ctx: TypeContext) -> Result<TypedTransaction, TypeError>
defn typecheck_definition(defn: Definition, ctx: TypeContext) -> Result<TypedDefinition, TypeError>
defn typecheck_struct(struct: StructDefinition, ctx: TypeContext) -> Result<TypedStruct, TypeError>
defn typecheck_trait(trait: Trait, ctx: TypeContext) -> Result<TypedTrait, TypeError>
defn typecheck_impl(impl: Impl, ctx: TypeContext) -> Result<TypedImpl, TypeError>

// Statement type checking
defn typecheck_statement(stmt: Statement, ctx: TypeContext) -> Result<TypedStatement, TypeError>
defn typecheck_let(name: String, ty: Option<Type>, expr: Option<Expr>, ctx: TypeContext) -> Result<TypedStatement, TypeError>
defn typecheck_assignment(lhs: Expr, rhs: Expr, ctx: TypeContext) -> Result<TypedStatement, TypeError>
defn typecheck_guarded(condition: Expr, body: List<Statement>, ctx: TypeContext) -> Result<TypedStatement, TypeError>

// Expression type checking
defn typecheck_expression(expr: Expr, ctx: TypeContext) -> Result<(TypedExpr, Type), TypeError>
defn infer_type(expr: Expr, ctx: TypeContext) -> Result<Type, TypeError>
defn check_type(expr: Expr, expected: Type, ctx: TypeContext) -> Result<TypedExpr, TypeError>

// Contract verification
defn typecheck_contract(contract: Contract, ctx: TypeContext) -> Result<TypedContract, TypeError>
defn verify_precondition(pre: Expr, ctx: TypeContext) -> Result<(), TypeError>
defn verify_postcondition(post: Expr, ctx: TypeContext) -> Result<(), TypeError>
```

**Files to modify:**
- `lib/std/typechecker.bv` - New module

**Acceptance criteria:**
- [ ] Infers types for all expressions
- [ ] Checks type annotations
- [ ] Enforces trait bounds
- [ ] Reports meaningful type errors

---

### 5.5 Type Error Types

**Implementation:**
```brief
enum TypeError {
    TypeMismatch(Type, Type, Span),
    UndefinedVariable(String, Span),
    UndefinedType(String, Span),
    UndefinedTrait(String, Span),
    TraitNotImplemented(String, Type, Span),
    GenericNotInferred(String, Span),
    OccursCheckFailed(TypeVar, Type, Span),
    UnificationFailed(Type, Type, Span),
    ConstraintNotSatisfied(String, Type, Span),
    DuplicateDefinition(String, Span)
}

defn format_type_error(err: TypeError) -> String
```

**Files to modify:**
- `lib/std/errors.bv` - Add TypeError

**Acceptance criteria:**
- [ ] All error variants defined
- [ ] Error messages show expected vs actual types
- [ ] Points to error location

---

## Tier 6: Proof Engine

**Status:** ❌ Not started  
**Priority:** HIGH - core Brief feature  
**Estimated:** 7-10 days

### 6.1 Symbolic Values

**Implementation:**
```brief
enum SymbolicValue {
    ConcreteInt(Int),
    ConcreteFloat(Float),
    ConcreteBool(Bool),
    ConcreteString(String),
    ConcreteChar(Char),
    Symbolic(String),  // Named symbolic variable
    BinaryOp(String, Box<SymbolicValue>, Box<SymbolicValue>),
    UnaryOp(String, Box<SymbolicValue>),
    FieldAccess(Box<SymbolicValue>, String),
    Index(Box<SymbolicValue>, Box<SymbolicValue>),
    Unknown
}

enum ConcreteValue {
    Int(Int),
    Float(Float),
    Bool(Bool),
    String(String),
    Char(Char)
}

defn from_expr(expr: Expr, env: HashMap<String, SymbolicValue>) -> SymbolicValue
defn to_concrete(sv: SymbolicValue) -> Option<ConcreteValue>
defn evaluate(sv: SymbolicValue) -> Option<ConcreteValue>
defn simplify(sv: SymbolicValue) -> SymbolicValue
defn substitute(sv: SymbolicValue, var: String, replacement: SymbolicValue) -> SymbolicValue
defn free_vars(sv: SymbolicValue) -> HashSet<String>
```

**Files to modify:**
- `lib/std/symbolic.bv` - New module

**Acceptance criteria:**
- [ ] Can create symbolic values from expressions
- [ ] Simplification reduces expressions
- [ ] Substitution works correctly

---

### 6.2 Symbolic State

**Implementation:**
```brief
struct SymbolicState {
    vars: HashMap<String, SymbolicValue>,
    path_constraints: List<SymbolicValue>,
    visited: HashSet<String>  // For cycle detection
}

defn initial_state() -> SymbolicState
defn update_var(state: SymbolicState, name: String, value: SymbolicValue) -> SymbolicState
defn lookup_var(state: SymbolicState, name: String) -> Option<SymbolicValue>
defn add_constraint(state: SymbolicState, constraint: SymbolicValue) -> SymbolicState
defn get_constraints(state: SymbolicState) -> List<SymbolicValue>
defn is_visited(state: SymbolicState, key: String) -> Bool
defn mark_visited(state: SymbolicState, key: String) -> SymbolicState
```

**Files to modify:**
- `lib/std/symbolic.bv`

**Acceptance criteria:**
- [ ] State updates work correctly
- [ ] Path constraints accumulate
- [ ] Visited tracking prevents cycles

---

### 6.3 Statement Execution

**Implementation:**
```brief
defn execute_statement(stmt: Statement, state: SymbolicState) -> List<SymbolicState>
defn execute_assignment(lhs: Expr, rhs: Expr, state: SymbolicState) -> List<SymbolicState>
defn execute_guarded(condition: Expr, body: List<Statement>, state: SymbolicState) -> List<SymbolicState>
defn execute_term(values: List<Option<Expr>>, state: SymbolicState) -> List<SymbolicState>
defn execute_escape(value: Option<Expr>, state: SymbolicState) -> List<SymbolicState>

// Forking for branches
defn fork_state(state: SymbolicState, constraint: SymbolicValue) -> (SymbolicState, SymbolicState)
defn merge_states(states: List<SymbolicState>) -> SymbolicState
```

**Files to modify:**
- `lib/std/symbolic.bv`

**Acceptance criteria:**
- [ ] Each statement transforms state correctly
- [ ] Guards fork execution paths
- [ ] All paths explored

---

### 6.4 Path Exploration

**Implementation:**
```brief
struct ExecutionPath {
    statements: List<Statement>,
    constraints: List<SymbolicValue>,
    final_state: SymbolicState
}

defn explore_all_paths(txn: Transaction) -> List<ExecutionPath>
defn is_path_feasible(path: ExecutionPath) -> Bool
defn path_constraints_satisfiable(constraints: List<SymbolicValue>) -> Bool
defn find_counterexample(post: Expr, paths: List<ExecutionPath>) -> Option<CounterExample>
defn check_mutual_exclusion(txn1: Transaction, txn2: Transaction) -> Result<(), ConflictError>
defn detect_deadlock(txns: List<Transaction>) -> Option<DeadlockCycle>
```

**Files to modify:**
- `lib/std/proof.bv` - New module

**Acceptance criteria:**
- [ ] Explores all execution paths
- [ ] Detects infeasible paths
- [ ] Finds counterexamples

---

### 6.5 Contract Verification

**Implementation:**
```brief
enum ProofResult {
    Verified,
    Failed(CounterExample),
    Timeout,
    Inconclusive
}

struct CounterExample {
    inputs: HashMap<String, ConcreteValue>,
    path: ExecutionPath,
    violation: String
}

defn verify_contract(txn: Transaction) -> ProofResult
defn verify_precondition(pre: Expr, state: SymbolicState) -> Bool
defn verify_postcondition(post: Expr, final_states: List<SymbolicState>) -> Bool
defn generate_verification_condition(txn: Transaction) -> VerificationCondition
defn check_vc(vc: VerificationCondition) -> Result<(), ProofError>
```

**Files to modify:**
- `lib/std/proof.bv`

**Acceptance criteria:**
- [ ] Verifies contracts correctly
- [ ] Produces counterexamples on failure
- [ ] Handles all transaction types

---

### 6.6 Proof Error Types

**Implementation:**
```brief
enum ProofError {
    PreconditionFailed(CounterExample),
    PostconditionFailed(CounterExample),
    MutualExclusionViolation(Transaction, Transaction, CounterExample),
    DeadlockDetected(List<Transaction>),
    UnreachablePostcondition(String, Span),
    Timeout
}

defn format_proof_error(err: ProofError) -> String
```

**Files to modify:**
- `lib/std/errors.bv` - Add ProofError

**Acceptance criteria:**
- [ ] All error variants defined
- [ ] Counterexamples include input values
- [ ] Error messages explain the violation

---

## Tier 7: Code Generation Backends

**Status:** ❌ Not started  
**Priority:** MEDIUM - can start with one backend  
**Estimated:** 5-7 days

### 7.1 Rust Backend

**Implementation:**
```brief
defn generate_rust(program: TypedProgram) -> String
defn generate_rust_transaction(txn: TypedTransaction) -> String
defn generate_rust_definition(defn: TypedDefinition) -> String
defn generate_rust_struct(struct: TypedStruct) -> String
defn generate_rust_enum(enum: TypedEnum) -> String
defn generate_rust_type(ty: Type) -> String
defn generate_rust_statement(stmt: TypedStatement) -> String
defn generate_rust_expression(expr: TypedExpr) -> String
defn generate_rust_literal(literal: Literal) -> String
defn generate_rust_contract(contract: TypedContract) -> String
defn rust_escape_string(s: String) -> String
defn rust_identifier(name: String) -> String  // Handle keywords
```

**Files to modify:**
- `lib/std/backend_rust.bv` - New module

**Acceptance criteria:**
- [ ] Generates valid Rust code
- [ ] Compiles with rustc
- [ ] Preserves contracts as assertions

---

### 7.2 C Backend

**Implementation:**
```brief
defn generate_c(program: TypedProgram) -> String
defn generate_c_header(program: TypedProgram) -> String
defn generate_c_transaction(txn: TypedTransaction) -> String
defn generate_c_type(ty: Type) -> String
defn generate_c_statement(stmt: TypedStatement) -> String
defn generate_c_expression(expr: TypedExpr) -> String
defn c_escape_string(s: String) -> String
defn c_identifier(name: String) -> String
```

**Files to modify:**
- `lib/std/backend_c.bv` - New module

**Acceptance criteria:**
- [ ] Generates valid C code
- [ ] Compiles with gcc/clang
- [ ] Header file has correct declarations

---

### 7.3 WASM Backend

**Implementation:**
```brief
defn generate_wasm(program: TypedProgram) -> WasmOutput
defn generate_wasm_rust(program: TypedProgram) -> String  // Via Rust
defn generate_wasm_bindings(program: TypedProgram) -> String
defn generate_wasm_js() -> String
```

**Files to modify:**
- `lib/std/backend_wasm.bv` - New module

**Acceptance criteria:**
- [ ] Generates valid WASM
- [ ] JS bindings work in browser
- [ ] Can call transactions from JS

---

## Tier 8: Infrastructure

**Status:** ❌ Not started  
**Priority:** HIGH - needed early for file I/O  
**Estimated:** 3-4 days

### 8.1 Source Spans

**Implementation:**
```brief
struct Span {
    file: String,
    line: Int,
    column: Int,
    offset: Int,
    length: Int
}

defn span_from_positions(file: String, start: Int, end: Int) -> Span
defn span_to_line_col(source: String, offset: Int) -> (Int, Int)
defn format_span(span: Span) -> String
defn format_error_with_span(message: String, span: Span, source: String) -> String
defn highlight_span(source: String, span: Span) -> String
defn merge_spans(a: Span, b: Span) -> Span
defn span_contains(parent: Span, child: Span) -> Bool
```

**Files to modify:**
- `lib/std/span.bv` - New module

**Acceptance criteria:**
- [ ] Can create spans from offsets
- [ ] Can convert to line/column
- [ ] Error formatting shows source snippet

---

### 8.2 File I/O (FFI initially, native later)

**Implementation:**
```brief
// FFI signatures (backed by Rust/C initially)
frgn __read_file(path: String) -> Result<String, IOError> from "std/fs.toml"
frgn __write_file(path: String, content: String) -> Result<Void, IOError> from "std/fs.toml"
frgn __file_exists(path: String) -> Result<Bool, IOError> from "std/fs.toml"
frgn __delete_file(path: String) -> Result<Void, IOError> from "std/fs.toml"
frgn __list_directory(path: String) -> Result<List<String>, IOError> from "std/fs.toml"
frgn __create_dir(path: String) -> Result<Void, IOError> from "std/fs.toml"
frgn __is_file(path: String) -> Result<Bool, IOError> from "std/fs.toml"
frgn __is_dir(path: String) -> Result<Bool, IOError> from "std/fs.toml"

// Native wrappers
defn read_file(path: String) -> Result<String, IOError> {
    __read_file(path)
}

// Path manipulation
defn join_path(parts: List<String>) -> String
defn split_path(path: String) -> List<String>
defn file_extension(path: String) -> String
defn file_stem(path: String) -> String
defn directory(path: String) -> String
defn absolute_path(path: String) -> String
defn normalize_path(path: String) -> String
```

**Files to modify:**
- `lib/std/fs.bv` - New module
- `lib/std/path.bv` - New module

**Acceptance criteria:**
- [ ] Can read and write files
- [ ] Path manipulation works
- [ ] Error handling for missing files

---

### 8.3 IO Error Types

**Implementation:**
```brief
enum IOError {
    NotFound(String),
    PermissionDenied(String),
    AlreadyExists(String),
    InvalidData(String),
    TimedOut,
    Other(String)
}

defn format_io_error(err: IOError) -> String
```

**Files to modify:**
- `lib/std/errors.bv` - Add IOError

**Acceptance criteria:**
- [ ] All error variants defined
- [ ] Can format errors with path info

---

### 8.4 Process Spawning (for bootstrap)

**Implementation:**
```brief
// For bootstrap: Brief compiler calls rustc/gcc
frgn __spawn(command: String, args: List<String>) -> Result<Int, IOError> from "std/process.toml"
frgn __spawn_with_output(command: String, args: List<String>) -> Result<(Int, String, String), IOError> from "std/process.toml"
frgn __env_var(name: String) -> Option<String> from "std/process.toml"
frgn __set_env_var(name: String, value: String) -> Result<Void, IOError> from "std/process.toml"
frgn __current_dir() -> Result<String, IOError> from "std/process.toml"
frgn __set_current_dir(path: String) -> Result<Void, IOError> from "std/process.toml"
```

**Files to modify:**
- `lib/std/process.bv` - New module

**Acceptance criteria:**
- [ ] Can spawn subprocesses
- [ ] Can capture stdout/stderr
- [ ] Environment access works

---

## Tier 9: Standard Library Extensions

**Status:** ❌ Not started  
**Priority:** MEDIUM - needed throughout  
**Estimated:** 3-4 days

### 9.1 Complete Result Type

**Implementation:**
```brief
enum Result<T, E> {
    Ok(T),
    Err(E)
}

// Basic operations
defn is_ok<T, E>(result: Result<T, E>) -> Bool
defn is_err<T, E>(result: Result<T, E>) -> Bool
defn unwrap<T, E>(result: Result<T, E>) -> T  // Panics on Err
defn unwrap_err<T, E>(result: Result<T, E>) -> E  // Panics on Ok
defn expect<T, E>(result: Result<T, E>, message: String) -> T

// Functional operations
defn map<T, E, U>(result: Result<T, E>, f: T -> U) -> Result<U, E>
defn map_err<T, E, F>(result: Result<T, E>, f: E -> F) -> Result<T, F>
defn and_then<T, E, U>(result: Result<T, E>, f: T -> Result<U, E>) -> Result<U, E>
defn or_else<T, E, F>(result: Result<T, E>, f: E -> Result<T, F>) -> Result<T, F>
defn unwrap_or<T, E>(result: Result<T, E>, default: T) -> T
defn unwrap_or_else<T, E>(result: Result<T, E>, f: E -> T) -> T
defn unwrap_or_default<T, E>(result: Result<T, E>) -> T  // Requires Default trait

// Combinators
defn and<T, E, U>(result: Result<T, E>, other: Result<U, E>) -> Result<U, E>
defn or<T, E>(result: Result<T, E>, other: Result<T, E>) -> Result<T, E>
defn filter<T, E>(result: Result<T, E>, pred: T -> Bool) -> Result<T, E>
```

**Files to modify:**
- `lib/std/result.bv` - Expand existing

**Acceptance criteria:**
- [ ] All methods work correctly
- [ ] Method chaining supported
- [ ] Integrates with pattern matching

---

### 9.2 Complete Option Type

**Implementation:**
```brief
enum Option<T> {
    Some(T),
    None
}

// Basic operations
defn is_some<T>(opt: Option<T>) -> Bool
defn is_none<T>(opt: Option<T>) -> Bool
defn unwrap<T>(opt: Option<T>) -> T  // Panics on None
defn expect<T>(opt: Option<T>, message: String) -> T
defn unwrap_or<T>(opt: Option<T>, default: T) -> T
defn unwrap_or_else<T>(opt: Option<T>, f: () -> T) -> T
defn unwrap_or_default<T>(opt: Option<T>) -> T  // Requires Default trait

// Functional operations
defn map<T, U>(opt: Option<T>, f: T -> U) -> Option<U>
defn map_or<T, U>(opt: Option<T>, default: U, f: T -> U) -> U
defn map_or_else<T, U>(opt: Option<T>, default: () -> U, f: T -> U) -> U
defn and_then<T, U>(opt: Option<T>, f: T -> Option<U>) -> Option<U>
defn or_else<T>(opt: Option<T>, f: () -> Option<T>) -> Option<T>
defn filter<T>(opt: Option<T>, pred: T -> Bool) -> Option<T>

// Combinators
defn and<T, U>(opt: Option<T>, other: Option<U>) -> Option<U>
defn or<T>(opt: Option<T>, other: Option<T>) -> Option<T>
defn xor<T>(opt: Option<T>, other: Option<T>) -> Option<T>  // One or the other, not both
```

**Files to modify:**
- `lib/std/option.bv` - New module

**Acceptance criteria:**
- [ ] All methods work correctly
- [ ] Method chaining supported
- [ ] Integrates with pattern matching

---

### 9.3 Iterators (Advanced)

**Implementation:**
```brief
// Iterator trait
trait Iterator<T> {
    defn next(self) -> Option<T>;
}

// Iterator adapters
defn map<I, T, U>(iter: I, f: T -> U) -> impl Iterator<U>
    where I: Iterator<T>
{
    // ...
}

defn filter<I, T>(iter: I, pred: T -> Bool) -> impl Iterator<T>
    where I: Iterator<T>
{
    // ...
}

defn take<I, T>(iter: I, n: Int) -> impl Iterator<T>
    where I: Iterator<T>
{
    // ...
}

defn skip<I, T>(iter: I, n: Int) -> impl Iterator<T>
    where I: Iterator<T>
{
    // ...
}

defn enumerate<I, T>(iter: I) -> impl Iterator<(Int, T)>
    where I: Iterator<T>
{
    // ...
}

// Collection iterators
defn iter<T>(list: List<T>) -> impl Iterator<T>
defn iter_mut<T>(list: List<T>) -> impl Iterator<&mut T>
defn keys<K, V>(map: HashMap<K, V>) -> impl Iterator<K>
defn values<K, V>(map: HashMap<K, V>) -> impl Iterator<V>
```

**Note:** This requires `impl Trait` syntax which may need compiler changes.

**Files to modify:**
- `lib/std/iterator.bv` - New module
- `src/ast.rs` - May need `impl Trait` support

**Acceptance criteria:**
- [ ] Can define iterator trait
- [ ] Adapters work correctly
- [ ] Lazy evaluation

---

### 9.4 Comparison and Ordering

**Implementation:**
```brief
enum Ordering {
    Less,
    Equal,
    Greater
}

defn compare_int(a: Int, b: Int) -> Ordering
defn compare_float(a: Float, b: Float) -> Ordering
defn compare_string(a: String, b: String) -> Ordering
defn compare_char(a: Char, b: Char) -> Ordering
defn compare_bool(a: Bool, b: Bool) -> Ordering

defn min<T>(a: T, b: T) -> T where T: Ord
defn max<T>(a: T, b: T) -> T where T: Ord
defn clamp<T>(value: T, min: T, max: T) -> T where T: Ord
```

**Files to modify:**
- `lib/std/ord.bv` - New module

**Acceptance criteria:**
- [ ] Ordering enum works
- [ ] Comparison functions correct
- [ ] Generic min/max/clamp work

---

## Implementation Phases

### Phase 1: Foundation (Week 1)
- [ ] Tier 1: Core Data Types
- [ ] Tier 2: String & Text Processing
- [ ] Tier 8: Infrastructure (partial - spans, basic I/O)

### Phase 2: Frontend (Week 2)
- [ ] Tier 3: Lexer Components
- [ ] Tier 4: Parser Components
- [ ] Tier 5: Type Checker (partial - without traits)

### Phase 3: Advanced Features (Week 3)
- [ ] Tier 5: Type Checker (complete - with traits)
- [ ] Tier 6: Proof Engine
- [ ] Tier 9: Standard Library Extensions

### Phase 4: Backends (Week 4)
- [ ] Tier 7: Code Generation Backends
- [ ] Integration testing
- [ ] Bootstrap: compile compiler with itself

---

## Testing Strategy

### Unit Tests
Each module should have comprehensive tests:
```brief
// Example: HashMap tests
defn test_hashmap_insert_get() -> Bool {
    let map = new_map<Int, String>();
    let map = insert(map, 1, "one");
    let map = insert(map, 2, "two");
    get(map, 1) == Some("one") && get(map, 2) == Some("two")
}

defn test_hashmap_missing_key() -> Bool {
    let map = new_map<Int, String>();
    get(map, 42) == None
}
```

### Integration Tests
- Lexer + Parser: tokenize and parse example programs
- Parser + Typechecker: typecheck parsed programs
- Full pipeline: compile Brief programs to Rust/C

### Bootstrap Test
Final test: compile the Brief compiler (written in Brief) using itself

---

## Risk Assessment

### High Risk
1. **Trait system** - Major language extension, may require compiler changes
2. **Proof engine** - Symbolic execution is complex
3. **Generics with trait bounds** - Type inference becomes harder

### Medium Risk
1. **HashMap/HashSet** - Need efficient implementation
2. **Unicode handling** - Edge cases in UTF-8
3. **Error messages** - Hard to make them as good as Rust's

### Low Risk
1. **Lexer/Parser** - Standard algorithms, well understood
2. **Code generation** - Straightforward tree traversal
3. **Standard library** - Can be built incrementally

---

## Success Metrics

### Milestone 1: Lexer Complete
- Can tokenize any valid Brief program
- Produces correct tokens with spans
- Error messages point to right location

### Milestone 2: Parser Complete
- Can parse any valid Brief program
- Produces correct AST
- Handles all syntax variants

### Milestone 3: Type Checker Complete
- Infers types correctly
- Enforces trait bounds
- Reports meaningful errors

### Milestone 4: Proof Engine Complete
- Verifies contracts correctly
- Finds counterexamples
- Detects conflicts

### Milestone 5: Self-Hosting Complete
- Compiler written in Brief
- Can compile itself
- Output matches Rust compiler output

---

## Notes

### Compiler Changes Required
Some features require changes to the Rust compiler:
1. **Char type** - New primitive type
2. **Trait system** - New language feature
3. **impl Trait** - For iterators
4. **HashMap/HashSet** - Built-in or FFI

### FFI Bootstrap Strategy
Initially, some features can be FFI-backed:
1. File I/O
2. Process spawning
3. HashMap/HashSet (until native implementation ready)
4. Unicode operations

Gradually replace FFI with native implementations.

### Parallel Development
Multiple tiers can be developed in parallel:
- Tier 1 + Tier 8 can start immediately
- Tier 2 depends on Tier 1 (Char type)
- Tier 3 depends on Tier 2 (string operations)
- Tier 4 depends on Tier 3 (lexer output)
- Tier 5 depends on Tier 4 (AST types)
- Tier 6 depends on Tier 5 (typed AST)
- Tier 7 depends on Tier 5 (typed AST)
- Tier 9 can be parallel with most tiers

---

**Last updated:** 2026-05-06  
**Status:** Planning complete, ready for implementation
