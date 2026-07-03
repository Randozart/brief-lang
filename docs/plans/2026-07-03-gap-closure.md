# Gap Closure — Safe `void*` Cleanup

**Date:** 2026-07-03  
**Status:** Plan — addressing deferred items from Phases 1-6

---

## Gap A — Layout Shape Caching (Phase 3 deferred)

**Problem:** Each call to a layout-parameterized function monomorphizes per call site.
`block_copy(x, y)` and `block_copy(a, b)` on two different types with the same
(8, 8) layout produce two identical compiled variants.

**Fix:** Add a layout shape cache: `HashMap<(u64, u64), String>` mapping
(bytes, alignment) → compiled function name. When emitting a call to a function
with layout parameters, check the cache first.

**Easy path:** Just verify the existing spatial intrinsics already avoid
monomorphization — they're direct `@llvm.memcpy` calls, not generic functions.
The shape cache is only needed for user-written generic functions with layout
constraints, which is an uncommon pattern. Defer to when a user actually writes one.

→ **Architecturally resolved:** Spatial intrinsics emit direct @llvm.* calls with
no intermediate function wrappers. No duplication to cache. The cache will be
added when `T: bytes == S` user syntax is implemented (then it's a 30-line
`HashMap<(u64, u64), String>` lookup in one file).

---

## Gap B — Module Boundary Enforcement (Phase 4)

**Problem:** Opaque handles rely on social convention. `Ptr<Bits @/24>` can be
cast to `Ptr<DbConnection>` from any module.

**Fix:** When an `as Ptr<ConcreteType>` cast is validated, check whether the
cast is within the module that defines `ConcreteType`. Add a `defining_module`
field to `ResolvedType` in TypeUniverse.

**Implementation:**

1. Add `defining_module: String` to `ResolvedType` (defaults to `"builtin"`)
2. Set it during `TypeUniverse::build()` for TypeDefs from the current program
3. In `is_cast_valid`, when casting `LayoutPtr → Ptr<CustomType>`, check
   that the current module matches `defining_module`
4. Add a test: define `DbConnection` in one module, cast from another → blocked

---

## Gap C — EOR Optimization (Phase 6)

**Problem:** `(val as Int) * factor as Meters` where `meld Meters <:> Int`
produces three IR instructions (cast, multiply, cast) instead of one (multiply).

**Fix:** Add a rewrite pass in the desugarer that detects the EOR pattern and
elides redundant casts. Pattern: `Cast(BinaryOp(Cast(a, Int), Cast(b, Int)), T)`
where `T <:> Int` or `T <:> Float`.

**Implementation:**

1. Add `rewrite_eor(expr: &Expr, tu: &TypeUniverse) -> Expr` in `ast.rs`
2. The function detects the EOR pattern and returns the inner BinaryOp with
   the outer type applied directly: `BinaryOp(Mul, a, b)` with `ty = T`
3. Call it in the typechecker's `resolve_type` path
4. LLVM backend emits native arithmetic without redundant casts
5. Add tests: compare IR output of EOR-optimized vs non-EOR code

---

## Gap D — `emit_indirect_call` Arg Marshalling

**Problem:** `emit_indirect_call` (call.rs) doesn't pass `%state`, doesn't
handle float return types, and doesn't marshal arguments by type.

**Fix:** The indirect call should replicate the same argument marshalling as
the internal call path (lines 157-262 of call.rs): pass `%state` as first arg,
handle Bool/Char/Float types, handle float return types.

**Implementation:**

1. Emit `ptr %state` as the first intrinsic argument for indirect calls
2. Check `let_binding_types` for the fn-ptr's parameter types to marshal
3. Handle `Type::Float` return — bitcast back from i64 box
4. Add tests: indirect call to float-returning function

---

## Gap E — Flat Control Flow Audit

**Problem:** Phase 1's match arm additions to annotator.rs, memory_spec.rs,
and proof_engine.rs weren't refactored for flatness.

**Fix:** Review each file for nesting > 2 levels. No code changes needed for
the additive arms (they're all 1-2 levels), but verify.

---

## Implementation Order

1. **Gap D** — `emit_indirect_call` marshalling (most likely to cause bugs)
2. **Gap C** — EOR optimization (cleanup, visible in IR)
3. **Gap B** — Module boundary enforcement (safety)
4. **Gap E** — Flat control flow audit (verification)
