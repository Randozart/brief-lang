# Phase 6: Cast, Contract Shorthands, Watchdog Prefixes

**Timestamp**: 2026-05-28T18:32:16Z  
**Prerequisites**: Phase 5 complete (interpreter gaps filled, import + unification fixes)  
**Rust compiler**: ✓ `cargo build`, ✓ `cargo test --lib` — 269/269 pass  
**Selfhost status**: Blocked on `as` type casting in `lib/std/char.bv`

---

## Objective

Three self-contained changes to the parser + interpreter that unblock selfhosting and clean up contract syntax:

1. **`as` type casting** — allows `char_to_int`, `int_to_char` in `lib/std/char.bv` to parse
2. **`[pre]]` / `[[post]` shorthands** — reduce boilerplate for single-sided contracts
3. **`?[wd]` / `?![wd]` watchdog prefixes** — explicit prefixes for third contract slot

---

## Priority 1: `as` Type Casting

### Why
`lib/std/char.bv:7` uses `c as Int` and `n as Char`. The lexer has `Token::As` (used for import aliasing), but no `Expr::Cast` exists in the AST. Without this, `char.bv` fails to parse, blocking all imports of `std.char`.

### Changes

#### `src/ast.rs` — **already added**
```rust
Cast(Box<Expr>, Type),
```

#### `src/parser.rs` — `parse_postfix()` (line 4261)
In the postfix loop, add a new arm after the `Token::Dot` arm:

```
} else if let Some(Ok(Token::As)) = self.current_token() {
    self.advance();
    let cast_type = self.parse_type()?;
    expr = Expr::Cast(Box::new(expr), cast_type);
```

#### `src/interpreter.rs` — after `Expr::Concat` (line 1591)
```
Expr::Cast(inner, target_type) => {
    let value = self.eval_expr(inner)?;
    match (value, target_type) {
        (Value::Char(c), Type::Int) | (Value::Int(c), Type::Int) => Ok(Value::Int(c as i64)),
        (Value::Int(n), Type::Char) => Ok(Value::Char(char::from_u32(n as u32).unwrap_or('?'))),
        (Value::Int(n), Type::Float) => Ok(Value::Float(n as f64)),
        (Value::Float(f), Type::Int) => Ok(Value::Int(f as i64)),
        (Value::String(s), Type::Char) if s.len() == 1 => Ok(Value::Char(s.chars().next().unwrap())),
        _ => Ok(value), // no-op for same-type or unimplemented casts
    }
}
```

#### `src/typechecker.rs` — before wildcard (line 1373)
```rust
Expr::Cast(_, target_type) => target_type.clone(),
```

#### All other files (~12 files) — add `| Expr::Cast(_, _)` to wildcard chains
- `desugarer.rs` — 2 sites
- `annotator.rs` — `format_expr` after `Concat` arm
- `symbolic.rs` — add to line 262 chain
- `proof_engine.rs` — 14 sites
- `backend/mod.rs` — 1 site
- `backend/c.rs` + `backend/rust.rs` — codegen catch-alls
- `analysis/call_graph.rs`, `analysis/dataflow.rs`, `analysis/entry_point.rs`, `analysis/protocol.rs` — 5 sites
- `ast.rs` — `extract_deps_recursive`

---

## Priority 2: `[pre]]` / `[[post]` Shorthands

### Why
Many contracts use only a precondition OR postcondition. Currently must write `[pre][true]` or `[true][post]`. The shorthand eliminates the `[true]` boilerplate.

### Syntax
| Full form | Shorthand | Meaning |
|---|---|---|
| `[x > 0][true]` | `[x > 0]]` | Precondition only |
| `[true][term > 0]` | `[[term > 0]` | Postcondition only |
| `[x > 0][term > 0]` | `[x > 0][term > 0]` | Both (unchanged) |

### Changes
**`src/parser.rs` — `parse_contract()` only:**

**`[[post]`** at line 2965 (count == 0):
```
if matches!(self.current_token(), Some(Ok(Token::RBracket))) {
    self.advance(); // consume ]
    pre_condition = Expr::Bool(true);
    count = 1;
    continue; // fall through to parse post
}
pre_condition = self.parse_expression()?;
```

**`[pre]]`** after line 2998 (after `expect(RBracket)` for post):
```
// [pre]] shorthand: if next token is ], post = true
if count == 1 && matches!(self.current_token(), Some(Ok(Token::RBracket))) {
    self.advance();
    post_condition = Expr::Bool(true);
    count = 2;
}
```

No other files change — `Expr::Bool(true)` is already the default for pre/post in `Contract::new`.

---

## Priority 3: `?[wd]` / `?![wd]` Watchdog Prefixes

### Why
Original watchdog syntax was bare `[watchdog]` which is ambiguous ("is this a watchdog or a missing postcondition?"). The `?` prefix makes it explicit.

### Syntax
| Form | Meaning |
|---|---|
| `[pre][post]?[wd]` | Optional watchdog (proof engine decides) |
| `[pre][post]?![wd]` | Mandatory watchdog (always emits runtime check) |
| `[pre][post][wd]` | **Error** — must prefix with `?` or `?!` |

### Changes
**`src/parser.rs` — `parse_contract()`**:

After the `while let Some(Ok(Token::LBracket))` loop (after line 2998), add:

```
// Watchdog: ?[cond] or ?![cond]
if let Some(Ok(Token::Question)) = self.current_token() {
    self.advance();
    let is_required = if let Some(Ok(Token::Bang)) = self.current_token() {
        self.advance();
        true  // ?!
    } else {
        false // ?
    };
    self.expect(Token::LBracket)?;
    let cond = self.parse_expression()?;
    self.expect(Token::RBracket)?;
    watchdog = Some(WatchdogSpec {
        condition: cond,
        is_required,
    });
}
```

Remove the old `count == 2` watchdog branch (lines 2969-2992). Keep the `count > 2` error at line 2993.

No other files change — `WatchdogSpec` already has `is_required: bool`.

---

## Verification

1. `cargo build` — compiles with new AST variant + parser + interpreter
2. `cargo test --lib` — 269/269 still pass
3. `cargo run --bin briev-compiler -- check lib/std/char.bv` — no parse errors for `as` casts
4. `cargo run --bin briev-compiler -- selfhost examples/counter.rbv` — pipeline passes `char.bv` import, proceeds past `is_digit`
5. Test `[pre]]` / `[[post]` in isolation with a small `.bv` file
6. Test `?[wd]` / `?![wd]` in isolation with a small `.bv` file

---

## After Phase 6

Pipeline should complete through `char.bv` → `lexer.bv` → `tokenizer` → `parse` → `call_graph` → backend stub. The next blocker will be whatever `call_graph.bv` or the backend encounters next.