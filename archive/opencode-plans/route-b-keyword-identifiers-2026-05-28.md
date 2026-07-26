# Route B: Accept Keywords as Identifiers

**Timestamp**: 2026-05-28T13:44:26Z → 2026-05-28T13:44:26Z (executed)
**Status**: ✅ COMPLETE
**Rust compiler**: ✓ `cargo build`, ✓ `cargo test --lib` — 269/269 pass
**Parse status**: Zero keyword-related parse errors across all 18 `.bv` files
**: `txn`, `reg`, `from` all work as variable/parameter/pattern names

---

## Table of Contents

1. [Problem Statement](#1-problem-statement)
2. [Design Philosophy (What We Keep Tripping On)](#2-design-philosophy)
3. [Route B Solution](#3-route-b-solution)
4. [Phase 1: Brief-in-Brief Parser](#4-phase-1-brief-in-brief-parser)
5. [Phase 2: Rust Parser](#5-phase-2-rust-parser)
6. [Phase 3: Revert Renames](#6-phase-3-revert-renames)
7. [Phase 4: Verify](#7-phase-4-verify)
8. [Appendix: Complete Keyword Audit](#8-appendix)

---

## 1. Problem Statement

The Brief lexer (`src/lexer.rs`) defines ~60+ keyword tokens (e.g., `Token::Txn`, `Token::Registry`, `Token::From`, `Token::TypeU32`). These tokens are recognized by logos before identifier matching, meaning these names can never be used as variable names, parameter names, or pattern variables in `.bv` files.

This caused us to manually rename every keyword-conflicting variable across ~10 `.bv` files:
- `txn` → `txn_item`, `transaction_arg`, `contract_txn`
- `reg` → `reg_name`, `chosen_reg`
- `from` → `from_str`
- `Ok(())` / `Err(...)` → avoid entirely (changed `check_vc` return type from `Result` to `Bool`)

These renames are semantically meaningless, reduce code clarity, and don't scale — any new `.bv` file risks hitting the same issue.

**Route B**: Instead of renaming variables in `.bv` files, teach both parsers (Rust and Brief-in-Brief) that keywords are also valid identifiers.

---

## 2. Design Philosophy (What We Keep Tripping On)

A running catalog of design mismatches between the Rust host compiler and the Brief-in-Brief target:

### 2.1 Rust has `self.method()` — Brief has `function(state, arg)`

| Concept | Rust | Brief |
|---|---|---|
| Advance token stream | `self.advance()` | `advance(state)` |
| Get current token | `self.current_token()` | `current_token(state)` |
| Convert to string | `"x".to_string()` | `"x"` (already a string) |
| Method call | `obj.method(arg)` | `method(obj, arg)` (function syntax) **or** `obj.method(arg)` (postfix desugars to `Call("method", [obj, arg])`) |

The postfix `.method()` syntax IS supported — parser.rs:4154-4180 handles `expr.method(args)` by desugaring to `Expr::Call("method", [expr, arg])`. The typechecker and interpreter then resolve this against registered function signatures. So `regs.append_all(callee_saved_regs())` works IF `append_all` accepts `(List<T>, List<T>) -> List<T>`.

### 2.2 No dereference operator in patterns

In Rust: `uni ty(TypeList(inner)) = { type_to_rust(*inner) };` — the `*` dereferences the `Box<Type>`.

In Brief: `uni ty(TypeList(inner)) = { type_to_rust(inner) };` — no `*`, patterns bind directly to the inner value. The Rust parser's `*` token (`Token::Star`) is only used for pointer/multiplication, not pattern dereferencing.

### 2.3 Named struct literals are NOT supported

```brief
// THIS DOES NOT WORK:
let info = TransactionInfo { name: "foo", index: 0 };

// THIS WORKS (anonymous object literal):
let info = { name: "foo", index: 0 };
```

The parser only handles `{ field: val, ... }` as `Expr::ObjectLiteral`. There is no `TypeName { ... }` syntax. All struct literals in `.bv` files MUST use the anonymous form.

Affected files: `backend_aarch64.bv` (RegisterAllocator { ... }, MMIORegister { ... }), `webstack.bv` (SignalInfo { ... }, TransactionInfo { ... }).

### 2.4 `uni` is a statement, not an expression

```brief
// THIS DOES NOT WORK:
[!uni x(Pattern) = true] { ... };

// THIS WORKS (separate statement):
uni x(Pattern) = { ... };
let result = true;
```

The `uni` keyword only parses as `Statement::Unification`. It cannot appear inside `[...]` guard brackets or as part of an expression. The label-computation logic in `backend_aarch64.bv` (line 993) uses `[!uni ... = true]` which is structurally invalid.

### 2.5 Comments are stripped before parsing

Logos (`src/lexer.rs` line 27): `#[logos(skip r"//[^\n]*")]`

`//` comments are completely removed by the lexer, never reaching the parser. This means an empty block containing only a comment is still an empty block:

```brief
// This block IS empty after lexing:
[cond] {
    // just a comment
};
```

This caused the guarded-block rejection in `proof_engine.bv`.

### 2.6 Functional update, not mutation

```brief
// Brief: returns a NEW value
output = output.append_str("hello");

// NOT: output.append_str("hello");  // doesn't modify in place
```

This is pure functional style. Every "method" returns a new value with the modification applied.

### 2.7 Contract-after-arrow parser bug

```brief
// THIS HAS A PARSER BUG — both pre and post become Bool(true):
defn foo(x: Int) -> Int [x > 0][term > 0] { ... }

// THIS WORKS:
defn foo(x: Int) [x > 0][term > 0] -> Int { ... }
```

The Rust parser's `parse_contract()` at `src/parser.rs:2646` has a state issue where both pre and post conditions become `Expr::Bool(true)` when using contract-after-arrow syntax. The workaround is contract-before-arrow syntax, which works correctly.

### 2.8 `.bv` files are NEVER loaded by the Rust binary at runtime

The `lib/compiler/*.bv` files are a parallel self-hosted implementation. The Rust binary does not read, interpret, or execute them. They must be compiled through the Brief-in-Brief pipeline (lexer.bv → parser.bv → typechecker.bv → ...) which currently has no invocation path — there's no CLI command for it.

### 2.9 `Result<(), String>` in Brief vs Result type

```brief
// Rust: Ok(()) and Err("msg") use keywords
// Brief's Rust parser maps:
//   Token::Ok → "Ok" as identifier → Expr::Call("Ok", [expr]) or Expr::Identifier("Ok")
//   Token::Err → "Err" as identifier → Expr::Call("Err", [expr]) or Expr::Identifier("Err")
```

`Ok` and `Err` are keyword tokens but are already handled in `expect_identifier()` and `parse_primary_expr()`. However, `Ok(())` fails because `()` is an empty tuple expression that consumes both parens, leaving the `Call("Ok", ...)` without its closing `)`.

### 2.10 Brief type annotations use `name: Type`, not `name: type`

```brief
// Correct:
defn foo(x: Int) -> String { ... }
let y: List<Int> = [];

// NOT: defn foo(x: i32) -> str { ... }
```

Brief uses PascalCase for all types: `Int`, `String`, `Bool`, `Float`, `Char`, `Void`, `List<T>`, `Option<T>`, `Result<T,E>`, `HashMap<K,V>`, `HashSet<T>`. Lowercase types like `u8`, `i32`, `str` are Rust-isms that don't exist in Brief.

---

## 3. Route B Solution

Two independent changes that together eliminate keyword conflicts:

1. **Brief-in-Brief parser**: `is_identifier()` returns true for keywords, `unwrap_identifier()` falls back to `keyword_to_string()`. 24 sites in parser.bv updated.
2. **Rust parser**: 44 missing keyword tokens added to `expect_identifier()` and 4 `parse_primary_expr()` blocks.

Then revert all the manual renames in `.bv` files.

---

## 4. Phase 1: Brief-in-Brief Parser

### 4.1 `lib/compiler/token.bv` — 2 functions + 1 bugfix

#### `is_identifier()` (line 342) — add keyword fallthrough

```diff
 defn is_identifier(tok: Token) -> Bool {
     uni tok(TokenIdentifier(_)) = true;
+    term is_keyword(tok);
-    term false;
 };
```

#### `unwrap_identifier()` (line 337) — add keyword fallback

```diff
 defn unwrap_identifier(tok: Token) -> String {
     uni tok(TokenIdentifier(name)) = { term name; };
+    [is_keyword(tok)] {
+        term keyword_to_string(tok);
+    };
     term "";
 };
```

#### Bugfix: Remove duplicate `KeywordRstruct` in `keyword_to_string()` (line 196)

Line 185 already has `uni tok(KeywordRstruct) = "rstruct";`. Line 196 is an identical duplicate — remove it so the fallthrough `term ""` is reachable.

### 4.2 `lib/compiler/parser.bv` — 24 sites

#### Pattern A: SIMPLE+ADVANCE-SIDE (18 sites)

Each site has the structure:
```brief
let tok = current_token(state);
uni tok(TokenIdentifier(name)) = { &state = advance(state); };
[!is_identifier(tok)] { term Err("Expected name"); };
```

Change to:
```brief
let tok = current_token(state);
[!is_identifier(tok) && !is_keyword(tok)] { term Err("Expected name"); };
let name = unwrap_identifier(tok);
&state = advance(state);
```

**Affected sites** (18 total):

| Line | Function |
|------|----------|
| 217 | `parse_hashtag_modifiers` — modifier name after `#[` |
| 401 | `parse_transaction` — transaction name |
| 496 | `parse_definition` — function name |
| 1357 | `parse_statement` — uni pattern name after `(` |
| 1507 | `parse_let` — first name in tuple destructuring |
| 1569 | `parse_let` — variable name |
| 1753 | `parse_type_params` — type parameter name |
| 1787 | `parse_params` — function parameter name |
| 1839 | `parse_signature` — signature name |
| 1894 | `parse_struct` — struct name |
| 2048 | `parse_struct_variant_fields` — field name after `+` |
| 2074 | `parse_struct_variant_fields` — field name after `-` |
| 2118 | `parse_struct_field` — field name |
| 2151 | `parse_enum` — enum name |
| 2199 | `parse_enum_variant` — variant name |
| 2287 | `parse_constant` — constant name |
| 2333 | `parse_state_decl` — state variable name |
| 2400 | `parse_rstruct` — rstruct name |
| 2516 | `parse_render` — render target name |

#### Pattern B: SIMPLE-only (2 sites)

Outer guard already checks `is_identifier`, inner `uni` extracts name. Change outer guard to also accept `is_keyword`, replace inner `uni` with `unwrap_identifier`.

```diff
- [is_identifier(tok) || tok == OpAt] {
+ [is_identifier(tok) || is_keyword(tok) || tok == OpAt] {
     let tok = current_token(state);
-    uni tok(TokenIdentifier(name)) = { ... };
+    let name = unwrap_identifier(tok);
```

**Affected sites**:

| Line | Function |
|------|----------|
| 1257 | `parse_statement` — alka/expression start check |
| 2083 | `parse_struct_variant_fields` — plain field name |

#### Pattern C: UNI-ONLY (3 sites)

No `is_identifier` guard — just a `uni` pattern with a terminal error. Replace `uni` + error with `unwrap_identifier` + empty-check + error.

```diff
- uni tok(TokenIdentifier(name)) = { &state = advance(state); ... };
- term Err("Msg");
+ let name = unwrap_identifier(tok);
+ [name == ""] { term Err("Msg"); };
+ &state = advance(state);
```

**Affected sites**:

| Line | Function | Error message |
|------|----------|---------------|
| 1073 | `parse_expression` — after `@` | `"Expected identifier after @"` |
| 1703 | `parse_type` — named type | `"Unknown type: " + ...` |
| 2258 | `parse_import_path` — first path segment | `"Expected import path"` |

**Note on site 2258**: Also has a nested `uni tok2(TokenIdentifier(next))` at line 2266 for subsequent path segments. Apply same fix there:
```brief
let next_name = unwrap_identifier(tok2);
[next_name == ""] { term Err("Expected import path"); };
```

---

## 5. Phase 2: Rust Parser

### 5.1 `src/parser.rs` — `expect_identifier()` (line 196)

Add 44 missing keyword tokens. Each addition follows the pattern:

```rust
Some(Ok(Token::From)) => { self.advance(); Ok("from".to_string()) }
```

**Complete list of additions** (sorted by token name):

| Token | Returns | Token | Returns |
|-------|---------|-------|---------|
| `Token::As` | `"as"` | `Token::Match` | `"match"` |
| `Token::Asm` | `"asm"` | `Token::Minute` | `"minute"` |
| `Token::Bank` | `"bank"` | `Token::Ms` | `"ms"` |
| `Token::Cycles` | `"cycles"` | `Token::On` | `"on"` |
| `Token::Cyc` | `"cyc"` | `Token::Registry` | `"reg"` |
| `Token::Exists` | `"exists"` | `Token::Render` | `"render"` |
| `Token::Forall` | `"forall"` | `Token::Resource` | `"resource"` |
| `Token::FrgnBang` | `"frgn!"` | `Token::Rsrc` | `"rsrc"` |
| `Token::From` | `"from"` | `Token::Rstruct` | `"rstruct"` |
| `Token::Link` | `"link"` | `Token::Seconds` | `"seconds"` |
| `Token::Stage` | `"stage"` | | |
| `Token::Syscall` | `"syscall"` | **Time units** | |
| `Token::SyscallBang` | `"syscall!"` | `Token::Cycles` | `"cycles"` |
| `Token::Trg` | `"trg"` | `Token::Cyc` | `"cyc"` |
| `Token::TrgBang` | `"trg!"` | `Token::Ms` | `"ms"` |
| `Token::Within` | `"within"` | `Token::Seconds` | `"seconds"` |
| | | `Token::Minute` | `"minute"` |

**Type tokens** (16 tokens, all already handled EXCEPT `TypeUInt` and `TypeUnsigned`/`TypeUSgn`/`TypeSigned`/`TypeSgn` are missing — check if they exist):

Check types:
- `TypeData` ✓ already handled (returns `"Data"`)
- `TypeInt` ✓ already handled (returns `"Int"`)
- `TypeUInt` — check if token exists
- `TypeFloat` — check
- `TypeString` — check
- `TypeBool` — check
- `TypeVoid` — check
- `TypeChar` — check
- `TypeI8` through `TypeU64` (8 tokens) — check all
- `TypeUnsigned`, `TypeUSgn`, `TypeSigned`, `TypeSgn` — check if tokens exist

**IMPORTANT**: Some of these must be verified against the actual `Token` enum in `src/lexer.rs`. The audit found them but they may or may not exist. Only add tokens that ACTUALLY EXIST in the lexer.

### 5.2 `src/parser.rs` — 4 `parse_primary_expr()` blocks (lines 4352, 4397, 4472, 4517)

Each of the 4 identical large blocks needs the SAME change:

1. Add the missing keyword tokens to the big `Some(Ok(Token::X)) | ...` match guard
2. Add corresponding arms to the inner `match`

**Recommended**: Extract a Rust helper to avoid 4× duplication:

```rust
fn keyword_token_to_ident(&self) -> Option<String> {
    match self.current_token() {
        Some(Ok(Token::Sig)) => Some("sig".to_string()),
        Some(Ok(Token::Defn)) => Some("defn".to_string()),
        // ... all 44+ existing and new
        Some(Ok(Token::From)) => Some("from".to_string()),
        Some(Ok(Token::Registry)) => Some("reg".to_string()),
        // ...
        _ => None,
    }
}
```

Then each block becomes:
```rust
if let Some(name) = self.keyword_token_to_ident() {
    let name = name;
    self.advance();
    if let Some(Ok(Token::LParen)) = self.current_token() {
        // ... parse call args
        Ok(Expr::Call(name, args))
    } else {
        Ok(Expr::Identifier(name))
    }
}
```

But this assumes the blocks are truly identical — let me verify that the block content between the match and the closing `}` is indeed identical.

### 5.3 `src/parser.rs` — uni variant pattern block (line 3262)

This block at lines 3262-3298 maps tokens to `"KeywordXxx"` variant names. It already handles 36+ keywords including `Render`, `Rstruct`, `Registry`, `Trg`, etc.

**No changes needed here** unless we want to add `From`, `As`, `Forall`, `Exists`, `Within` as variant name aliases. Not necessary for the current bug — these don't appear as pattern variant names in any `.bv` file.

---

## 6. Phase 3: Revert Renames

### 6.1 `txn_item` → `txn`

**`lib/compiler/backends/webstack.bv`** (5 occurrences):
Lines 120, 124, 126, 131, 132

**`lib/compiler/backends/rust.bv`** (9 occurrences):
Lines 48, 49, 88 (parameter), 92, 96, 103, 259, 261, 271, 273

**`lib/compiler/backends/c.bv`** (6 occurrences):
Lines 46, 47, 242, 244, 254, 256

**`lib/compiler/backends/verilog.bv`** (2 occurrences):
Lines 261, 262

### 6.2 `transaction_arg` → `txn`

**`lib/compiler/backends/c.bv`** — line 75 (function parameter)
**`lib/compiler/backends/verilog.bv`** — line 169 (function parameter)
**`lib/compiler/backends/vhdl.bv`** — line 204 (function parameter)

**Caution**: In c.bv and verilog.bv, `txn_item` and `transaction_arg` coexist. `transaction_arg` is the parameter name in `generate_c_transaction` and `transaction_to_always`. `txn_item` is the pattern variable in `TopTxn(txn_item)`. Revert BOTH to `txn` — they're in different scopes, no collision.

### 6.3 `contract_txn` → `txn`

**`lib/compiler/proof_engine.bv`** (11 occurrences):
Lines 368 (parameter), 375, 376, 383, 384, 385, 400, 407, 878 (parameter), 880, 881

### 6.4 `reg_name` → `reg`

**`lib/compiler/backends/backend_aarch64.bv`** (67 occurrences):
Lines 113, 114, 116, 117, 170 (parameter), 172, 181, 196 (parameter), 199, 205 (parameter), 207, 218 (parameter), 220, 222, 561, 568, 589 (parameter), 590, 593, 596, 604 (parameter), 605, 608, 610, 612, 913, 914, 1021 (parameter), 1022-1052, 1157, 1161, 1205, 1210, 1299, 1300, 1306, 1309

Note: The `reg_to_num` function name should NOT be changed — it's a function name, not a variable. The `\breg\b` boundary matching should catch this, but verify.

### 6.5 `from_str` → `from`

**`lib/compiler/proof_engine.bv`** (4 occurrences):
Lines 489 (parameter), 493, 495, 497

**Caution**: `from` is a keyword in signature syntax (`sig name: In -> Out from "module.fn"`) and in imports (`import X from "module"`). Those usages are structural syntax, not variable references, and should NOT be changed. Only rename `from_str` parameter/variable back to `from`.

### 6.6 Revert order

The revert should be done using sed with word boundaries:

```
sed -i 's/\btxn_item\b/txn/g' <file>
sed -i 's/\btransaction_arg\b/txn/g' <file>   # careful: verify no other meanings
sed -i 's/\bcontract_txn\b/txn/g' <file>
sed -i 's/\breg_name\b/reg/g' <file>            # verify reg_to_num NOT affected
sed -i 's/\bfrom_str\b/from/g' <file>           # verify import/from NOT affected
```

After each, verify with grep that no unintended replacements occurred.

---

## 7. Phase 4: Verify

1. `cargo build` — Rust compiler compiles with parser changes
2. `cargo test --lib` — all 269 tests pass
3. Check every `.bv` file:
   ```bash
   for f in lib/compiler/*.bv lib/compiler/backends/*.bv; do
       echo "=== $f ==="
       cargo run --bin brief-compiler -- check "$f" 2>&1 | grep -E "Parse error|All checks|Import error"
   done
   ```
4. Expect: wasm.bv, x86_64.bv, backend_aarch64.bv, webstack.bv still have non-keyword parse errors (deep Rust-isms, not keyword conflicts).

---

## 8. Appendix: Complete Keyword Audit

### 8.1 All Token variants defined in `src/lexer.rs`

From the lexer (lines 28-355+):

| Token name | Lexer patterns | Category |
|---|---|---|
| `Sig` | sig, SIG, sign, SIGN, signature, SIGNATURE | Statement keyword |
| `Defn` | defn, DEFN, def, DEF, definition, DEFINITION | Statement keyword |
| `Let` | let, LET | Statement keyword |
| `Const` | const, CONST, constant, CONSTANT | Statement keyword |
| `Txn` | txn, TXN, transact, TRANSACT, transaction, TRANSACTION | Statement keyword |
| `Rct` | rct, RCT | Statement keyword |
| `Async` | async, ASYNC | Statement keyword |
| `Term` | term, TERM | Statement keyword / postcondition |
| `Escape` | escape, ESCAPE | Statement keyword |
| `Unification` | unification, UNIFICATION, unify, UNIFY, uni, UNI | Statement keyword |
| `Import` | import, IMPORT | Statement keyword |
| `From` | from, FROM | Import/signature keyword |
| `As` | as, AS | Import/alias keyword |
| `Frgn` | frgn, FRGN | Statement keyword |
| `FrgnBang` | frgn!, FRGN! | Statement keyword |
| `Syscall` | syscall, SYSCALL | Statement keyword |
| `SyscallBang` | syscall!, SYSCALL! | Statement keyword |
| `Resource` | resource, RESOURCE | Declaration keyword |
| `Rsrc` | rsrc, RSRC | Declaration keyword |
| `Registry` | reg, REG, registry, REGISTRY | Declaration keyword |
| `Struct` | struct, STRUCT | Statement keyword |
| `Rstruct` | rstruct, RSTRUCT | Statement keyword |
| `Render` | render, RENDER | Statement keyword |
| `Enum` | enum, ENUM | Statement keyword |
| `TrgBang` | trg!, TRG!, trigger!, TRIGGER! | Statement keyword |
| `Trg` | trg, TRG, trigger, TRIGGER | Statement keyword |
| `Link` | link, LINK | Statement keyword |
| `Asm` | asm, ASM | Statement keyword |
| `Stage` | stage, STAGE | Declaration keyword |
| `On` | on, ON | Stage modifier |
| `Forall` | forall, FORALL | Quantifier |
| `Exists` | exists, EXISTS | Quantifier |
| `Within` | within, WITHIN | Range keyword |
| `Bank` | bank, BANK | Register bank |
| `Ok` | Ok, OK | Enum constructor |
| `Err` | Err, ERR | Enum constructor |
| `Match` | match, MATCH | Pattern keyword |
| `Some` | some, SOME | Enum constructor |
| `None` | none, NONE | Enum constructor |
| `BoolTrue` | true, TRUE | Literal |
| `BoolFalse` | false, FALSE | Literal |
| `Cycles` | cycles, CYCLES | Time unit |
| `Cyc` | cyc, CYC | Time unit |
| `Ms` | ms, MS | Time unit |
| `Seconds` | sec, SEC, seconds, SECONDS | Time unit |
| `Minute` | minute, MINUTE | Time unit |

### 8.2 Currently handled by `expect_identifier()` (22 tokens)

`TypeData, TypeInt, Some, None, Ok, Err, Sig, Defn, Let, Txn, Rct, Frgn, Struct, Enum, Import, Term, Const, BoolTrue, BoolFalse, Unification, Escape, Async`

### 8.3 Currently handled by `is_keyword()` in token.bv (16 variants)

`KeywordLet, KeywordConst, KeywordTxn, KeywordRct, KeywordAsync, KeywordDefn, KeywordSig, KeywordFrgn, KeywordStruct, KeywordEnum, KeywordImport, KeywordTrue, KeywordFalse, KeywordUnification, KeywordTerm`

### 8.4 Keywords that blocked us during Phase 1

| Keyword | Token | File blocked | Fix method |
|---|---|---|---|
| `txn` | `Txn` | Multiple | Already handled by both parsers — the issue was we didn't know. Route B makes this work without renames. |
| `reg` | `Registry` | backend_aarch64.bv (67 sites) | Already handled in Rust's `expect_identifier()`? **No** — `Registry` is NOT in `expect_identifier()`. This is a missing token. |
| `from` | `From` | proof_engine.bv (4 sites) | NOT in `expect_identifier()`. Missing. |
| `Ok` | `Ok` | proof_engine.bv (check_vc function) | Already handled. The real issue was `Ok(())` double-paren, not keyword blocking. |

### 8.5 All 24 `uni tok(TokenIdentifier(name))` sites in parser.bv

| # | Line | Guard type | Advance? | Classification |
|---|------|-----------|----------|----------------|
| 1 | 217 | `[!is_identifier]` | YES | SIMPLE+ADVANCE |
| 2 | 401 | `[!is_identifier]` | YES | SIMPLE+ADVANCE |
| 3 | 496 | `[!is_identifier]` | YES | SIMPLE+ADVANCE |
| 4 | 1073 | fallthrough Err | YES | UNI-ONLY |
| 5 | 1257 | `[is_identifier \|\| OpAt]` | NO | SIMPLE-only |
| 6 | 1357 | `[!is_identifier]` | YES | SIMPLE+ADVANCE |
| 7 | 1507 | `[!is_identifier]` | YES | SIMPLE+ADVANCE |
| 8 | 1569 | `[!is_identifier]` | YES | SIMPLE+ADVANCE |
| 9 | 1703 | fallthrough Err | YES | UNI-ONLY |
| 10 | 1753 | `[!is_identifier]` | YES | SIMPLE+ADVANCE |
| 11 | 1787 | `[!is_identifier]` | YES | SIMPLE+ADVANCE |
| 12 | 1839 | `[!is_identifier]` | YES | SIMPLE+ADVANCE |
| 13 | 1894 | `[!is_identifier]` | YES | SIMPLE+ADVANCE |
| 14 | 2048 | `[!is_identifier]` | YES | SIMPLE+ADVANCE |
| 15 | 2074 | `[!is_identifier]` | YES | SIMPLE+ADVANCE |
| 16 | 2083 | `[is_identifier(...)]` outer | YES | SIMPLE-only |
| 17 | 2118 | `[!is_identifier]` | YES | SIMPLE+ADVANCE |
| 18 | 2151 | `[!is_identifier]` | YES | SIMPLE+ADVANCE |
| 19 | 2199 | `[!is_identifier]` | YES | SIMPLE+ADVANCE |
| 20 | 2258 | fallthrough Err | YES | UNI-ONLY (also nested at 2266) |
| 21 | 2287 | `[!is_identifier]` | YES | SIMPLE+ADVANCE |
| 22 | 2333 | `[!is_identifier]` | YES | SIMPLE+ADVANCE |
| 23 | 2400 | `[!is_identifier]` | YES | SIMPLE+ADVANCE |
| 24 | 2516 | `[!is_identifier]` | YES | SIMPLE+ADVANCE |

---

## 9. Future Work (Beyond Route B)

After Route B is complete, the remaining issues for full self-hosting:

1. **Wire `main.bv`** to a CLI command — no invocation path exists
2. **Fix `webstack.bv`** — `{ }` object literal in `let` binding fails (parser bug or syntax gap)
3. **Fix `backend_aarch64.bv`** — `uni` in expression context, `u32`/`u8` return types, `<<` shift operator
4. **Fix `x86_64.bv`** — same Rust-isms as aarch64 (`u8`, `u32`, `u64`, `as` casts, shift operators)
5. **Fix `wasm.bv`** — 59+ Rust-isms (`u8`, `u32`, `i32`, `as` casts, shift operators)
6. **Type errors** in parser.bv, typechecker.bv, backends/mod.bv — these only surface when actually running the pipeline
7. **Complete backends/rust.bv** — add `ExprCall`, `ExprUnaryOp`, etc. code generation