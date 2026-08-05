# sig: Verified Output Projection + Output Type Algebra

**Date:** 2026-06-06  
**Status:** Design — To Implement  
**Spec Version Target:** 5.0

---

## 1. The Problem: Observability & Liveness

Briv's optimizer eliminates dead code. A value is "dead" if no FFI call consumes it. This is correct — a program that produces no observable effect IS dead code.

But the optimizer cannot see through libc functions. `putchar()` gets eliminated via LLVM TargetLibraryInfo even with `memory(write)` on the declaration after LTO inlines the wrapper. `fprintf(stderr, ...)` survives but is a fragile workaround.

Earlier attempts at fixing this:
- `#out` pragma on `frgn` — works but is a band-aid
- `io_pending` guard hack — ugly, not idiomatic
- `#!out(x)` field annotation — escapes hatches, not language

The real solution is **`sig`** — a mechanism that existed in the v4 spec (§5.1–5.6) but was never implemented. Reviving and extending it to cover both output-type projection AND side-effect visibility.

---

## 2. Output Type Algebra

A `defn` declares its output type as a **sum of products** with named slots:

```
defn allPersonalData() -> account: Int | rejections: Bool[], name: String, age: Int {
    ...
};
```

### Composition Rules

| Syntax | Name | Meaning | Set interpretation |
|--------|------|---------|-------------------|
| `Type` | Atom | Single value | Exactly one value |
| `name: Type` | Named slot | Labeled output | `name → Type` |
| `Type, Type` | Product (tuple) | All values together | Cross product |
| `Type \| Type` | Sum (union) | Exactly one branch | Disjoint union |
| `Type[]` | Array | Dynamic-length collection | `List<Type>` |

### Precedence

```
[]  >  ,  >  |
```

- `Bool, Int[]` → `Bool, (Int[])` — tuple of Bool and Int array
- `Bool, Int | String[]` → `(Bool, Int) | (String[])` — tuple OR array
- `rejections: Bool[], name: String, age: Int` → named tuple: `(Bool[], String, Int)`

### Examples

```
// Single value
defn get_status() -> String { ... };

// Named single
defn get_user() -> name: String { ... };

// Tuple (product)
defn divide(a: Int, b: Int) -> quotient: Int, remainder: Int { ... };

// Union (sum)
defn fetch() -> User | Error { ... };

// Sum of products (the general form)
defn api() -> account: Int | rejections: Bool[], name: String, age: Int { ... };

// Array
defn search(q: String) -> Result[] { ... };

// Complex composition
defn process() -> String | Bool, Int | Float[], Float[] { ... };
```

---

## 3. `let` Destructuring as Implicit Projection

The **caller shapes the request** by how they destructure the result. The compiler checks the projection matches:

```
let account: Int = allPersonalData();
// → checks variant 1 has account: Int — ✅

let (name, age): (String, Int) = allPersonalData();
// → checks variant 2 has name: String, age: Int — ✅

let x: (Bool[], String, Int) = allPersonalData();
// → checks variant 2 Bool[], String, Int — ✅

let (flag, n): (Bool, String) = allPersonalData();
// → variant 2 has Bool[], not Bool — type mismatch — ❌

let items: (String, Int, Bool) = allPersonalData();
// → no variant has (String, Int, Bool) — ❌
```

The `let` destructuring is sugar for an anonymous `sig`. The compiler must prove the requested shape is reachable from the defn's output type.

### Named Destructuring

Named slots can be projected by name:

```
let (name, age) as person: (String, Int) = allPersonalData();
// Or with explicit slot names:
let (n: name_str, a: age_int): (String, Int) = allPersonalData();
```

---

## 4. `sig` — Explicit Verified Projection

`sig` is the **named, reusable** projection of a defn's output type. It is NOT a black box — it is a verified contract:

- For every input the defn would actually take at call sites in the program
- The compiler proves the projected output is reachable
- If unprovable → **compile error**

### Basic sig

```
sig get_name() -> String from get_status;
// → projects `get_status`'s output to just String
// → compiler verifies: does get_status ever produce String? ✅
```

### sig `-> true` Assertion

```
defn always_true() -> Bool { term true; };
sig always_true() -> true;
// → compiler proves: output is always true ✅

defn maybe_true(b: Bool) -> Bool { term b; };
sig maybe_true() -> true;
// → b could be false → ❌ compile error

defn bool_return(b: Bool) -> Bool { term b; };
// If all call sites pass `true`:
sig bool_return() -> true;
// → compiler proves: at every call site, b is true → ✅
```

### sig with tuple projection

```
sig personal_data() -> Bool[], String, Int from allPersonalData;
// → projects variant 2: (Bool[], String, Int) — ✅

sig personal_account() -> Int from allPersonalData;
// → projects variant 1: account: Int — ✅
```

---

## 5. `sig #out` / `sig #inline` — Side-Effect Modifiers

`#out` and `#inline` attach side-effect information to a `sig` that cannot be inferred from types alone. They are **sig properties** that control how the optimizer treats calls.

### `#out` — Observable Output

```
sig #out OUT__print_int(n: Int) -> Bool from __print_int;
```

| Property | Value |
|----------|-------|
| LLVM attribute | `#6 = { nocallback nofree nosync nounwind willreturn memory(write) }` |
| Optimizer | Call NOT eliminated — memory(write) observed |
| When to use | FFI functions that write to stdout/stderr/files/network |
| Meaning | "This call has external effects the compiler cannot see" |

### `#inline` — Pure / Safe to Fold

```
sig #inline strlen(s: String) -> Int from __strlen;
```

| Property | Value |
|----------|-------|
| LLVM attribute | `#1 = { nocallback nofree nosync nounwind willreturn }` |
| Optimizer | Can be folded, inlined, or eliminated if unused |
| When to use | Pure computations, math functions, pure FFI |
| Meaning | "This call is safe to optimize — it has no external effects" |

### Applicable at Multiple Levels

```
// 1. Declaration level (always affects calls to this sig)
sig #out OUT__putchar(c: Int) -> Int from __putchar;

// 2. Call site level (overrides declaration)
sig #inline __putchar(0);
// → This specific call treated as inline, even if declared #out

// 3. Block scope (all calls in block inherit)
sig #out {
    putchar('A');
    print_int(42);
    print("hello");
};

// 4. File scope (all calls in file inherit)
sig #out;
```

### Interaction with `let` destructuring

```
defn api() -> String | Int {
    [cached] term "ok";
    term 42;
};

// let destructuring with #out modifier
sig #out (label: String) = api();
// → This call to api is treated as #out
// → Projection: String variant of api
```

---

## 6. Multi-Output `term` Syntax

The `term` keyword provides values for all declared output positions.

### Standard term

```
term value;
```

### Multi-slot term (tuple outputs)

Positions correspond to `,` slots in the output type:

```
defn print_status() -> Bool, Bool {
    [ok]   term true, false;    // slot 0 = true (success), slot 1 = false (not empty)
    term false, true;            // slot 0 = false (fail),  slot 1 = true (empty)
};
```

### Named-slot term

```
defn divide(a: Int, b: Int) -> quotient: Int, remainder: Int {
    term a / b, a % b;
};
```

### term with union branching

Different `term` paths select different union variants:

```
defn try_parse(s: String) -> Int | String {
    [s.is_digit()] term parse_int(s);   // produces Int variant
    term s;                              // produces String variant
};
```

### `term!` — Hard Exit

```
term!;           // immediate program exit, no swan song
```

### `term -> swan_song;` — Commit Action

```
term "done" -> cleanup();   // exit value "done", run cleanup() as swan song
```

---

## 7. `trg` — Proper Trigger Syntax (Supersedes `io_pending`)

`trg` already exists for reactive edges. The `io_pending` hack was a workaround — now deprecated.

### Declaration

```
trg io_ready: Bool @ link __io_pending;
trg sigint: Bool @ link __sigint_flag;
trg clock_100hz: Int @ link __timer_100hz;
```

### Usage in guards

```
// Old (deprecated — ugly):
node work [io_pending && count < N][count == N] { ... };

// New (idiomatic — uses trg):
trg io_ready: Bool @ link __io_pending;
node work [io_ready && count < N][count == N] { ... };
```

### All triggers are just FFI links

```
trg io_pending: Bool @ link __io_pending;
// → Reads external global __io_pending
// → Fires transaction when true
```

---

## 8. OUT Library (`lib/std/out.bv`)

The standard library provides pre-declared `sig #out` functions with the `OUT__` prefix. Every `OUT__` function has `#out` baked in — the user just calls them.

### Implementation

```
// lib/std/out.bv
import "link/briv_rt.o";

// Underlying FFI (neutral — no sig, no #out)
frgn __print_int(n: Int) -> Bool from "libruntime";
frgn __putchar(c: Int) -> Int from "libruntime";
frgn __print(msg: String) -> Bool from "libruntime";
frgn __print_float(f: Float) -> Bool from "libruntime";

// Output-marked signatures (sig #out — WILL NOT be eliminated)
sig #out OUT__print_int(n: Int) -> Bool from __print_int;
sig #out OUT__putchar(c: Int) -> Int from __putchar;
sig #out OUT__print(msg: String) -> Bool from __print;
sig #out OUT__print_float(f: Float) -> Bool from __print_float;

// Convenience wrappers
sig #out OUT__println(msg: String) -> Bool from __print;
// → appends \n before calling __print
```

### Usage

```
import { OUT__print_int, OUT__println } from "std/out.bv";

OUT__print_int(42);
OUT__println("hello world");
```

### Naming Convention

`OUT__` prefix = "this call WILL produce observable output." Visually unmistakable at call sites. The double underscore separates the category (OUT) from the name (print_int).

---

## 9. Verbose Compilation (`--explain`)

A new compiler flag (`--explain`) prints optimization decisions step by step:

### Sig Resolution

```
sig #out applied to OUT__putchar at benchmarks/fasta.bv:9
  → LLVM #6 attribute: memory(write)
  → call will NOT be eliminated
```

### Liveness Trace

```
Liveness analysis:
  Field  seed — live (consumed by OUT__putchar(seed % 26 + 97))
  Field  count — live (exit condition: count == N)
  Field  IM    — dead (inlined: const 139968, eliminated)
```

### Fold Decisions

```
Transaction "fasta":
  BOUND = __get_env_int("BOUND") — runtime determined
  → precomputation blocked — emitting runtime loop
  
Transaction "print_loop":
  N = 50000000 (const)
  → exceeds optimize-budget (256)
  → runtime loop emitted (budget insufficient to precompute)
```

### Dead Elimination

```
Field  tmp — eliminated
  Reason: no sig #out, no FFI call, no exit condition references it
```

---

## 10. Learn Briv Documentation (`docs/learn/liveness.md`)

Sections:

### 10.1 Liveness Model

"A value is live if an FFI call consumes it."

Briv's optimizer eliminates code that produces no observable effect. This is correct — a program that doesn't print, write files, or make network calls is dead code.

### 10.2 Ways Code Stays Live

| Mechanism | Level | Example |
|-----------|-------|---------|
| FFI call in body | Call | `__putchar(x)` |
| `sig #out` | Declaration / call / block / file | `sig #out foo() -> T from bar` |
| `sig #inline` override | Call site | `sig #inline pure_calc(x)` |
| `let` destructuring | Call site | `let x: T = fn()` — implicit sig |
| `#!out(x)` field annotation | Field | `#!out(result)` |
| `#!exit` condition | Exit check | `#!exit count == N` |
| `trg` trigger | Guard | `node [trg_name] { ... }` |

### 10.3 OUT Library — No Magic

The `OUT__*` functions are `sig #out` signatures over raw FFI. Nothing magical:

- `OUT__print_int` calls `__print_int` (which calls `fprintf(stderr, "%lld\n", n)`)
- `sig #out` tells the compiler: "this has external effects — don't eliminate"
- The `#out` modifier maps to `memory(write)` in LLVM IR

### 10.4 What Dead Code Means

If the compiler eliminates your code, it means: "I can prove this produces no observable output." The fix is NOT hacks (`x == x`, `io_pending`). The fix IS:
- Use `sig #out` on output calls
- Use the OUT library (`OUT__print_int`, `OUT__println`)
- Make output actually observable via `trg` + `node`

### 10.5 Debugging Elimination

Use `--explain` to see:
- Which fields the compiler eliminated (and why)
- Which sig annotations are active
- How the output type projection is verified

---

## 11. Implementation Phases

### Phase 1: Parser + AST (sig keyword, output type algebra)

- Add `Token::Sig` to lexer
- Add `OutputType` enum to AST:
  ```
  OutputType::Atom(Type)
  OutputType::Named { name: String, typ: Type }
  OutputType::Tuple(Vec<OutputType>)
  OutputType::Union(Vec<OutputType>)
  OutputType::Array(Box<OutputType>)
  ```
- Add `TopLevel::SigDecl { modifier, name, params, output_type, source }`
- Add `Expr::SigCall { modifier, expr }` for call-site sig
- Parse `let (x, y): (A, B) = fn()` destructuring
- Parse multi-slot `term a, b, c;` syntax

### Phase 2: Verification (sig projection checking)

- Implement output-type subset checking
- Implement `-> true` assertion verification (path analysis)
- Implement `let` destructuring type matching
- Compile error on unreachable projections

### Phase 3: LLVM Codegen (sig #out / #inline)

- `sig #out` → calls emit `#6 { memory(write) }` attribute
- `sig #inline` → calls emit `#1 { willreturn }` attribute
- File/block scope sig defaults propagate to calls
- `let` destructuring inherits sig modifier from enclosing scope

### Phase 4: OUT Library

- Create `lib/std/out.bv` with `sig #out OUT__*` declarations
- Update all benchmarks to import from `out.bv`
- Remove raw `frgn #out` declarations from benchmarks

### Phase 5: Deprecate `io_pending`

- Migrate all benchmarks from `io_pending` guard to `trg` + proper guard
- Add deprecation warning when `io_pending` is used as liveness hack
- Remove `io_pending` from documentation

### Phase 6: Verbose Compilation (`--explain`)

- New CLI flag `--explain`
- Print sig resolution, liveness decisions, fold decisions
- Print dead-field elimination reasons

### Phase 7: Learn Briv + SPEC Update

- Write `docs/learn/liveness.md`
- Update `spec/SPEC.md` with output type algebra (§ new section)
- Update `spec/old_docs/language_specs/v4-briv-lang-spec.md` to v5

---

## 12. Full Examples

### Fasta Benchmark (Before → After)

**Before** (current — uses `#out` pragma and `io_pending` hack):
```
#!exit count == N;
import "link/briv_rt.o";
import { io_pending } from "std/briv_rt.bv";

frgn #out __putchar(c: Int) -> Int from "libruntime";
frgn __get_env_int(name: String) -> Int from "libruntime";

let count: Int = 0;
let N: Int = __get_env_int("BOUND");
let seed: Int = 42;

node fasta [io_pending && count < N][count == N] {
    &seed = seed * 3877 + 29573 % 139968;
    __putchar(seed % 26 + 97);
    &count = count + 1;
    term;
};
```

**After** (uses `sig #out` and OUT library):
```
#!exit count == N;
import "link/briv_rt.o";
import { OUT__putchar } from "std/out.bv";

frgn __get_env_int(name: String) -> Int from "libruntime";

let count: Int = 0;
let N: Int = __get_env_int("BOUND");
let seed: Int = 42;

node fasta [count < N][count == N] {
    &seed = seed * 3877 + 29573 % 139968;
    OUT__putchar(seed % 26 + 97);
    &count = count + 1;
    term;
};
```

### Multi-Output Function with sig Projection

```
// Define a function with sum-of-products output
defn process_data(input: String) -> ok: Int | error: String, code: Int {
    [input.is_digit()] term parse_int(input);          // ok: Int variant
    [input.len() > 100] term "too long", 413;          // (error: String, code: Int) variant
    term "invalid", 400;                                 // (error: String, code: Int) variant
};

// Caller projects via let
let result: Int = process_data("42");
// → sig projection checks: Int variant reachable ✅

let (msg, err_code): (String, Int) = process_data("hello");
// → sig projection checks: (String, Int) variant reachable ✅

// Named projection
sig process_ok() -> Int from process_data;
// → named sig: verified once, reused everywhere

sig #out process_safe() -> Int from process_data;
// → same, but with #out — optimizer preserves the call
```
