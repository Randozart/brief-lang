# Briev 3.0 Specification

**Version:** 3.0  
**Date:** 2026-06-09  
**Status:** Active — supplements `spec/SPEC.md`

> **2026-06-09 Addendum — Phase 1.5: Type Derivation System**
> 
> Briev now supports `Type Name : Base { ... }` declarations. See §10 below.

---

## 1. Introduction

Briev 3.0 is a **general-purpose systems programming language** built on a unified cognitive grammar. The computational primitive is the **reactive transaction** (`node`). Briev compiles to multiple targets — native assembly (x86_64, AArch64), LLVM IR, C, SystemVerilog — from a single source.

### 1.1 Core Philosophy

- **Contract-First**: Preconditions and postconditions are the source of truth. The compiler proves loop termination and memory safety at compile time.
- **No Magic**: Every language construct has a visible syntactic distinction. No hidden transformations, no compiler intrinsics masquerading as library calls.
- **Syntactic Radical Honesty**: If an operation has distinct physical or compiler-level behavior, its visual representation reflects that boundary.

### 1.2 Core Constructs

- **State (`let`)**: Mutable variables representing physical registers or memory.
- **Transactions (`txn`)**: Atomic state-changing operations with contracts.
- **Reactive Transactions (`node`)**: Auto-firing transactions that loop until their postcondition is satisfied.
- **Contracts (`[pre][post]`)**: Formal compile-time verification targets.
- **System Calls (`syscall!`)**: Direct kernel-level transitions to the operating system.
- **Foreign Functions (`frgn`)**: FFI calls to external libraries.

### 1.3 Target Diversity

| Target | Backend | Use Case |
|:---|:---|:---|
| `.bv` → LLVM IR | `llvm` | Hosted native binaries (Linux, macOS) |
| `.bv` → C | `c` | Portable hosted or embedded C |
| `.bv` → x86_64 asm | `x86_64` | Direct assembly output |
| `.bv` → AArch64 asm | `aarch64` | ARM64 assembly output |
| `.ebv` → SystemVerilog | `verilog` | FPGA / ASIC synthesis |
| `.rbv` → Web | `webstack` | Browser-based UI |
| `.bv` → Rust | `rust` | Rust source generation |
| `.bv` → COBOL | `cobol` | Mainframe targets |

---

## 2. The Universal Symbol Language

Briev's symbols are not arbitrary. Each symbol's **visual shape** maps to a **cognitive metaphor**, which maps to a **systems meaning**. All uses of a given symbol share that core metaphor.

### 2.1 Symbol-to-System Mapping

| Symbol | Cognitive Metaphor | Systems Meaning | Group |
|:---:|---|---|---|
| **`;`** | A hard stop, a reset | Universal statement termination | — |
| **`.`** | Puncturing, reaching into | Struct field access / UFCS | — |
| **`->`** | Forward motion | Dataflow / State transition | — |
| **`<-`** | Backward motion | Mutation / Discard | **Transfer** |
| **`:`** | Identity, equivalence | Static type / definition | — |
| **`:>`** | Identity projecting outward | Compile-time metadata extraction — reveals meaning through a semantic lens | **Lens (Projection)** |
| **`<:`** | Identity projecting inward | Type derivation — restricts what conforms to a type | **Lens (Derivation)** |
| **`[]`** | Containment, boundary | Constraints, guards, partitions — segments a layout into addressable sub-ranges | **Partition** |
| **`{}`** | Grouping, bundling | Code block / organizational unit | — |
| **`()`** | Holding, containing | Parameter / argument enclosure | — |
| **`@`** | Position, location, anchor | Spatial / Temporal / Dimensional / Chronological anchor | **Anchor** |
| **`&`** | Connection, conjunction | Mutation marker (required for all state mutation) | — |
| **`!`** | An exclamation, a warning | Control flow anomaly / fire-and-forget | — |
| **`~`** | Oscillation, flipping | Boolean toggle / atomic lock | — |
| **`?`** | A question, a check | Watchdog / timeout | — |
| **`_`** | A gap, a placeholder | Ignored / unused value | — |

### 2.2 Detailed Symbol Specifications

#### `;` — Universal Statement Termination
Every statement must end in `;`, including blocks denoted by `{}` (transaction bodies, struct definitions). The parser uses `;` as an absolute synchronization token during error recovery.

#### `.` — Struct Field Access & UFCS
The dot `.` uses a strict two-tier priority:
1. **Internal**: If `subject` has a field or internal `defn` defined in its struct body, access it directly.
2. **UFCS fallback**: `subject.method(args)` desugars at parse time to `method(subject, args)`.

`list.len()` becomes `len(list)`, which resolves to the standard library definition using `list .#Size`. The compiler has zero hardcoded knowledge of the name `len`.

#### `->` / `<-` — Directional Dataflow
- `&list <- x`: Push x into list.
- `x <- &list`: Pop last element into x.
- `<- &list`: Discard (pop and throw away).
- `term -> &x = 1`: Swan song — executes only on successful commit.
- `input -> output`: Signature transition.

#### `<-` — Discard Operator
`<- expr` explicitly discards the result of an expression. Required for syscall results that are intentionally ignored:
```briev
<- syscall! @ 3 (fd);
```
This ensures no system-level side-effect can ever be silently ignored.

#### `:` — Ontological Identity
Declares the static type of a symbol:
```briev
let x: Int = 0;
trg button: Bool @ 0x1000;
```

#### `:>` — Metadata Projection (The Metadata Lens)
Projects compiler-held metadata about an entity at compile time with zero runtime overhead:
```briev
list .#Size     → element count
str .#Bytes     → byte footprint
list .#Ptr      → base memory address
x .#Range       → SMT-proven value boundaries
x :> Popcount    → population count
x :> LeadingZeros → leading zero bits
x :> TrailingZeros → trailing zero bits
x :> Absolute    → absolute value
x :> BitReverse  → bit-reversed value
x :> Type        → type discriminant
input :> Match("^[a-z]+$") → compiled DFA table
```

#### `@` — Universal Anchoring
Four distinct dimensions:
- **Spatial (Addresses)**: `let led: Bool @ 0x40020000;`
- **Temporal (Frequency)**: `node simulate @ 100Hz;`
- **Dimensional (Position)**: `tensor[@12: 0..16]`
- **Chronological (History)**: `@balance` (prior tick value)

#### `&` — Mutation Marker
Required for all state mutation. `&x = x + 1` — the `&` links the name to the mutable location.

#### `!` — Cautionary Boundary
Signals unusual control flow:
```briev
frgn! log_message(msg);     // fire-and-forget FFI
syscall! exit(code);        // kernel call, never returns
term!;                       // immediate process termination
trg! interrupt();            // async trigger with rollback risk
```

#### `~` — Boolean Toggle
Shorthand for boolean state transitions:
```briev
[~/ready]                    // Shorthand
[~ready][ready]              // Expanded: "fire when ready is false, make it true"
```
Represents atomic lock acquisition or test-and-set barriers.

#### `?` — Watchdog / Timeout
Physical runtime bound on transaction execution:
```briev
txn long_operation() [true][done] ?[5000ms] {
    do_work();
    &done = true;
    term;
};
```

---

## 3. System Calls

### 3.1 Declaration Syntax

System calls are declared like foreign functions, using the `syscall` keyword:

```briev
syscall SYS_WRITE(fd: Int, buf: Int, count: Int) -> Result<Int, Error>;
syscall! SYS_EXIT(code: Int);   // fire-and-forget, no return
```

### 3.2 Number Resolution

Syscall numbers are target-specific and defined declaratively in `.dbvs` specification files or TOML target specs — never hardcoded in the compiler:

```toml
# lib/targets/hosted_c.toml
[syscalls]
SYS_READ = 0
SYS_WRITE = 1
SYS_OPEN = 2
SYS_CLOSE = 3
SYS_MMAP = 9
SYS_EXIT = 60
```

```dbvs
# targets/x86_64.dbvs
schema SyscallMap {
    SYS_READ: Int = 0;
    SYS_WRITE: Int = 1;
    SYS_EXIT: Int = 60;
};
```

### 3.3 LLVM Codegen

Syscall declarations compile to target-specific inline assembly:

- **x86_64**: `call i64 asm sideeffect "syscall", "={rax},{rax},{rdi},{rsi},{rdx},{r10},{r8},{r9}"(...)`
- **AArch64**: `call i64 asm sideeffect "svc #0", "={x0},{x8},{x0},{x1},{x2},{x3},{x4},{x5}"(...)`

### 3.4 Mandatory Result Handling

Every syscall must either bind its result to a variable or explicitly discard it with `<-`:
```briev
let bytes = syscall! @ SYS_WRITE(1, buf, count);   // bound
<- syscall! @ 3 (fd);                                // explicitly discarded
```

---

## 4. The Reactive Pipeline

### 4.1 Transactions as Atomic Units

A transaction (`txn`) is the fundamental computational primitive. It executes atomically: either all state changes commit, or none do.

```briev
txn withdraw(amount: Int)
    [amount > 0 && balance >= amount]
    [balance == @balance - amount]
{
    &balance = balance - amount;
    term;
};
```

### 4.2 Reactive Transactions as Inherent Loops

`node` auto-fires when its precondition is true and loops until its postcondition is satisfied:

```briev
node fill_buffer()
    [buffer .#Size < 100]
    [buffer .#Size == 100]
{
    &buffer = buffer + [new_item];
    term;
};
```

### 4.3 Swan Song (Commit Action)

The `term ->` block is the **Atomic Commit Phase** — it only executes when the postcondition is proven satisfied:
```briev
term -> &order_status = 1;
```

### 4.4 Variant Bodies (Multi-Guard Transactions)

Multiple preconditions can select different execution paths:
```briev
txn handle [x > 0] { &pos = x; }
         [x < 0] { &neg = -x; }
         [true]  { &zero = 1; };
```

---

## 5. Collections & Memory

### 5.1 List<T>

Dynamic, growable collection with 2-slot stack header `[data_ptr, length]`:
```briev
let items: List<Int> = [1, 2, 3];
&items <- 4;                     // push
let x <- &items;                 // pop
items[0]                         // index access
items .#Size                    // length (compiler projection)
```

### 5.2 Vector<T, dims...>

Fixed-size, contiguous, multidimensional. Hardware-friendly (compiles to BRAM on FPGA):
```briev
let matrix: Vector<Int, 10, 20>;
let tensor: Vector<Float, 3, 32, 32>;
```

### 5.3 Structs & UFCS

Structs are defined with named fields. Method-like calls use Uniform Function Call Syntax:
```briev
struct Point { x: Int; y: Int; };
let p = Point { x: 10, y: 20 };
p.x                                       // field access
p.distance(origin)                        // UFCS → distance(p, origin)
```

---

## 6. Target Specifications

Target behavior is driven by declarative TOML (`.toml`) and DBriev schema (`.dbvs`) files — never hardcoded in the compiler Rust source.

### 6.1 TOML Target Spec (Loaded at Compile Time)

```toml
[target]
name = "hosted-c"
backend = "c"
capabilities = ["logic"]

[codegen]
backend = "c"
extension = "c"
state_allocation = "dynamic"

[syscalls]
SYS_WRITE = 1
SYS_OPEN = 2
SYS_EXIT = 60
```

### 6.2 DBriev Architecture Spec (Hardware Reference)

```dbvs
schema X86_64Target {
    name: String;
    architecture: String;
    bits: Int;
    endian: String;
    os: String;
    abi: String;
};

schema SyscallMap {
    SYS_READ: Int = 0;
    SYS_WRITE: Int = 1;
    SYS_EXIT: Int = 60;
};
```

---

## 7. Hardware Topology & Types (SV Target)

### 7.1 Bit-Level Precision

| Briev Type | SV Representation | Physical Implementation |
|:---|:---|:---|
| `Bool` | `logic` | 1-bit wire/register |
| `UInt @/0..7` | `logic [7:0]` | 8-bit unsigned bus |
| `Int @/0..7` | `logic signed [7:0]` | 8-bit signed bus |
| `Type[N]` | `logic [W:0] name [0:N-1]` | Unpacked Array (BRAM/Registers) |

### 7.2 Memory-Mapped I/O & Pins

- **`trg`**: Synthesized as `input logic`.
- **`let @ address`**: Synthesized as `output logic`.
- Addresses can be specified in `0x4000` or `0x00004000` formats.

### 7.3 Reactor → Synchronous Logic

All `node` blocks synthesize to a global `always_ff @(posedge clk)` block. The `&` operator maps to Non-Blocking Assignments (`<=`).

### 7.4 Geometric SIMD

Vector operations synthesize using SystemVerilog `generate` blocks: N elements = N physical ALUs.

---

## 8. Compiler Constraints & Safety

1. **No Combinational Loops**: Proof Engine errors if wire feedback exists without a register.
2. **No Multi-Driver Violations**: Two transactions cannot drive the same `&` register in the same cycle.
3. **Floating Point Prohibited for SV**: `Float` types result in a compile error for Verilog targets.
4. **No Magic String Matching**: The compiler has zero hardcoded knowledge of function names like `len`, `is_empty`, `unwrap` — these are all defined in the standard library.
5. **Every Syscall Must Bind or Discard**: Syscall results cannot be silently ignored.
6. **Void Type**: Empty parentheses `()` are parsed as `Type::Void`.

---

## 9. Keywords

| Abbrev | Full | Meaning |
|-------|------|--------|
| `txn` | `transaction` | State-changing atomic operation |
| `rct` | `reactive` | Auto-fires when precondition is met |
| `defn` | `definition` | Pure function (no state mutation) |
| `frgn` | `foreign` | FFI call (returns Result) |
| `frgn!` | `foreign!` | FFI fire-and-forget (void) |
| `syscall` | `syscall` | Kernel system call (returns Result) |
| `syscall!` | `syscall!` | Fire-and-forget syscall (void) |
| `let` | `let` | Mutable state declaration |
| `const` | `const` | Compile-time constant |
| `term` | `terminate` | Successful transaction commit |
| `escape` | `escape` | Rollback all changes |
| `trg` | `trigger` | Top-level trigger / hardware input |
| `trg!` | `trigger!` | Local trigger with async rollback |

---

## 10. Type Derivation (`type` Keyword)

> **Added 2026-06-09 (Phase 1.5)**

Briev types are defined using the `Type Name : Base { ... }` declaration. The `<:` operator (read as "derives from" or "is a refinement of") connects a new type to its base type. Properties and constraints within the `{ }` body define how the new type differs from the base.

### 10.1 Primitive Kernel

The compiler natively understands a small set of ~13 type properties. These are the only hardcoded type concepts in the Rust compiler — everything else (`String`, `Stack`, `Queue`, `HashMap`, etc.) is defined in user-space Briev in `lib/std/`.

| Property | Type | Default | Meaning |
|----------|------|---------|---------|
| `Bytes` | `Int` | _required_ | Physical width in bytes — LLVM `alloca`, VHDL width |
| `Alignment` | `Int` | `= Bytes` | Alignment boundary — LLVM `align` |
| `Endian` | `Enum` | `Little` | Byte order — LLVM `bswap`/load-store order |
| `Volatile` | `Bool` | `false` | LLVM `load volatile`/`store volatile` |
| `Atomic` | `Bool` | `false` | LLVM atomic operations |
| `ElementType` | `Type` | _(none)_ | Unlocks `[]` and slicing — compiler synthesizes GEP/address-decoding |
| `FixedSize` | `Bool` | _(none)_ | `false` unlocks `<-` / `->` — heap/circular buffer strategy |
| `InsertAt` | `Expr` | _(none)_ | Index expression for insertion position |
| `ExtractFrom` | `Expr` | _(none)_ | Index or `<:{}` query for extraction position |
| `AllowIndex` | `Bool` | `true` | Override to `false` to block `[]` |
| `AllowSlice` | `Bool` | `true` | Override to `false` to block slicing |
| `AllowArrow` | `Bool` | `true` | Override to `false` to block `<-`/`->` |
| `Codec` | `Struct` | _(none)_ | Struct with `encode`/`decode` — literal translation at compile-time |

### 10.2 Expressing InsertAt / ExtractFrom

`InsertAt` and `ExtractFrom` accept index expressions that the compiler recognizes in Pass 1:

| Expression | Strategy | Example |
|---|---|---|
| `0` | Constant front, head-pointer advance | Queue pop |
| `.#Size` | Append position, pointer increments | List/Queue push |
| `.#Size - N` | Offset from end, pointer decrements | Stack pop |
| `: { MIN(.key) }` | Maintain heap by key | Priority queue |
| `: { MAX(.key) }` | Maintain heap by key | Priority queue |

Any unrecognized expression form is a compile-time error.

### 10.3 Example: Scalar Type Derivation

```briev
Type U8  : Bits { Bytes = 1; Alignment = 1; };
Type U16 : Bits { Bytes = 2; Alignment = 2; };
Type U32 : Bits { Bytes = 4; Alignment = 4; };
Type U64 : Bits { Bytes = 8; Alignment = 8; };
Type Int : U64;
Type Float : Bits { Bytes = 8; Alignment = 8; };
Type MmioReg : U32 { Volatile = true; };
```

### 10.4 Example: Collection Type Derivation

```briev
Type List<T> : Bits {
    ElementType = T;
    FixedSize = false;
    InsertAt = .#Size;
    ExtractFrom = .#Size - 1;
};

Type Stack<T> : List<T> {
    AllowIndex = false;
};

Type Queue<T> : List<T> {
    ExtractFrom = 0;
    AllowIndex = false;
};
```

Properties not overridden are inherited from the base type. `Stack` inherits `ElementType`, `FixedSize`, `InsertAt`, and `AllowSlice` from `List`.

### 10.5 Example: Codec-Bearing Types

Codecs are imported structs with `encode`/`decode` signatures, validated in Pass 1:

```briev
import { UTF8 } from "std/UTF8.bv";

Type String : List<U8> {
    Codec = UTF8;
};
```

The compiler uses the codec to translate string literals at compile time — `"Hello"` stored as `String` runs `UTF8::encode("Hello")` during compilation, emitting the encoded bytes directly into the binary.

### 10.6 Refinement Constraints

Inline constraints with implicit `_` subject can appear in the type body:

```briev
Type PositiveInt : Int {
    [ > 0 && < 100 ]
};
```

Pass 1 validates literals against these constraints. The backend synthesizes runtime guards for dynamic values.

### 10.7 The Two-Pass Pipeline

```
PASS 1: Type-Universe Pass
  - Collect all Type Name : Base { ... } declarations
  - Resolve derivation chain to Bits
  - Inherit + override metadata properties
  - Validate Bytes required on all Bits-derived types
  - Validate InsertAt/ExtractFrom expression forms
  - Validate Codec has encode/decode
  - Evaluate refinement constraints
  - FREEZE: type universe immutable for Pass 2

PASS 2: Executable Pass
  - Parse and typecheck defn/txn/rct
  - Resolve let x: Stack<T> against the universe
  - Validate :> projections against defined metadata
  - Synthesize bracket/arrow from AllowIndex/AllowArrow gates
  - Encode literals via Codec
  - Emit LLVM IR / VHDL with frozen metadata
```

### 10.8 Comparison with Other Languages

| Language | Type definition mechanism | User-space control |
|----------|--------------------------|--------------------|
| **C** | `typedef`, `struct` | Layout only, no semantics |
| **Rust** | `struct`, `enum + impl` | Trait implementations, no layout control |
| **Ada** | `subtype`, representation clauses | Layout control, no programmable codecs |
| **Zig** | `comptime` + struct generation | Powerful but no formal refinement |
| **Briev** | `Type ... : ... { ... }` | Layout, codecs, access gates, all in user-space |

## 11. Type/Metadata Check Expressions: `is`, `from`, `like`

**Added 2026-06-14 (Phase 15)**

Three infix expressions that inspect types and structure at runtime:

| Expression | Meaning | Returns |
|-----------|---------|---------|
| `x is T` | `x`'s runtime type is `T` | `Bool` |
| `x is Some` | `x`'s enum variant is `Some` | `Bool` |
| `x from T` | `x`'s type derives from `T` | `Bool` |
| `x like y` | `x` and `y` have structurally equivalent layout | `Bool` |

### Precedence

```
!x is Some      → !(x is Some)
x is Some == true → (x is Some) == true
x from T == false → (x from T) == false
```

`is`/`from`/`like` bind tighter than `==`/`!=` but looser than unary `!` and comparison operators.

### `is` — Type and Variant Check

```briev
let x: Int = 42;
let is_int = x is Int;       // → true

let y: Option[Int] = some(42);
let is_some = y is some;     // → true
let is_none = y is none;     // → false
```

The RHS of `is` can be:
- A **type name** (`Int`, `String`, `Option[Int]`, `MyStruct`) — checks if the LHS value's runtime type matches.
- A **variant keyword** (`some`, `none`, `ok`, `err`) — checks if the LHS enum value's discriminant matches the named variant.

### `from` — Derivation Check

```briev
struct Foo { x: Int; }
struct Bar : Foo { y: Int; }

let obj = Bar { x: 1, y: 2 };
let is_from_foo = obj from Foo;   // → true
let is_from_baz = obj from Baz;   // → false
```

Checks whether the LHS value's type is the target type or a subtype of it. For structs, this walks the `<:` derivation chain. For enums, this checks the enum type name.

### `like` — Structural Equality

```briev
42 like 42             // → true
42 like 1              // → false
"hello" like "hello"   // → true
[1, 2] like [1, 2]     // → true (recursive element comparison)
```

`like` compares the structural layout of two values, not their nominal type. Two structs with different names but identical fields can be `like` each other.

### Implementation Notes

- **Interpreter**: Fully implemented with recursive structural comparison for lists, structs (Instance), enums, and primitive types.
- **LLVM Backend**: Currently emits stubs (compile-time `true` for `is`/`from`, delegate to `fcmp` for `like`). Full runtime type-tag comparison is future work.
- **Typechecker**: Returns `Type::Bool` without deeper structural analysis. Future work includes compile-time folding and variant resolution.

---

*End of Briev 3.0 Specification*
