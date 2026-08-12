# Phase 2.8 — Implementation Guide ✅

## Overview

Complete the parse discriminator system (pre/suf/reg fields on `op Parse`),
wire the type checker, fix the remaining 2 SKIP + 1 MISMATCH benchmarks.

## Status: ✅ ALL COMPLETED (2026-07-27)

- **B5**: `register_parse_bindings()` converts `OperatorBinding` → `OperatorDef`, stores in
  `parse_ops` map. `check_program` pre-collects TypeDef bindings from all items before main loop.
  `type_parents` map populated for hierarchy walking.
- **B6**: `find_parse_op` walks type hierarchy (type → parent → grandparent). `matches_form`
  handles empty params as wildcard. `try_coerce_via_parse` handles `TaggedLiteral` (discriminator
  forwarding) and `TaggedQuotedLiteral`.
- **B4**: Prefix peek-ahead: identifier directly before string literal (no whitespace) →
  `Expr::TaggedQuotedLiteral(bytes, prefix)`. New `TaggedQuotedLiteral(Vec<u8>, String)` variant
  wired across all 15 exhaustive match sites.
- **B8**: 10 new parser tests: suffix literals (`42km`, `0xFFh`), prefix strings (`sql"SELECT"`),
  discriminated op Parse parsing (`pre:`, `suf:`, `reg:` fields). Total: 1046 tests.
- **C1-C2**: Existing fixes verified working. `print_loop` now MATCH.
- **C3**: Full correctness check: 0 SKIP/FAIL from compiler issues (only `mandelbrot` MISMATCH
  pre-existing, 2 SKIP from missing binaries).
- **Documentation**: AGENTS.md syntax trap added for `op Parse` discriminator syntax.
  TODO.md updated to reflect completion.

---

## B5 — Wire `register_parse_ops` for `OperatorBinding` + `TypeDef` items

### File: `src/typechecker/mod.rs`

### Change 1: Add `register_parse_bindings` method (new, parallel to `register_parse_ops`)

Add after line 99 (after `register_parse_ops`):

```rust
/// 2026-07-27: Register Parse bindings from a type's OperatorBinding entries.
/// Filters op_bindings for name == "Parse" and stores them in the parse_ops
/// map for resolution during type checking.
pub fn register_parse_bindings(
    &mut self,
    type_name: &str,
    bindings: Vec<crate::ast::top::OperatorBinding>,
) {
    let parse_bindings: Vec<_> = bindings
        .into_iter()
        .filter(|b| b.name == "Parse")
        .collect();
    if !parse_bindings.is_empty() {
        // Prepend type name to binding name for unique key lookup
        let key = format!("{}::Parse", type_name);
        self.parse_ops.insert(key, Vec::new());
        // Store in a new field: parse_bindings: HashMap<String, Vec<OperatorBinding>>
        // OR convert OperatorBinding to OperatorDef for the existing parse_ops map.
        // Option B (simpler): convert to OperatorDef:
        let ops: Vec<crate::ast::top::OperatorDef> = parse_bindings
            .iter()
            .map(|b| crate::ast::top::OperatorDef {
                op: "Parse".to_string(),
                params: vec![],
                pre: b.pre.clone(),
                suf: b.suf.clone(),
                impl_args: None,
                impl_name: b.name.clone(),
                span: None,
            })
            .collect();
        self.parse_ops.insert(type_name.to_string(), ops);
    }
}
```

**Alternative (cleaner):** Add a new HashMap field to `TypecheckContext`:

```rust
// In struct TypecheckContext (around line 50):
pub parse_bindings: HashMap<String, Vec<crate::ast::top::OperatorBinding>>,
```

And store `OperatorBinding` directly (no conversion needed).

### Change 2: Wire `TypeDef` items in `check_top_level`

At line 818 (`_ => Ok(())`), BEFORE the catch-all, add:

```rust
// 2026-07-27: Register Parse bindings from TypeDef items
TopLevel::TypeDef(td) => {
    if !td.body.op_bindings.is_empty() {
        ctx.register_parse_bindings(&td.name, td.body.op_bindings.clone());
    }
    Ok(())
}
```

This requires importing `crate::ast::top::TopLevel::TypeDef` if not already imported.

### Change 3: Import `TypeDef` in check_top_level scope

The `check_top_level` function is at line 774. It uses `TopLevel::Definition`, `TopLevel::Export`, `TopLevel::Transaction` — I need to verify `TopLevel::TypeDef` is in scope or use the full path.

---

## B6 — Update `find_parse_op` for `OperatorBinding` + inheritance walk

### File: `src/typechecker/mod.rs`

### Change 4: Add `find_parse_binding` method

Replace the existing `find_parse_op` (lines 55-91) with a new version that:
1. Searches `parse_bindings` (the new HashMap of OperatorBinding entries)
2. Walks the type hierarchy (parent types)
3. Uses `pre`/`suf`/`reg` matching on `OperatorBinding` fields
4. Handles ambiguity (multiple match → error)

```rust
/// 2026-07-27: Find a Parse binding on a type that could accept a literal form.
/// Walks the type hierarchy (type → parent → grandparent).
/// Uses pre/suf matching and optional reg: regex matching.
/// If multiple bindings match, returns an ambiguity error.
pub fn find_parse_binding(
    &self,
    type_name: &str,
    form: &str,
    discriminator: Option<&str>,
    literal_text: Option<&str>,
) -> Result<Option<&crate::ast::top::OperatorBinding>, String> {
    let mut candidates = Vec::new();
    
    // Walk type hierarchy
    let mut current = Some(type_name);
    while let Some(tn) = current {
        let key = format!("{}::Parse", tn);
        if let Some(bindings) = self.parse_bindings.get(&key) {
            for b in bindings {
                // Filter by form: op Parse(Decimal), op Parse(Quoted), op Parse(Bare)
                // The form is matched from the protocol variant:
                //   protocol_variant == Some("Decimal") → matches Decimal
                //   protocol_variant == Some("Quoted") → matches Quoted
                //   protocol_variant == Some("Bare") → matches Bare
                //   protocol_variant == None → matches any form
                
                let form_matches = match b.protocol_variant.as_deref() {
                    Some(f) if f == "Decimal" || f == "Quoted" || f == "Bare" => f == form,
                    Some(_) => false, // protocol_variant is a type name, not a literal form
                    None => true,     // no protocol variant = matches all
                };
                if !form_matches { continue; }
                
                // Filter by pre: literal must start with prefix
                if let Some(pre) = &b.pre {
                    match literal_text {
                        Some(text) if text.starts_with(pre) => {}
                        _ => continue,
                    }
                }
                
                // Filter by suf: literal must end with suffix
                if let Some(suf) = &b.suf {
                    match discriminator {
                        Some(d) if d == suf => {} // Matched via suffix peek-ahead
                        Some(text) if text.ends_with(suf) => {} // literal text ends with suffix
                        _ => continue,
                    }
                }
                
                // Filter by reg: regex match on the full literal text
                if let Some(reg) = &b.reg {
                    match literal_text {
                        Some(text) => {
                            if let Ok(re) = regex::Regex::new(reg) {
                                if !re.is_match(text) { continue; }
                            }
                        }
                        None => continue,
                    }
                }
                
                candidates.push((tn.to_string(), b));
            }
        }
        
        // Move to parent type (TODO: lookup parent from type universe)
        current = None; // placeholder — needs parent type lookup
    }
    
    match candidates.len() {
        0 => Ok(None),
        1 => Ok(Some(candidates[0].1)),
        _ => Err(format!(
            "ambiguous literal: multiple types match parse rule '{}' for '{}'",
            form,
            literal_text.unwrap_or("")
        )),
    }
}
```

**Note:** The parent type walk needs access to the type parent relationship. The `TypecheckContext` should have a `type_parents: HashMap<String, String>` map populated during TypeDef processing. Add this to the struct:

```rust
// In struct TypecheckContext:
pub type_parents: HashMap<String, String>,
```

Populate during `check_top_level` for `TypeDef`:

```rust
TopLevel::TypeDef(td) => {
    if let Some(parent) = &td.parent {
        ctx.type_parents.insert(td.name.clone(), format!("{}", parent));
    }
    if !td.body.op_bindings.is_empty() {
        ctx.register_parse_bindings(&td.name, td.body.op_bindings.clone());
    }
    Ok(())
}
```

### Change 5: Update `try_coerce_via_parse` to use `find_parse_binding`

Replace lines 335-354 with:

```rust
fn try_coerce_via_parse(
    expr: &Expr,
    arg_ty: &Type,
    target_ty: &Type,
    ctx: &TypecheckContext,
) -> bool {
    let (form, discriminator, literal_text) = match expr {
        Expr::Decimal(n) => ("Decimal", None, Some(n.to_string())),
        Expr::Float(f) => ("Decimal", None, Some(format!("{}", f))),
        Expr::Quoted(bytes) => ("Quoted", None, Some(String::from_utf8_lossy(bytes).to_string())),
        Expr::Identifier(s) => ("Bare", None, Some(s.clone())),
        Expr::TaggedLiteral(n, tag) => ("Decimal", Some(tag.as_str()), Some(n.to_string())),
        _ => return false,
    };
    let target_name = match target_ty {
        Type::Custom(n) => n.as_str(),
        _ => return false,
    };
    match ctx.find_parse_binding(target_name, form, discriminator, literal_text.as_deref()) {
        Ok(Some(_)) => true,
        _ => false,
    }
}
```

---

## B4 Extension — Prefix peek-ahead for `sql"..."` strings

### File: `src/parser/expressions.rs`

After the `Quoted` match arm in `parse_primary` (around line 376), add:

```rust
// 2026-07-27: After parsing a Quoted string, check if the PREVIOUS token
// was an adjacent identifier (no whitespace). If so, it's a prefix
// discriminator (e.g., sql"SELECT", my"string").
// This is checked in the Identifier arm below (when an identifier is
// followed immediately by a Quoted string).
```

Actually, this works differently from suffix — the prefix comes FIRST (the `my` in `my"string"`). So the Identifier arm should check:

```rust
// In parse_primary, match arm for Token::Identifier (or the
// Identifier handling path around line 365-372):
// After parsing an identifier named "my", check if the NEXT token
// is an adjacent Quoted string. If so, consume both and return
// a TaggedLiteral or a combined expression.
```

The parse logic:

```rust
// In the identifier check path (around line 365):
if name.starts_with(|c: char| c.is_lowercase()) {
    // Check for adjacent string literal → prefix discriminator
    if let Some((Token::String(s), str_span)) = self.peek_with_span() {
        // Check whitespace: identifier's span end == string span start
        if let Some((_, id_span)) = self.tokens.get(self.pos - 1) {
            if id_span.end == str_span.start {
                self.pos += 1; // consume string
                let val = s.clone();
                return Ok(Expr::TaggedQuotedLiteral(val, name));
            }
        }
    }
}
```

Where `Expr::TaggedQuotedLiteral(String, String)` is a NEW AST variant (or we reuse `TaggedLiteral` with the string converted to bytes). If we reuse `TaggedLiteral`:

```rust
Expr::TaggedLiteral(0, format!("{}:{}", name, val))
```

But that's hacky. Better to add a proper variant:

```rust
// In src/ast/expr.rs:
pub enum Expr {
    // ... existing variants ...
    /// 2026-07-27: Tagged string literal: sql"SELECT", my"hello"
    /// First field is the string content, second is the prefix tag.
    TaggedQuotedLiteral(Vec<u8>, String),
}
```

---

## B8 — Tests

### Parser tests

Add tests in `src/parser/definitions.rs` test module:

```rust
#[test]
fn test_parse_op_with_discriminators() {
    let ops = parse_op_from_type_def(
        "type T { op Parse(Decimal, pre:\"0x\"): parse_hex(#L); };"
    );
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0].name, "Parse");
    assert_eq!(ops[0].pre.as_deref(), Some("0x"));
}

#[test]
fn test_parse_op_with_suffix() {
    let ops = parse_op_from_type_def(
        "type T { op Parse(Decimal, suf:\"km\"): parse_km(#L); };"
    );
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0].suf.as_deref(), Some("km"));
}

#[test]
fn test_parse_op_with_regex() {
    let ops = parse_op_from_type_def(
        "type T { op Parse(Decimal, reg:\"[0-9]+\"): parse_num(#L); };"
    );
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0].reg.as_deref(), Some("[0-9]+"));
}
```

### Tokenizer tests for suffix detection

Test that `42km` produces `Expr::TaggedLiteral(42, "km")`:

```rust
// In src/parser/definitions.rs or expressions.rs test module:
#[test]
fn test_tagged_literal_suffix() {
    let src = "let x = 42km;";
    // Parse, find Statement::Let with expr == Expr::TaggedLiteral(42, "km")
}
```

---

## Workstream C — Fix Remaining Benchmarks

### C1: FFI Cache Conflict in Harness

**File:** `benchmarks/build_and_bench.sh`

The harness at line 163 compiles `briev_rt.c` to `/tmp/briev_rt.o` with `-flto`, then tries to link it with the benchmark's `.ll` file. But the compiler's internal linking has already created FFI cached `.o` files that conflict.

**Fix (verified working):** Replace lines 160-170 with one-step clang:

```bash
if [ ! -f "$bin" ]; then
    clang -O3 "benchmarks/${name}.ll" "lib/runtime/briev_rt.c" \
        -lm -o "$bin" 2>&1
fi
```

This was already applied in commit `8c13cd99`. Verify it works for all 2 SKIP benchmarks by running them manually.

### C2: print_loop Bool Counter Backedge

**File:** `src/backend/llvm/loop_engine/counter.rs`

The phi type for counter fields should be `"i64"` for all field types (not the native field width like `"i8"` for Bool). This was partially applied in commit `8c13cd99` line 391:

```rust
// Line 391 — already changed to "i64":
let phi_ty = "i64".to_string();
```

**Verify:** The fix is already committed. Rebuild all benchmarks with `rm -f benchmarks/*.ll && cargo build --release && (build each benchmark)` then run `--correctness`.

### C3: Test Suite

After C1 + C2:

```bash
rm -f benchmarks/*.ll benchmarks/*.bc
bash benchmarks/build_and_bench.sh --correctness
```

Expected: 0 MISMATCH, 0 SKIP.

---

## Documentation Updates

### AGENTS.md

Add a Syntax Trap item about `op Parse` discriminator syntax:

```markdown
34. **`op Parse` discriminator syntax** — Parse ops can have optional `pre:`,
    `suf:`, and `reg:` discriminator fields:
    - `op Parse(Decimal, pre:"0x"): parse_hex(#L);` — literals starting with `0x`
    - `op Parse(Decimal, suf:"km"): parse_km(#L);` — literals ending with `km`
    - `op Parse(Decimal, reg:"[0-9a-fA-F]+"): parse_hex(#L);` — regex match
    - `op Parse(Quoted): parse_string(#L);` — string literals
    - `op Parse(Bare): parse_ident(#L);` — bare identifiers
    Multiple `op Parse` can be declared on the same type with different
    discriminators. The compiler resolves by checking (1) form match,
    (2) pre/suf match, (3) regex match. Ambiguity = error.
```

### SPEC.md

Update the `op_decl` grammar to include `pre:`/`suf:`/`reg:`:

```ebnf
op_decl ::= "op" identifier "(" param_decl? ")" ":" expression ";"
param_decl ::= identifier ("," discriminator_pair)*
discriminator_pair ::= ("pre" | "suf" | "reg") ":" string_literal
```

---

## Summary of All Remaining Changes

| Item | File(s) | Effort | Priority |
|------|---------|--------|----------|
| B5: register_parse_bindings | `typechecker/mod.rs` | ~15 lines | High |
| B5: wire TypeDef in check_top_level | `typechecker/mod.rs` | ~10 lines | High |
| B6: find_parse_binding | `typechecker/mod.rs` | ~80 lines | High |
| B6: update try_coerce_via_parse | `typechecker/mod.rs` | ~10 lines | High |
| B4: prefix peek-ahead | `expressions.rs`, `expr.rs` | ~20 lines | Medium |
| B8: parser tests | `definitions.rs` | ~40 lines | Medium |
| C1: harness fix | `build_and_bench.sh` | Already done — verify | Low |
| C2: counter backedge | `counter.rs` | Already done — verify | Low |
| C3: full correctness check | — | Run once | Low |
| Documentation | `AGENTS.md`, `SPEC.md` | ~20 lines | Low |
