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

### Phase 1.5 — `TopLevel::TypeDef` (Type Derivation via Primitive Kernel)

> ⚠️ **Supersedes prior 1.5 design.** The original—hardcoded `ProjectionTarget` variants (Volatile, Atomic, Endian, etc.) as settable type properties—is preserved as-is below under **"Superseded Design"** for reference. The new design drastically shrinks the primitive kernel. Old content is non-destructively retained.

**Rationale**: "What is the smallest set of primitives the Rust compiler must hardcode so that everything else can be defined in Brief?"

The answer: **~10 primitives.** `Bytes`, `Alignment`, `Endian`, `Volatile`, `Atomic` describe physical layout. `ElementType`, `FixedSize`, `InsertAt`, `ExtractFrom`, `AllowIndex`, `AllowSlice`, `AllowArrow` describe collection behavior. Codecs provide encoding/decoding. Everything else (`String`, `Stack`, `Queue`, `HashMap`, etc.) is user-space Brief in `std/core.bv`.

#### Primitive Kernel (compiler natively understands these)

| Property | Type | Default | Meaning |
|----------|------|---------|---------|
| `Bytes` | `Int` | _required_ | Physical width in memory — LLVM `alloca`, VHDL width |
| `Alignment` | `Int` | `= Bytes` | Alignment boundary — LLVM `align` |
| `Endian` | `Enum` | `Little` | Byte order — LLVM `bswap`/load-store order |
| `Volatile` | `Bool` | `false` | LLVM `load volatile`/`store volatile` |
| `Atomic` | `Bool` | `false` | LLVM atomic operations |
| `ElementType` | `Type` | _(none)_ | Unlocks `[]` and slicing — compiler synthesizes GEP/address-decoding |
| `FixedSize` | `Bool` | _(none)_ | `false` unlocks `<-` / `->` — heap/circular buffer strategy |
| `InsertAt` | `Expr` | _(none)_ | Index expression for insertion position: `0`, `:> Size`, `:> Size - N` |
| `ExtractFrom` | `Expr` | _(none)_ | Index or `<: {}` query for extraction position |
| `AllowIndex` | `Bool` | `true` | Override to `false` to block `[]` (Stack, Queue) |
| `AllowSlice` | `Bool` | `true` | Override to `false` to block slicing |
| `AllowArrow` | `Bool` | `true` | Override to `false` to block `<-`/`->` |
| `Codec` | `Struct` | _(none)_ | Struct with `encode`/`decode` — literal translation at compile-time |

`InsertAt`/`ExtractFrom` **expression forms the compiler recognizes:**

| Expression | Strategy | Example |
|---|---|---|
| `0` | Constant front, head-pointer advance | `Queue` pop |
| `:> Size` | Append position, pointer increments | `List`/`Queue` push |
| `:> Size - N` | Offset from end, pointer decrements | `Stack` pop |
| `<: { MIN(.k) }` | Maintain heap by key `k` | Priority queue |
| `<: { MAX(.k) }` | Maintain heap by key `k` | Priority queue |

Any other expression form is a **compile-time error** in Pass 1.

#### Two-Pass Pipeline

```
┌─────────────────────────────────────────────┐
│ PASS 1: Type-Universe Pass                  │
│  - Collect all TopLevel::TypeDef            │
│  - Resolve derivation chain to Bits         │
│  - Inherit + override metadata              │
│  - Validate Bytes required on all Bits types│
│  - Validate InsertAt/ExtractFrom forms      │
│  - Validate Codec has encode/decode         │
│  - Evaluate refinement constraints [> 0]   │
│  - FREEZE: type universe immutable          │
└──────────────────┬──────────────────────────┘
                   │
                   ▼
┌─────────────────────────────────────────────┐
│ PASS 2: Executable Pass                     │
│  - Parse defn/txn/rct                       │
│  - Resolve let x: Stack<T> against universe │
│  - Validate :> projections against metadata │
│  - Synthesize bracket/arrow from gates      │
│  - Encode literals via Codec                │
│  - Emit LLIR/VHDL with frozen metadata      │
└─────────────────────────────────────────────┘
```

**What lives in `std/core.bv` (user-space, not in Rust compiler):**

```brief
Type U8    <: Bits { Bytes = 1; Alignment = 1; };
Type U16   <: Bits { Bytes = 2; Alignment = 2; };
Type U32   <: Bits { Bytes = 4; Alignment = 4; };
Type U64   <: Bits { Bytes = 8; Alignment = 8; };
Type Int   <: U64;
Type Float <: Bits { Bytes = 8; Alignment = 8; };

Type List<T> <: Bits {
    ElementType = T;
    FixedSize = false;
    InsertAt = :> Size;
    ExtractFrom = :> Size - 1;
};
Type Stack<T> <: List<T> { AllowIndex = false; };
Type Queue<T> <: List<T> { ExtractFrom = 0; AllowIndex = false; };

import { Utf8 } from "std/utf8.bv";
Type String <: List<U8> { Codec = Utf8; };
```

#### Implementation Steps

| Step | Action | Files |
|------|--------|-------|
| 1.5.1 | Add `TopLevel::TypeDef`, `TypeProperty` enum, `Expr::TypeRef` | `ast.rs` |
| 1.5.2 | Create `src/type_universe.rs` — Pass 1 resolver | NEW |
| 1.5.3 | Implement `parse_type_def()` in parser | `parser.rs` |
| 1.5.4 | Create `features/toplevel/typedef.rs` with 5 stub impls + TypeProperty processing | NEW |
| 1.5.5 | Router arms: typechecker, interpreter, annotator (skip — compile-time only) | `typechecker.rs`, `interpreter.rs`, `annotator.rs` |
| 1.5.6 | Router arms: LLVM synthesize load/store from metadata; VHDL width | `llvm.rs`, `vhdl.rs`, `webstack.rs` |
| 1.5.7 | Tests: parse typedef, resolve U8/Bytes, verify AllowIndex=false blocks access | various |
| 1.5.8 | Kani fast harnesses for TypeProperty dispatch | `typedef.rs` |
| 1.5.9 | Architecture docs | `docs/architecture/features/typedef.md` |

**Gate**: `cargo test --lib` + `cargo kani --lib` (fast group).

---

#### Superseded Design (Original Phase 1.5 — preserved for reference)

The original plan hardcoded a fixed set of `ProjectionTarget` variants as settable type properties within `<:` blocks:

```brief
Type Queue<T> <: List<T> {
    Access = "FIFO";
    IndexAccess = false;
    Push = "back";
    Pop = "front";
};
```

**Settable targets** (removed in new design): `Volatile`, `Atomic`, `Endian`, `ClockDomain`, `BitWidth`, `Access`, `IndexAccess`, `Push`, `Pop`, `Unique`, `SIMD`, `Width`.

**Deleted from code**: These were never added to `ast.rs` — the design was superseded before implementation. No cleanup needed.

**Deprecation warning**: `AsStack`/`AsQueue` projection targets remain in the codebase but will be deprecated in Phase 6+ once behavioral type constraints (`InsertAt`/`ExtractFrom`/`AllowIndex`) provide equivalent functionality via `<:` derivation.

---

### Phase 1.5+ — Deferred Design (Important, Not Yet Implemented)

The Phase 1.5 design conversation surfaced several profound ideas that are **NOT implemented yet** but must be preserved for future phases. They are documented here and marked `DEFERRED` in code.

#### D-1: Expression Type Parameters (Universal Ordering)

Instead of hardcoding `KeyExpr` or field names like `.priority`, a generic type like `KeyedQueue<T, K>` receives the ordering expression as a type parameter. The compiler validates at instantiation that `T → K` is a valid projection.

```brief
// DEFERRED — syntax and instantiation rules TBD
Type KeyedQueue<T, K: Ordered> <: List<T> {
    ExtractFrom = <: { MAX(K) };
};
```

**Blockers**: Need a way to pass expressions as type parameters (not just types). The mechanism for binding `T →`→ `K` at instantiation is unresolved.

#### D-2: Full Codec Signature Validation

Codecs are structs imported from `std/` files. Pass 1 validates they have `encode`/`decode`.

**Future scope**:
- Duck-typing vs strict signature (`encode(self, LogicalType) -> StorageType`)
- Generic codecs that work across multiple type pairs
- Codec-defined `:>` targets (e.g., `:> Graphemes` for string character count)
- Compile-time literal encoding via internal interpreter

**Current implementation**: Minimal — just checks the struct has `encode` and `decode` fields. Full validation deferred.

#### D-3: InsertAt/ExtractFrom Synthesized Strategies

The compiler recognizes expression forms (`0`, `:> Size`, etc.) and validates them in Pass 1, but **does not yet synthesize**:
- Shift strategies for `InsertAt = 1`
- Circular buffer strategies for `InsertAt = 0` (prepend)
- Heap maintenance for `ExtractFrom = <: { MAX(.k) }`

**Current implementation**: Pass 1 validates the expression form. Strategy synthesis is a stub.

#### D-4: Deprecation of AsStack/AsQueue Projection Targets

Once `InsertAt`/`ExtractFrom`/`AllowIndex` are feature-complete, `ProjectionTarget::AsStack` and `ProjectionTarget::AsQueue` should be deprecated with a migration warning. Arrow dispatch should check type metadata instead of matching on `Value::Stack`/`Value::Queue`.

**Current implementation**: Both variants still live in `ProjectionTarget` enum. Not yet deprecated.

#### D-5: Bits Codec + :> Size Uniformity

The relationship between `Bytes` (physical width) and `Size` (element count) for scalar types needs resolution:

- `Int :> Size` currently errors ("requires collection type")
- `String :> Size` returns byte count, not character count
- Codecs should define codec-specific projections (`:> Runes`, `:> Graphemes`)

**Current implementation**: Projection target matching is still hardcoded per value type in interpreter. Not yet codec-extensible.

#### D-6: Volatile/Atomic as Both Pragma and Metadata

The design decision: `#volatile` pragma on field declarations for ergonomics, `Volatile = true` on `Type` blocks for structural queries. The pragma sets the metadata. Implemented as a desugaring pass.

**Current implementation**: Neither exists yet — both are deferred.

#### D-7: Constraint-to-Self Refinement Syntax

Refinement constraints in type bodies use implicit self:

```brief
Type PositiveInt <: Int {
    [ > 0 && < 100 ]
};
```

Pass 1 validates literals at compile time. Backend synthesizes runtime guards for dynamic values. The implicit self binding is `_` (consistent with `<:` query element binding).

**Current implementation**: Syntax exists (`[ expr ]` within `TypeDefBody`), but runtime guard synthesis is deferred.

#### D-8: CFG Files for Field-Level Metadata Matching

Discussion touched on using `.` prefix expressions (`FILTER(.active)`) inside `<:` query blocks. `_` binds the current element; `.active` desugars to `_.active` for field access.

**Current implementation**: `_` binding works in `<:` queries. `.active` field access on `_` works via existing FieldAccess desugaring. No special logic needed — already works.

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
