# Callbacks — host → Briv → host

**Date:** 2026-08-03
**Status:** Implemented (C path verified; Python/Node wrappers pending)
**Request origin:** RamKumar Revanur (MAKER.AI) — "a means to update a first
level primitive — progress bar updates in client code based on async updates
of status from Briv code." C#-events/delegates-style.

## What was built

A host passes a function pointer into an exported Briv function; Briv calls
it back for first-level-primitive updates (e.g. per-file progress).

```briv
export defn apply(cb: fn(Int) -> Int, x: Int) -> Int {
    term CallPtr#(cb, x);
};
```

Host side (C):
```c
int64_t doubler(int64_t x) { return x * 2; }
int64_t r = apply(doubler, 21);   // → 42
```

## Pieces

1. **Syntax:** `fn(P1, P2) -> R` type annotation in params (parser). `R`
   defaults to Void when omitted. `Type::Function` existed in the AST; only
   the surface was missing.
2. **`CallPtr#(cb, args...)` intrinsic** — explicit `#` marker per rule 2
   (no hidden function-pointer calling behind ordinary syntax):
   - typechecker infers the return from `cb`'s fn type;
   - LLVM emits `inttoptr i64 %cb to ptr` then `call <ret> %cb(args...)`
     (a fn VALUE lowers to `ptr` under opaque pointers; the param is
     ptrtoint'd at function entry);
   - interpreter: no host callback in-process (CallPtr# has no value there).
3. **Export ABI:** a `fn(...)` param crosses as an opaque function pointer.
   `format_type` renders `fn(P)->R`; `resolve_protocol`/`fn_pointer_parts`
   build the per-language ABI; the config's `fn_param_decl` template embeds
   the name where required (C: `int64_t (*cb)(int64_t)`).
4. **Demo:** `examples/glue-host/callback.bv`.
5. **Test:** `tests/c_driver_callback.rs` (toolchain-guarded) — C passes
   `doubler`/`plus_one`, Briv calls them, returns 42/42.

## Status / follow-ups

- C path verified end-to-end (`brivc build --library` + C driver).
- Python (`ctypes.CFUNCTYPE`) and Node (`ffi-napi`) wrapper templates do not
  yet render fn-typed params — the callback type must be added to their
  config templates. The metadata (`bridge-exports.dbvl`) already carries the
  fn-typed param for any language to consume.
- Interpreter lacks a real function VALUE (lambdas are `Value::Void`), so
  CallPtr# is a runtime no-op there — the reference for host-boundary
  callbacks is the C round-trip test.

## Undo

- Remove the `fn(...)` arm from `src/parser/types.rs::parse_type` +
  `parse_fn_type`, the `CallPtr#` signature/intrinsics arms, and the
  `Type::Function` → `ptr` LLVM lowering.
