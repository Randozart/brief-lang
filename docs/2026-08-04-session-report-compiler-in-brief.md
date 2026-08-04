# Session Report — Compiler-in-Brief (2026-08-04)

**Scope:** the `compiler-in-brief` branch (worktree `../brief-compiler-dogfood`),
P1–P5, merged into main here. Two compiler passes are now written in Brief,
compiled by `briefc` itself, and loaded through the GLUE C ABI at runtime — the
compiler dogfoods the FFI it ships.
**Purpose:** record every finding, decision, and bug from the session so a
future session can reconstruct the reasoning without re-deriving it.

---

## 1. Goal

The compiler's own GLUE FFI (merged earlier in `glue-host-callable`) had no
in-compiler consumer. This plan made the compiler use it: **one Rust compiler
pass rewritten in Brief, compiled to a native library by `briefc`, and linked
in at runtime via dlopen** — the same way a host language calls a Brief bridge.
Two passes shipped: `needs_state` (which exports carry `ptr %state`) and
`soa_reorder` (the AoS → SoA field permutation).

## 2. Phases delivered

| Phase | Commit | What |
|-------|--------|------|
| Plan | `ea6c4687` | `docs/plans/2026-08-04-compiler-in-brief-dogfood-ffi.md` |
| P1 | `5d281c92` | `needs_state_projection.rs`: the tagged flat-preorder handoff |
| P2 | `dc4c9b3b` | `needs_state.bv` matches the Rust reference on the 5-bridge corpus |
| P3 | `ede34733` | root `build.rs` + `brief_pass.rs` dlopen loader (Option 1: runtime dlopen) |
| P4 | `ede34733` | both production call sites route through `brief_pass::compute_export_needs_state` |
| P5 | `a23df1ab` | `soa_reorder.bv` (second pass) + `reader.bv` (shared scanner) + recipe doc |

The pattern: Rust serializes a **tagged Data Brief projection**; Brief decides;
build.rs compiles the pass with a prebuilt briefc; brief_pass.rs dlopens it and
calls `compute(state, proj) -> i64` (the i64's meaning is pass-specific —
bitmask for needs_state, a permutation-buffer address for soa_reorder); the
first build has no briefc yet (self-hosted bootstrap) and every pass falls back
to its Rust reference. Transition tests assert Brief == Rust on a corpus.

## 3. Verified Brief language facts that shaped the passes

- **`when cond { body }` is an if-guard, not a while loop** (interpreter and
  LLVM backend agree). All pass iteration is tail RECURSION.
- A **String param / frgn result is an i64 HANDLE** at a call/store boundary; a
  String in a register is a ptr. `.^Len` and `==` must inttoptr the handle.
- **Dynamic String slices return the whole array** (emit_expr.rs:992); **String
  `+` codegens a register collision**; **`List<String>` element reads return the
  generic `T`**. The passes therefore use `brief_str_substr` + `char_at` frgns
  and re-scan the projection string instead of building collections.
- A stateful export's C signature takes the **state handle first**
  (`<pass>(state, proj)`).
- `"42" as Int` parses (casting-graph lane); `Malloc#` + `Ptr<Int>` indexed
  stores build output buffers; the buffer address returns as Int.

## 4. Real compiler bugs found and fixed (each committed)

1. **Import resolver swallowed parse errors** (`unwrap_or_default`) — now a
   visible warning. This surfaced a systemic `onst` → `const` typo (546×, 21
   `std/os` files), `Slice<T>.prop Size: len` (prop parser needs call syntax),
   and string.bv's legacy `..` slices. (`916b332b`)
2. **Generic struct layouts were silently zeroed** — `List<T>.len` collided
   with `inner.cap` at offset 8. Three interacting causes: `type_size(Ptr)`
   returned 0 via the universe path; the normalizer's slot-sum read raw
   `rt.bytes` (0 for flexible Int/String); re-registering `type Int: #Int`
   wiped the Cast.#* properties. (`768d6bc7`)
3. **`List.init` allocated 16 bytes but advertised cap 16 elements** (overflow);
   `grow(cap)` added, using the `Copy#` memcpy intrinsic. (`768d6bc7`,
   `84a528b9`)
4. **A let reassigned inside a guard demoted to an alloca at the assignment
   site** → LLVM dominance violation. `emit_definition` pre-scans and
   pre-declares entry allocas for reassigned top-level lets. (`84a528b9`)
5. **`.^Len` on a boxed String param/frgn result panicked** (Phase-1b) —
   `is_semantic_string` + `string_ptr` (mirrors the `==` operand fix).
   (`ef5c476f`)
6. **`let_binding_allocas` leaked across functions** (reg numbers rewind per
   function; the manual clears missed it) — replaced with `clear_locals()`.
   (`ef5c476f`)
7. **`expr_needs_state` and the needs_state projection dropped wrapping Expr
   kinds** (Cast/MethodCall/Reflect/Index/Slice/AddrOf) — a cast-wrapped call
   to a regular defn produced a STATELESS export shim that referenced `%state`
   (opt: "use of undefined value '%state'"). Both now recurse. (`a23df1ab`)
8. **Dynamic String slicing had no runtime implementation** — added
   `brief_str_substr` (substring) and `brief_str_char_at` (Int byte, no
   allocation) to brief_rt.c. (`ef5c476f`)

## 5. The "heap corruption" that wasn't

A C driver called the stateful `needs_state_compute` with ONE argument; the
projection landed in `%state` and `%arg0` was garbage, so the meld read a
random length across runs (60 / 5 / 3). With the correct two-argument call
(`needs_state_compute(state, proj)`), the pass is deterministic. Recorded in
BUGS.md as a correction — the lesson: a stateful export's C signature takes
the state handle first; the `brief bindings` header is the source of truth.

## 6. Verification

- `needs_state_compute` == `compute_export_needs_state` on boundary (0),
  node_bridge (31), cancel (1), rank (2), bench (2) — asserted by
  `tests/c_driver_needs_state.rs` and `brief_pass.rs`'s unit test (which also
  asserts the dlopen path was USED, not the fallback).
- `soa_reorder_compute` == `reorder_fields` on `nbody_newton.bv` — asserted by
  `soa_reorder.rs::brief_reorder_matches_reference_on_nbody` and
  `brief_pass.rs::soa_pass_matches_reorder_fields`.
- All c_driver glue tests (boundary/node/callback/cancel/library) pass through
  the dlopen'd passes.
- Full `cargo test`: 1569 passed, 0 failed.

## 7. Known gaps (see BUGS.md)

- Dynamic String slices still return the whole array (worked around via the
  `brief_str_substr` frgn); a real fix would make slices construct substrings.
- String `+` codegens a register collision (the passes avoid it).
- `List<String>` element reads return the generic `T` (the passes re-scan
  instead of reading collections).
- string.bv does not type-check (legacy bodies: `StrBytes#`, `List+`, method
  calls on String) — surfaced by the error-not-swallowed fix; the in-Brief
  compiler's `main.bv`/backends (which import std/string) need the repair.
- The `.so` paths are embedded at compile time — a moved checkout rebuilds them.

## 8. Recipe

`docs/architecture/compiler-in-brief.md` is the how-to: the model, a 6-step
"add a new pass" checklist, the verified language facts, and the Rust-side
codegen rules learned the hard way. A third pass would justify generalizing the
section-line writer in the Rust serializers into a shared helper.
