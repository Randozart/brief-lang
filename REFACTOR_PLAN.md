# Brief Compiler Refactor: Pattern B Architecture

**Date**: 2026-06-09  
**Branch**: `refactor/pattern-b` (from `main`)  
**Status**: Plan — awaiting execution signal

---

## Overview

Refactor the ~78K-line Brief compiler from monolithic match-arm dispatching to a **Pattern B (Struct-Variant Delegation)** architecture, where each AST construct lives in its own feature file with co-located parsing, typechecking, evaluation, and codegen logic.

This eliminates ~232 Praetor complexity violations, makes the codebase navigable by feature ("open one file, see one concept"), and prepares the compiler for self-hosting.

---

## Pragmas

### `#test("group1", "group2")` — Per-item decoration, no semicolon

```brief
#test("config")
txn test_parse_config() [true][true] { ... };

#test("vars", "config")
txn test_validate_vars() [true][true] { ... };
```

- Parsed via extended `parse_hashtag_modifiers`
- `Hashtag` gets new field `groups: Vec<String>`
- Wraps next item as `TopLevel::Test { item: Box<TopLevel>, groups: Vec<String> }`

### `#assert [pre] -> fnY -> fnZ` — Inline assertion, no semicolon

```brief
#assert [number == 5] -> funcY -> funcZ
txn my_func() [true][true] { ... };
```

- The item's own name is implicit start of the transition chain
- Proof engine verifies the chain at compile time

### `#!assert [pre] fnX -> fnY -> fnZ;` — Global assertion, semicolon required

```brief
#!assert [trigger == true] funcX -> funcY -> funcZ;
```

- Parsed in the `#!` directive loop (parallel to `#!exit`)
- Stored as `TopLevel::Assertion { pre: Expr, chain: Vec<String> }`

### Compilation modes

| Mode | Tests compiled? | Folding budget |
|------|----------------|----------------|
| `--dev` (default) | ✅ Included | 256 |
| `--prod` | ❌ Excluded | `u64::MAX` |
| `--prod --include-tests` | ✅ Included | `u64::MAX` |
| `--dev --exclude-tests` | ❌ Excluded | 256 |

CLI: `brief run --test --group config` runs only tests tagged with `"config"`.

---

## Directory Structure

```
src/
  features/
    mod.rs                   # Module declarations, re-exports
    traits.rs                # Trait definitions

    # Expression features
    literal.rs               # Integer, Float, String, Char, Bool, Term
    identifier.rs            # Identifier, OwnedRef, PriorState
    binary_op.rs             # 18 arithmetic/comparison/logical/bitwise ops
    unary_op.rs              # Not, Neg, BitNot
    call.rs                  # Call
    projection.rs            # Projection + 18 ProjectionTarget variants
    collection.rs            # ListLiteral, ListIndex, Slice, MultiSlice
    map.rs                   # MapLiteral
    set.rs                   # SetLiteral
    tuple.rs                 # Tuple, TupleDestructure
    field.rs                 # FieldAccess, StructInstance, ObjectLiteral
    pattern_match.rs         # PatternMatch
    match_expr.rs            # Match
    block.rs                 # Block
    arrow.rs                 # ArrowMut, ArrowDiscard, ArrowTransfer
    subtype.rs               # SubtypeProjection + 14 SubtypeOp variants
    cast.rs                  # Cast
    concat.rs                # Concat
    sig_call.rs              # SigCall
    dbvl.rs                  # DbvlTable
    ellipsis.rs              # Ellipsis

    # Statement features
    stmt/
      mod.rs
      assignment.rs
      let_binding.rs
      guarded.rs
      term.rs
      escape.rs
      expression.rs
      unification.rs
      inline_asm.rs
      local_trigger.rs
      alka.rs
      on_exit.rs
      sync_block.rs

    # TopLevel features
    toplevel/
      mod.rs
      signature.rs
      definition.rs
      transaction.rs
      state_decl.rs
      trigger.rs
      constant.rs
      import_lnk.rs          # Import + LinkDependency
      foreign.rs
      resource.rs
      struct_def.rs
      rstruct.rs
      enum_def.rs
      render.rs
      svg.rs
      sync_group.rs
      typedef.rs             # Type derivation via `<:` constraints
      test.rs                # #test pragma
      assertion.rs           # #!assert / #assert pragma

  backend/
    router.rs                # Central backend dispatch
    llvm.rs                  # Slimmed to ~3,000 lines (see LLVM strategy)
    llvm_optimizer.rs        # NEW — folded loop, SSA, decision tree
    vhdl.rs                  # Slimmed to ~400 lines
    webstack.rs              # Slimmed to ~800 lines
    ...                      # Other backends unchanged

  ast.rs                     # ~400 lines (enum variants → boxed feature structs)
  interpreter.rs             # ~500 lines (pure router)
  typechecker.rs             # ~400 lines (pure router)
  parser.rs                  # ~2,000 lines (token dispatch + precedence climbing)
  proof_engine.rs            # ~800 lines (symbolic engine)

  _monolithic/               # Old files, Praetor-ignored, deleted in Phase 8
```

---

## Trait Definitions (`features/traits.rs`)

One trait per concern, per backend. Separate traits = separate compilation units (changing VHDL emission doesn't recompile LLVM).

```rust
/// Parse: Parser routes tokens to feature struct constructors
pub trait ExprParse {
    type Output;
    fn parse(parser: &mut Parser) -> Self::Output;
}

/// Typecheck: Typechecker routes each Expr variant to its feature
pub trait ExprTypecheck {
    fn typecheck(
        &self,
        ctx: &mut TypecheckContext,
        dispatch: &ExprDispatch,
    ) -> Result<Type, TypeError>;
}

/// Eval: Interpreter routes each Expr variant to its feature
pub trait ExprEval {
    fn evaluate(
        &self,
        ctx: &mut EvalContext,
        dispatch: &ExprDispatch,
    ) -> Result<Value, RuntimeError>;
}

/// LLVM Codegen — feature structs reference &mut LlvmBackend directly
pub trait ExprCodegenLLVM {
    fn emit_llvm(
        &self,
        ctx: &mut LlvmBackend,
        out: &mut String,
        dispatch: &ExprDispatch,
    ) -> TypedRegister;
}

/// VHDL Codegen
pub trait ExprCodegenVHDL {
    fn emit_vhdl(
        &self,
        ctx: &mut VHDLContext,
        dispatch: &ExprDispatch,
    ) -> String;
}

/// Webstack Codegen
pub trait ExprCodegenWebstack {
    fn emit_js(
        &self,
        ctx: &mut WebstackContext,
        dispatch: &ExprDispatch,
    ) -> String;
}
```

Each feature struct implements only the traits relevant to it. Missing backend impls fall through to the router's default stub (existing behavior: `add i64 0, 0` for LLVM, `'0'` for VHDL, etc.).

---

## AST Transformation (`ast.rs`)

Each enum variant wraps a boxed feature struct:

```rust
pub enum Expr {
    Literal(Box<features::literal::LiteralExpr>),
    Add(Box<features::binary_op::BinaryOpExpr>),
    Sub(Box<features::binary_op::BinaryOpExpr>),
    Call(Box<features::call::CallExpr>),
    Projection(Box<features::projection::ProjectionExpr>),
    // ... 54 variants total
}

pub enum Statement {
    Assignment(Box<features::stmt::assignment::AssignmentStmt>),
    Let(Box<features::stmt::let_binding::LetStmt>),
    // ... 13 variants total
}

pub enum TopLevel {
    Definition(Box<features::toplevel::definition::DefinitionItem>),
    Transaction(Box<features::toplevel::transaction::TransactionItem>),
    Struct(Box<features::toplevel::struct_def::StructItem>),
    Test { item: Box<TopLevel>, groups: Vec<String> },
    Assertion { pre: Expr, chain: Vec<String> },
    // ... 17 variants total
}
```

---

## Router Pattern

Each main pass becomes a pure dispatch function. Feature structs call `dispatch` for sub-expression recursion.

```rust
// interpreter.rs — ~500 lines
pub fn eval_expr(expr: &Expr, ctx: &mut EvalContext, dispatch: &ExprDispatch) -> Result<Value, RuntimeError> {
    match expr {
        Expr::Literal(n) => n.evaluate(ctx, dispatch),
        Expr::Add(n) => n.evaluate(ctx, dispatch),
        Expr::Call(n) => n.evaluate(ctx, dispatch),
        // 54 variants, 1 line each
    }
}
```

---

## Feature File Template (`features/literal.rs`)

```rust
// ── Struct ──────────────────────────────────────────────
pub enum LiteralExpr {
    Integer(i64),
    Float(f64),
    String(String),
    Char(char),
    Bool(bool),
    Term,
}

// ── Parse ───────────────────────────────────────────────
impl ExprParse for LiteralExpr { ... }

// ── Typecheck ───────────────────────────────────────────
impl ExprTypecheck for LiteralExpr { ... }

// ── Eval ────────────────────────────────────────────────
impl ExprEval for LiteralExpr { ... }

// ── LLVM Codegen ────────────────────────────────────────
impl ExprCodegenLLVM for LiteralExpr {
    fn emit_llvm(&self, ctx: &mut LlvmBackend, out: &mut String, dispatch: &ExprDispatch) -> TypedRegister {
        match self {
            LiteralExpr::Integer(n) => { ctx.writeln!(out, "..."); }
            // ...
        }
    }
}

// ── VHDL Codegen ────────────────────────────────────────
impl ExprCodegenVHDL for LiteralExpr { ... }

// ── Webstack Codegen ────────────────────────────────────
impl ExprCodegenWebstack for LiteralExpr { ... }

// ── Unit tests ──────────────────────────────────────────
#[cfg(test)]
mod tests { ... }
```

---

## LLVM Backend Strategy — Pragmatic Extraction

The LLVM backend (7,799 lines) has optimizations deeply interwoven with codegen:

| Optimization | Location | Strategy |
|---|---|---|
| `simplify()` pre-pass | Inline in `emit_expr` | ✅ Hoist to pre-pass on `Program` before codegen |
| Peephole folding | Inline in `emit_binop`/`emit_fcmp` | ✅ Move to pre-pass or keep in helpers |
| `emit_expr` match arms (22 variants) | `emit_expr` (622 lines) | ✅ Extract into feature `ExprCodegenLLVM` impls |
| `emit_stmt` match arms (13 variants) | `emit_stmt` (385 lines) | ✅ Extract into statement feature files |
| Optimization decision tree | `generate()` (898 lines) | ⚠️ Extract to `backend/llvm_optimizer.rs` (self-contained) |
| Folded loop engine (4 functions) | 700 lines | ⛔ Keep centralized — spans features, manages phi/SSA state |
| SSA mode + pre-extraction | 150 lines | ⛔ Keep centralized |
| Parallel reactor + dispatch chain | 260 lines | ⛔ Keep centralized |
| Perfect hashing | 30 lines | ✅ Extract to separate helper |
| SLP hazard analysis | 100 lines | ✅ Already separate |
| `LlvmBackend` struct (48 fields) | 78 lines | ⛔ Keep as-is — context object shared by all feature impls |
| Tests (86 tests) | 2,837 lines | ⛔ Keep intact — integration tests |

**Result**: `llvm.rs` shrinks from 7,799 to ~3,000 lines. The struct stays. The tests stay. The folded loop engine stays. The match arms move to feature files.

Feature files reference `&mut LlvmBackend` directly:

```rust
impl ExprCodegenLLVM for LiteralExpr {
    fn emit_llvm(&self, ctx: &mut LlvmBackend, out: &mut String, dispatch: &ExprDispatch) -> TypedRegister {
        // ctx.txn_counter, ctx.let_bindings, ctx.writeln!() — all available
    }
}
```

**Why this works**: Each match arm is self-contained — takes `expr`, produces `TypedRegister`, uses `&mut self` for shared state. Moving them to feature files is pure mechanical extraction with zero behavioral change.

---

## VHDL and Webstack Backends

Same treatment: expression/statement codegen moves into feature files. Their optimization strategies are deferred — developed after the main refactor, using LLVM as the testing ground.

---

## Old File Retention (`_monolithic/`)

Old files move to `_monolithic/` as they are superseded. Praetor ignores the directory:

```toml
# .praetor.toml
[files]
ignore = ["src/_monolithic/**"]
```

Deleted only in Phase 8 after full test parity is confirmed.

---

## Migration Phases

### Phase 0 — Scaffolding

| Step | Action |
|------|--------|
| 0.1 | Create branch `refactor/pattern-b` |
| 0.2 | Create `src/features/mod.rs`, `features/traits.rs` |
| 0.3 | Create `src/backend/router.rs` (empty dispatch) |
| 0.4 | Create `src/_monolithic/` with `.gitkeep`, update `.praetor.toml` |
| 0.5 | Update `src/lib.rs` to declare `pub mod features;` |

**Gate**: `cargo build` succeeds.

### Phase 1 — Expr Features (28 files, 4 sub-steps)

| Step | Feature | Rationale | Status |
|------|---------|-----------|--------|
| 1.1 | literal | Simplest — proof of concept | ✅ **Done** |
| 1.2 | binary_op, unary_op | Mechanical extraction (18+3 variants) | 🔜 Next |
| 1.3 | call, projection, collection, map, set, tuple, field | Medium complexity | ⏳ |
| 1.4 | pattern_match, match, block, arrow, subtype, cast, concat, sig_call, dbvl, ellipsis | Higher complexity | ⏳ |

**Per sub-step**:
1. Create feature file with struct + trait impls
2. Add new variant to `Expr` enum in `ast.rs`
3. Add delegation arm in all 38 router methods
4. `cargo test --lib` ✅
5. Remove old inline match arm logic
6. `cargo test --lib` ✅
7. Move old code to `_monolithic/`

---

### Phase 1.5 — `TopLevel::TypeDef` (Type Derivation)

**Rationale**: Unifies type aliasing, refinement types, bit-width declarations, fixed-size collections, and behavioral type constraints (Queue/Stack) under a single `<:` derivation operator. `List<T>` becomes the only primitive sequential collection; `Queue`, `Stack`, `OrderedSet`, etc. become `<:` derivations with behavioral constraints.

| Step | Action | Files |
|------|--------|-------|
| 1.5.1 | Add `TypeDef` keyword to lexer | `lexer.rs` |
| 1.5.2 | Add `TopLevel::TypeDef` variant + new `ProjectionTarget` variants (Volatile, Atomic, Endian, ClockDomain, BitWidth, Access, IndexAccess, Push, Pop, Unique, SIMD, Width) | `ast.rs` |
| 1.5.3 | Implement `parse_type_def()` + constraint target validation in parser | `parser.rs` |
| 1.5.4 | Create `features/toplevel/typedef.rs` with typecheck + codegen | NEW |
| 1.5.5 | Router arm in typechecker (constraint type validation + compile-time regex enforcement) | `typechecker.rs` |
| 1.5.6 | Router arms in backends (LLVM: emit width/volatile/atomic/endian/bswap; VHDL: clock domain/width) | `llvm.rs`, `vhdl.rs`, `webstack.rs` |
| 1.5.7 | Skip arm in interpreter (type defs are compile-time only) | `interpreter.rs` |
| 1.5.8 | Tests: parser (6+), typechecker (5+), Kani fast harnesses (pure match dispatch) | `parser.rs`, `typechecker.rs`, + kani |
| 1.5.9 | Architecture docs | `docs/architecture/features/typedef.md` |

**Constraint grammar**:

```brief
Type Queue<T> <: List<T> {
    Access = "FIFO";
    IndexAccess = false;
    Push = "back";
    Pop = "front";
};
```

**Settable vs query-only targets**:

| Category | Targets |
|----------|---------|
| **Layout constraints** (set + read) | `Size`, `Bytes`, `Alignment`, `Range`, `BitWidth`, `Volatile`, `Atomic`, `Endian`, `ClockDomain`, `Match` |
| **Behavioral constraints** (set + read) | `Access`, `IndexAccess`, `Push`, `Pop`, `Unique`, `SIMD`, `Width` |
| **Query only (`:>`) — rejected in `<:` constraint block** | `Keys`, `Values`, `Contains`, `Pop`, `Index`, `Get`, `Top`, `Front`, `Elements`, `AsStack`, `AsQueue`, `Ptr`, `PtrBang`, `Type`, `Offset`, `Popcount`, `LeadingZeros`, `TrailingZeros`, `Absolute`, `BitReverse` |

**Future (Phase 6+)**: After TypeDef infrastructure is mature, deprecate `Value::Stack`/`Value::Queue` variants in the interpreter. Arrow dispatch checks type constraints instead of matching on enum variants. `AsStack`/`AsQueue` projection targets deprecated with migration warning.

**Total estimate**: ~600–800 lines. **Gate**: `cargo test --lib` + `cargo kani --lib` (fast group).

### Phase 2 — Statement Features (13 files)

| Step | Feature |
|------|---------|
| 2.1 | assignment |
| 2.2 | let_binding |
| 2.3 | guarded |
| 2.4 | term (Term + TermBang) |
| 2.5 | escape, expression |
| 2.6 | unification, inline_asm, local_trigger, alka, on_exit, sync_block |

Same per-step workflow. **Gate**: `cargo test --lib` after each.

### Phase 3 — TopLevel Features (17 files)

| Step | Feature |
|------|---------|
| 3.1 | signature, definition, transaction |
| 3.2 | struct_def, rstruct, enum_def |
| 3.3 | state_decl, trigger, constant |
| 3.4 | import_lnk, foreign, resource |
| 3.5 | render, svg, sync_group |

**Gate**: `cargo test --lib` after each.

### Phase 4 — Router Pass Simplification

| File | Before | After | Change |
|------|--------|-------|--------|
| `interpreter.rs` | 5,504 | ~500 | Inline logic → pure router |
| `typechecker.rs` | 2,157 | ~400 | 450-line `infer_expression` → pure router |
| `parser.rs` | 7,389 | ~2,000 | Token dispatch + precedence remains; constructors delegate |
| `proof_engine.rs` | 3,655 | ~800 | `SymbolicValue::from_expr` → router; feature files handle symbolic conversion |
| `ast.rs` | 1,240 | ~400 | Enum variants → boxed feature structs |
| `backend/mod.rs` | 841 | ~200 | Tree-walk helpers → feature files |
| `backend/llvm.rs` | 7,799 | ~3,000 | `emit_expr`/`emit_stmt` → routers; `simplify` hoisted; decision tree extracted |
| `backend/router.rs` | 0 | ~200 | NEW — central backend dispatch |
| `backend/vhdl.rs` | 1,261 | ~400 | `expr_to_string` → router |
| `backend/webstack.rs` | 2,230 | ~800 | `expr_to_js_value` → router |

**Gate**: `cargo test --lib` + verify 0 new Praetor violations.

### Phase 5 — `#test("group")` Pragma

| File | Change |
|------|--------|
| `ast.rs` | Add `TopLevel::Test { item, groups }`, `groups` field on `Hashtag` |
| `parser.rs` | Extend `parse_hashtag_modifiers` for `#test(...)` |
| `features/toplevel/test.rs` | New: `TestItem` struct with typecheck/eval/codegen |
| `interpreter.rs` | Skip `TopLevel::Test` in prod; include in test mode |
| `typechecker.rs` | Typecheck inner item normally |
| `backend/*.rs` | Omit `TopLevel::Test` in prod output |
| `main.rs` | Add `--test`, `--group`, `--include-tests`, `--exclude-tests` flags |

**Test**: `#test("group") txn test_a() ...;` compiles only under `--dev` or `--prod --include-tests`. `--group config` filters to matching groups.

### Phase 6 — `#!assert` / `#assert` Pragma

| File | Change |
|------|--------|
| `ast.rs` | Add `TopLevel::Assertion { pre, chain }` |
| `parser.rs` | Add `#!assert` arm in `#!` directive loop; `#assert` in `parse_hashtag_modifiers` |
| `features/toplevel/assertion.rs` | New: symbolic chain verification |
| `proof_engine.rs` | Integrate assertion verification into `verify_program` |
| `typechecker.rs` | Validate chain element names exist as functions/transactions |

**Test**: `#!assert [x > 0] fnA -> fnB -> fnC;` triggers symbolic chain at compile time.

### Phase 7 — Backend Optimization Prep

| File | Change |
|------|--------|
| `backend/llvm_optimizer.rs` | NEW — extract decision tree from `generate()`, reference folded loop engine |
| `backend/llvm.rs` | Remove decision tree; keep `emit_*` routers + struct + tests |

**Nothing behavioral changes**. Pure file reorganization.

**Gate**: `cargo test --lib`

### Phase 8 — Final Cleanup

| Step | Action |
|------|--------|
| 8.1 | Verify zero old-file references in imports |
| 8.2 | Delete `src/_monolithic/` |
| 8.3 | Run Praetor: 0 diagnostics on all new feature files |
| 8.4 | `cargo test --lib` — all 526+ tests pass |
| 8.5 | Run benchmark suite — no regression vs `main` |
| 8.6 | Commit final state |

---

## Key Principles

| Principle | Rationale |
|-----------|-----------|
| **One file, one concept** | Open `features/call.rs` → see parse/typecheck/eval/LLVM/VHDL/Webstack/tests for `Call` in one place |
| **LLVM: pragmatic extraction** | Extract match arms into features; keep folded loop / SSA / decision tree centralized. 80% benefit, 20% risk. |
| **Backend-independent** | Separate traits per backend. Missing trait = router's default stub. Backend work is purely additive. |
| **Incremental, test-gated** | `cargo test --lib` after every sub-step. Never more than one migration away from green. |
| **Praetor-clean from day one** | Every new function ≤ 100 lines, complexity ≤ 15, params ≤ 6. Old code quarantined in `_monolithic/`. |
| **Semicolon rule** | `#!` global directives get `;`. `#` decorations do not — they belong to the item they prepend. |

---

## Total Impact

| Metric | Current | Target |
|--------|---------|--------|
| Feature files | 1 (literal) | ~60 |
| Files dispatching `Expr::` | 38 | 1 (the router) |
| `interpreter.rs` | 5,507 lines | ~500 |
| `parser.rs` | 7,452 lines | ~2,000 |
| `typechecker.rs` | 2,166 lines | ~400 |
| `llvm.rs` | 7,861 lines | ~3,000 |
| `ast.rs` | 1,422 lines | ~400 |
| Praetor violations | ~232 | 0 |
| Kani fast harnesses | 14 | 20–30 |
