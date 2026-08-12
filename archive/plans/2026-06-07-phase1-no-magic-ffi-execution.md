# Phase 1 Execution Plan — No-Magic FFI Dispatch

**Date**: 2026-06-07
**Status**: Ready to execute
**Preceding work**: Phases 11–13 (sync domains, HashMap/HashSet/Stack/Queue/Tuple primitives, BracketOp refactor, MultiSlice mask/stride eval)

## Core Idea

The stdlib currently imports `__builtin_*` frgn functions from `lib/std/__builtin/*.bv`, which resolve through `ffi_name_to_location` → `foreign_functions` → Rust closures in `src/ffi/registry.rs`. This is "magic" — hardcoded Rust string match arms behind a `frgn` facade.

New architecture: **stdlib implemented entirely in Briev** using arrow syntax (`<-`), projections (`:>`), and `uni` pattern matching — zero Rust-side string matching for collection operations.

## What Fits Where

| Type | `<-` arrow syntax | `:>` projections | `uni` matching | Literal |
|------|-------------------|------------------|----------------|---------|
| List | `&list <- v`, `v <- &list` | Size | — | `[1,2,3]` |
| Stack | `&stack <- v`, `v <- &stack`, `<- &stack` | Size, Top | — | `[] :> AsStack` |
| Queue | `&queue <- v`, `v <- &queue`, `<- &queue` | Size, Front | — | `[] :> AsQueue` |
| HashMap | `&map <- (k,v)`, `v <- &map[k]`, `<- &map[k]` | Size, Keys, Values, Contains(k), Get(k) | — | `{"a":1}` |
| HashSet | `&set <- v`, `v <- &set`, `<- &set`, `<- &set[item]` | Size, Contains(item), Elements | — | `{1,2,3}` |
| Tuple | — | Index(usize) | — | `(1,"a")` |
| Option | — | — | `uni opt(Some(v))` | constructor |
| Result | — | — | `uni r(Ok(v))` | constructor |

Enum constructors (`Some`, `None`, `Ok`, `Err`) dispatch natively via variant metadata registered at program load time — no FFI entry needed.

## Step 1 — AST: New projection targets + EnumVariantInfo

**File**: `src/ast.rs`

### `ProjectionTarget` additions (line 330):
```
Get(Box<Expr>)    // HashMap read: map :> Get(key) → Option<V>
Top               // Stack peek: stack :> Top → Option<V>
Front             // Queue front: queue :> Front → Option<V>
Elements          // HashSet enumeration: set :> Elements → List<Value>
AsStack           // List → Stack: list :> AsStack
AsQueue           // List → Queue: list :> AsQueue
```

Also add `EnumVariantInfo` struct somewhere in the interpreter or near the interpreter's import section:
```rust
struct EnumVariantInfo {
    enum_name: String,
    variant_name: String,
    field_names: Vec<String>,
}
```

## Step 2 — Parser: Projection keyword recognition

**File**: `src/parser.rs`

In `parse_projection_target`, add match arms for:
- `"Get"` → parse parenthesized expr → `ProjectionTarget::Get(expr)`
- `"Top"` → `ProjectionTarget::Top`
- `"Front"` → `ProjectionTarget::Front`
- `"Elements"` → `ProjectionTarget::Elements`
- `"AsStack"` → `ProjectionTarget::AsStack`
- `"AsQueue"` → `ProjectionTarget::AsQueue`

No new literal syntax — `[] :> AsStack` uses existing `ListLiteral` + `Projection`.

## Step 3 — Interpreter: New projection handlers

**File**: `src/interpreter.rs`

| Target | On Value | Returns |
|--------|----------|---------|
| `Get(key)` | `Value::HashMap` | `Enum("Option","Some",{value:v})` if key exists, else `Enum("Option","None",{})` |
| `Top` | `Value::Stack` | `Enum("Option","Some",{value:stack.last()})` if non-empty, else `None` |
| `Front` | `Value::Queue` | `Enum("Option","Some",{value:queue.front()})` if non-empty, else `None` |
| `Elements` | `Value::HashSet` | `Value::List` of `Value::String` (cloned from internal strings) |
| `AsStack` | `Value::List` | `Value::Stack(Vec::from(items))` |
| `AsQueue` | `Value::List` | `Value::Queue(VecDeque::from(items))` |

All non-mutating — no state changes.

## Step 4 — Interpreter: HashSet indexed arrow

**File**: `src/interpreter.rs`

In both `ArrowDiscard` (line 1597) and `ArrowMut::Pop` (line 1516), add a match arm for `Value::HashSet` when `index` is not `Term`:

```
(Value::HashSet(set), _) if index != Term => {
    let key_val = self.eval_expr(index)?;
    let elem = self.value_to_string(&key_val)?;
    // remove the element, error if not found
    if set.remove(&elem) {
        self.store_arrow_value(...);
        Ok(removed_value)  // or Void for ArrowDiscard
    } else {
        Err(...)
    }
}
```

## Step 5 — Interpreter: Enum variant dispatch in Expr::Call

**File**: `src/interpreter.rs`

### 5a — Registration during `load_program()`

For each `TopLevel::EnumDefn`, iterate variants and build `EnumVariantInfo`:
```
for variant in enum_defn.variants:
    self.enum_variants.insert(variant.name, {
        enum_name: enum_defn.name,
        variant_name: variant.name,
        field_names: variant.fields.map(|f| f.name)
    })
```

### 5b — New dispatch step in `Expr::Call`

Insert after step 4 (defn alias from state), before step 5 (FFI registry):
```
4a. if let Some(variant_info) = self.enum_variants.get(&fn_name) {
        let mut fields = HashMap::new();
        for (i, arg) in args.iter().enumerate() {
            let val = self.eval_expr(arg)?;
            if let Some(field_name) = variant_info.field_names.get(i) {
                fields.insert(field_name.clone(), val);
            }
        }
        return Ok(Value::Enum {
            type_name: variant_info.enum_name.clone(),
            variant: variant_info.variant_name.clone(),
            fields
        });
    }
```

After this, `Ok(value)` in Briev source never hits any FFI registry — it's caught here.

## Step 6 — Rewrite stdlib `.bv` files

### `lib/std/hashmap.bv`

```briev
defn new_map<K,V>() -> HashMap<K,V> [true][term :> Size == 0] {
    term {};
};
defn insert<K,V>(map: HashMap<K,V>, key: K, value: V)
    [true][term :> Contains(key) && term :> Get(key) == Some(value)] -> HashMap<K,V>
{
    term &map <- (key, value);
};
defn get<K,V>(map: HashMap<K,V>, key: K)
    [true][term.is_some() == map :> Contains(key)] -> Option<V>
{
    term map :> Get(key);
};
defn contains_key<K,V>(map: HashMap<K,V>, key: K) -> Bool {
    term map :> Contains(key);
};
defn remove<K,V>(map: HashMap<K,V>, key: K)
    [map :> Contains(key)][!(term :> 1) :> Contains(key)] -> (Option<V>, HashMap<K,V>)
{
    let v = map :> Get(key);
    <- &map[key];
    term (v, map);
};
defn len<K,V>(map: HashMap<K,V>) -> Int {
    term map :> Size;
};
defn is_empty<K,V>(map: HashMap<K,V>) -> Bool {
    term (map :> Size) == 0;
};
defn keys<K,V>(map: HashMap<K,V>) -> List<K> {
    term map :> Keys;
};
defn values<K,V>(map: HashMap<K,V>) -> List<V> {
    term map :> Values;
};
defn clear<K,V>(map: HashMap<K,V>) -> HashMap<K,V> {
    term {};
};
txn iter_loop<K,V>(ks: List<K>, vs: List<V>, result: List<(K,V)>, i: Int)
    [i < ks :> Size][i == ks :> Size] -> List<(K,V)>
{
    &result = result + [(ks[i], vs[i])];
    &i = i + 1;
    term result;
};
defn iter<K,V>(map: HashMap<K,V>) -> List<(K,V)> {
    term iter_loop(map.keys(), map.values(), [], 0);
};
txn merge_loop<K,V>(bk: List<K>, bv: List<V>, result: HashMap<K,V>, i: Int)
    [i < bk :> Size][i == bk :> Size] -> HashMap<K,V>
{
    &result <- (bk[i], bv[i]);
    &i = i + 1;
    term result;
};
defn merge<K,V>(a: HashMap<K,V>, b: HashMap<K,V>) -> HashMap<K,V> {
    term merge_loop(b.keys(), b.values(), a, 0);
};
txn filter_loop<K,V>(mk: List<K>, mv: List<V>, pred: (K, V) -> Bool, result: HashMap<K,V>, i: Int)
    [i < mk :> Size][i == mk :> Size] -> HashMap<K,V>
{
    [pred(mk[i], mv[i])] {
        &result <- (mk[i], mv[i]);
    };
    &i = i + 1;
    term result;
};
defn filter<K,V>(map: HashMap<K,V>, pred: (K, V) -> Bool) -> HashMap<K,V> {
    term filter_loop(map.keys(), map.values(), pred, new_map(), 0);
};
defn entry<K,V>(map: HashMap<K,V>, key: K) -> Option<V> {
    term map :> Get(key);
};
```

Delete the `import __builtin_HashMap_*` line.

### `lib/std/hashset.bv`

```briev
defn new_set<T>() -> HashSet<T> [true][term :> Size == 0] {
    term {};
};
defn insert<T>(set: HashSet<T>, item: T) -> HashSet<T> {
    term &set <- item;
};
defn remove<T>(set: HashSet<T>, item: T)
    [set :> Contains(item)][!term :> Contains(item)] -> HashSet<T>
{
    <- &set[item];
    term set;
};
defn contains<T>(set: HashSet<T>, item: T) -> Bool {
    term set :> Contains(item);
};
defn len<T>(set: HashSet<T>) -> Int {
    term set :> Size;
};
defn is_empty<T>(set: HashSet<T>) -> Bool {
    term (set :> Size) == 0;
};
defn to_list<T>(set: HashSet<T>) -> List<T> {
    term set :> Elements;
};
defn iter<T>(set: HashSet<T>) -> List<T> {
    term set :> Elements;
};
defn clear<T>(set: HashSet<T>) -> HashSet<T> {
    term {};
};
txn union_loop<T>(other_list: List<T>, result: HashSet<T>, i: Int)
    [i < other_list :> Size][i == other_list :> Size] -> HashSet<T>
{
    &result <- other_list[i];
    &i = i + 1;
    term result;
};
defn union<T>(a: HashSet<T>, b: HashSet<T>) -> HashSet<T> {
    term union_loop(b.to_list(), a, 0);
};
txn intersection_loop<T>(a_list: List<T>, b: HashSet<T>, result: HashSet<T>, i: Int)
    [i < a_list :> Size][i == a_list :> Size] -> HashSet<T>
{
    [b :> Contains(a_list[i])] {
        &result <- a_list[i];
    };
    &i = i + 1;
    term result;
};
defn intersection<T>(a: HashSet<T>, b: HashSet<T>) -> HashSet<T> {
    term intersection_loop(a.to_list(), b, new_set(), 0);
};
txn difference_loop<T>(a_list: List<T>, b: HashSet<T>, result: HashSet<T>, i: Int)
    [i < a_list :> Size][i == a_list :> Size] -> HashSet<T>
{
    [!b :> Contains(a_list[i])] {
        &result <- a_list[i];
    };
    &i = i + 1;
    term result;
};
defn difference<T>(a: HashSet<T>, b: HashSet<T>) -> HashSet<T> {
    term difference_loop(a.to_list(), b, new_set(), 0);
};
txn from_list_loop<T>(items: List<T>, result: HashSet<T>, i: Int)
    [i < items :> Size][i == items :> Size] -> HashSet<T>
{
    &result <- items[i];
    &i = i + 1;
    term result;
};
defn from_list<T>(items: List<T>) -> HashSet<T> {
    term from_list_loop(items, new_set(), 0);
};
```

Delete `import __builtin_HashSet_*`.

### `lib/std/stack.bv`

```briev
defn new_stack<T>() -> Stack<T> [true][term :> Size == 0] {
    term [] :> AsStack;
};
defn push<T>(stack: Stack<T>, item: T) -> Stack<T> {
    term &stack <- item;
};
defn pop<T>(stack: Stack<T>)
    [stack :> Size > 0][term :> 0 :> Size == stack :> Size - 1] -> (T, Stack<T>)
{
    let v: T = <- &stack;
    term (v, stack);
};
defn peek<T>(stack: Stack<T>) -> Option<T> {
    term stack :> Top;
};
defn len<T>(stack: Stack<T>) -> Int {
    term stack :> Size;
};
defn is_empty<T>(stack: Stack<T>) -> Bool {
    term (stack :> Size) == 0;
};
defn to_list<T>(stack: Stack<T>) -> List<T> {
    let result: List<T> = [];
    let s: Stack<T> = stack;
    [!s.is_empty()] {
        uni s :> Top(Some(v)) = {
            let v: T = <- &s;
            &result = [v] + result;
        };
    };
    term result;
};
txn from_list_loop<T>(items: List<T>, result: Stack<T>, i: Int)
    [i < items :> Size][i == items :> Size] -> Stack<T>
{
    &result <- items[i];
    &i = i + 1;
    term result;
};
defn from_list<T>(items: List<T>) -> Stack<T> {
    term from_list_loop(items, new_stack(), 0);
};
```

### `lib/std/queue.bv`

```briev
defn new_queue<T>() -> Queue<T> [true][term :> Size == 0] {
    term [] :> AsQueue;
};
defn enqueue<T>(queue: Queue<T>, item: T) -> Queue<T> {
    term &queue <- item;
};
defn dequeue<T>(queue: Queue<T>)
    [queue :> Size > 0][term :> 0 :> Size == queue :> Size - 1] -> (T, Queue<T>)
{
    let v: T = <- &queue;
    term (v, queue);
};
defn front<T>(queue: Queue<T>) -> Option<T> {
    term queue :> Front;
};
defn head<T>(queue: Queue<T>) -> T {
    uni queue :> Front(Some(v)) = { term v; };
};
defn len<T>(queue: Queue<T>) -> Int {
    term queue :> Size;
};
defn is_empty<T>(queue: Queue<T>) -> Bool {
    term (queue :> Size) == 0;
};
defn to_list<T>(queue: Queue<T>) -> List<T> {
    let result: List<T> = [];
    let q: Queue<T> = queue;
    [!q.is_empty()] {
        let v: T = <- &q;
        &result = result + [v];
    };
    term result;
};
txn from_list_loop<T>(items: List<T>, result: Queue<T>, i: Int)
    [i < items :> Size][i == items :> Size] -> Queue<T>
{
    &result <- items[i];
    &i = i + 1;
    term result;
};
defn from_list<T>(items: List<T>) -> Queue<T> {
    term from_list_loop(items, new_queue(), 0);
};
```

### `lib/std/result.bv`

Delete the `import __builtin_Result_*` line. Rewrite inspection using `uni`:
```briev
defn Ok<T,E>(value: T) [true][term.is_ok()] -> Result<T,E> { term __builtin_Result_Ok(value); };
defn Err<T,E>(error: E) [true][term.is_err()] -> Result<T,E> { term __builtin_Result_Err(error); };
```

Keep `Ok`/`Err` constructors as FFI entries for now (they'll dispatch through the enum variant path once Step 5 is wired). The functional combinators (`result_map`, `and_then`, etc.) already use `uni` matching and don't need FFI.

Note: After Step 5, `Ok(value)` in Briev source will dispatch natively via enum variant lookup, never hitting FFI. The `__builtin_Result_Ok` frgn can be deleted once that's verified.

### `lib/std/option.bv`

Delete `import { __builtin_Option_Some, __builtin_Option_None } from "std/__builtin/option.bv";`. Everything else already uses `uni` pattern matching.

### `lib/std/collections.bv`

No changes needed — already uses arrow syntax.

## Step 7 — Clean up FFI registry

**File**: `src/ffi/registry.rs`

In `resolve_location_to_impl` / the function that builds `FOREIGN_REGISTRY`:
- Remove all `"__builtin.HashMap.*"` arms
- Remove all `"__builtin.HashSet.*"` arms
- Remove all `"__builtin.Stack.*"` arms
- Remove all `"__builtin.Queue.*"` arms
- Remove all `"__builtin.StringBuilder.*"` arms
- Remove `"__builtin.Result.*"` arms (except keep temporarily until enum dispatch is verified)
- Remove `"__builtin.Option.*"` arms (same)
- Keep only `"__builtin.clone"` (used internally)

## Step 8 — Delete unused `__builtin` .bv files

Delete from `lib/std/__builtin/`:
- `hashmap.bv`, `hashset.bv`, `stack.bv`, `queue.bv`, `string_builder.bv`, `string.bv`
- Keep `result.bv` and `option.bv` temporarily until enum dispatch is verified

## Step 9 — Backend & analysis stubs

Add `_ => {}` or stub match arms for all 6 new `ProjectionTarget` variants in:
- `src/backend/llvm.rs`
- `src/backend/rust.rs`
- `src/backend/webstack.rs`
- `src/annotator.rs`
- `src/analysis/dataflow.rs`
- `src/analysis/transition_graph.rs`
- `src/analysis/region.rs`
- `src/proof_engine.rs`
- `src/symbolic.rs`
- `src/typechecker.rs`

Each follows the existing pattern for collection projections (returning `0`/`Value::Void`/no-op).

## Step 10 — Tests

`cargo test --lib` must pass with 0 regressions. Debug and fix any failures.

## Verification Gate

| Check | Expectation |
|-------|-------------|
| `cargo build` | Clean |
| `cargo test --lib` | All pass, existing + any new |
| `lib/std/__builtin/hashmap.bv` | Deleted |
| `src/ffi/registry.rs` | No collection `__builtin_*` match arms |
| `src/interpreter.rs` | No string-match dispatch for collection operations |
| `hashmap.bv` insert/get/remove | Works via arrow syntax + projections |
| `stack.bv` push/pop/peek | Works via arrow syntax + projections |
| `option.bv` Some/None/unwrap | Works via enum dispatch + `uni` |
