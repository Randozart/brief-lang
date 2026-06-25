# Function Lens Properties — `defn/inop!/txn :> Metadata`

**Date:** 2026-06-25
**Status:** Planned
**Replaces:** `docs/plans/2026-06-25-native-brief-io-followup.md` Item 2 (Extension B)

---

## Motivation

Functions are first-class citizens in Brief's contract system, but there is no way to
query their metadata at runtime. This plan extends the `:>` projection/lens system
(already used for `list :> Size`, `map :> Keys`, etc.) to work on callable declarations:
`defn`, `inop!`, and `txn`.

A programmer writes:
```brief
defn add(x: Int, y: Int) -> Int [x > 0][y > 0] { term x + y; };

// Function properties via lens
let addr: Int    = add :> FnPtr;       // entry address (for FFI callbacks)
let name: String = add :> FnName;       // "add"
let arity: Int   = add :> FnArity;      // 2
let params: String = add :> FnParams;   // "Int, Int"
let returns: String = add :> FnReturns; // "Int"
let loc: String  = add :> FnLoc;        // "src/lib/math.bv:14"
let doc: String  = add :> FnDoc;        // doc comment or ""
let hash: Int    = add :> FnHash;       // stable content hash
let contracts: String = add :> FnContracts; // "[x > 0][x > 0 && y > 0]"
let module: String = add :> FnModule;   // "std/math"
let is_pure: Bool = add :> FnIsPure;     // true (defn is always pure)
```

The lens works identically on `inop!` and `txn`:
```brief
let h: Int    = my_isr :> FnPtr;   // ISR address for IVT
let doc: String = compute :> FnDoc; // txn doc string
```

---

## Design

### 1. New ProjectionTarget Variants

Add to `ProjectionTarget` in `src/ast.rs`:

| Variant | Returns | Description | Stable | Cost |
|---|---|---|---|---|
| `FnPtr` | `Int` | Function entry point address (lowered to `ptrtoint`) | ✅ | Trivial |
| `FnName` | `String` | The declaration name as written | ✅ | Trivial |
| `FnParams` | `String` | Comma-separated parameter types | ✅ | Trivial |
| `FnReturns` | `String` | Comma-separated return types | ✅ | Trivial |
| `FnArity` | `Int` | Number of parameters | ✅ | Trivial |
| `FnLoc` | `String` | Source location `file:line:col` | ✅ | Trivial |
| `FnDoc` | `String` | Doc comment text (or empty) | ✅ | Trivial |
| `FnHash` | `Int` | Stable content hash (fxhash of name+params+body) | ✅ | Tiny |
| `FnContracts` | `String` | Serialized pre/post condition | ✅ | Trivial |
| `FnModule` | `String` | Module path (from `import` path) | ✅ | Trivial |
| `FnIsPure` | `Bool` | True for `defn`, `inop` (without `!`), `txn` (callable) | ✅ | Trivial |
| `FnSpan` | `(Int, Int)` | Start and end line numbers (1-indexed) | ✅ | Trivial |

**Deferred** (not in first implementation):

| Variant | Returns | Reason for deferral |
|---|---|---|
| `FnBody` | `String` | Full source text — potentially massive, low utility without eval |
| `FnCalls` | `[String]` | Requires full call graph analysis pass; backend-dependent |
| `FnSize` | `Int` | Instruction count is backend-dependent (LLVM vs WASM vs CIRCT) |

### 2. ProjectionTarget Name Convention

All function metadata variants use the `Fn` prefix to avoid ambiguity with
value-type projections. A function variable `f :> Ptr` extracts the Ptr property
of `f`'s value (existing behavior) whereas `f :> FnPtr` extracts the function
entry address.

### 3. Parser

**No changes.** `f :> FnPtr` is parsed by the existing `parse_projection_target`
function, which already has a `_` catch-all for unrecognized names. New variants
are handled by adding explicit `"FnPtr" => Ok(ProjectionTarget::FnPtr)` arms.

**File:** `src/parser.rs`
**Function:** `parse_projection_target` (line 6973)
**Change:** Add 12 new `"FnName" => Ok(ProjectionTarget::FnName)` match arms.
**Lines added:** ~14

### 4. Typechecker

**File:** `src/typechecker.rs`
**Function:** `infer_expression` — `Expr::Projection` arm (line 2357)

**Logic:**

When the source expression is `Expr::Identifier(name)` and `name` resolves to a
`defn`/`inop!`/`txn`, dispatch on the projection target for function metadata:

```
match target {
    ProjectionTarget::FnPtr => Type::Int,
    ProjectionTarget::FnName => Type::String,
    ProjectionTarget::FnParams => Type::String,
    ProjectionTarget::FnReturns => Type::String,
    ProjectionTarget::FnArity => Type::Int,
    ProjectionTarget::FnLoc => Type::String,
    ProjectionTarget::FnDoc => Type::String,
    ProjectionTarget::FnHash => Type::Int,
    ProjectionTarget::FnContracts => Type::String,
    ProjectionTarget::FnModule => Type::String,
    ProjectionTarget::FnIsPure => Type::Bool,
    ProjectionTarget::FnSpan => Type::Tuple(vec![Type::Int, Type::Int]),
}
```

**Detection of callable identifiers:**

The typechecker needs a method `is_callable_name(name: &str) -> Option<FnMeta>`
that checks whether `name` is registered in:
- `self.definitions` (HashMap<String, &Definition>)
- `self.inop_decls` (HashMap<String, InopDeclaration>)
- `self.transactions` (HashMap<String, &Transaction>) — only callable txns

**Error handling:**
- If `name` is not a callable identifier, and the projection target is `Fn*`,
  emit a diagnostic: "`FnPtr` projection requires a function, transaction, or
  inop declaration name"
- If `name` IS callable but the projection target is NOT `Fn*`, fall through
  to the existing projection logic (the function returns an Int, so `f :> Size`
  is the Size of the return value, not the function)

**How function return types currently resolve:**

In the current typechecker, `self.definitions.get(name)` gives a `&Definition`,
which has `outputs: Vec<Type>` and `output_type: Option<OutputType>`. The
typechecker uses these when typechecking `Expr::Call(name, args)`.

The `OutputType` enum:
```rust
pub enum OutputType {
    Single(Box<Type>),
    Multi(Vec<Type>),
    Variable(Vec<Type>),
}
```

For `defn`, the type is determined by `output_type`. For `inop!`, by `outputs`.
For `txn`, by `output_type` and `outputs`.

The `FnParams` and `FnReturns` projections serialize these to strings. The
format is:
- `FnParams`: `"Int, Int, Bool"` — comma-separated type names
- `FnReturns`: `"Int"` or `"Int, Bool"` — comma-separated type names

The element type names come from the `Type::display()` method (or a more
compact serialization).

### 5. Interpreter

**File:** `src/features/projection.rs` — `ExprEval::evaluate` (line 25)
**File:** `src/interpreter.rs` — `Expr::Projection` dispatch (line 5796)

When the source expression is `Expr::Identifier(name)` and `name` resolves to a
callable declaration in the interpreter's state, return the appropriate metadata:

| Target | Interpreter return value |
|---|---|
| `FnPtr` | `Value::Int(0)` — sentinel; real addresses only exist in codegen |
| `FnName` | `Value::String(name.clone())` |
| `FnParams` | `Value::String(params.join(", "))` |
| `FnReturns` | `Value::String(returns.join(", "))` |
| `FnArity` | `Value::Int(params.len() as i64)` |
| `FnLoc` | `Value::String(loc)` — from declaration's span |
| `FnDoc` | `Value::String(doc)` — extracted from comments |
| `FnHash` | `Value::Int(hash)` — stable hash of name+params+returns+body |
| `FnContracts` | `Value::String(contracts)` — pre/post as serialized string |
| `FnModule` | `Value::String(module)` — from import path (or "") |
| `FnIsPure` | `Value::Bool(true/false)` — false for `inop!` and reactive `txn` |
| `FnSpan` | `Value::Tuple(vec![Value::Int(start), Value::Int(end)])` |

**Detection of callable identifiers in interpreter:**

The interpreter stores:
- `inop_decls: HashMap<String, InopDeclaration>` (line ~511)
- Definitions and txns in `TypeUniverse` or program state
- Or we add a `fn_meta: HashMap<String, FnMeta>` map populated at init time

**Storing FnMeta at program load time:**

A struct to hold function metadata:
```rust
#[derive(Debug, Clone)]
pub struct FnMeta {
    pub kind: FnKind,
    pub params: Vec<Type>,
    pub outputs: Vec<Type>,
    pub span: Option<Span>,
    pub doc: String,
    pub module_path: String,
    pub has_side_effects: bool,
}
```

Populated once during interpreter initialization by scanning:
- `TopLevel::Definition(d)` → FnMeta { kind: FnKind::Defn, ... }
- `TopLevel::Inop(i)` → FnMeta { kind: FnKind::Inop, ... }
- `TopLevel::Transaction(t)` → FnMeta { kind: FnKind::Txn, ... } (only callable)

### 6. LLVM Backend

**File:** `src/backend/llvm/emit_expr.rs` — `Expr::Projection` arm (line 2695)

| Target | LLVM IR |
|---|---|
| `FnPtr` | `%addr = ptrtoint @fn_name to i64` |
| `FnName` | `@fn_name_str = private unnamed_addr constant [N x i8] c"name\00"` + `getelementptr` |
| `FnParams` | Same as FnName — constant string global |
| `FnReturns` | Same — constant string global |
| `FnArity` | `add i64 0, N` |
| `FnLoc` | Constant string global |
| `FnDoc` | Constant string global |
| `FnHash` | `add i64 0, HASH` |
| `FnContracts` | Constant string global |
| `FnModule` | Constant string global |
| `FnIsPure` | `add i64 0, 1` (true) or `add i64 0, 0` (false) |
| `FnSpan` | Insert into a 2-element `%Tuple` struct |

**Implementation strategy for string constants:**

For each function metadata string (FnName, FnParams, FnReturns, FnLoc, FnDoc,
FnContracts, FnModule), the LLVM backend emits an anonymous global constant
the first time it's requested and caches it. Subsequent references reuse the
same global.

For number-valued targets (FnPtr, FnArity, FnHash, FnIsPure), inline constants
are used.

**Detection of callable identifiers in LLVM backend:**

The LLVM backend stores function metadata:
- `self.defn_params: HashMap<String, Vec<Type>>` (line 1597)
- `self.inop_decls: HashMap<String, InopDeclaration>` (line 1605)
- `self.defn_return_types: HashMap<String, Vec<Type>>` (line 1599)

When `Expr::Projection { source: Expr::Identifier(name), target: FnPtr }`
is encountered, check if `name` is in any of these maps. If so, emit the
function metadata projection. Otherwise, fall through to existing projection
logic.

### 7. Webstack Backend

**File:** `src/backend/webstack.rs`

Both TypeScript and Rust codegen need at minimum a return value for each `Fn*`
projection target. Since metadata strings are statically known, they can be
inlined:

```rust
ProjectionTarget::FnName => format!("\"{}\"", fn_name),
ProjectionTarget::FnArity => format!("{}", arity),
ProjectionTarget::FnParams => format!("\"{}\"", params),
// etc.
```

`FnPtr` returns `0` (no real pointer in WASM). `FnIsPure` returns `true`/`false`.

### 8. CIRCT Backend

**File:** `src/backend/circt.rs`

No current `Expr::Projection` handling exists. Add a catch-all for `Fn*`
targets that produces a constant 0 or empty string, matching the existing
pattern of minimal CIRCT support.

### 9. Analysis Files

Every file that matches `Expr::Projection { source, target }` and recurses
into `source` works without changes — the source is still an expression, and
function metadata projections have the same structural shape.

Files that need explicit arm additions:

#### `src/features/projection.rs` — `ExprEval::evaluate` (line 25)
Add 12 new match arms for `Fn*` targets. These check if the source evaluates
to an `Expr::Identifier(name)` and look up `name` in a metadata map. If not
a function name, error.

#### `src/analysis/transition_graph.rs` — `projection_target_name` (line 929)
Add 12 new arms mapping `Fn*` variants to their string names.

#### `src/typechecker.rs` — `infer_expression`
Add 12 new arms for `Fn*` target type inference.

#### `src/backend/llvm/emit_expr.rs` — main projection match
Add 12 new arms for `Fn*` LLVM codegen.

#### `src/backend/webstack.rs`
Add 12 new arms in both `expr_to_ts` and `expr_to_rust`.

#### `src/backend/circt.rs`
Add 12 new catch-all arms returning constants.

#### `src/symbolic.rs`
Add 12 new arms returning `SymbolicValue::Unknown`.

---

## Implementation Order

### Phase 1: Core (2-3 hr)

1. **`src/ast.rs`**: Add 12 `Fn*` variants to `ProjectionTarget`
2. **`src/parser.rs`**: Add 12 match arms in `parse_projection_target`
3. **`src/typechecker.rs`**: Add type inference in `infer_expression` with `is_callable_name` check
4. **`src/features/projection.rs`**: Add interpreter evaluation with `FnMeta` lookup
5. **`src/interpreter.rs`**: Initialize `fn_meta` map at program load time
6. **`src/backend/llvm/emit_expr.rs`**: Add LLVM codegen for all 12 targets
7. **`src/analysis/transition_graph.rs`**: Add `projection_target_name` arms
8. **`src/symbolic.rs`**: Add `SymbolicValue::Unknown` arms
9. Tests for each target in interpreter + parser + LLVM

### Phase 2: Auxiliary backends (30 min)

1. **`src/backend/webstack.rs`**: Add TypeScript + Rust codegen arms
2. **`src/backend/circt.rs`**: Add constant-returning arms

### Phase 3: Docs (30 min)

1. **`docs/architecture/features/projection.md`**: Document all 12 `Fn*` targets
2. **`examples/function-metadata.bv`**: Example file

---

## Per-commit checklist

- `cargo test --lib` — all tests pass
- `cargo build` — no warnings
- `_ => return None;` fallthrough unchanged in all optimization passes
- No weakening of existing optimization paths
- Praetor on new/changed files (complexity ≤ 15, lines ≤ 100, params ≤ 6)
- `projection_target_name()` in `transition_graph.rs` stays in sync
- Kani harnesses for all safety-critical code

---

## Design Decision: No Indirect Calls

`fn_name :> FnPtr` provides the **address** of a function as an integer. This is
sufficient for FFI callbacks (passing to C via `frgn`) and embedded interrupt
vector tables (storing via `volatile_store#`).

**Indirect calls from within Brief are intentionally not supported.** Calling
through a function pointer would bypass:

- **Contracts** — no pre/post conditions on an indirect call
- **Call graph analysis** — dead code elimination, inlining, and the proof
  engine rely on known call targets
- **Optimization** — LLVM cannot inline or SROA across indirect call boundaries

Every use case for indirect calls has a better Brief-native mechanism:

| C pattern | Brief equivalent | Guarantees |
|---|---|---|
| `dispatch_table[i](arg)` | `rct txn` with contract convergence | Contract-proven, optimizable |
| `sort(&cmp_fn, list)` | Generics + `:>` type dispatch | Static dispatch, inlinable |
| `signal(SIGINT, &handler)` | `frgn signal(sig: Int, handler: Int)` + `handler :> FnPtr` | Address at boundary, contracts intact |

### Embedded Brief note

For Embedded Brief targets, `fn_name :> FnPtr` is critical: it allows storing
function addresses into interrupt vector tables, callback registration slots,
and linker-specified entry points — all typed through contract-proven `inop!`
declarations with `volatile_store#`. The address is just an integer; Brief's
lens syntax makes querying it a first-class language operation while keeping
the interior of the function under full contract protection.

## Replaces: Cancelled from Previous Plan

This plan **replaces and cancels** the following from
`docs/plans/2026-06-25-native-brief-io-followup.md`:

### Item 2: Extension B — `fn(T) -> U` function pointer type + `&f` address-of

**Status: ❌ CANCELLED**

Reason: The `fn(T) -> U` type and `&f` address-of operator required:
- New `Type::Fn` AST variant (affecting type_universe, parser, typechecker)
- New `Expr::AddressOf` AST variant
- Parser changes for `&f` disambiguation from `Expr::OwnedRef`
- Updates at 204 `OwnedRef` match sites across 22+ source files
- LLVM indirect call codegen

All of this is obviated by `f :> FnPtr` lens syntax, which:
- Uses existing `:>` projection infrastructure (22 files already handle it)
- Requires zero parser changes (existing `:>` parsing works)
- Requires zero AST changes (new `ProjectionTarget` variants only)
- Requires zero `OwnedRef` match site changes
- Is more idiomatic: "extract the address property of f" vs "address-of operator"
- Consistent with Brief's existing lens philosophy

### What survives from the follow-up plan unchanged:

| Item | Status |
|------|--------|
| ✅ Item 1: Extension D — symexec fallthrough | DONE this session |
| ✅ Item 3: Extension C — `#section` on `inop!` | DONE this session |
| ✅ Item 4: Phase 3 — `lib/std/syscall.bv` | DONE this session |
| ✅ Item 5: Phase 4 — `#!cfg` conditional compilation | DONE this session |
| ✅ Item 7: Missing example files | DONE this session |
| ⏳ Item 6: Phase 5 — DBS/DBL device address maps | PENDING |
| ⏳ Item 8: Phase 6 — Stdlib I/O rewrite | PENDING |
