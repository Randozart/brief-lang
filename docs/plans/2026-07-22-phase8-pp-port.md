# Phase 8: AST Pretty-Printer Port to Brief

**Date:** 2026-07-22
**Status:** Plan
**Depends on:** Phases 0-7 (complete), stdlib frgn cleanup (complete)

---

## Goal

Port the AST pretty-printer (`src/ast/display.rs`, ~505 lines, 12 Display impls)
from Rust to Brief, then wire it through the export/GLUE bridge so the Rust
compiler calls Brief code to format AST nodes.

This exercises the full FFI pipeline in reverse (Brief → Host), validates
protocol transforms on recursive types, and provides the foundation for
incremental self-hosting.

## Current Architecture

`src/ast/display.rs` contains 12 `Display` impls:

| Impl | Lines | Variants | Complexity |
|------|-------|----------|------------|
| `Type` | 73 | ~6 | Low — flat structure |
| `Statement` | 105 | ~12 | Medium — keywords, block formatting |
| `Expr` | 96 | ~30 | **High** — deeply recursive |
| `TopLevel` | 79 | ~25 | Medium — dispatch wrapper |
| `BinaryOpKind` | 26 | ~20 | Low — op symbol strings |
| `UnaryOpKind` | 10 | ~6 | Low |
| `Dimension` | 9 | ~2 | Low |
| `OutputType` | 29 | ~3 | Low |
| `Contract` | 12 | ~1 | Low |
| `Pattern` | 35 | ~5 | Medium |
| `DerivationBlock` | 10 | ~1 | Low |
| `DerivationExample` | 10 | ~1 | Low |

Total: ~505 lines.

No `ForeignBinding::Display` exists (the plan draft assumed it does).

## Execution

### Step 1: Brief pretty-printer library (`lib/pp/`)

Create `lib/pp/` directory with Brief functions for each Display impl.
One file per AST node category:

| File | Contents |
|------|----------|
| `lib/pp/types.bv` | `pp_type`, `pp_dimension`, `pp_output_type` |
| `lib/pp/statements.bv` | `pp_statement` |
| `lib/pp/exprs.bv` | `pp_expr`, `pp_binop`, `pp_unary_op`, `pp_pattern` |
| `lib/pp/toplevel.bv` | `pp_toplevel`, `pp_contract`, `pp_derivation_block`, `pp_derivation_example` |

Each function takes a structured representation (or serialized string) and
returns a formatted string. The simplest approach: pass the AST node's
variants as string tags + sub-strings, matching the Rust Display pattern.

```brief
// lib/pp/types.bv
defn pp_type_int() -> String { term "Int"; }
defn pp_type_float() -> String { term "Float"; }
defn pp_type_bool() -> String { term "Bool"; }
defn pp_type_string() -> String { term "String"; }
defn pp_type_ptr(elem: String) -> String {
    term "Ptr<" ++ elem ++ ">";
};
defn pp_type_custom(name: String, params: String) -> String {
    [params == ""] { term name; };
    term name ++ "<" ++ params ++ ">";
};
```

### Step 2: Bridge file (`bridge/pp-bridge.bv`)

Export the pretty-printer functions via `export defn`:

```brief
export defn brief_pp_type(tag: String, payload: String) -> String {
    [tag == "Int"] { term pp_type_int(); };
    [tag == "Float"] { term pp_type_float(); };
    [tag == "Bool"] { term pp_type_bool(); };
    [tag == "String"] { term pp_type_string(); };
    [tag == "Ptr"] { term pp_type_ptr(payload); };
    [tag == "Custom"] { term pp_type_custom(payload, ""); };
    term "unknown";
};
```

### Step 3: Rust adapter in `src/ast/display.rs`

Each Display impl tries the Brief version first, falls back to native Rust:

```rust
impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(s) = call_brief_pp("Type", self) {
            return write!(f, "{}", s);
        }
        // native fallback...
    }
}
```

### Step 4: Round-trip test

```rust
#[test]
fn test_pp_roundtrip_type() {
    let ast = make_test_type();
    let rust_pp = format!("{}", ast);
    let brief_pp = call_brief_pp_type(&ast);
    assert_eq!(rust_pp, brief_pp);
}
```

Start with `Type` (simplest), extend to `Statement`, then `Expr`.

### Step 5: Full cutover

Once all 12 Display impls delegate to Brief and all round-trip tests pass,
remove the native fallback code from `display.rs`. The file becomes a thin
adapter that serializes AST nodes and calls `brief_pp_*` via the bridge.

## Risk Assessment

| Step | Risk | Mitigation |
|------|------|------------|
| Type pp | Low | Simple struct, few variants |
| Statement pp | Medium | Block formatting, indentation |
| Expr pp | **High** | 30 variants, deeply recursive, tests recursion depth |
| Bridge wiring | Medium | Requires export + GLUE bridge to work end-to-end |
| Round-trip parity | Medium | Must match Rust output character-for-character |

## Success Criteria

- `Type::Display` delegates to Brief (verified by round-trip test)
- `Statement::Display` delegates to Brief
- `Expr::Display` delegates to Brief (all 30+ variants)
- `TopLevel::Display` and all remaining impls delegate to Brief
- Full test suite passes with GLUE bridge enabled
- Native Rust fallback produces identical output
