# Pure Bits Interpreter Refactor

**Phase:** 8A–8F  
**Plan doc:** `docs/plans/2026-07-11-pure-bits-refactor.md`  
**Thesis doc:** `docs/architecture/bits-thesis.md`  
**Total scope:** ~34 files, ~2300 lines changed  
**Strategy:** Strangler Fig — build new path alongside old, migrate one type at a time, delete dead code only after green tests

---

## Motivation

The interpreter currently has 24 `Value` enum variants. Ten of them (`Int`,
`Float`, `Bool`, `Char`, `String`, `Data`, `StringBuilder`, `Stack`, `Queue`,
`Ptr`) are typed representations of specific types. They violate the Bits
thesis: the compiler should not know about `Int` or `Float` at the interpreter
level — it should only know about `Bits`, with all semantics injected from
type properties.

The typed variants also produce deeply nested arrow code throughout the
interpreter, the FFI registry, the binary operator dispatch, the collection
operations, and the projection evaluator. A `match (kind, &l, &r)` on 50+
combinations of typed variants is the textbook definition of arrow-shaped code.

This refactor eliminates all typed representational variants, replaces them
with `Value::Bits(Vec<u8>)`, and routes all operations through a
property-based intrinsic dispatch engine. The result is a dramatically
simpler interpreter with flat control flow.

---

## Core Principles

1. **Add, don't delete** — New infrastructure built alongside old. Legacy
   paths remain active until migration is verified.
2. **One type at a time** — Migrate `Int`, then `Bool`, then `Char`, then
   `Float`. Each step is independently testable.
3. **Flat control flow** — Every phase eliminates specific arrow-shaped match
   arms. Target: max 2 nesting levels in all modified functions.
4. **Green tests after every commit** — `cargo test --lib` must pass before
   moving to the next sub-step.

---

## Phase 7.5 — Prelude Cache (Preparatory)

**Goal:** Cache the evaluated prelude so recompilation skips virtual-heap
allocation and type resolution for `lib/std/`. Reduces the pain of
VirtualHeap allocation (Phase 8A) by making it a one-time cost.

### Step 7.5.0 — Define cache format

**File:** `src/cache/mod.rs` (new) or add to `src/type_universe.rs`

```rust
/// Snapshot of the interpreter state after prelude evaluation.
/// 2026-07-11: Phase 7.5 — eliminates prelude re-evaluation.
struct PreludeCache {
    compiler_version: u64,
    prelude_timestamps: HashMap<String, SystemTime>,
    virtual_heap: VirtualHeap,
    type_universe: TypeUniverse,
    ffi_registry: HashMap<String, FfiEntry>,
}
```

Serialize via `bincode` with `serde` derives. Cache file at
`cache/prelude.bincode`. On cache hit, skip loading `lib/std/*.bv`
entirely — deserialize the cached state instead.

### Step 7.5.1 — Add VirtualHeap

**File:** `src/interpreter.rs`

```rust
/// Sandboxed virtual memory space for compile-time execution.
/// Maps virtual addresses to allocated byte blocks.
/// 2026-07-11: Phase 7.5.
#[derive(Serialize, Deserialize)]
pub struct VirtualHeap {
    allocations: HashMap<u64, Vec<u8>>,
    next_address: u64,
}
```

Add `__malloc#` and `__free#` intrinsics that operate on this heap.

### Step 7.5.2 — Cache invalidation

- Check file timestamps of all `lib/std/*.bv` files
- Compare against `compiler_version` (bumped when intrinsic registry changes)
- `--no-prelude-cache` flag forces re-evaluation

### Gate

```
cargo test --lib    # 1485 pass (no behavior change, cache not used yet)
```

---

## Phase Overview

| Phase | What | Files touched | Lines changed | Arrow killed |
|-------|------|---------------|---------------|--------------|
| **7.5** | Prelude cache + VirtualHeap | 4 | ~300 | 0 (new infrastructure) |
| **8A** | Add `Value::Bits`, build intrinsic engine | 20 | ~200 | 0 (new code — parallel path) |
| **8B** | Route operators through properties | 4 | ~150 | ~50 match arms in `binary_op.rs` |
| **8C** | Migrate scalars one-by-one | 8 | ~600 | ~200 match arms across 6 files |
| **8D** | Migrate String, Data, remove Stack/Queue/StringBuilder | 4 | ~300 | ~80 match arms in `ffi/registry.rs`, `arrow.rs` |
| **8E** | Build `op Drop` pass, remove `storage`/`box`/`unbox` | 6 | ~400 | ~30 match arms in `emit_stmt.rs`, `mod.rs` |
| **8F** | Legacy cleanup, Void declaration, structural variant removal | 15 | ~350 | Remove List/HashMap/Tuple etc. |

---

## Phase 8A — Parallel Value Variant, VirtualHeap & Intrinsic Engine

**Goal:** Add `Value::Bits(Vec<u8>)` alongside all existing variants. Build
the VirtualHeap sandbox and the intrinsic dispatch engine. No behavior
change — all legacy paths remain. The VirtualHeap is seeded by the prelude
cache (Phase 7.5) and handles the sandboxed pointer arithmetic needed for
List, HashMap, and Box operations at compile time.
**Verification:** 1485 tests still pass. `cargo build` clean.

### Step 8A.0 — Add Value::Bits variant

**File:** `src/interpreter.rs` — Value enum, line ~64

Add `Bits(Vec<u8>)` to the enum. All ~20 exhaustive match sites on `Value`
must get a new arm. A template for every site:

```rust
// Every existing match arm stays. New arm:
Value::Bits(_) => { /* no-op fallthrough for now */ }
```

**Location of match sites** (exhaustive, not wildcard):
- `src/interpreter.rs` — `eval_expr`, `call_defn`, `call_custom_fn`, `call_txn`,
  `is_empty_value`, `value_to_string`, `clone`, `eq`, `Display`
- `src/proof_engine.rs` — value comparison, equality checks
- `src/symbolic.rs` — symbolic value construction
- `src/backend/llvm/tests.rs` — test assertions (9 sites)

Each site adds a single line: `Value::Bits(_) => ...`. No deeper nesting.

### Step 8A.1 — Add Intrinsic::name() method

**File:** `src/interpreter.rs` — Intrinsic enum

Currently `Intrinsic` is a Rust enum with ~50 variants. Each variant is
matched in the interpreter to produce behavior. We add a method that maps
each variant to its intrinsic string name:

```rust
impl Intrinsic {
    pub fn name(&self) -> &'static str {
        match self {
            Intrinsic::AddI64 => "__add_i64",
            Intrinsic::FaddF64 => "__fadd_f64",
            Intrinsic::SubI64 => "__sub_i64",
            // ... one arm per variant, flat match
        }
    }
}
```

This is the **naming control plane**. The string name is what appears in
`op Add <~ "__add_i64#"` in type declarations. The old typed dispatch still
uses the enum variants; the new engine uses the strings.

### Step 8A.2 — Build execute_intrinsic()

**File:** `src/interpreter.rs` — new function

```rust
/// Execute a named intrinsic on raw byte-slice arguments.
/// 2026-07-11: Phase 8A — pure Bits intrinsic dispatch.
fn execute_intrinsic(name: &str, args: &[Value]) -> Result<Value, RuntimeError> {
    match name {
        "__add_i64" => {
            let a = bits_to_i64(&args[0])?;
            let b = bits_to_i64(&args[1])?;
            Ok(Value::Bits(i64_to_bits(a.wrapping_add(b))))
        }
        "__sub_i64" => {
            let a = bits_to_i64(&args[0])?;
            let b = bits_to_i64(&args[1])?;
            Ok(Value::Bits(i64_to_bits(a.wrapping_sub(b))))
        }
        "__fadd_f64" => {
            let a = bits_to_f64(&args[0])?;
            let b = bits_to_f64(&args[1])?;
            Ok(Value::Bits(f64_to_bits(a + b)))
        }
        // ... one arm per intrinsic, flat match
        _ => Err(RuntimeError::UnknownIntrinsic(name.to_string()))
    }
}
```

**Helper functions** (also new, same file):

```rust
fn bits_to_i64(bits: &Value) -> Result<i64, RuntimeError> {
    let b = match bits { Value::Bits(b) => b, _ => return Err(...) };
    let mut arr = [0u8; 8];
    arr.copy_from_slice(&b[..b.len().min(8)]);
    Ok(i64::from_le_bytes(arr))
}

fn i64_to_bits(i: i64) -> Value {
    Value::Bits(i.to_le_bytes().to_vec())
}

// Same pattern for f64, u32, bool, etc.
```

### Step 8A.3 — Expected type propagation

**File:** `src/typechecker.rs` — `infer_expression` and `check_program`

The interpreter needs to know the expected type of an expression to look up
`op` bindings. Currently the `Expr` AST nodes carry their own types for
literals but not for compound expressions like `a + b`.

**Change:** After type checking, annotate each `Expr` node with its inferred
type. Add an `expected_type: Option<Type>` field to key AST nodes, or pass
the expected type into `eval_expr` as a parameter.

The simplest approach: thread `expected_type: &Option<Type>` through
`Interpreter::eval_expr()`. The typechecker already infers the type of every
expression — we just need to pass it down.

**Risk:** This changes the signature of `eval_expr`, which is called from
~50 sites in the interpreter and its features. Each call site must be
updated to pass `&None` (temporary fallback — new path not yet active).

**Arrow-elimination note:** This doesn't kill arrows itself; it enables
later phases to kill them.

### Gate: Phase 8A

```
cargo build --lib    # clean
cargo test --lib     # 1485 pass, 0 fail
```

---

## Phase 8B — Property-Based Operator Dispatch

**Goal:** Route `Expr::Add`, `Expr::Sub`, etc. through the property system
when both operands are `Value::Bits`. Legacy typed variants still fall back
to the old match arms.

**Verification:** Add a test that explicitly creates `Value::Bits` operands
with an expected type and confirms the new path fires. All existing tests
still pass through the legacy path.

### Step 8B.0 — Add get_operator_intrinsic()

**File:** `src/type_universe.rs` — new method on TypeUniverse

```rust
/// Look up the intrinsic mapped to an operator for a given type.
/// "op Add <~ "__add_i64#"  →  Some("__add_i64")
/// 2026-07-11: Phase 8B
pub fn get_operator_intrinsic(&self, type_name: &str, op: &str) -> Option<&str> {
    let rt = self.types.get(type_name)?;
    let key = format!("op {}", op);
    let prop = rt.properties.get(&key)?;
    match prop {
        PropertyValue::String(s) => Some(s.trim_end_matches('#')),
        _ => None,
    }
}
```

**Nesting:** 3 levels max, converted to 2 via guard clauses:

```rust
pub fn get_operator_intrinsic(&self, type_name: &str, op: &str) -> Option<&str> {
    let rt = self.types.get(type_name)?;
    let key = format!("op {}", op);
    let prop = rt.properties.get(&key)?;
    match prop {
        PropertyValue::String(s) => Some(s.trim_end_matches('#')),
        _ => None,
    }
}
```

### Step 8B.1 — Refactor BinaryOpExpr::evaluate()

**File:** `src/features/binary_op.rs` — `evaluate()` method, lines 40–124

**Current shape:** 50+ arms of `match (kind, &l, &r)` with combinations like
`(Add, Value::Int(a), Value::Int(b))`, `(Add, Value::Float(a), Value::Float(b))`,
`(Sub, Value::Int(a), Value::Int(b))`, etc. This is the worst arrow-shaped
code in the codebase.

**New shape:**

```rust
fn evaluate(&self, ctx: &mut Interpreter, dispatch: &ExprDispatch) -> Result<Value, RuntimeError> {
    let l = ctx.eval_expr(&self.left, &None)?;
    let r = ctx.eval_expr(&self.right, &None)?;

    // Fast path: both operands are pure Bits → property dispatch
    if let (Value::Bits(_), Value::Bits(_)) = (&l, &r) {
        if let Some(expected_type) = ctx.current_expected_type() {
            let intrinsic = ctx.universe()
                .and_then(|u| u.get_operator_intrinsic(&expected_type, &self.kind.name()))?;
            return execute_intrinsic(intrinsic, &[l, r]);
        }
    }

    // Legacy fallback: typed variants
    Ok(match (self.kind, &l, &r) {
        (Add, Value::Int(a), Value::Int(b)) => Value::Int(a + b),
        (Add, Value::Float(a), Value::Float(b)) => Value::Float(a + b),
        // ... remaining legacy arms stay until migration
        _ => return Err(RuntimeError::TypeMismatch(...))
    })
}
```

**Arrow eliminated:** The entire 50-arm typed match becomes a dead code path
once migration is complete. For now it's the fallback — never removed, just
unreachable with new-style values.

### Step 8B.2 — UnaryOpExpr::evaluate()

**File:** `src/features/unary_op.rs`

Same pattern as 8B.1. Currently ~10 arms (`Neg Int`, `Neg Float`, `Not Bool`,
etc.). Replace with property lookup + intrinsic call when operand is
`Value::Bits`. Legacy fallback kept.

### Gate: Phase 8B

```
cargo test --lib     # all existing tests pass through legacy path
# New test:
cargo test --lib binary_op::tests::test_bits_dispatch  # new test, new path
```

---

## Phase 8C — Migrate Scalars One by One

**Goal:** Move literal construction from typed variants to `Value::Bits` for
each prelude scalar type. After each sub-step, delete the corresponding
`Value` variant.

### Step 8C.0 — Migrate Int

**Files changed:**

| File | Change | Arrow eliminated |
|------|--------|------------------|
| `src/interpreter.rs` | `Expr::Integer(i)` → `Value::Bits(i.to_le_bytes().to_vec())` | Kills `match` at `eval_expr` line 2975 |
| `src/interpreter.rs` | Delete `Value::Int` match arms in `eval_expr`, `is_empty_value`, `value_to_string`, etc. | ~40 arms across 6 match sites |
| `src/features/binary_op.rs` | Delete `(Add, Value::Int(a), Value::Int(b))` arms — now unreachable | ~15 arms |
| `src/features/unary_op.rs` | Delete `Value::Int` arms | ~3 arms |
| `src/features/projection.rs` | Delete `Value::Int` arms in projection eval | ~10 arms |
| `src/ffi/registry.rs` | Delete `match &arg { Value::Int(i) => ... }` | ~15 arms |
| `src/ffi/dynamic.rs` | Delete `Value::Int` conversion | ~3 arms |
| `src/ffi/types.rs` | Delete `Value::Int` type match | ~2 arms |
| `src/features/collection.rs` | Delete `Value::Int` in `decompose_atomic_to_chars` etc. | ~2 arms |

**After this step:** `Value::Int` is deleted from the enum. Any code that
pattern-matches `Value::Int` will fail to compile, revealing every site
that needs updating.

**The delete-and-compile technique:**

1. Remove `Int(i64)` from the `Value` enum
2. Run `cargo build` — the compiler lists every match arm that needs updating
3. Fix each site: either delete the arm (now unreachable) or change to
   `Value::Bits(...)` handling
4. Repeat until `cargo build` passes
5. Run `cargo test --lib` — all 1485+ tests pass

### Step 8C.1 — Migrate Bool

Same pattern as 8C.0. `Expr::Bool(b)` → `Value::Bits(vec![b as u8])`.

**Arrow eliminated:** Binary op arms like `(And, Value::Bool(a), Value::Bool(b))`
and comparison arms like `(Eq, Value::Bool(a), Value::Bool(b))` — these route
through the new intrinsic path: `__and_i1`, `__or_i1`, `__eq_i1`.

### Step 8C.2 — Migrate Char

`Expr::Char(c)` → `Value::Bits((c as u32).to_le_bytes().to_vec())`.

**Arrow eliminated:** Match arms in `binary_op.rs` for `(Eq, Value::Char(a), Value::Char(b))`,
`(Lt, Value::Char(a), Value::Char(b))`, etc. Route through `__eq_i32`,
`__lt_i32`.

### Step 8C.3 — Migrate Float

`Expr::Float64(f)` → `Value::Bits(f.to_le_bytes().to_vec())`.

**Arrow eliminated:** Match arms in `binary_op.rs` for `(Add, Value::Float(a), Value::Float(b))`,
`(Sub, Value::Float(a), Value::Float(b))`, etc. Route through `__fadd_f64`,
`__fsub_f64`, etc.

### Gate: Phase 8C

After 8C.3, `Value::Int`, `Value::Float`, `Value::Bool`, `Value::Char` are
deleted from the enum. Scalar operations route entirely through:
- `Value::Bits` construction from AST literals
- Property lookup (`op Add` → `"__add_i64"`)
- `execute_intrinsic` on raw byte arrays

```
cargo test --lib     # all pass
```

---

## Phase 8D — Migrate String + Data, Remove Stack/Queue/StringBuilder

### Step 8D.0 — String to Bits

`Expr::String(s)` → `Value::Bits(s.into_bytes())`.

The FFI impl functions (`concat_impl`, `contains_impl`, `to_string_impl`,
`trim_impl`, `split_impl`, etc.) currently match on `Value::String(s)`.
Each is updated to match `Value::Bits(b)` and interpret `b` as UTF-8
internally.

**Current shape of every impl function** (arrow anti-pattern):

```rust
fn concat_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if let Value::String(a) = &args[0] {            // level 1
        if let Value::String(b) = &args[1] {        // level 2
            Ok(Value::String(format!("{}{}", a, b)))
        } else {
            Err(TypeMismatch("expected string"))     // level 3
        }
    } else {
        Err(TypeMismatch("expected string"))         // level 2
    }
}
```

**New shape** (flat):

```rust
fn concat_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let bits: Vec<Vec<u8>> = args.into_iter()
        .map(|v| match v {
            Value::Bits(b) => Ok(b),
            _ => Err(TypeMismatch("expected Bits")),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let a = String::from_utf8_lossy(&bits[0]);
    let b = String::from_utf8_lossy(&bits[1]);
    Ok(Value::Bits(format!("{}{}", a, b).into_bytes()))
}
```

**Arrow eliminated:** All ~15 FFI impl functions (`concat_impl`,
`contains_impl`, `to_string_impl`, `to_int_impl`, `trim_impl`, `split_impl`,
`starts_with_impl`, `ends_with_impl`, `find_impl`, `replace_impl`,
`parse_int_impl`, `parse_float_impl`) — each flattens from 3-level `if let`
chains to 1-level `map`/`collect`.

### Step 8D.1 — Data to Bits

`Value::Data(Vec<u8>)` → `Value::Bits(Vec<u8>)`. This is a direct rename —
the inner type is already `Vec<u8>`. All sites that construct or match
`Value::Data` are changed to `Value::Bits`.

### Step 8D.2 — Remove StringBuilder

The interpreter has `Value::StringBuilder(String)`. This is a stdlib type,
not a compiler primitive. `StringBuilder` is a `List<UInt8>` with an
`op InsertAt` contract for efficient appending.

**Change:** Remove `Value::StringBuilder` from the enum. All match sites that
handle it are deleted (they become unreachable). The stdlib definition of
`StringBuilder` becomes a wrapper around `List<UInt8>`:

```brief
type StringBuilder: Bits {
    buf: List<UInt8>;
    op InsertAt(self, pos: Int, val: UInt8) = sb_append;
};
```

### Step 8D.3 — Remove Stack and Queue

Same reasoning as StringBuilder. `Stack` and `Queue` are `List<T>` with
specific `op InsertAt`/`op ExtractFrom` conventions:

```brief
// Stack: LIFO
type Stack<T>: Bits {
    inner: List<T>;
    op InsertAt(self, 0, val: T) = stack_push;     // push at front
    op ExtractFrom(self, 0) -> T = stack_pop;      // pop from front
};

// Queue: FIFO
type Queue<T>: Bits {
    inner: List<T>;
    op InsertAt(self, inner:> Size, val: T) = queue_enqueue;  // push at end
    op ExtractFrom(self, 0) -> T = queue_dequeue;             // pop from front
};
```

**Arrow eliminated in `arrow.rs`:** The current dispatch has a 3-level nested
match on collection type × direction × value:

```rust
match &mut collection {
    Value::List(list) => match dir { Push => ..., Pop => ... },
    Value::Stack(stack) => match dir { Push => ..., Pop => ... },
    Value::Queue(queue) => match dir { Push => ..., Pop => ... },
    // ...
}
```

After removal, this collapses to:

```rust
match &mut collection {
    Value::List(list) => /* InsertAt/ExtractFrom by name */,
    _ => {},
}
```

### Gate: Phase 8D

```
cargo test --lib     # all pass
```

---

## Phase 8E — Op Drop Pass + Storage Cleanup

### Step 8E.0 — Build DropInjector pass

**New file:** `src/lifetime.rs`

A new compiler pass that runs after type checking. It walks the AST and:

1. Identifies all variable bindings (`let x: T = ...`)
2. Checks if `T` (or any slot of `T`) has `op Drop`
3. At the end of each scope where `x` is bound, inserts a destructor call:
   `__builtin_drop(x)` → eventually `T::op_Drop(x)`
4. Skips variables that are moved or returned (ownership tracking — Phase 8E.1)

```rust
/// Inject destructor calls for types implementing op Drop.
/// 2026-07-11: Phase 8E.
pub fn inject_drop_calls(program: &mut Program, universe: &TypeUniverse) {
    for item in &mut program.items {
        match item {
            TopLevel::Transaction(txn) => {
                inject_drop_calls_in_body(&mut txn.body, universe);
            }
            TopLevel::Definition(defn) => {
                inject_drop_calls_in_body(&mut defn.body, universe);
            }
            _ => {}
        }
    }
}
```

**Arrow control:** The body walker is flat — a single `for stmt in body` loop
with guard clauses for each statement type. Max 2 levels.

### Step 8E.1 — Remove storage, box, unbox properties

**Files:** `lib/std/types/bootstrap.bv`, all type definitions in `lib/std/`

**Old:**

```brief
type String: Bits {
    ptr: Ptr<UInt8>;
    len: Int;
    codec: UInt8;
    bytes <~ 24;
    storage <~ "Native";
    box <~ "ptrtoint#";
    unbox <~ "inttoptr#";
};
```

**New:**

```brief
type String: Bits {
    ptr: Ptr<UInt8>;
    len: Int;
    codec: UInt8;
    bytes <~ 24;
    op Drop(self) = __free_string_allocation#;
};
```

Every type definition in the stdlib that uses `storage`, `box`, or `unbox`
must be updated. The semantic equivalent is expressed through composition
and `op Drop`.

**Compiler changes:**
- `src/type_universe.rs` — remove the match arms for `"storage"`, `"box"`,
  `"unbox"` in `apply_binding()`
- `src/backend/llvm/emit_toplevel.rs` — stop looking at `storage` to decide
  whether to box/unbox a value. The LLVM backend relies on the `llvm`
  property and the struct layout, not storage tags.

**Arrow eliminated:** The `apply_binding()` function in `type_universe.rs`
has a ~200-line match on binding name strings. Removing `storage`, `box`,
`unbox` eliminates ~30 lines of branching. The `emit_toplevel.rs` code that
checks `if rt.storage == "Boxed"` is deleted entirely.

### Step 8E.2 — Update backend emission

The LLVM backend's `emit_initializer` for state fields currently checks
`storage` to decide whether to emit a heap allocation or a stack value.
After removal, all state fields are sized by `bytes <~ N` and laid out
sequentially in the `%State` struct. "Boxed" types become `%State` fields
of pointer width (8 bytes) pointing to heap allocations managed by
`op Drop` functions.

### Gate: Phase 8E

```
cargo test --lib     # all pass
cargo build --release && bash benchmarks/build_and_bench.sh --correctness
                     # all benchmarks match
```

---

## Phase 8F — Legacy Cleanup + Void

### Step 8F.0 — Delete all structural variants

Remove from `Value` enum:
- `Value::List(Vec<Value>)` — collections are Bits structs with VirtualHeap
- `Value::Tuple(Vec<Value>)` — tuples are Bits structs with sequential slots
- `Value::HashMap(HashMap<String, Value>)` — maps are Bits structs with VirtualHeap
- `Value::Instance { typename, fields }` — struct instances are Bits at their byte width
- `Value::Enum(String, String, HashMap<String, Value>)` — enums are Bits at discriminant + data width
- `Value::Ptr(u64)` — pointers are `Value::Bits` with `bytes <~ 8` on 64-bit targets

Delete all now-unreachable match arms across the codebase. The `arrow.rs`
dispatch for collection operations (`match &mut collection { Value::List => ...,
Value::HashMap => ..., }`) collapses to a single property lookup:

### Step 8F.1 — Add Void to bootstrap

**File:** `lib/std/types/bootstrap.bv`

```brief
type Void: Bits {
    bytes <~ 0;
    alignment <~ 1;
    llvm <~ "void";
};
```

### Step 8F.2 — Verify LLVM backend independence

The LLVM backend must emit `void` by reading the `llvm` property, not by
matching the string `"Void"`. Check `src/backend/llvm/helpers.rs` and
`emit_toplevel.rs` for any hardcoded `"Void"` or `"void"` string matches.
Replace with property lookup.

**Arrow eliminated:** Match arms like `Type::Custom(n) if n == "Void" => "void"`
become `rt.get_property_str("llvm").unwrap_or("void")`.

### Gate: Phase 8F

```
cargo test --lib     # all pass
cargo build --release && bash benchmarks/build_and_bench.sh --correctness
                     # all benchmarks match
```

---

## Arrow Elimination Summary

| File | Current depth | After | Improvement |
|------|---------------|-------|-------------|
| `src/features/binary_op.rs` | 3 (50-arm match) | 1 (property lookup) | Eliminates 50 match arms |
| `src/features/unary_op.rs` | 3 (10-arm match) | 1 (property lookup) | Eliminates 10 match arms |
| `src/interpreter.rs` eval_expr | 4 (nested if-let) | 2 (guard clauses) | 10+ match sites flattened |
| `src/features/arrow.rs` | 3 (6-variant × direction) | 1 (property lookup) | 5 collection variant branches gone |
| `src/features/projection.rs` | 3 (type × projection) | 2 (property lookup) | Type-specific arms removed |
| `src/ffi/registry.rs` | 3 (impl functions) | 1 (map/collect) | 15 functions flattened |
| `src/type_universe.rs` apply_binding | 3 (200-line match) | 2 (removed storage/box/unbox) | ~30 branches deleted |
| `Value` enum | 24 variants | **11** (Bits + 10 compiler-internal) | All structural/storage variants removed |

---

## File-by-File Change List

| File | Phase | What |
|------|-------|------|
| `src/interpreter.rs` | 8A/8C/8D/8F | Add Bits variant, remove Int/Float/Bool/Char/String/Data/StringBuilder/Stack/Queue/Ptr, add execute_intrinsic() |
| `src/features/binary_op.rs` | 8B/8C | Property dispatch route, delete typed arms |
| `src/features/unary_op.rs` | 8B/8C | Property dispatch route, delete typed arms |
| `src/features/arrow.rs` | 8D | Remove Stack/Queue branches |
| `src/features/projection.rs` | 8C/8D | Delete typed Value::Int/Float/Bool/Char arms |
| `src/features/collection.rs` | 8C/8D | Delete typed Value arms |
| `src/ffi/registry.rs` | 8D | Flat impl functions with Bits |
| `src/ffi/dynamic.rs` | 8C/8D | Delete typed value conversions |
| `src/ffi/types.rs` | 8C/8D | Delete typed type matches |
| `src/ffi/native_mapper.rs` | 8C | Delete Int/Float conversion |
| `src/ffi/sentinel.rs` | 8C | Delete Int conversion |
| `src/ffi/orchestrator.rs` | 8C | Delete Int/Data conversion |
| `src/type_universe.rs` | 8B/8E | Add get_operator_intrinsic(), remove storage/box/unbox handling |
| `src/typechecker.rs` | 8A | Expected type propagation |
| `src/lifetime.rs` | 8E (new) | DropInjector pass |
| `src/backend/llvm/emit_toplevel.rs` | 8E | Remove storage/box checks |
| `src/backend/llvm/helpers.rs` | 8F | Remove Void string match |
| `src/backend/llvm/tests.rs` | 8A/8C | Update test assertions |
| `src/proof_engine.rs` | 8A | Bits match arm |
| `src/symbolic.rs` | 8A | Bits match arm |
| `lib/std/types/bootstrap.bv` | 8E/8F | Remove storage/box/unbox, add Void, String gets op Drop |

---

## Rollback Plan

If a phase breaks the test suite:

1. **Revert the last commit** — every sub-step is a single commit
2. **Git bisect** if the breakage is unclear — each commit is independently testable
3. **Restore deleted Value variant** — if Phase 8C.0 reveals a missed match
   site, add `Value::Int` back temporarily, fix the site, re-delete in the
   next commit

The Strangler Fig pattern means the legacy path never disappears until the
new path is verified. If Phase 8C.0 successfully deletes `Value::Int` and
all tests pass, the migration is proven correct. If tests fail, the old code
is still in the history, undeletable.
