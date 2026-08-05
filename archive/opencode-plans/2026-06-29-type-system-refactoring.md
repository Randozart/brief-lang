# Type System Refactoring: Types as First-Class Universe Citizens

**Date:** 2026-06-29
**Status:** Draft
**Target:** Safe, self-hosting, backend-agnostic type system

## 1. Executive Summary

Currently, the compiler's understanding of types is split across **two worlds**:

1. **TypeUniverse (data):** Stores layout metadata (bytes, alignment, endian)
   and collection semantics (element_type, insert_at, extract_from). Built
   at compile time, frozen before analysis.

2. **Backend match arms (code):** Hardcoded `match ty { Type::Float => "float",
   Type::Bool => "i8", ... }` scattered across 7+ locations. Every type
   mapping decision is made by a match arm, not by querying the universe.

**The gap:** A user-defined `type MyFloat <: Bits { Bytes = 4; Alignment = 4; }`
gets layout metadata but falls through to `_ => "i64"` in every backend
match arm. It works, but it's opaque — no TBAA, no native float registers,
no SROA, no optimizer visibility.

**The fix:** Close the gap by moving ALL type→backend mappings from match
arms into the TypeUniverse. Make the universe the single source of truth
for EVERY property a type has. Then the backend becomes a query engine:
instead of `match ty { Type::Float => ... }`, it asks
`universe.lookup(type_name).llvm_type`.

When the universe is self-hosting (built-in types are populated the same
way user types would be), a user can define ANY type from `Bits` and get
the same compiler treatment as the built-in Float.

## 2. Design Principles

### P1. No magic — everything is a property

Float isn't special. It's `Bits` with a specific configuration:
- `LLVMType = float; Storage = Native; TBAA = FloatNode;`
- `Add = fadd#; Sub = fsub#; Box = box.f32.to.i64#; Unbox = unbox.i64.to.f32#;`

If a user declares the same properties, the compiler treats it the same way.

### P2. Intrinsics as the primitive operation unit

Every type operation maps to an `Intrinsic` variant. Intrinsics are:
- **Backend-checked:** At compile time, the target backend's intrinsic
  registry is queried. If `fadd#` exists on LLVM but not on CIRCT, the
  type simply can't compile for CIRCT. No fallback, no silent emulation.
- **User-callable:** Intrinsics are first-class callables. A savvy
  programmer can call `fadd#(x, y)` directly, outside the type system.
- **Curated:** The intrinsic set is the complete set of primitive
  operations the compiler understands. Adding a new intrinsic is a
  compiler change (new variant, new backend lowering).

### P3. Safety through compile-time validation

The compiler validates, never silently falls back:
- Missing operator → compile error, not `i64` opaque behavior
- Unsupported intrinsic → compile error for that target backend
- Wrong intrinsic type signature → compile error at check time
- Contract violation → contract checker catches it at check time

### P4. Backend query contract

Every intrinsic variant has a `fn supported_on(&self, backend: TargetBackend) -> bool`
method. At universe-build time, the compiler checks every operator→intrinsic
mapping against the selected backend. If a type uses an intrinsic that
doesn't exist on the target, compilation fails with a clear error:

```
error: type 'BFloat16' uses intrinsic 'bf16_add#' which is not supported
       on target 'webstack'
```

### P5. Inheritance with override

`type MyFloat <: Float {}` inherits ALL of Float's properties (TBAA,
boxing, operations). Users override only what differs. "From Bits" is
for the completeness proof — in practice, users inherit from primitives.

## 3. The Complete Type Property Model

Every type has these categories of properties:

### 3a. Structural Properties (already in ResolvedType)

| Property | What it stores | Example (Float) |
|----------|---------------|-----------------|
| `Bytes` | Physical width in bytes | `4` |
| `Alignment` | Alignment requirement | `4` |
| `Endian` | Byte order | `Little` |
| `Volatile` | MMIO flag | `false` |
| `Atomic` | Atomic access flag | `false` |
| `ElementType` | Element type name | `None` |
| `FixedSize` | Compile-time fixed size | `true` |
| `InsertAt` | Insertion strategy | `None` |
| `ExtractFrom` | Extraction strategy | `None` |
| `AllowIndex` | `[]` indexing allowed | `false` |
| `AllowSlice` | Slice syntax allowed | `false` |
| `AllowArrow` | `<-` mutation allowed | `false` |
| `Codec` | Encoding | `None` |
| `OnExit` | Destructor | `None` |

### 3b. LLVM-Specific Properties (NEW — Phase A)

| Property | Type | Default | Purpose |
|----------|------|---------|---------|
| `LLVMType` | String | `"i64"` | LLVM IR type string for register values |
| `Storage` | Enum: `Native | Boxed` | `Native` = stored in native representation, `Boxed` = stored as i64 in %State |
| `TBAA` | String | `"Int"` | TBAA type tree node name (for `!tbaa !N` attachment) |
| `BoxOp` | Intrinsic | `None` | Intrinsic that converts native → i64 for state storage |
| `UnboxOp` | Intrinsic | `None` | Intrinsic that converts i64 → native from state load |
| `NativeByteSize` | u64 | `Bytes` | Byte size in native representation (may differ from stored bytes) |

**TBAA values map to nodes:**
| TBAA Name | Node Index | Types |
|-----------|-----------|-------|
| `"Int"` | 1 | Int, UInt, all boxed values |
| `"Bool"` | 2 | Bool |
| `"Char"` | 3 | Char |
| `"String"` | 4 | String, Data |
| `"Float"` | 5 | Float, Float64, any float-derived |

### 3c. Operator Properties (NEW — Phase B)

Each operator maps a rune (`+`, `-`, `*`, `/`, `=`, `<`, `>`, `<-`, `:>`, `[]`) to
an intrinsic or defn:

```rust
pub struct OpMapping {
    /// The rune this operator implements (+, -, =, <-, etc.)
    pub rune: OpRune,
    /// Type signature: (Self, param_type) -> return_type
    pub param_type: Option<Type>,
    pub return_type: Type,
    /// Implementation: intrinsic, inop, or defn
    pub implementation: OpImpl,
}

pub enum OpRune {
    Add, Sub, Mul, Div, Mod, Neg,        // arithmetic
    Eq, Ne, Lt, Le, Gt, Ge,            // comparison
    And, Or, Not,                       // logical
    Index, Slice, ArrowPush, ArrowPop,  // collection
    Box, Unbox,                         // conversion
    Cast,                               // type coercion
}

pub enum OpImpl {
    Intrinsic(Intrinsic),
    Inop(String),      // backend-defined inop by name
    Defn(String),      // Briv defn by name
    Composed(Vec<OpImpl>),  // automatic composition (e.g., Box + Add + Unbox)
}
```

### 3d. Example: Float's Complete Property Table

```briv
// Float is a primitive. Its properties are defined by the compiler,
// but this is what they LOOK like:
type Float <: Bits {
    Bytes = 4;
    Alignment = 4;
    LLVMType = float;
    Storage = Native;
    TBAA = FloatNode;
    Box = bitcast.f32.to.i64#;
    Unbox = bitcast.i64.to.f32#;

    op Add(Float)   -> Float = fadd#;
    op Sub(Float)   -> Float = fsub#;
    op Mul(Float)   -> Float = fmul#;
    op Div(Float)   -> Float = fdiv#;
    op Neg()        -> Float = fneg#;
    op Eq(Float)    -> Bool  = fcmp.oeq#;
    op Lt(Float)    -> Bool  = fcmp.olt#;
    op Gt(Float)    -> Bool  = fcmp.ogt#;

    // Cross-type: these are composed automatically by the compiler
    // from Box/Unbox + the primitive operation
    op Add(Int)     -> Float = compose;
    op Add(Float64) -> Float = compose;
};
```

### 3e. Example: User-Defined BFloat16

```briv
type BFloat16 <: Bits {
    Bytes = 2;
    Alignment = 2;
    LLVMType = i16;
    Storage = Boxed;        // stored as i64 in %State
    TBAA = FloatNode;       // inherits float aliasing semantics

    Box = inop! bf16_box(val: float) -> i64 { /* ... */ };
    Unbox = inop! bf16_unbox(val: i64) -> float { /* ... */ };

    // Software arithmetic via inop (works on any backend)
    op Add(BFloat16) -> BFloat16 = bf16_add#;
    op Sub(BFloat16) -> BFloat16 = bf16_sub#;
    op Eq(BFloat16)  -> Bool    = bf16_eq#;
};
```

`bf16_add#` is an Intrinsic variant. It has a function body defined as
an `inop!` block. The Intrinsic enum has a method:

```rust
impl Intrinsic {
    /// Return the inop body for this intrinsic, if one is defined.
    /// If none, the intrinsic is backend-provided (e.g., llvm intrinsic).
    pub fn inop_body(&self) -> Option<&InopDeclaration>;
}
```

For `bf16_add#`, it returns the inop body. The backend:
- LLVM: attempts to lower via `llvm.fadd.bf16` if available, otherwise
  uses the inop body (which compiles to plain Briv→LLVM)
- Webstack: uses the inop body (no bfloat support, but it works)
- CIRCT: uses the inop body

This is how portability works: **inops are the portable fallback for
intrinsics that not all backends support natively.**

## 4. Intrinsic Registry

### 4a. Current Intrinsic Enum (src/ast.rs)

The `Intrinsic` enum already has ~100+ variants: `Sqrt`, `Sin`, `Cos`,
`Print`, `Println`, `Fabs`, `Ceil`, `Floor`, `Ctpop`, `Ctlz`, `Cttz`,
`Abs`, `Min`, `Max`, etc.

### 4b. New Intrinsic Variants (to add)

**Arithmetic:**
```rust
FAdd32, FSub32, FMul32, FDiv32, FNeg32,  // float (f32)
FAdd64, FSub64, FMul64, FDiv64, FNeg64,  // double (f64)
IAdd8, IAdd16, IAdd32, IAdd64,            // integer add by width
ISub8, ISub16, ISub32, ISub64,            // integer sub by width
IMul8, IMul16, IMul32, IMul64,            // integer mul by width
IDiv8, IDiv16, IDiv32, IDiv64,            // integer div by width
USub8, USub16, USub32, USub64,            // unsigned variants
UAdd8, UAdd16, UAdd32, UAdd64,
UMul8, UMul16, UMul32, UMul64,
UDiv8, UDiv16, UDiv32, UDiv64,
```

**Comparison:**
```rust
FCmpOEq32, FCmpOLt32, FCmpOGt32, FCmpOLe32, FCmpOGe32, FCmpONe32,
FCmpOEq64, FCmpOLt64, FCmpOGt64, FCmpOLe64, FCmpOGe64, FCmpONe64,
ICmpEq8, ICmpEq16, ICmpEq32, ICmpEq64,
ICmpSlt8, ICmpSlt16, ICmpSlt32, ICmpSlt64,
ICmpSgt8, ICmpSgt16, ICmpSgt32, ICmpSgt64,
```

**Conversion:**
```rust
BitcastF32ToI32, BitcastI32ToF32,          // float ↔ i32
BitcastF64ToI64, BitcastI64ToF64,          // double ↔ i64
BitcastI8ToI64, BitcastI64ToI8,            // i8 ↔ i64 (trunc/zext)
PTrtoInt, IntToPtr,                        // pointer ↔ i64
SExt8, SExt16, SExt32,                     // signed extend to i64
ZExt8, ZExt16, ZExt32,                     // zero extend to i64
TruncToI8, TruncToI16, TruncToI32,         // truncate from i64
SIToFP, FPToSI,                            // int ↔ float conversion
```

### 4c. Backend Query Contract

```rust
impl Intrinsic {
    /// Does this intrinsic exist on the given backend?
    pub fn supported_on(&self, backend: TargetBackend) -> bool {
        match backend {
            TargetBackend::LLVM => match self {
                Intrinsic::FAdd32  | Intrinsic::FSub32  | ... => true,
                Intrinsic::BF16Add => true,  // has inop body fallback
                Intrinsic::VecInsert => false, // not yet implemented
                _ => true,  // most intrinsics are LLVM-supported
            },
            TargetBackend::Webstack => match self {
                Intrinsic::FAdd32 | Intrinsic::FSub32 | ... => true,
                Intrinsic::BF16Add => true,  // inop body works everywhere
                Intrinsic::Ctpop => false,   // WebAssembly has popcnt
                // Most LLVM-specific intrinsics use inop fallback
                _ => self.inop_body().is_some(), // has portable fallback
            },
        }
    }
}
```

## 5. Implementation Phases

### Phase A: Backend Internal Refactoring (no user-facing changes)

**Goal:** Move all hardcoded type→LLVM mappings from match arms to
universe queries. Backend behavior does not change — the code path is
just redirected through data.

#### A1. Add New Properties to ResolvedType (1 week)

Add to `src/type_universe.rs`:

```rust
pub struct ResolvedType {
    // ... existing fields ...

    // ── Phase A: LLVM-specific metadata ──
    pub llvm_type: String,              // default: "i64"
    pub storage: StorageKind,           // Native or Boxed
    pub tbaa_node: String,              // default: "Int"
    pub box_op: Option<String>,         // intrinsic name for box
    pub unbox_op: Option<String>,       // intrinsic name for unbox
}

pub enum StorageKind {
    Native,  // stored in native representation
    Boxed,   // stored as i64 in %State
}
```

#### A2. Populate Built-in Type Properties (1 week)

In `TypeUniverse::build()`, after collecting user TypeDefs, insert entries
for all built-in primitive types (Int, Float, Float64, Bool, Char, String,
Data, etc.) with their full property tables.

This is a single match arm in the universe builder — ONE match arm replaces
the 7+ scattered across the backend:

```rust
fn init_primitive_type(name: &str) -> ResolvedType {
    match name {
        "Int" => ResolvedType {
            llvm_type: "i64".into(), storage: Boxed,
            tbaa_node: "Int".into(), bytes: 8, alignment: 8,
            box_op: None, unbox_op: None, // identity — already i64
            ..
        },
        "Float" => ResolvedType {
            llvm_type: "float".into(), storage: Native,
            tbaa_node: "Float".into(), bytes: 4, alignment: 4,
            box_op: Some("box.f32.to.i64#"), unbox_op: Some("unbox.i64.to.f32#"),
            ..
        },
        // ... one entry per built-in type
    }
}
```

#### A3. Replace Backend Match Arms with Universe Queries (1 week)

Replace each of these with a universe lookup:

| Function | Location | Current | New |
|----------|----------|---------|-----|
| `llvm_type()` | `emit_toplevel.rs:198` | `match ty { Type::Float => "float", ... }` | `universe.lookup(ty).llvm_type` |
| `tbaa_node()` | `mod.rs:503` | `match ty_str { "float" => 5, ... }` | `universe.lookup(ty).tbaa_node` |
| `adapt_to_i64()` | `emit_stmt.rs:29` | `match r.ty { Type::Float => ... }` | `universe.box_op(ty)` or generate from table |
| `TypeConverter::box_to_i64()` | `builder.rs:477` | same match | same query |
| `TypeConverter::unbox_from_i64()` | `builder.rs:516` | same match | same query |
| `ensure_float_reg()` | `emit_toplevel.rs:226` | `match r.ty { Type::Float64 => ... }` | `universe.storage(ty) == Native` |
| `llvm_type_byte_size()` | `mod.rs:93` | `match t { "float" => 4, ... }` | `universe.lookup(ty).bytes` |
| `primitive_from_name()` | `mod.rs:107` | `match name { "Float" => Type::Float, ... }` | keep — maps names to Type enum |
| `trg_llvm_storage_ty()` | `mod.rs:490` | `match ty { ... }` | `universe.lookup(ty).llvm_type` |

#### A4. Remove Redundant `llvm_type()` / `TypedRegister::llvm()` Duality

`llvm_type()` returns `"i8"` for Bool (C ABI compatible) while
`TypedRegister::llvm()` returns `"i1"` for Bool (LLVM SSA form). After
the universe is the single source, choose one representation. The
recommendation: use `"i8"` for state storage (C ABI compatible) and
`trunc i8 to i1` at the specific sites that need i1 (phi nodes, branch
conditions). The universe can store both:

```rust
pub struct ResolvedType {
    pub llvm_type: String,       // for storage ("i8")
    pub llvm_native_type: Option<String>, // for SSA ("i1" for Bool)
}
```

#### A5. Consolidate adapt_to_i64 and TypeConverter

`adapt_to_i64` (in `emit_stmt.rs`) and `TypeConverter::box_to_i64` (in
`builder.rs`) duplicate the same logic. After the universe is the source
of truth, `TypeConverter` becomes the single conversion dispatcher:

```rust
impl TypeConverter {
    pub fn box_to_i64(builder: &mut LLVMBuilder, val: &str, ty: &Type, universe: &TypeUniverse) -> String {
        let rt = universe.lookup(ty);
        match rt.box_op.as_deref() {
            Some("identity") => val.to_string(),
            Some(op_name) => {
                let intrinsic = Intrinsic::from_name(op_name);
                // Emit the intrinsic call
                emit_intrinsic(builder, &intrinsic, &[val])
            },
            None => val.to_string(), // already i64
        }
    }
}
```

`adapt_to_i64` becomes a thin wrapper that delegates to `TypeConverter`.

### Phase B: User-Facing Operator System (new capability)

**Goal:** Users can declare operator→intrinsic mappings in type bodies.
Types become self-describing. The backend validates operator completeness.

#### B1. Add Operator Properties to TypeDef Syntax (1 week)

Extend the type body syntax to accept operator declarations:

```ebnf
type_body ::= "{" (property | "op" rune type_sig "=" intrinsic ";")* "}"
property ::= ident "=" expr ";"
op_decl  ::= "op" rune ["(" type_expr ")"] "->" type_expr "=" intrinsic ";"
rune     ::= "+" | "-" | "*" | "/" | "=" | "<" | ">" | "<-" | "#" | ":>"
```

Parser changes in `src/parser.rs`:
- In `parse_type_body()` (around line 3248), after parsing regular properties
  and constraints, look for `op` keyword
- Parse `op Add(Float) -> Float = fadd#;`
- Store in a new field `TypeDefBody.operators: Vec<OpDeclaration>`

```rust
pub struct OpDeclaration {
    pub rune: OpRune,
    pub param_type: Option<Box<Expr>>,  // None for unary
    pub return_type: Box<Expr>,
    pub intrinsic: Expr,                // intrinsic name or defn name
    pub span: Option<Span>,
}
```

#### B2. Resolve Operator Mappings in Universe Builder (1 week)

In `TypeUniverse::build()`, after processing structural properties:
1. Parse operator declarations
2. Validate that the intrinsic exists in the Intrinsic enum
3. Invalidate on unsupported intrinsic for target backend
4. Store in a new `ResolvedType.operators: HashMap<(OpRune, TypeKey), OpMapping>`

```rust
pub struct ResolvedType {
    // ... existing + Phase A fields ...
    pub operators: HashMap<(OpRune, Option<TypeKey>), OpMapping>,
}
```

#### B3. Operator Resolution at Compile Time (1 week)

When the typechecker encounters `x + y`:
1. Infer types of `x` (Self) and `y` (param)
2. Query: `universe.lookup(typeof(x)).operators.get(&(OpRune::Add, typeof(y)))`
3. If found → verify return type matches expected
4. If not found → try cross-type composition (compose Box/Unbox + primitive op)
5. If composition fails → compile error: "type X does not support Add(Y)"

The backend never sees "operator lookup" — it just sees the intrinsic
call that the operator resolved to.

#### B4. Cross-Type Composition Engine (1 week)

When `Float + Int` is declared as `op Add(Int) -> Float = compose;`,
the compiler:
1. Looks at `Float.Box` and `Int.Unbox` (both intrinsic calls)
2. Generates: `tmp = Unbox(rhs); result = FAdd(self, tmp);`
3. Stores this as a `Composed` OpMapping

This is the mechanism that gives every type automatic cross-type
operations for free, as long as Box/Unbox are defined.

#### B5. Backend Validation (1 week)

At compile time, after the universe is built:
1. Iterate all types in the universe
2. For each operator mapping, call `intrinsic.supported_on(selected_backend)`
3. If any fails, produce a compile error listing offending types + operators

```rust
fn validate_type_ops(universe: &TypeUniverse, backend: TargetBackend) -> Result<(), Vec<TypeError>> {
    let mut errors = vec![];
    for (name, rt) in &universe.types {
        for ((rune, _), op) in &rt.operators {
            if let OpImpl::Intrinsic(intrinsic) = &op.implementation {
                if !intrinsic.supported_on(backend) {
                    errors.push(TypeError::IntrinsicNotSupported {
                        type_name: name.clone(),
                        rune: *rune,
                        intrinsic: intrinsic.name(),
                        backend,
                    });
                }
            }
        }
    }
    if errors.is_empty() { Ok(()) } else { Err(errors) }
}
```

## 6. Migration Plan

### Phase A Timeline (3-4 weeks)

| Week | Task | Verification |
|------|------|-------------|
| 1 | Add properties to ResolvedType + populate built-in types | `cargo test --lib` — 1318 pass |
| 2 | Replace llvm_type(), tbaa_node(), llvm_type_byte_size() | `cargo test --lib` — same |
| 3 | Replace adapt_to_i64(), TypeConverter, ensure_float_reg() | `cargo test --lib` — same |
| 4 | Consolidate duplicates, remove dead code | `cargo test --lib` — same |

At the end of Phase A, the backend has ZERO type→LLVM match arms. All
type properties come from the universe. No behavioral change.

### Phase B Timeline (4-6 weeks)

| Week | Task | Verification |
|------|------|-------------|
| 1 | Add operator syntax to parser | Parse test cases |
| 2 | Add operator resolution to universe builder | `cargo test --lib` |
| 3 | Build operator→intrinsic resolution for typechecker | Operator tests |
| 4 | Build cross-type composition engine | Cross-type test cases |
| 5 | Build backend validation pass | Error message tests |
| 6 | Migrate built-in types to explicit declarations | Full test suite |

## 7. Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Phase A changes introduce subtle behavior differences | Low | High | Every match arm replacement gets its own test; compare generated IR before/after |
| Operator syntax conflicts with existing syntax | Low | High | Use `op` keyword prefix (distinct from property names) |
| Performance regression from universe lookups | Low | Medium | Universe is a `HashMap<String, ResolvedType>` — O(1) lookup. Cache the lookup in hot paths if needed |
| Backend validation is too strict | Medium | Medium | Always provide inop fallback before rejecting; error only if no path exists |
| Phase B scope creep | Medium | Medium | Strictly limit Phase B to operator→intrinsic mapping; defer "full generic type inference" |
| Cross-type composition is exponential | Low | High | Limit composition depth to 1 box/unbox pair + 1 primitive op |

## 8. Success Criteria

1. ALL backend match arms that match on `Type::Float`, `Type::Int`, etc.
   for LLVM representation decisions are replaced with universe queries.
2. `cargo test --lib` passes with no behavioral change (Phase A).
3. A user can define `type BFloat16 <: Bits { ... Add = bf16_add#; ... }`
   and it compiles with full backend support.
4. `BFloat16 + BFloat16` generates the same IR as writing `bf16_add#(x, y)`
   directly.
5. `BFloat16` compiles for LLVM; errors at compile time for a backend
   that doesn't support `bf16_add#` with no inop fallback.
6. No intrinsic/op mapping ever silently falls back to `i64` opaque
   behavior — it either resolves or errors.

---

## 9. The Clean Backend Vision: Single Match Arm

The refactored backend has exactly **one** type match arm — `Type::universe_key()` —
that maps `Type` enum variants to canonical string names for universe lookup.
After this, every backend decision is a universe query, not a match arm:

```rust
impl Type {
    /// Canonical name for universe lookup. This is the ONLY place
    /// in the codebase where the Type enum is matched for backend
    /// representation decisions.
    pub fn universe_key(&self) -> &str {
        match self {
            Type::Int => "Int",
            Type::Bool => "Bool",
            Type::Float => "Float",
            Type::Float64 => "Float64",
            Type::Int8 => "Int8",
            Type::UInt8 => "UInt8",
            Type::Int16 => "Int16",
            Type::UInt16 => "UInt16",
            Type::Int32 => "Int32",
            Type::UInt32 => "UInt32",
            Type::Char => "Char",
            Type::String => "String",
            Type::Data => "Data",
            Type::Void => "Void",
            Type::Custom(name) => name.as_str(),
            Type::Enum(name) => name.as_str(),
            Type::Sig(name) => name.as_str(),
            _ => "Int",  // safe fallback — already i64
        }
    }
}
```

Every backend function becomes a trivial universe query:

```rust
// BEFORE: 7 scattered match arms, 50+ lines
fn llvm_type(&self, ty: &Type) -> &str { match ty { ... } }
fn tbaa_node(ty_str: &str) -> i32 { match ty_str { ... } }
fn byte_size(&self, ty: &Type) -> u64 { ... }

// AFTER: zero match arms, all from universe
fn llvm_type(&self, ty: &Type) -> &str {
    &self.universe.get(ty.universe_key()).llvm_type
}
fn byte_size(&self, ty: &Type) -> u64 {
    self.universe.get(ty.universe_key()).bytes
}
fn storage_kind(&self, ty: &Type) -> StorageKind {
    self.universe.get(ty.universe_key()).storage
}
```

### Performance: Compile-Time

`Type::universe_key()` compiles to an LLVM jump table — identical cost to the
current 7 scattered match arms. The universe lookup (HashMap<String, ResolvedType>)
is ~10-20ns. For benchmarks, this is noise. If profiling ever shows it matters,
introduce `TypeKey` — an integer index into a `Vec<ResolvedType>`:

```rust
#[derive(Copy, Clone)]
pub struct TypeKey(pub usize);

impl Type {
    pub fn universe_key(&self) -> TypeKey {
        match self {
            Type::Int => TypeKey(0),   // O(1) jump table
            Type::Float => TypeKey(8), // Vec index — no hashing
            Type::Custom(name) => TypeKey::resolve(name), // HashMap for custom
            _ => TypeKey(0),
        }
    }
}

impl TypeUniverse {
    /// O(1) flat array lookup — zero hashing overhead
    pub fn lookup(&self, key: TypeKey) -> &ResolvedType {
        &self.types[key.0]
    }
}
```

### Performance: Runtime (Compiled Briv Programs)

The universe approach produces **byte-for-byte identical** machine code for
built-in types. The `fadd` instruction emitted for `Float + Float` is the
same `fadd` whether the backend got `"float"` from a match arm or a
universe query.

**Why they're identical:** The universe doesn't change WHAT IR gets emitted.
It changes WHERE the backend LOOKS to decide what to emit. The decision
output is the same. The binary has no trace of the decision path.

**User-defined types** get the same optimization treatment as built-in
types when they use the same intrinsics:

```briv
type MyFloat <: Float {}               // inherits — identical codegen
type MyFloat <: Bits { Add = fadd#; }  // same intrinsic — same IR
```

A user-defined type is only slower when the type author makes an explicit
choice that trades performance for semantics:

```briv
type SaturatingU8 <: Bits {
    Add = llvm.uadd.sat.i8#;  // saturating add: different intrinsic, ~1 extra instr
}
type BFloat16 <: Bits {
    Add = inop! { /* software */ };  // software fallback: slower by design
}
```

These aren't limitations of the universe. They're the type author making
explicit trade-offs that the current system hides as opaque magic.

---

## 10. Dynamic TBAA Tree Generation

Today's `tbaa_node()` maps LLVM type STRINGS to hardcoded integer indices:

```rust
fn tbaa_node(ty_str: &str) -> i32 {
    match ty_str {
        "i64" => 1, "i8" => 2, "i32" => 3,
        "i8*" | "ptr" => 4, "float" | "double" => 5,
        _ => 1,
    }
}
```

This is fragile — it matches on LLVM type strings, not Briv types. A
`Custom("MyStruct")` type whose underlying LLVM type is `"float"` gets
TBAA node 1 (Int) instead of 5 (Float).

**After refactoring:** Store the TBAA GROUP NAME in the universe, generate
metadata indices dynamically at module emission time:

```rust
pub struct ResolvedType {
    // ...
    pub tbaa_group: String,  // "Int", "Float", "Bool", "Char", "String"
}
```

At module emission, a pass collects all unique `tbaa_group` values from
types referenced in the current compilation unit and generates:

```llvm
!0 = !{!"Briv"}            ; root
!1 = !{!"Int", !0}          ; all boxed integers
!2 = !{!"Float", !0}        ; all float types (Float, Float64, user float types)
!3 = !{!"Bool", !0}
!4 = !{!"Char", !0}
!5 = !{!"String", !0}
```

A user-defined type declaring `TBAA = Float` automatically gets TBAA node
`!2` — no new match arm, no code change. LLVM's alias analyzer sees it
as "same as Float" and optimizes accordingly.

---

## 11. Risk Analysis & Edge Cases

### R1. Cross-Type Composition Coercion Gap

When composing `Float + Int`, the naive engine does:
`result = Unbox(rhs) → FAdd(self, tmp)`

But `Unbox(Int)` emits `i64` and `FAdd32` expects `float` (f32). Passing
`i64` to `fadd` triggers an LLVM verifier error.

**Solution:** The composition engine must resolve a **coercion path**.
Each type declares `Coercion(to_type) → intrinsic`:

```rust
pub struct ConversionPath {
    pub source_ty: TypeKey,
    pub target_ty: TypeKey,
    pub conversion_op: Intrinsic, // e.g., SIToFP, FPExt, FPTrunc
}
```

The composition engine queries: given `TypeA + TypeB`, where `TypeA.op(Add)`
expects parameter type `Param`, does `TypeB.unbox() → Param` exist? If not,
query for `ConversionPath(TypeB, Param)`.

For `Float + Int`: `Unbox(Int) → i64`, `Convert(i64 → f32) = SIToFP`,
`FAdd(self, converted)`.

This is automatically derivable for all built-in type pairs — no user
declaration needed. User-defined types can override or add conversion paths.

### R2. Opaque Pointers (LLVM 15+)

LLVM 15+ uses opaque `ptr` instead of typed `i8*`, `i64*`. If the universe
stores `llvm_type = "ptr"`, the backend loses context for GEP calculations
and aggregate access.

**Solution:** Add an `IndirectionKind` property:

```rust
pub enum IndirectionKind {
    Scalar,   // points to a scalar value (e.g., i8*)
    Struct,   // points to a struct with named fields
    Array,    // points to a homogenous array
    Opaque,   // opaque pointer — no structure known
}
```

The backend checks this property to decide whether GEP requires field
offsets or array index calculations.

### R3. `TypeKey` for Hot-Path Performance

`HashMap<String, ResolvedType>` lookups add hashing overhead in hot codegen
paths. For deeply nested expressions (e.g., `a + b * c - d / e`), each
type lookup adds ~20ns.

**Solution:** Use `TypeKey` (integer index) instead of `String` keys for
all hot paths. Maintain a `HashMap<String, TypeKey>` for name→index
resolution; codegen uses `TypeKey` exclusively.

```rust
impl TypeUniverse {
    pub fn get_by_key(&self, key: TypeKey) -> &ResolvedType {
        &self.types[key.0]  // O(1), no hashing, no branching
    }
}
```

### R4. Embedded Mode Allocation Strategy

If a type declares `OnExit = __rust_vec_drop#;` (heap deallocation), but the
target is embedded (freestanding, no heap), compilation should fail.

**Solution:** Add `AllocationStrategy` property:

```rust
pub enum AllocationStrategy {
    Stack,  // stack-allocated, no destructor
    Static, // statically allocated, no destructor
    Heap,   // heap-allocated, requires free/drop
}
```

The backend validation pass checks: if `target.freestanding == true` and
any type in the universe has `AllocationStrategy::Heap`, emit a compile error.

### R5. Compile-Time Validation (Safety)

The contract system (`[pre][post]`) constrains intrinsic behavior.
Operator→intrinsic mappings specify implementation. They're orthogonal:

- `Float.Add` uses `fadd#` intrinsic.
- Contract: `[pre: true][post: result == x + y]`.
- The contract checker verifies the intrinsic satisfies the contract.
- The backend emits the intrinsic call.
- If the intrinsic doesn't exist on the target, compilation fails.

No unsafe path exists — every operator maps to a validated intrinsic,
every intrinsic is checked against the target backend before emission.

---

## 12. Refined Implementation Path

### Phase A: Backend Internal Refactoring (3-4 weeks)

| Week | Task | Verification |
|------|------|-------------|
| 1 | Add `Type::universe_key()`, populate built-in types | `cargo test --lib` |
| 2 | Replace `llvm_type()`, `tbaa_node()`, `byte_size()` | Generated IR diff |
| 3 | Replace `adapt_to_i64()`, `ensure_float_reg()`, `TypeConverter` | Generated IR diff |
| 4 | Dynamic TBAA generation, consolidate duplicates, remove dead code | `cargo test --lib` |

### Phase B: User-Facing Operator System (4-6 weeks)

| Week | Task | Verification |
|------|------|-------------|
| 1 | ✅ **Add operator syntax to parser** — DONE | Parse test cases |
| 2 | ✅ **Add operator→intrinsic resolution to universe builder** — DONE | `cargo test --lib` |
| 3 | ✅ **Build cross-type composition engine** — DONE | Cross-type test cases |
| 4 | ✅ **Wire into type-checker and codegen** — DONE | Example `compile` works |
| 5 | 🔲 Build backend validation pass (intrinsic support checking) | Error message tests |
| 6 | 🔲 Build `TypeKey` optimization for hot paths | Benchmark no regression |
| 7 | 🔲 Migrate built-in types to explicit universe declarations | Full test suite |
