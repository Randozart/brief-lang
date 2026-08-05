# Tier 3: Lexer Components - COMPLETE

**Status:** ✅ Complete (2026-05-06)  
**Implementation Time:** ~45 minutes  
**Files:** 2 new stdlib modules

---

## Overview

Tier 3 implements the complete lexer (tokenizer) for Briv in pure Briv. No FFI required - uses only Tier 1 and Tier 2 native functions.

**Components:**
1. **Token Type** (`token.bv`) - Token enum and utilities
2. **Lexer Implementation** (`lexer.bv`) - Full tokenizer

---

## 3.1 Token Type (token.bv)

**File:** `lib/std/token.bv`

### Token Enum Variants

**Literals:**
- `TokenInt(Int)` - Integer literals
- `TokenFloat(Float)` - Float literals
- `TokenString(String)` - String literals
- `TokenChar(Char)` - Char literals

**Keywords (30+):**
- State: `KeywordLet`, `KeywordConst`, `KeywordState`
- Transactions: `KeywordTxn`, `KeywordRct`, `KeywordAsync`, `KeywordTerm`, `KeywordEscape`
- Functions: `KeywordDefn`, `KeywordSig`, `KeywordSign`, `KeywordSignature`
- FFI: `KeywordFrgn`, `KeywordSyscall`, `KeywordResource`, `KeywordRsrc`
- Types: `KeywordStruct`, `KeywordRstruct`, `KeywordEnum`, `KeywordType`
- Control: `KeywordImport`, `KeywordFrom`, `KeywordAs`, `KeywordForall`, `KeywordExists`, `KeywordWithin`
- Hardware: `KeywordTrg`, `KeywordLink`, `KeywordBank`
- Assembly: `KeywordAsm`, `KeywordStage`, `KeywordOn`
- Rendered: `KeywordRender`
- Literals: `KeywordTrue`, `KeywordFalse`, `KeywordOk`, `KeywordErr`

**Operators (single-char):**
- `OpPlus` (+), `OpMinus` (-), `OpStar` (*), `OpSlash` (/), `OpPercent` (%)
- `OpEq` (=), `OpBang` (!), `OpAmp` (&), `OpPipe` (|), `OpCaret` (^)
- `OpTilde` (~), `OpQuestion` (?), `OpAt` (@), `OpDot` (.)
- `OpColon` (:), `OpSemicolon` (;), `OpComma` (,)

**Operators (multi-char):**
- Comparison: `OpEqEq` (==), `OpNeq` (!=), `OpLt` (<), `OpGt` (>), `OpLtEq` (<=), `OpGtEq` (>=)
- Logical: `OpAnd` (&&), `OpOr` (||)
- Shift: `OpLtLt` (<<), `OpGtGt` (>>)
- Arrow: `OpArrow` (->), `OpFatArrow` (=>)
- Compound: `OpPlusEq` (+=), `OpMinusEq` (-=), etc.
- Range: `OpDotDot` (..), `OpDotDotEq` (..=)

**Delimiters:**
- `DelimLParen` ((), `DelimRParen` ())
- `DelimLBrace` ({), `DelimRBrace` })
- `DelimLBracket` ([), `DelimRBracket` ])

**Special:**
- `TokenIdentifier(String)` - Identifiers
- `TokenComment(String)` - Comments
- `TokenWhitespace(String)` - Whitespace
- `TokenNewline` - Newline
- `TokenEof` - End of file
- `TokenError(String)` - Lexing error

### Token Utilities

```briv
// Keyword detection
defn is_keyword(tok: Token) -> Bool

// Keyword to string
defn keyword_to_string(tok: Token) -> String

// Operator utilities
defn is_operator(tok: Token) -> Bool
defn operator_precedence(tok: Token) -> Int
defn is_right_associative(tok: Token) -> Bool

// Token utilities
defn token_eq(a: Token, b: Token) -> Bool
defn token_to_string(tok: Token) -> String  // For error messages
defn is_literal(tok: Token) -> Bool
defn literal_to_string(tok: Token) -> String
```

---

## 3.2 Lexer Implementation (lexer.bv)

**File:** `lib/std/lexer.bv`

### Lexer State

```briv
struct LexerState {
    source: String,       // Source code
    position: Int,        // Current position
    line: Int,            // Current line (1-indexed)
    column: Int,          // Current column (1-indexed)
    start_pos: Int,       // Start of current token
    start_line: Int,      // Start line of current token
    start_column: Int     // Start column of current token
}
```

### Lexer Construction

```briv
defn new_lexer(source: String) -> LexerState
```

### Character Access

```briv
defn current_char(state: LexerState) -> Option<Char>
defn peek_char(state: LexerState, offset: Int) -> Option<Char>
defn advance(state: LexerState) -> LexerState
defn skip_chars(state: LexerState, count: Int) -> LexerState
```

### Token Extraction

```briv
defn current_token_text(state: LexerState) -> String
defn start_token(state: LexerState) -> LexerState
```

### Lexing Functions

**Whitespace/Comments:**
```briv
defn skip_whitespace(state: LexerState) -> LexerState
defn skip_comment(state: LexerState) -> LexerState
```

**Token Readers:**
```briv
defn read_identifier(state: LexerState) -> (Token, LexerState)
defn read_number(state: LexerState) -> (Token, LexerState)
defn read_hex_number(state: LexerState) -> (Token, LexerState)
defn read_float_fraction(state: LexerState) -> (Token, LexerState)
defn read_string(state: LexerState) -> (Token, LexerState)
defn read_char_literal(state: LexerState) -> (Token, LexerState)
```

### Main Lexer Function

```briv
defn next_token(state: LexerState) -> Result<(Token, LexerState), String>
```

**Handles:**
- Whitespace skipping
- Comment skipping (// style)
- EOF detection
- Single-char operators
- Multi-char operators (with lookahead)
- Literals (int, float, string, char)
- Identifiers and keywords
- Error reporting

### Full Tokenization

```briv
defn tokenize(source: String) -> Result<List<Token>, String>
```

---

## Usage Examples

### Basic Tokenization

```briv
import std.lexer;
import std.token;

let source = "let x: Int = 42;";
let result = tokenize(source);

[result.is_ok()] {
    let tokens = result.unwrap();
    // tokens: [KeywordLet, TokenIdentifier("x"), OpColon, 
    //          KeywordInt, OpEq, TokenInt(42), OpSemicolon, TokenEof]
};
```

### Manual Lexing

```briv
let state = new_lexer("let x = 1;");

let (tok1, state) = next_token(state).unwrap();  // KeywordLet
let (tok2, state) = next_token(state).unwrap();  // TokenIdentifier("x")
let (tok3, state) = next_token(state).unwrap();  // OpEq
let (tok4, state) = next_token(state).unwrap();  // TokenInt(1)
let (tok5, state) = next_token(state).unwrap();  // OpSemicolon
let (tok6, state) = next_token(state).unwrap();  // TokenEof
```

### Error Handling

```briv
let result = tokenize("let x = @invalid;");

[result.is_err()] {
    let error = result.unwrap_err();
    println("Lex error: " + error);
};
```

---

## Implementation Details

### Keyword Recognition

Keywords are recognized by reading an identifier, then checking against a list:

```briv
defn read_identifier(state: LexerState) -> (Token, LexerState) {
    // Read alphanumeric + underscore
    [current_char(state).is_some()] {
        let c = current_char(state).unwrap();
        [is_alphanumeric(c) || c == '_'] {
            &state = advance(state);
            let (tok, state) = read_identifier(state);
            term (tok, state);
        };
    };
    
    // Check if keyword
    let text = current_token_text(state);
    [text == "let"] { term (KeywordLet, state); };
    [text == "txn"] { term (KeywordTxn, state); };
    // ... etc
    term (TokenIdentifier(text), state);
};
```

### Number Parsing

Supports decimal, hex, and float:

```briv
defn read_number(state: LexerState) -> (Token, LexerState) {
    // Check for hex (0x...)
    [current_char(state).unwrap() == '0'] {
        [peek_char(state, 1).unwrap() == 'x'] {
            &state = advance(state);
            &state = advance(state);
            let (tok, state) = read_hex_number(state);
            term (tok, state);
        };
    };
    
    // Read decimal digits
    [is_digit(current_char(state).unwrap())] {
        &state = advance(state);
        let (tok, state) = read_number(state);
        term (tok, state);
    };
    
    // Check for float
    [current_char(state).unwrap() == '.'] {
        [is_digit(peek_char(state, 1).unwrap())] {
            &state = advance(state);
            let (tok, state) = read_float_fraction(state);
            term (tok, state);
        };
    };
    
    // Return integer
    let text = current_token_text(state);
    term (TokenInt(text.to_int()), state);
};
```

### String Escape Sequences

Handles \n, \t, \r, \\, \":

```briv
defn read_string(state: LexerState) -> (Token, LexerState) {
    &state = advance(state);  // Skip opening "
    
    let sb = new_builder();
    
    [current_char(state).unwrap() != '"'] {
        [current_char(state).unwrap() == '\\'] {
            &state = advance(state);
            let escaped = current_char(state).unwrap();
            [escaped == 'n'] { sb = sb.append_char('\n'); };
            [escaped == 't'] { sb = sb.append_char('\t'); };
            // ... etc
            &state = advance(state);
        };
        [current_char(state).unwrap() != '\\'] {
            sb = sb.append_char(current_char(state).unwrap());
            &state = advance(state);
        };
    };
    
    &state = advance(state);  // Skip closing "
    term (TokenString(sb.to_string()), state);
};
```

---

## Performance Characteristics

| Operation | Complexity | Notes |
|-----------|------------|-------|
| **next_token** | O(n) | n = token length |
| **tokenize** | O(n) | n = source length |
| **read_identifier** | O(n) | Single pass |
| **read_number** | O(n) | Single pass |
| **read_string** | O(n) | Single pass with StringBuilder |
| **skip_whitespace** | O(n) | Single pass |
| **keyword lookup** | O(1) | Direct comparison |

---

## Testing

All lexer features tested:
- ✅ Integer literals (decimal, hex)
- ✅ Float literals
- ✅ String literals (with escapes)
- ✅ Char literals (with escapes)
- ✅ All keywords
- ✅ All operators
- ✅ All delimiters
- ✅ Identifiers
- ✅ Comments
- ✅ Whitespace
- ✅ Error handling

---

## Integration with Parser

The lexer produces a `List<Token>` that the parser consumes:

```briv
defn parse_program(source: String) -> Result<Program, ParseError> {
    let tokens = tokenize(source)?;
    let parser = new_parser(tokens);
    parser.parse()
};
```

---

## Next Steps

With Tier 3 complete, the lexer is ready. Next is **Tier 4: Parser Components**:

1. Define AST types in Briv (structs and enums)
2. Implement recursive descent parser
3. Use Stack for expression parsing
4. Error reporting with spans

---

*Last updated: 2026-05-06*  
*Status: Tier 3 COMPLETE ✅*
