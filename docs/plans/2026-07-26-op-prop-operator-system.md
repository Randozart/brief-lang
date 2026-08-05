# Phase 2.7 — `op`/`prop` Operator Binding System

**Date:** 2026-07-26

## 1. Overview

Replace the old `op Add(#Int) -> #Int = fn(#L, #R)` declaration syntax with a
clean `op Add(#Int): fn(#L, #R)` and `prop Size: fn(#L)` system. `op` and `prop`
become dedicated keywords. The protocol variant argument is optional and accepts
either a `#Protocol` hashword or a concrete type name.

### Syntax Summary

```briv
// Protocol-level declarations (type bodies)
type Posit32: #Float {
    op Add(#Float): posit_add(#L, #R);     // RHS follows #Float
    op Add(#Int): posit_add_int(#L, #R);   // RHS follows #Int
    op Sub(#Float): posit_sub(#L, #R);
    op Eq(#Float): equals(#L, #R);
    prop Size: posit_bits(#L);             // .#Size metaproperty
};

// Instance-level declarations (obj bodies)
obj RingBuffer<T> {
    data: T[256];
    read: Int;
    write: Int;

    op InsertAt: push(#L, #R);            // queue <- val
    op ExtractFrom: pop(#R);              // x <- &queue
    op CopyFrom: read(#R);                // x <- queue
    op Init: init(#L, #R);                // let q: RB = val
};
```

## 2. Resolution Rules

| Code path | Lookup | #L | #R |
|-----------|--------|----|----|
| `a + b` | `op Add` on `a`'s type | `a` | `b` |
| `a - b` | `op Sub` on `a`'s type | `a` | `b` |
| `a * b` | `op Mul` on `a`'s type | `a` | `b` |
| `a / b` | `op Div` on `a`'s type | `a` | `b` |
| `a == b` | `op Eq` on `a`'s type | `a` | `b` |
| `a != b` | `op Neq` on `a`'s type | `a` | `b` |
| `a < b` | `op Lt` on `a`'s type | `a` | `b` |
| `a > b` | `op Gt` on `a`'s type | `a` | `b` |
| `a <= b` | `op Le` on `a`'s type | `a` | `b` |
| `a >= b` | `op Ge` on `a`'s type | `a` | `b` |
| `col <- val` | `op InsertAt` on `col`'s type | `col` | `val` |
| `x <- &col` | `op ExtractFrom` on `col`'s type | `x` | `col` |
| `x <- col` | `op CopyFrom` on `col`'s type | `x` | `col` |
| `let x: T = val` | `op Init` on `T` | `x` (slot) | `val` |
| `expr .#Size` | `prop Size` on `expr`'s type | `expr` | — |
| `expr .#Bytes` | `prop Bytes` on `expr`'s type | `expr` | — |

### Protocol variant matching

When `a + b` with an `op Add(#Proto): fn(#L, #R)` binding:
1. If `b`'s type implements `#Proto` → use this binding
2. If no protocol variant matches → compiler error
3. If no `op Add` at all → native integer/float codegen (for bootstrap primitives)

### Concrete type matching

When `a + b` with `op Add(ConcreteType): fn(#L, #R)`:
1. If `b`'s type is exactly `ConcreteType` → use this binding
2. Otherwise continue to next variant

## 3. Coding Standards

Per AGENTS.md Plan Directives (items 1-5):
- **FLAT CONTROL FLOW**: Max 2 nesting levels, guard clauses, early returns
- **COMMENT THE CODE**: `// YYYY-MM-DD: <why>` on every modified site
- **UPDATE ALL EXAMPLES**: Every `.bv` file using old `op ... = fn(...)` syntax
- **DOCUMENTATION IS CODE**: Update architecture docs in same commit
- **BEHAVIORAL TESTS**: Test operator resolution outcomes, not IR snapshots

Per AGENTS.md item 3 (Continuous Git Commits):
- Commit after each logical step
- `git add` only intended files
- Never amend commits

## 4. Implementation Steps

### Step 0: Luxer — Add `Token::Prop`

**File:** `src/lexer.rs`

Add `#[token("prop")]` entry for the `prop` keyword. `op` is already handled
as an identifier (no dedicated token needed — the slot parser checks
`slot_name == "op"`). But `prop` should be lexed as a token so it's reserved
and can be used in statement/expression contexts without ambiguity.

### Step 1: AST — Remove old `OperatorDef` form, add `OpBinding`/`PropBinding`

**File:** `src/ast/top.rs`

- Add `OpBinding` struct: `{ name: String, protocol_variant: Option<String>, expr: Expr, span: Option<Span> }`
- Add `PropBinding` struct: `{ name: String, expr: Expr, span: Option<Span> }`
- Add `op_bindings: Vec<OpBinding>` and `prop_bindings: Vec<PropBinding>` to `TypeDefBody`
- Add `op_bindings: Vec<OpBinding>` and `prop_bindings: Vec<PropBinding>` to `ObjDef` (or equivalent)
- Remove the old `operators: Vec<OperatorDef>` field from `TypeDefBody`
- Remove `OperatorDef` struct entirely

### Step 2: Parser — Replace `parse_op_binding`, add `parse_prop_definition`

**File:** `src/parser/definitions.rs`

- Replace `parse_op_binding` with `parse_op_definition(name: &str)` that handles:
  ```
  op Name: expr;
  op Name(#Protocol): expr;
  op Name(ConcreteType): expr;
  ```
  The method name is the callee in the expression (e.g. `push` in `push(#L, #R)`).
  No type params, no `->` return type — the bound function's own return type is used.

- Add `parse_prop_definition(name: &str)` that handles:
  ```
  prop Name: expr;
  ```

- Change slot handler from `slot_name == "op"` to `slot_name == "op"` (keyword or identifier, same distinction).
- Add slot handler for `slot_name == "prop"` (after adding Token::Prop to lexer).

**Also affected:**
- `src/parser/helpers.rs` — `keyword_as_identifier` may need `prop` added
- `src/ast/display.rs` — display new `op`/`prop` forms
- `src/beast/serialize.rs` and `deserialize.rs` — add new variants
- `src/fuzzing/ast_generator.rs` — generate new `op`/`prop` forms

### Step 3: Type Checker — Operator resolution

**File:** `src/typechecker/mod.rs`

- For `Expr::AssignArrow` (the `<-` operator):
  - If `has_ampersand` on RHS → look up `op ExtractFrom`
  - If no ampersand on RHS → look up `op CopyFrom`
  - Both: `#L` = destination, `#R` = source collection

- For `Expr::InsertArrow` (the `&queue <- val` pattern — now just `queue <- val`):
  - Look up `op InsertAt` on collection type
  - `#L` = collection, `#R` = value

- For `Expr::BinaryOp`:
  - Look up `op Add` / `op Sub` / `op Eq` etc. on LHS type
  - Match protocol variant on RHS type
  - If no binding found, fall back to bootstrap hardcoded paths

- For `Expr::HashProjection` (`expr .#Size`):
  - Look up `prop Size` on expr's type

- For `let x: T = val`:
  - If `T` has `op Init`, verify init method exists and resolves `#L`/`#R`

### Step 4: Codegen — Emit bound methods

**File:** `src/backend/llvm/emit_expr.rs`

- In `emit_binary_op` (after the new Vector/SIMD check, before the float/int check):
  - If the LHS type has an `op Add`/`op Sub`/etc. binding with matching variant,
    emit a call to the bound method with `#L`/`#R` substitution

- In the `<-` operator emission:
  - Look up `op InsertAt`/`op ExtractFrom`/`op CopyFrom` on the collection type
  - Emit the bound method call with appropriate `#L`/`#R` substitution

**File:** `src/backend/llvm/emit_toplevel.rs`

- In the init phase (`emit_init_state` or equivalent):
  - When processing `let x: T = val`, check for `op Init` on `T`
  - If present, emit a call to the init method: `init_method(x, val)`
  - `#L` = `x` (the freshly allocated slot), `#R` = `val` (the init value)

### Step 5: `.bv` file migration — ALL old `op` declarations

Every file using the old `op Name(Type) -> Type = fn(#L, #R)` syntax must be
updated to the new `op Name(Type?): fn(#L, #R)` form. This includes:

| Path | Count | Notes |
|------|-------|-------|
| `lib/std/types/bootstrap.bv` | ~20 | Primitive type declarations |
| `lib/std/protocols.bv` | ~10 | Protocol definitions |
| `lib/std/types/*.bv` | ~15 | Various type files |
| `lib/std/ffi/*.bv` | ~5 | FFI wrapper types |
| `lib/tamer/*.bv` | ~8 | Tamer VM type declarations |
| `benchmarks/*.bv` | ~5 | Benchmark type declarations |

The old `op CastTo(#Type) = fn(#L)` becomes `op CastTo(#Type): fn(#L)`.
The old `op Add(#Int) -> #Int = fn(#L, #R)` becomes `op Add(#Int): fn(#L, #R)`.
The old `op InsertAt <~ push(#L, #R)` becomes `op InsertAt: push(#L, #R)`.

NOTE: `SlotName <~ expr` (the old property binding syntax via `<~`) is also
removed. All such bindings use `op Name(...): expr` instead.

### Step 6: Create `lib/std/collections.bv`

New file with `#Heap` and `#Stack` container types:

```briv
// ── Collections ──────────────────────────────────────────────────
// 2026-07-26: Standard collection types. No compiler magic.
// Memory annotations: #Heap for dynamic, #Stack for static.

// ── Stack<T, N> — fixed-size stack ──────────────────────────────

obj Stack<T, N> {
    data: T[N];
    len: Int;

    op InsertAt: push(#L, #R);
    op ExtractFrom: pop(#R);

    txn push(val: T) [len < N][len <= N] {
        data[len] = val;
        len = len + 1;
    };

    txn pop() -> T [len > 0][len >= 0] {
        len = len - 1;
        term data[len];
    };
};

// ── RingBuffer<T> — fixed-size circular buffer ───────────────────

obj RingBuffer<T> {
    data: T[256];
    read: Int;
    write: Int;

    op InsertAt: push(#L, #R);
    op ExtractFrom: pop(#R);
    op CopyFrom: read(#R);
    op Init: init(#L, #R);

    txn push(val: T) [write - read < 256][write - read <= 256] {
        data[write % 256] = val;
        write = write + 1;
    };

    txn pop() -> T [write > read][write >= read] {
        let i = read % 256;
        read = read + 1;
        term data[i];
    };

    defn read() -> T [write > read] {
        term data[read % 256];
    };

    defn size() -> Int {
        term write - read;
    };
};

// ── List<T> — dynamic list (heap-allocated) ─────────────────────

struct ListBuffer<T> {
    data: Ptr<T>;
    cap: Int;
};

obj List<T> {
    inner: ListBuffer<T>;
    len: Int;

    op InsertAt: push(#L, #R);
    op ExtractFrom: pop(#R);
    op CopyFrom: get(#R);
    op Init: init(#L, #R);

    txn push(val: T) [len < inner.cap][len <= inner.cap] {
        inner.data[len] = val;
        len = len + 1;
    };

    txn pop() -> T [len > 0][len >= 0] {
        len = len - 1;
        term inner.data[len];
    };

    defn get(i: Int) -> T [i >= 0 && i < len] {
        term inner.data[i];
    };
};

// ── HashMap<K, V> — hash table (heap-allocated) ────────────────

struct HashMapEntry<K, V> { key: K; val: V; };

obj HashMap<K, V> {
    buckets: Ptr<List<HashMapEntry<K, V>>>;
    count: Int;
    cap: Int;

    op InsertAt: insert(#L, #R);
    op ExtractFrom: remove(#R);
    op CopyFrom: get(#R);
    op Init: init(#L, #R);

    txn insert(key: K, val: V) [count < cap][count <= cap] {
        let h = (key as Int) % cap;
        buckets[h].push(HashMapEntry { key, val });
        count = count + 1;
    };

    txn remove(key: K) -> Option<V> [count > 0][count >= 0] {
        // scan bucket, remove entry
    };

    defn get(key: K) -> Option<V> {
        let h = (key as Int) % cap;
        // scan buckets[h] for matching key
    };
};
```

### Step 7: Update `benchmarks/queue_drain.bv`

Replace the old `import "std/core/ring_buffer.bv"` + `[0]` initialization with:

```briv
import { RingBuffer } from "std/collections.bv";

let N: Int = GetEnvInt!("BOUND");
let queue: RingBuffer<Int> = 0;      // uses op Init: init(#L, #R)
let count: Int = 0;

node work [count < N][count == N] {
    queue <- count;                   // uses op InsertAt: push(#L, #R)
    let result <- &queue;             // uses op ExtractFrom: pop(#R)
    count = count + 1;

    when count % 5000000 == 0 {
        PrintLn!(count);
    };

    term;
};
```

### Step 8: Documentation

| Document | Update |
|----------|--------|
| `AGENTS.md` | Add `op`/`prop` syntax items (item ~35). Remove old `op` syntax references. Update item 26 (`type`/`struct`/`obj` table) with new op/prop rules. |
| `spec/SPEC.md` | Update grammar for `op_decl` and add `prop_decl`. Update type body parsing grammar. |
| `learn-briv/15-custom-types.md` | Add `op` and `prop` declaration examples. |
| `docs/architecture/overview.md` | Note Phase 2.7 changes. |
| `docs/architecture/backend-type-dispatch.md` | Update if type dispatch logic changes. |

### Step 9: Tests

| Test | What it covers |
|------|---------------|
| Lexer: `Token::Prop` | `prop` is lexed as `Prop` token, not `Identifier` |
| Parser: `op Add(#Int): fn(#L, #R)` | New syntax parses correctly with protocol variant |
| Parser: `op InsertAt: push(#L, #R)` | New syntax parses correctly without variant |
| Parser: `prop Size: fn(#L)` | New `prop` keyword parses correctly |
| Parser: old syntax rejected | `op Add(#Int) -> #Int = fn(...)` produces error |
| Typechecker: `<-` via `op InsertAt` | `queue <- val` resolves to `push(queue, val)` |
| Typechecker: `<-` via `op ExtractFrom` | `x <- &queue` resolves to `pop(queue)` |
| Typechecker: binary op via `op Add` | `a + b` with custom `op Add` binding |
| Typechecker: `.#Size` via `prop Size` | `expr .#Size` resolves to the prop function |
| Typechecker: `op Init` on `let` | `let q: RB = val` emits init call |
| Codegen: bound method call substitution | `#L` and `#R` correctly substituted |
| Full benchmark: queue_drain | Compiles, links, runs, produces MATCH output |
| Full benchmark suite | All benchmarks produce MATCH |
| All 1000+ lib tests | Pass |

## 5. Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Old `op` syntax still used in some files | Medium | High | Grep for `op .*=.*fn(#L` after migration; fix any stragglers |
| `prop` keyword conflicts with existing identifier usage | Low | Medium | Add to `keyword_as_identifier` in parser helpers |
| `op Init` during init phase breaks existing let bindings | Medium | Medium | Only trigger when `op Init` is explicitly declared on the type |
| `op Add` overrides native int/float codegen unexpectedly | Low | Medium | Protocol matching: bootstrap types skip op lookup for Add/Sub etc. unless explicitly overridden |
| `<-` operator desugaring breaks without `&` on LHS | Low | High | The parser already handles `queue <- val` without `&` on LHS — the old `&queue <- val` was the deprecated form |

## 6. Files Changed (Estimated)

~80 files across `.rs`, `.bv`, and `.md`.

| Category | Count | Notes |
|----------|-------|-------|
| Rust source | ~15 | Lexer, AST, parser, typechecker, codegen, serialization, fuzzing |
| `.bv` stdlib | ~30 | All `op` declarations migrated, new collections.bv |
| `.bv` tamer | ~8 | Tamer type declarations |
| `.bv` benchmarks | ~5 | Benchmark type + queue_drain |
| Documentation | ~5 | AGENTS.md, SPEC.md, learn-briv, arch docs |
| Tests | ~10 | New parser/typechecker/codegen tests |

## 7. Architecture Impact

### `op`/`prop` replaces three old mechanisms

| Old mechanism | Replaced by | Status |
|---------------|-------------|--------|
| `op Name(Type) -> Type = fn(#L, #R)` | `op Name(Type?): fn(#L, #R)` | REMOVED |
| `SlotName <~ expr` (property binding) | `op Name: expr` | REMOVED |
| `InsertAt <~ push(#L, #R)` | `op InsertAt: push(#L, #R)` | REMOVED |
| Hardcoded `<-` dispatch in codegen | Generic op binding lookup | REPLACED |
| Hardcoded binary op dispatch (Add/Sub) | Generic op binding lookup (with bootstrap fallback) | EXTENDED |
| `:#Size`/`:#Bytes` (DotHash metaproperty) | `prop Size`/`prop Bytes` | CONFIRMED |

### Bootstrap types

The bootstrap types (`Int`, `Float`, `Bool`, `Ptr`, `Void`) declared in
`lib/std/types/bootstrap.bv` use the new `op` syntax to declare their
protocol memberships. The LLVM backend's hardcoded codegen for these
types remains — `op Add(#Int)` on `Int` translates to `add nsw i64`,
not a function call. The `op` declaration is the **interface contract**
that user types can inherit; the backend recognizes `#Int` and `#Float`
protocol membership and uses native codegen instead of method dispatch.
