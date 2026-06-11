# Kani Harnesses — Formal Verification Proofs

**Date:** 2026-06-11
**Status:** Current

## Overview

Kani (AWS's bounded model checker) is used to prove absence of panics,
overflows, out-of-bounds access, and undefined behavior in safety-critical
compiler code. Harnesses are co-located with their module in
`#[cfg(kani)] mod kani_tests {}` blocks.

## Fast Group (`#[cfg(kani)]`)

Provable in <5s. NO formatting, heap allocation, struct construction >3
fields, loops, or recursion.

| Module | Harness | What it proves |
|--------|---------|---------------|
| `ast.rs` | `verify_as_integer_dual_path` | `Expr::Integer(42).as_integer()` matches `LiteralExpr::Integer(42).as_integer()` |
| `literal.rs` | `verify_literal_as_integer` | LiteralExpr to integer conversion |
| `literal.rs` | `verify_literal_format_no_panic` | Format does not panic (full group) |
| `features/traits.rs` | `verify_trait_dispatch` | Trait dispatch does not panic |
| `features/binary_op.rs` | `verify_binary_op_dispatch` | Binary op dispatch does not panic |
| `features/unary_op.rs` | `verify_unary_op_dispatch` | Unary op dispatch does not panic |
| `features/call.rs` | `verify_call_dispatch` | Call dispatch does not panic |
| `features/stmt/assignment.rs` | `verify_assignment_dispatch` | Assignment dispatch does not panic |
| `features/literal.rs` | `verify_literal_expr_new` | LiteralExpr::new does not panic |
| `features/literal.rs` | `verify_literal_expr_match` | LiteralExpr enum matching is exhaustive |
| `features/toplevel/typedef.rs` | `verify_typedef_new` | TypeDef::new does not panic |
| `parser.rs` | `verify_parse_literal` | Parsing literals does not panic |
| `annotator.rs` | `verify_annotator_dispatch` | Annotator dispatch does not panic |
| `analysis/dataflow.rs` | `verify_dataflow_transfer` | Dataflow transfer function is total |

## Full Group (`#[cfg(all(kani, feature = "kani_full"))]`)

Requires `--features kani_full`. May use formatting, heap allocation, loops,
or recursion.

| Module | Harness | What it proves |
|--------|---------|---------------|
| `literal.rs` | `verify_literal_format_no_panic` | Format does not panic (uses Display) |
| `interpreter.rs` | `verify_interpreter_eval_no_panic` | Expression eval does not panic |
| `proof_engine.rs` | `verify_proof_engine_no_panic` | Proof engine dispatch does not panic |
| `symbolic.rs` | `verify_symbolic_eval_no_panic` | Symbolic evaluation does not panic |
| `backend/llvm/mod.rs` | `verify_llvm_emit_no_panic` | LLVM emission does not panic |
| `backend/vhdl.rs` | `verify_vhdl_emit_no_panic` | VHDL emission does not panic |
| `backend/webstack.rs` | `verify_webstack_emit_no_panic` | Webstack emission does not panic |
| `typechecker.rs` | `verify_typechecker_infer` | Type inference does not panic |
| `analysis/transition_graph.rs` | `verify_transition_graph` | Transition graph construction is total |
| `analysis/dataflow.rs` | `verify_dataflow_full` | Full dataflow analysis (uses loops) |
| `features/toplevel/typedef.rs` | `verify_typedef_full` | TypeDef processing (uses Vec) |

## Harness Requirements

A Kani harness MUST only contain:

1. **Pure match dispatch only** — `match self { A => B, C => D }` returning
   a concrete result
2. **Concrete inputs only** — no `kani::any()`, no symbolic values (they
   trigger unbounded exploration)
3. **No formatting** — no `.to_string()`, `format!()`, `writeln!()`, string
   concatenation, or any `Display` impl
4. **No heap allocation** — no `Box::new()`, `Vec::new()`, `String::new()`,
   `HashMap::new()`
5. **No struct construction** unless the struct has <= 3 fields and no
   heap-allocated fields
6. **No loops or recursion** in the function being verified OR any function
   it transitively calls

A harness is **unprovable** (will timeout) if it transitively calls ANY
function that:
- Converts integers to strings (`.to_string()`, `format!("{}", n)`)
- Formats output (`format!`, `writeln!`)
- Constructs `Box`, `Vec`, `String`, `HashMap`, `HashSet`
- Constructs any struct with > 3 fields
- Iterates with loops or recurses

## Reference Harness: Fast Group

```rust
#[cfg(kani)]
mod kani_tests {
    use super::*;

    #[kani::proof]
    fn verify_as_integer_dual_path() {
        let old = Expr::Integer(42);
        let new = Expr::Literal(Box::new(LiteralExpr::Integer(42)));
        assert_eq!(old.as_integer(), new.as_integer());
    }
}
```

## Reference Harness: Full Group

```rust
#[cfg(all(kani, feature = "kani_full"))]
mod kani_full_tests {
    use super::*;

    #[kani::proof]
    fn verify_literal_format_no_panic() {
        let lit = LiteralExpr::Integer(42);
        let s = lit.format();
        assert!(!s.is_empty());
    }
}
```

## Running

```bash
# Fast group (all provable harnesses)
cargo kani

# Full group (requires optional feature)
cargo kani --features kani_full
```
