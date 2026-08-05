# Briv: Plugin / Macro Syntax Rework + The Bits Model (String as `#String` Protocol)

**Date:** 2026-08-01
**Status:** Approved. Part A (macro rework) approved at start; Part B (the bits
model — String as `#String` protocol) added and approved 2026-08-01. **Merged**
(see `docs/2026-08-01-plugin-macro-rework-completion.md`).
**Worktree:** `../briv-compiler-plugin-rework` (new, from `main` `d6c6c818`)
**Baseline worktree:** `../briv-compiler-baseline` — synced to `d6c6c818` on 2026-08-01

**Plan map:** merged. The master plan —
[2026-08-01-consumptive-operators-lifetime-and-c-surface.md](2026-08-01-consumptive-operators-lifetime-and-c-surface.md) —
builds on this merged base (the `Print#` consolidation, the consumptive `~op`
operators, the stream symbols, the free-check).
**Part B is in the queue NOW:** Phase B0 is a hard prerequisite for completing
Part A Phase 1 (the format-demo clang error) and for Part A Phases 3-4.
Phase B1 is a hard prerequisite for Part A Phase 3 (`entry_cmd() == "cmd"`
needs content-based String equality). See §5.6 and the Execution order (§6).

---

## 1. Goal

Rework the plugin / macro surface of Briv to match the language's disclosure
principle (AGENTS.md #2):

1. **Remove `[#]`** as a special-cased entry precondition in the parser. It is
   dead weight — parsed into `Contract.is_entry` but consumed by no backend or
   analysis pass.
2. **Replace it with `entry!("command")`** — a user-facing macro, unwrapped by a
   compiler plugin, that inserts a CLI-command precondition, creates top-level
   bindings/helpers to fetch that command, and composes a one-shot firing guard.
3. **Add `args!("flag")`** — the companion macro for CLI flags (Bool presence and
   typed value retrieval).
4. **Standardize macro naming**: user-facing `!` macros are lowercase /
   snake_case (`println!`, `print!`, `get_env!`, `get_env_int!`, `entry!`,
   `args!`). PascalCase is reserved for compiler-knowns (`Sqrt#`, `Tag$`, `#Int`,
   `.^`). PascalCase `!` intercepts become compile errors with a rename hint.
5. **`println!` / `print!`** gain Rust-style curly-brace substitution (`{}`,
   `{n}`) with positional arguments, rewritten at compile time to typed print
   intrinsic calls — zero runtime formatting.
6. **Flat-scripting plugin** synthesizes a one-shot opening node. **No generated
   node ever uses `[true]` as a precondition** (continuous-fire eligibility).
   Instead a top-level `Bool` guard is flipped at the end of the inserted node.
7. **Enforce the concurrency gate** (AGENTS.md #21) — an eligible-to-fire pair
   with no XOR read-write overlap and no `async`/`sync<group>` classification is
   a hard compile error.
8. **Research the FFI "native performance" regression** — full audit of the
   `frgn` Inline path, print/env intercept resolution, and the bridge path, with
   a documented baseline.

**Part B (added 2026-08-01) — The Bits Model.** Make `String` the `#String`
protocol, not a primitive with a compiler-invented representation:

9. A String value is a **pointer to a length-prefixed buffer** `[len: i64][bytes]`
   in every type-claiming site. Value moves (param/return/store/FFI) pass the
   address; **operands deref by default** through one central helper.
10. **Encoding is a cast property**: `#Bit → #String` default = UTF8 wrap;
    sub-protocols override via `CastFrom(#Bit)`; the unread `!> encoding`
    metadata is removed. UTF8 exists nowhere as a symbol.
11. **Length is an overloadable protocol op** (`prop Size` / `prop Bytes`);
    `Bytes` default = O(1) header read; `Size` default = UTF8 char count (runtime).
12. **Retire the competing representations** — fat pointer `{i64,i64}`, `i128`
    state slots, the dead SSO layer, and the `is_string_like` structural
    heuristic — and **fix String `Eq`/`Ne` to compare content** (currently
    address-based in both interpreter and backend, which breaks
    `entry_cmd() == "cmd"`).

---

## 2. Operating contract

Every step honors: contract-first (#1), no hidden special treatment (#2),
interpreter-is-reference (#4), additive-only match arms (#5), ALWAYS FINISH (#6),
never discard uncommitted work (#7), tests-or-it-doesn't-exist (#8), no
prototyping (#9), plan-with-benchmarks (#11), baseline worktree A/B (#11b),
stdlib-is-the-extension-mechanism (#13), no compiler knowledge of specific types
(#14), full provenance tracking (#15), DRY (#16), migrate-when-touched (#17), no
type-name matching (#18), measure-before-build (#19), delimiter semantic load
(#20), no implicit concurrency (#21), and the Performance Recovery Protocol.

---

## 3. Current-state research findings (verified 2026-08-01)

| Area | Finding | Location |
|------|---------|----------|
| `[#]` parsing | Special-cased in `parse_contract`; sets `Contract.is_entry` | `src/parser/definitions.rs:872-959` (branch at `:880-908`) |
| `is_entry` consumption | **None.** Only display, beast serialization, tests | `src/ast/display.rs:476`, `src/beast/serialize.rs:52,81-83,365`, `src/ast/top.rs:137`, `src/ast/mod.rs:8` |
| `is_entry` constructor sites | ~50 `is_entry: false` (mechanical sweep) | `backend/llvm/tests.rs`, `backend/mod.rs:645`, `backend/circt.rs:795`, `backend/spirv/mod.rs:66`, `backend/webstack.rs:1108`, `fuzzing/*`, `assertion_verify.rs`, `reactor.rs`, `plugin/intrinsics.rs`, `analysis/*`, `hardware_validator.rs` |
| `defn main` | Emitted as `briv_main` (`emit_toplevel.rs:1133`) but **never called** by the runtime — reactor only fires reactive `txn`/`node` | `src/backend/llvm/emit_toplevel.rs:1133` |
| Implicit entry wrap | `wrap_implicit_entry` is an empty placeholder | `src/parser/definitions.rs:1210-1213` |
| Plugin intercept syntax | `name!(args)` postfix → `Expr::PluginIntercept` | `src/parser/expressions.rs:319-339`, `src/ast/expr.rs:80-86` |
| Intercept rewriting | Rust plugins at Parsed stage: `Print`/`PrintLn` → `PrintInt#`/`PrintStr#`/`PrintFloat#`/`PrintChar#`; `GetEnv`/`GetEnvInt` → stdlib `get_env`/`get_env_int` | `src/plugin/print_plugin.rs`, `src/plugin/env_plugin.rs` |
| Plugin registration | `EnvPlugin` + `PrintPlugin` hard-registered | `src/compile.rs:861-862`; `config/targets.toml [".bv"].plugins = ["prelude","env","print"]` |
| Typechecker intercept arm | Recognizes `GetEnvInt`, `GetEnv`, `GetEnvOrDefault`, `PrintLn`, `println`; unknown intercepts → error | `src/typechecker/mod.rs:544-557` |
| Interpreter on intercepts | `Expr::PluginIntercept` → runtime error (no plugin pass before eval) | `src/interpreter/eval.rs:136` |
| CLI args | **None.** `main` is `define i32 @main()` with no args in all loop-engine paths | `loop_engine/counter.rs` (×5), `ssa.rs` (×4), `mod.rs` (×1) |
| Env vars | briv_rt.c provides `__getenv_briv` / `__getenv_int`; `frgn` wrappers in `lib/std/ffi/env.bv` | `lib/runtime/briv_rt.c:127-157`, `lib/std/ffi/env.bv` |
| FFI dispatch | `frgn` `.c/.cpp/.rs` → Inline (compile+link+LTO); `#System`/`#Link<x>` → Inline direct `-l`; GLUE-mapped ext → Bridge; native `.o/.so/.a` → Inline | `src/analysis/frgn_dispatch.rs:143-219` |
| Print codegen | `PrintInt#` → `call i64 @__print_int` (briv_rt.c, `always_inline` + LTO) | `src/backend/llvm/intrinsics.rs:65-90`, `lib/runtime/briv_rt.c:178` |
| Concurrency gate | Documented, **not enforced** — only auto-selects Sequential/Parallel | `src/backend/llvm/strategy.rs:50-102`, `docs/architecture/concurrency-and-modifiers.md` |
| XOR helpers | `collect_assigned_identifiers` / `collect_read_identifiers` | `src/backend/mod.rs` |
| SAT check | `check_satisfiable(a, b) -> bool` | `src/proof_engine/mod.rs:291` |
| Print format today | No formatting — `PrintLn!(x)` prints a single value | `src/plugin/print_plugin.rs:227-250` |

**Critical consequence:** removing `[#]` is behavior-neutral (nothing consumes
`is_entry`), but the flat-scripting opening node is **genuinely unimplemented**
and `defn main` is **dead code** — Phase 4 must make scripts actually runnable.

---

## 4. Architecture decisions (locked)

### 4.1 Naming convention

| Category | Convention | Examples |
|----------|-----------|----------|
| User-facing `!` macros | lowercase / snake_case | `println!`, `print!`, `get_env!`, `get_env_int!`, `entry!`, `args!` |
| Compiler-known intrinsics | PascalCase + `#` | `Sqrt#`, `PrintInt#`, `Malloc#` |
| Compile-time `$` intrinsics | PascalCase + `$` | `Tag$`, `Insert$`, `StrReplace$` |
| Hashwords | `#PascalCase` | `#Int`, `#String<UTF8>`, `#System` |
| Reflection | `.^` / `.^^` | `x.^Len`, `x.^^Size` |
| Compile-time fn definitions | lowercase + `$` prefix | `$defn`, `$txn`, `$let`, `$const` (unchanged) |

**Enforcement:** any `Expr::PluginIntercept` whose name is not in the known
lowercase set is a compile error at the typechecker. If the name is
PascalCase (`PrintLn`, `GetEnvInt`, ...), the error includes a rename hint
(`PrintLn!` → `println!`). This is the migration path; no transitional alias is
kept.

### 4.2 `println!` / `print!` — Rust-style formatting (Phase 1)

Grammar (format literal):

```
format      ::= ( text | placeholder | escape )*
text        ::= any char except '{' '}'
escape      ::= '{{' | '}}'                    // literal { }
placeholder ::= '{' index? '}'
index       ::= decimal                        // {0}, {1}, ...
```

Expansion (compile-time, `print_plugin.rs`): a `println!("...", a0, a1, ...)` /
`print!("...", a0, a1, ...)` intercept is rewritten to a `Statement::Block` of:

1. For each leading/interspersed literal segment (non-empty): `PrintStr#(seg)`.
2. For each placeholder: the corresponding argument, printed by type:
   `PrintInt#` / `PrintFloat#` / `PrintStr#` / `PrintChar#` (protocol-derived
   dispatch via `TypeUniverse`, rule #18 — the current name-based
   `kind_from_type`/`kind_from_expr` is replaced).
3. `println!` appends `PrintChar#(10)`.

Errors (compile-time):
- Placeholder index `{n}` with no argument `n` → "format argument {n} out of
  range in println!".
- More arguments than placeholders → allowed (Rust-compatible) — unused
  trailing args are a compile warning.
- `{` not followed by `}` or digits → malformed format error.

No runtime formatting machinery — the block IS the output. `println!()` with no
args emits only `PrintChar#(10)`.

### 4.3 `entry!` and `args!` — placement and expansion (Phase 3)

**Placement:** inside contract brackets, as a Bool expression:

```briv
node build [entry!("build")][result == 0] { ... }
txn  serve [entry!("serve")][running == false] { ... }
```

**`entry!("<cmd>")` expansion** (for the decorated node/defn `N`):

1. Inject a top-level one-shot guard (deduped per command; `__` prefix is
   compiler-reserved):
   ```briv
   let __entry_<cmd>_done: Bool = false;
   ```
2. Rewrite the `entry!` expression in the contract to:
   ```briv
   entry_cmd() == "<cmd>" && !__entry_<cmd>_done
   ```
   composed into `N`'s existing precondition with `&&` (precedence: parenthesize).
   **`[true]` is never used.**
3. Append to the end of `N`'s body: `__entry_<cmd>_done = true;` — the node fires
   at most once. **One-shot by default.** A deliberately persistent node declares
   its own explicit contract (its own counter/state guard) alongside `entry!`.
4. If `N` is a `defn` (non-reactive), the plugin also injects a reactive wrapper:
   ```briv
   let __entry_<cmd>_done: Bool = false;
   node __entry_<cmd> [entry_cmd() == "<cmd>" && !__entry_<cmd>_done][__entry_<cmd>_done] {
       <call to N>;
       __entry_<cmd>_done = true;
   };
   ```
   This is the "helper node" path — CLI-addressable defns become subcommands.

**`args!("--flag")` expansion:**

```briv
let arg_flag: Bool = __argv_has("--flag");
```

Inserted as a top-level state field initializer, and the `args!` expression
rewrites to the identifier `arg_flag`. **`args!` reads snapshot state only** — no
guard, no flip (the enclosing node's one-shot guard governs firing).

**`args!("--flag", T)` expansion (typed value):**

```briv
let arg_flag: T = __argv_value_as::<T>("--flag");
```

The type argument `T` is parsed from the second intercept argument (an
`Expr::Identifier` naming the type). The plugin type-checks the conversion
(Int/Float/String/Bool) and rewrites the expression to `arg_flag`.

**Top-level binding naming / collisions:** helper names are
`arg_<sanitized-flag>` where `<sanitized-flag>` = flag with leading `-` stripped
and remaining `-`→`_` (`--out` → `arg_out`). If a user binding already exists,
the plugin errors (no silent shadowing).

**Stdlib (rule #13):** `lib/std/cli.bv` (new) provides the FFI + helper surface:

```briv
frgn __argv_count() -> Int        from "lib/runtime/briv_rt.c" fallback 0;
frgn __argv_get(i: Int) -> String from "lib/runtime/briv_rt.c" fallback "";
frgn __argv_has(flag: String) -> Bool   from "lib/runtime/briv_rt.c" fallback false;
frgn __argv_value(flag: String) -> String from "lib/runtime/briv_rt.c" fallback "";

defn entry_cmd() -> String { term __argv_command(); };
defn arg_present(flag: String) -> Bool { term __argv_has(flag); };
```

The entry plugin ensures `import "std/cli.bv"` exists (like the prelude injects
stdlib imports) before rewriting intercept expressions.

**Command semantics (precise):** `__argv_command()` scans `argv[1..]`, skips
tokens beginning with `-`, and returns the first remaining token; `""` if none.
So `<prog> --verbose build` → `"build"`.

**Env-var fallback:** `entry_cmd()` also honors `$BRIV_ENTRY_CMD` if set (test /
embedded path without argv); documented in the runtime helper. This is the sole
environment dependency and is additive.

### 4.4 CLI runtime capture (Phase 3)

- Emitted `main` changes from `define i32 @main()` to
  `define i32 @main(i32 %argc, ptr %argv)` in **every** loop-engine main
  emission site (`counter.rs` ×5, `ssa.rs` ×4, `mod.rs` ×1).
- At the top of `main`, store into module globals:
  `@__briv_argc = internal global i32 0`, `@__briv_argv = internal global ptr null`,
  via `store i32 %argc, ptr @__briv_argc` / `store ptr %argv, ptr @__briv_argv`.
- briv_rt.c gains: `__argv_count`, `__argv_get`, `__argv_has`, `__argv_value`,
  `__argv_command` (reading the globals; `extern int64_t __briv_argc; extern void* __briv_argv;`).
- **Scope:** native (LLVM) targets only. Non-native backends (WASM/SPIR-V/Webstack)
  receive a compile-time warning if `entry!`/`args!` are used on a target without
  argv support, and the helpers degrade to their fallbacks (documented behavior,
  not silent).

### 4.5 One-shot script node (Phase 4)

New `src/plugin/script_plugin.rs` (Parsed stage). When a `.bv` has **bare
top-level statements** (`TopLevel::Statement`), `TopLevel::Constant`, or
`TopLevel::Let`) and **zero** explicit `defn`/`txn`/`node`:

```briv
let __script_done: Bool = false;
node __script_main [__script_done == false][__script_done] {
    <script statements, in order>
    __script_done = true;
};
```

- Precondition `[__script_done == false]` is true exactly once; the final flip
  makes it false afterward. **`[true]` is never emitted.**
- The guard is read by the reactor's per-tick precondition check → live, no DCE.
- `defn main` wiring: if a `defn main()` exists (no explicit `entry!`), the
  plugin synthesizes the same one-shot node calling `briv_main()` once, fixing
  the current dead-code gap.
- Naming: `__script_main`, `__script_done` are compiler-reserved; collision with
  a user top-level binding is a compile error (not silent shadowing).

### 4.6 Concurrency gate (Phase 3)

New `src/analysis/concurrency_gate.rs` (frontend-computed per the
frontend-driven-dispatch pillar; invoked from `compile.rs` after typechecking).

For every unordered pair of **reactive** txns `(A, B)`:

1. `sat = check_satisfiable(pre_A, pre_B)` (`src/proof_engine/mod.rs:291`).
2. `xor_overlap` = `(A.writes ∩ (B.reads ∪ B.writes)) ≠ ∅` OR
   `(B.writes ∩ A.reads) ≠ ∅` (via `collect_assigned_identifiers` /
   `collect_read_identifiers`).
3. If `!sat` OR `xor_overlap` → pair is safe without classification (mutually
   exclusive, or sequential-by-dependency). Continue.
4. Else (eligible to fire together): the pair must be classified —
   both `async` (explicit simultaneous firing) or both `sync<group>` (same
   group barrier). Otherwise → **hard compile error**:

   ```
   error: nodes A and B can fire together; declare 'async' on both or
   'sync<group>' on both.
   ```

Generated entry/script nodes are **never** `async` and **never** `sync<group>`.
Consequences:
- Two `entry!` nodes with mutually exclusive commands (`cmd == "a"` vs
  `cmd == "b"`) → `pre_A ∧ pre_B` UNSAT → legal subcommand dispatch.
- An entry/script node overlapping a user node with no XOR dependency →
  gate demands classification; since generated nodes cannot be classified, the
  program is **denied** unless the developer restructures (the intended behavior:
  no implicit concurrency).
- **Existing programs** (examples/benchmarks) with multiple auto-firing nodes are
  audited in this phase and reclassified with explicit `async`/`sync<group>`
  where concurrent firing is intended, or restructured. This is a first-class
  part of the phase (no silent breakage; every change is reviewed).

### 4.7 Additive-only rule (#5)

No existing optimization match arm is modified. New behavior is added as new
match arms / new plugin passes. The `_ => return None;` / `_ => Err(...)`
fallthroughs remain unchanged except where a *diagnostic* is improved (error
messages, never semantics).

---

## 5. PART B — The Bits Model: String as `#String` Protocol

### 5.1 Motivation

The compiler treats `String` as a primitive with a compiler-invented
representation — and the invention has **four contradictory answers** to "what
is a String value?". Depending on which code path asks, a String is a `ptr`, an
`i64` handle, a `{ i64, i64 }` fat pointer, or an `i128`. This is not cosmetic:

- The first real String-typed FFI call (`PrintStr#` on a format-string literal)
  fails at clang link time: the call passes a `ptr`/`i64` register but the
  declare says `{ i64, i64 }`.
- **String equality is address-based** in both the interpreter (`eval.rs:332`,
  `lv.as_i64() == rv.as_i64()`) and the backend (`emit_expr.rs:2437-2454`,
  `icmp eq` on the raw register). Two equal-content strings at different
  addresses are `!=`. This silently breaks `entry_cmd() == "build"` (Part A
  Phase 3) — a hard cross-part dependency, not a style preference.
- The SSO path (`feature_sso_strings`, hardcoded `false` in
  `main.rs:324,373` and `mod.rs:778`) is dead code that still drags a
  `{ i64, i64 }` ABI claim and a structural heuristic (`is_string_like`: "any
  `{Int,Int}`-shaped type is a string") along.
- `lib/std/types/bootstrap.bv:79-86` declares `data`/`len` fields and an
  `!> encoding: "UTF-8"` metadata that **nothing reads**
  (`type_universe/mod.rs:89,285-287`).

The fix is to stop inventing a representation and let String be what it
actually is: a **protocol** (`#String`) whose concrete reality is a pointer to
a length-prefixed buffer `[len: i64][bytes]`. The compiler represents the value
honestly (a pointer), derefs operands by default, routes encoding through casts
(default UTF8), and exposes length as an overloadable op. This is the
mainstream, performant design (C++ `std::string`, Python, Java, Swift), not a
purity exercise: it matches the backend's actual internal representation and
the runtime's `int64_t`-handle contract, and it **removes** conversion churn
(`ptrtoint`, `i128` loads, `extractvalue`, `inttoptr`) rather than adding it.

### 5.2 Target model (locked)

1. **`type String: #String;`** — no fields, no props, no `encoding` metadata.
   The protocol carries defaults; sub-protocols overload. (Props move to length
   ops, B3.)
2. **A String value is a pointer** to `[len: i64][bytes]`. The value register
   is the address (`ptr`); state slots store it as an `i64` machine word (via
   the existing Ptr-state `ptrtoint`/`inttoptr` adapters — String follows the
   `Ptr<T>` pattern). Matches the runtime contract: `briv_str_to_c(int64_t)`
   (`lib/runtime/briv_rt.c:48-102`), `__print(int64_t)`, `__print_str(int64_t)`
   — the `int64_t` *is* the address.
3. **Value moves never deref; operands deref by default.** Params, returns,
   stores, FFI args are the address. Ops/casts on `#String` operands deref
   through **one central helper**, read the header for region bounds, and
   operate on `bytes[0..len)`. The header is bookkeeping, never part of the bits.
4. **Casts change interpretation, never memory.** **`#String → #Bit`** = the
   buffer pointer (content view; zero instructions — the value already is the
   pointer). **`#Bit → #String`** is the encoding door: default = wrap raw
   bytes as UTF8 (the bytes already *are* UTF8; wrapping is the only act).
   Sub-protocols override via the existing `CastFrom(#Bit)` machinery
   (`graph.rs:97`, `mod.rs:1020`, `emit_expr.rs:2927`).
   **The header is never part of the bits.** In both directions the cast moves
   the address unchanged; a `#Bit → #String` wrap does NOT inherit a header
   from the bits — it *materializes* one by construction (C-side length scan,
   as `briv_cstr_to_briv` does). If the source already was a `#String`, the
   address still points at a header-prefixed buffer, so the header stays
   *reachable* for region ops — but it is carried by the buffer's layout, not
   by the value, and never included in the bits the cast interprets.
5. **Length is an op** (`prop Size` / `prop Bytes`): `Bytes` default = header
   read (O(1)); `Size` default = UTF8 char count (runtime). Overloadable by
   subtypes. This is the same category of knowledge as `#Int` hardcoding `add`
   — protocol default behavior, not type-name matching (rule #14).
6. **Length-prefixed buffers are mandatory.** Null-terminated C strings are
   explicitly rejected: O(n) length on every access is the actual hot-loop
   killer that pushed the industry to length-prefixing.
7. **`Slice<T>` stays a `{ ptr, i64 }` fat pointer for now** — it is a sequence
   view, a separate concern from String. Deferred (not in this plan).
8. **Flexible vs fixed width (the width rule).** `Int`, `UInt`, `String`, and
   all types without explicit `!> bits`/`maxbits`/`minbits` metadata are
   **one machine word** (`int_bits`, derived from the target data-layout
   pointer width or `--int-bits`); `Int32` is always 32 bits and `Int64` always
   64 (fixed types are absolute). `Int` carries no bits metadata and has a
   *derived* width. String is flexible for the same reason its value is a
   pointer: its storage width must follow the machine word on every target, so
   its primordial is `(bytes=0, min_bits=0, max_bits=0)` exactly like Int/UInt
   (`type_universe/mod.rs`), and `type_size` returns the pointer word (8) as
   the conservative default for `Cast.#String` (`backend/llvm/types.rs`). This
   is the *uniform* rule — String gets no special-cased width, and a fixed
   `bytes: 8`/`max_bits: 64` primordial would be a cross-target lie on 32-bit
   targets. Documented in `docs/architecture/backend-type-dispatch.md`
   (Flexible vs Fixed Width) and `spec/SPEC.md`.

### 5.3 Current-state findings (verified 2026-08-01)

| Type-claiming site | Claims String is | Status |
|---|---|---|
| `llvm_type` SSO-off branch (`emit_toplevel.rs:323-328`) | `ptr` | honest — the one correct site |
| `protocol_llvm_type` (`mod.rs:366-384`) | `{ i64, i64 }` | never produced; breaks frgn declares |
| casting graph `#String` (`graph.rs:273,285-286`) | `{ i64, i64 }` | never produced |
| `push_field_type` (`mod.rs:883-910`) | `i128` | wrong; fixed to `i64` in the working tree |
| SSO emission (`emit_expr.rs:1139-1220`, shim `:1856-1883`) | `{ i64, i64 }` | dead (`feature_sso_strings` hardcoded `false`) |
| `is_string_like` (`type_universe/mod.rs:289`) | "{Int,Int} shape" | structural heuristic propping up the fat pointer |
| actual registers (literals, state, runtime) | `i64` handle / `ptr` | the real representation |
| `bootstrap.bv:79-86` | `{data,len}` + `encoding` | layout claim + unread metadata |

**Verified consequences:**

- **Format-demo clang error** (the Part A Phase 1 blocker): `brivc build`
  succeeds, but `clang` rejects `'%t4' defined with type 'i64' but expected
  'ptr'` on `call i64 @__print_str(ptr %t4)`. The IR also shows
  `declare void @__print_str({ i64, i64 })` — sourced from the dead
  `frgn __print_str(msg: String) -> Void` at `lib/std/ffi/io.bv:9` — and
  `load i128` for String state fields.
- **Address-based equality** (latent semantic bug): interpreter `eval.rs:332`,
  backend `emit_expr.rs:2437-2454`. Breaks `entry_cmd() == "cmd"` (Phase 3).
- **`encoding` metadata is unread** — removing it is behavior-neutral.
  Encoding variants already ride hashwords in op signatures (`#String<UTF8>`)
  and `CastTo`/`CastFrom` edges (`parser/definitions.rs:1888-1919`).
- **`CastFrom(#Bit)` override machinery exists** — the encoding door is nearly free.
- **`prop` bodies are documentation-only** (`bootstrap.bv:11-13`; parsed at
  `definitions.rs:1483`, not dispatched) — the length-op mechanism is syntax
  without dispatch. This is exactly the "op declaration for length, hardcoded
  by default, overloadable" the model requires.
- **Legacy types**: `StaticString` (`bootstrap.bv:95-98`) unused; `UTF8View`
  ops in `lib/std/types/utf8view.bv`; `SmallString64` ops in
  `lib/std/types/small_string.bv`. Special-cased only in
  `emit_toplevel.rs:280-282` and `mod.rs`. **No callers outside their own
  modules** (verified).
- **No benchmark uses `String`** (verified: `grep` over `benchmarks/*.bv`).
  The suite is numeric; String work is print/I/O output. A String
  representation change is invisible to the benchmark matrix.
- The literal emission is **already** `@str.N = <{ i64, [N x i8] }>`
  (`emit_expr.rs:1262-1289`) — header + bytes, exactly the target model. The
  register is currently a `ptrtoint` → `i64` handle with `Type::int()`.

### 5.4 Architecture decisions (locked)

- **B-D1 (representation):** A String value is a `ptr` to `[len][bytes]` in
  every type-claiming site (`llvm_type`, `protocol_llvm_type`, casting graph).
  State slots are `i64` words (the address) via the existing `Ptr<T>` state
  adapters (`mod.rs:837-843`, `adapt_to_i64`/`ensure_typed_value`). SSO remains
  a *runtime* encoding detail inside `briv_str_to_c` — never a compiler type.
- **B-D2 (deref positions):** value moves pass the address; operands in
  ops/casts deref by default through **one central helper**. Two-position rule;
  no scattered deref decisions. This is what kills the split-brain: the
  four-way register ambiguity collapses to "address in value position, deref in
  operand position."
- **B-D3 (`#String → #Bit`):** the buffer pointer (content view,
  self-describing, zero instructions). The header stays reachable so region ops
  bound the content; ops never include the header in the bits.
- **B-D4 (encoding door):** `#Bit → #String` default = UTF8 wrap via the
  existing `CastFrom(#Bit)` machinery; sub-protocols override the lane. Remove
  `!> encoding` (already unread). UTF8 exists nowhere as a symbol.
  **Header-in-cast rule (explicit):** the cast is a pure re-interpretation of
  the address — it never copies, drops, or inherits a header. When the source
  is a raw `#Bit` buffer, the wrap introduces a header *by construction*
  (length derived from the bytes, C-side scan), never by adopting one; when
  the source is already a `#String`, the address still points at its
  header-prefixed buffer, so `Bytes`/`Size` keep reading `[0]` = len from it.
  In every case the header is bookkeeping reachable through the address, never
  payload carried by the value.
- **B-D5 (length ops):** `Bytes` = header read (O(1)); `Size` = UTF8 char count
  (runtime default). Protocol defaults for `#String`, overloadable by subtypes.
- **B-D6 (scope):** **B0** (representation coherence) + **B1** (deref operands +
  content equality) are **mandatory** — B0 unblocks Part A Phase 1, B1 is a
  hard prerequisite for Part A Phase 3. **B2** (content-view casts + encoding
  door) + **B3** (length-op dispatch) are **gated behind a checkpoint** (§6)
  — the philosophical core, low-risk, not blockers. **B4** (legacy retirement +
  docs + benchmarks) is **mandatory** — it deletes the dead fat-pointer/SSO
  machinery and closes Part B.
- **B-D7 (buffer format):** length-prefixed. Null-terminated rejected (§5.2.6).
- **B-D8 (legacy):** delete `StaticString`; retire `UTF8View` + `SmallString64`
  (ops migrate into `#String` defaults or die). `Slice<T>` deferred.
- **B-D9 (additive-only, interpreter-first):** existing optimization arms
  unchanged; new behavior is new arms + new dispatch. Interpreter changes land
  with (before) their backend counterparts (rule #4).

### 5.5 Performance policy

- **No benchmark exercises `String`** (verified §5.3). String work is
  print/I/O output; the change is invisible to the benchmark matrix.
- The model **removes** conversion churn (`ptrtoint`, `i128` loads,
  `extractvalue`, `inttoptr`) rather than adding it; `#String → #Bit` becomes
  zero instructions.
- The one theoretical regression — a header load per `Bytes` in a length-only
  loop vs a fat pointer's register-resident length — is L1-hot, rare, and
  A/B-able. The fat pointer's real cost (a 2-word `{i64,i64}` value the
  codebase already tried and failed to agree on) is avoided.
- Rule #11: a benchmark baseline (clean `cargo build --release` + full
  `bash benchmarks/build_and_bench.sh --runtime`) is recorded at the commit
  *before* Part B work and again after B4. Rule #19 governs any perf fix.

### 5.6 Cross-part dependencies (Part B in the queue)

| Prerequisite | Required for | Why |
|---|---|---|
| **Phase B0** | Part A Phase 1 (format demo) | the `__print_str` declare/call/register mismatch blocks `clang` link |
| **Phase B0** | Part A Phases 3, 4 | `args!("--flag", String)`, `entry!`, script printing need a coherent String value |
| **Phase B1** | Part A Phase 3 | `entry_cmd() == "cmd"` is String `Eq`; address-based `Eq` never fires |
| Phase B2 / B3 | nothing hard | deferred; gated behind the §6 checkpoint |
| Phase B4 | closes Part B | legacy retirement + docs + final benchmarks |

Execution order therefore interleaves the two parts (§6).

---

## 6. Phase plan

**Execution order (the queue — no ambiguity).** B-phases are Part B (bits
model); others are Part A (macro rework). Dependencies are hard unless noted.

| # | Phase | Requires | Notes |
|---|-------|----------|-------|
| 1 | Phase 0 — FFI audit | — | **DONE** |
| 2 | Phase 1 — lowercase macros + formatting | — | core **DONE** (working tree); format demo blocked on B0 |
| 3 | **Phase B0 — String representation coherence** | — | unblocks Phase 1 (clang link) |
| 4 | Phase 1 completion | B0 | format-demo end-to-end + tests |
| 5 | **Phase B1 — deref operands + content equality** | B0 | hard prerequisite for Phase 3 (`entry_cmd() == "cmd"`) |
| 6 | Phase 2 — `[#]` removal | none (may slide earlier) | mechanical |
| 7 | Phase 3 — CLI / `entry!` / `args!` / gate | B0, B1 | |
| 8 | Phase 4 — flat-scripting | B0 | |
| 9 | **Phase B2 — content-view casts + encoding door** | B0, B1 | **gated** (checkpoint below) |
| 10 | **Phase B3 — length-op dispatch** | B2 | **gated** (checkpoint below) |
| 11 | Phase 5 — docs / SPEC / highlighter / verification | B0, B1, Phases 2-4 | |
| 12 | **Phase B4 — legacy retirement + docs + benchmarks** | B0-B3 as landed | closes Part B |

**B2/B3 checkpoint (B-D6):** proceed only when B0 and B1 landed green **and**
the B2 example subtype (`Latin1String`) demonstrates the override path
end-to-end. If the checkpoint is not met, B2/B3 defer to a follow-up plan —
they are not blockers.

### Phase 0 — FFI full audit (research; deliverable = documented findings + regression target)

**Baseline (rule #11):** clean `cargo build --release`, then
`bash benchmarks/build_and_bench.sh --runtime`. Record the full ratio table for
all runtime benchmarks at `d6c6c818` in `benchmarks/results/2026-08-01-plugin-rework-baseline.md`.

**Audit protocol (Performance Recovery Protocol §1-6):**
1. Inspect `benchmarks/*.ll` at baseline for every FFI shape:
   - `frgn` `.c` / `#System` / `#Link<x>` / native `.o/.so/.a`: `call @sym(...)`
     direct + LTO-inline evidence.
   - `PrintInt#`/`PrintStr#`/`PrintFloat#`/`PrintChar#` → `call @__print_*`.
   - `get_env_int` defn → inlined `frgn__getenv_int` → `call @__getenv_int`.
   - Bridge path (`emit_bridge_frgn_call`) — verify it is never selected for
     `.c/.rs` on the LLVM backend (ext dispatch ordering in `frgn_dispatch.rs`).
   - SSO string shims (`extractvalue {i64,i64} ..., 0` + `inttoptr`) and
     `i64↔ptr` coercions (`coerce_to_param_type`) — quantify any added
     instructions in hot loops.
   - `frgn!`/`frgn?!` fire-and-forget codegen (dead-skip vs call).
2. A/B against `../briv-compiler-baseline` (now at `d6c6c818` = current state,
   so A/B isolates only our subsequent changes) via `compare_baseline.sh`.
3. `git log --oneline` over `src/backend/llvm/emit_expr.rs`,
   `emit_toplevel.rs`, `intrinsics.rs`, `src/plugin/{print,env}_plugin.rs` to
   identify the syntax change that introduced any indirection between the
   "native" era and now; cross-check `benchmarks/results/` history.
4. **Deliverable:** `docs/plans/2026-08-01-ffi-audit-findings.md` with the exact
   IR evidence, the identified regression (if any), and a regression target
   benchmark + a guard test asserting plugin/print rewrites do not change the
   emitted FFI call sequence.

### Phase 1 — Lowercase macros, `println!`/`print!` formatting

| File | Change |
|------|--------|
| `src/plugin/print_plugin.rs` | Handle `print`/`println`; format-string parse + positional substitution; protocol-derived type dispatch via `TypeUniverse`; newline for `println!`; compile-time errors for bad placeholders |
| `src/plugin/env_plugin.rs` | Rename `GetEnv`→`get_env`, `GetEnvInt`→`get_env_int` |
| `src/typechecker/mod.rs:544-557` | Recognize `print`/`println`/`get_env`/`get_env_int`; PascalCase intercept → rename-hint error |
| `src/interpreter/eval.rs:136` | Evaluate `print`/`println`/`get_env`/`get_env_int` natively (interpreter-is-reference) |
| `benchmarks/*.bv` (~40) | `PrintLn!`→`println!`, `GetEnvInt!`→`get_env_int!` |
| `lib/std/{io,env,ffi/io,ffi/env}.bv` | Update comments/defn doc strings |
| `examples/*.bv` | Same rename sweep |
| `learn-briv/*`, `spec/SPEC.md` | Tutorial + spec sweep |
| `syntax-highlighter/` | Grammar: lowercase macro tokens |

**Tests:** format expansion (literal/`{}`/`{0}`/`{1}`/`{{}}`/out-of-range/
trailing-args-warning), newline semantics, PascalCase rename-hint error,
interpreter parity (rule #4), FFI call-sequence guard (Phase 0 #4).

**Blocked by Phase B0:** the format-string path crosses a String-typed FFI
boundary (`PrintStr#`), which is where the representation split-brain bites.
Phase 1 does not "finish" until B0 lands and `format_demo.bv` links + runs.

### Phase B0 — Bits model: String representation coherence (mandatory; unblocks Phase 1)

**Goal:** one representation — a String value is a `ptr` to `[len][bytes]` —
across every type-claiming site; fix the declares that emit broken FFI; fix the
`i128` state slot (done in the working tree). Absorbs the two working-tree
fixes (`push_field_type` String slot → `i64`; `PrintStr#` → `i64` handle).

| File | Change |
|------|--------|
| `src/casting/graph.rs:273,285-286` | `#String` LLVM type → `Fixed("ptr")` (base + UTF8/ASCII variants) |
| `src/backend/llvm/mod.rs:366-384` | `protocol_llvm_type` string branch → `"ptr"` (keep `is_string_shaped` only while legacy types exist; B4 removes) |
| `src/backend/llvm/emit_toplevel.rs:269-328` | `llvm_type`: SSO-off `ptr` branch already correct; drop SSO-on `{ i64, i64 }` branches (309-318) as unreachable (flag removed in B4) |
| `src/backend/llvm/mod.rs:836-913` | `push_field_type` String slot → `i64` (**DONE** in working tree; keep provenance comment) |
| `src/backend/llvm/intrinsics.rs:86-90` | `PrintStr#`: finalize to the target register/declare type (ptr value; `call @__print_str` with matching type) |
| `src/backend/llvm/emit_expr.rs:1262-1289` | `emit_legacy_string_literal`: keep `@str.N = <{ i64, [N x i8] }>`; register becomes the `ptr` address (align all String consumers) |
| `src/backend/llvm/emit_expr.rs:700-716` (`emit_len`), String consumers | adapt to `ptr`-register String values (drop `inttoptr i64` on handles; use the `Ptr<T>` state adapters for load/store) |
| `lib/std/ffi/io.bv:9` | delete dead `frgn __print_str(msg: String) -> Void` (source of the `{ i64, i64 }` declare) |
| `lib/std/types/bootstrap.bv:79-86` | `type String: #String;` — drop `data`/`len`/`encoding` (props defer to B3) |
| `src/backend/llvm/mod.rs:2122-2151` | frgn declare loop: verify String params/returns emit `ptr` matching call sites |

**Acceptance:** `format_demo.bv` links with clang and prints correctly (String +
Int + Float placeholders, positional `{n}`, bare `println!()`); a String frgn
round-trip (base64, `lib/std/ffi/encoding.bv`) links and runs; a String state
field loads as one machine word (no `i128`); **no `{ i64, i64 }` or `i128`
remains in emitted IR for String**.

**Tests:** format-demo build+link+run; print_loop; String var + param printing;
base64/encoding frgn round-trip; emitted-IR assertion (no `{ i64, i64 }` / `i128`
for String); full `cargo test --lib`; `cargo build` no new warnings.

### Phase B1 — Bits model: deref-by-default operands + content equality (mandatory; prerequisite for Phase 3)

**Goal:** `#String` operands deref by default through one central helper;
`Eq`/`Ne` compare content, not addresses; bitwise ops operate on content bytes.

| File | Change |
|------|--------|
| new central helper (`intrinsics.rs` or a small module) | `emit_string_operand`: deref → buffer, read header, bound region; used by every `#String` op default |
| `src/interpreter/eval.rs:332` | `Eq`/`Ne` on String operands → content comparison (**rule #4: interpreter first**) |
| `src/backend/llvm/emit_expr.rs:2437-2454` | `Eq`/`Ne` on `#String` operands → content compare (len + bytes; runtime `briv_str_eq` or inline) |
| op dispatch | add `#String` defaults for `band`/`bor`/`bxor`/`bnot` (content, same-length result), `Concat`, `Slice` as protocol defaults |
| value moves | params/returns/stores/FFI remain address-only — never deref |

**Acceptance:** two equal-content strings at different addresses compare `==`;
`entry_cmd() == "build"` semantics verified; bitwise ops transform content and
preserve length; interpreter and backend agree.

**Tests:** content-eq (equal content/different address; differing content;
differing length); bitwise on strings; interpreter parity; the `entry!`-shaped
comparison; full `cargo test --lib`.

**Status (2026-08-01): IMPLEMENTED.** Content Eq/Ne landed interpreter-first
(`interpreter/eval.rs` via the shared `Value::string_bytes` deref helper) and in
the backend (`emit_expr.rs` Eq/Ne arms → `briv_str_eq` runtime call, declared
in `mod.rs`). Bitwise `&`/`|`/`^`/`~` on `#String` operate on content bytes via
`briv_str_band/bor/bxor/bnot` (same-length result), gated by the central
`is_string_operand` helper (`helpers.rs`, rule #16 — the 7 inline protocol
checks were centralized). `emit_binop_from_config` returns None for `#String`
operands so the dedicated arms own them (the flexible primordial's `bytes=0`
would otherwise derive `i0` in the integer template). Verified end-to-end:
`.smoke/eq_demo.bv` (heap vs literal content-eq), `.smoke/bit_demo.bv` (bitwise
identity/XOR-NUL/bnot-nonempty). Two real bugs found & fixed during B1: (1) the
SSO tag bit was OR-ed onto String literal addresses in the inline-init path,
corrupting pointers for `briv_str_eq` — now gated on `feature_sso_strings`;
(2) the interpreter's `Value::Int(addr)` heap-handle deref must require a valid
heap allocation so numeric comparisons fall through (see `BUGS.md`). `Slice` on
String deferred per B-D8.

### Phase 2 — Remove `[#]`

| File | Change |
|------|--------|
| `src/parser/definitions.rs:880-908` | Delete the `[#]` branch in `parse_contract` |
| `src/ast/top.rs` | Remove `Contract.is_entry`; delete `:137` comment |
| `src/ast/mod.rs:8` | Remove comment line |
| `src/ast/display.rs:476` | Remove `[#]` display branch |
| `src/beast/serialize.rs:52,81-83,365` | Remove `is_entry` serialize/round-trip + test |
| ~50 constructor sites | `git grep -n "is_entry"` sweep → remove the field from every initializer |
| `src/main.rs:731` | Init template `defn main() -> Int [#]` → script form (Phase 4 output), or `defn main() -> Int { term 0; }` |

**Tests:** `[#]` is a syntax error; a script compiles to a one-shot node; BEAST
round-trip still passes without `is_entry`.

**Status (2026-08-01): IMPLEMENTED.** `Contract.is_entry` removed (ast/top.rs)
and swept from all ~50 constructor sites (`git grep "is_entry"` now returns
zero field references). `parse_contract` rejects `[#]` with a clear
"entry-point syntax removed" error, and the array-dimension path
(`parser/types.rs`) rejects the `-> Int [#]` form (it previously parsed as a
named dimension `Int[#]`). serialize/deserialize/display no longer carry the
marker; BEAST round-trip test updated to assert no `(entry)` atom. `main.rs`
init template now emits `defn main() -> Int { term 0; }`. SPEC §2.5 + §3.24
updated (entry!/args! replace the marker; SPEC is the reference for the Phase 3
macros). The lexer still tokenizes `#` as an identifier (unchanged); the PARSER
is what rejects `[#]`.

### Phase 3 — CLI runtime, `entry!`/`args!`, concurrency gate

| File | Change |
|------|--------|
| `src/backend/llvm/loop_engine/{mod,counter,ssa}.rs` | `main(i32 %argc, ptr %argv)` + global capture stores (all 10 sites) |
| `lib/runtime/briv_rt.c` | `__argv_count/__argv_get/__argv_has/__argv_value/__argv_command` |
| `lib/std/cli.bv` (new) | frgn declarations + `entry_cmd`/`arg_present` defns (§4.3) |
| `src/plugin/entry_plugin.rs` (new) | `entry!`/`args!` expansion, guard injection, `std/cli.bv` import, collision checks |
| `src/plugin/mod.rs`, `src/compile.rs:861` | Register `EntryPlugin` |
| `config/targets.toml [".bv"]` | `plugins = ["prelude","env","print","entry"]` |
| `src/analysis/concurrency_gate.rs` (new) | Gate algorithm (§4.6) |
| `src/analysis/mod.rs`, `src/compile.rs` | Invoke gate after typechecking |
| examples/benchmarks with multi-auto-fire | Audit + explicit `async`/`sync<group>` classification |

**Tests:** expansion to precondition + guard + flip; one-shot (no re-fire);
`args!` Bool + typed value; collision errors; target-without-argv warning;
gate deny/allow matrix (UNSAT, XOR-overlap, async, sync<group>, unclassified);
`entry!`-vs-`entry!` subcommand dispatch.

**Status (2026-08-01): 3a (CLI runtime capture) IMPLEMENTED.** Every loop-engine
main is now `define i32 @main(i32 %argc, ptr %argv)` via a central
`emit_main_header` helper (helpers.rs) that stores argc/argv into
`@__briv_argc`/`@__briv_argv` (external globals the runtime links against).
`lib/runtime/briv_rt.c` gains `__argv_count/__argv_get/__argv_has/__argv_value/
__argv_command` (command = first non-flag argv[1..], honoring `$BRIV_ENTRY_CMD`).
`lib/std/cli.bv` declares the frgns + `entry_cmd`/`arg_present`. Verified
end-to-end in `.smoke/cli_demo.bv` (build/run/flag/env fallback) and a backend
IR test (`test_main_signature_and_argv_capture`). Remaining: 3c (concurrency
gate).

**Status (2026-08-01): 3b (`entry!`/`args!` plugin) IMPLEMENTED.** New
`src/plugin/entry_plugin.rs` (Parsed stage, registered in compile.rs + `.bv`
targets.toml): `entry!("cmd")` rewrites to `entry_cmd() == "cmd" &&
!__entry_<cmd>_done`, injects the done-flag as a top-level `let`, and appends
the flip; `args!("--flag")` / `args!("--flag", T)` rewrite to `arg_<flag>`
snapshot state fields bound from `__argv_has`/`__argv_value` (Bool/typed), in
contracts AND bodies. Non-reactive `defn` entry points get a synthesized
reactive wrapper (helper-node path) that calls the defn and flips the
done-flag; its postcondition is the done-flag so the reactor converges
(one-shot). Helper names are compiler-reserved (collision = error);
`import "std/cli.bv"` is injected. `[true]` is never emitted. Verified
end-to-end: `.smoke/entry_demo2.bv` (build/run subcommand dispatch),
`.smoke/entry_args.bv` (body args! snapshot), `.smoke/entry_defn.bv` (defn
wrapper). 5 plugin unit tests.

**Bug fixed (BUGS.md):** type-driven `!range` metadata emitted `!{ i64 0,
i64 256 }` on `load i8` (Bool/UInt8/Int8 state fields) — malformed LLVM (range
bounds must match the load width) that crashed clang
(computeKnownBitsFromRangeMetadata). Exposed by the entry plugin's Bool
done-flags. `emit_range_metadata` now emits bounds in the field's LLVM integer
width and skips vacuous ranges (256 does not fit i8). Regression test
`test_bool_field_no_malformed_i8_range`.

**Status (2026-08-01): 3c (concurrency gate) IMPLEMENTED.** New
`src/analysis/concurrency_gate.rs` (frontend-computed, invoked in compile.rs
after typechecking): for every unordered reactive pair, `sat =
check_satisfiable(pre_A, pre_B)` and XOR read-write overlap decide safety;
eligible-but-unclassified pairs are a hard compile error (rule #21).
`check_satisfiable` extended to detect `f() == "a"` vs `f() == "b"` (same lhs,
different constant) as UNSAT — the entry! subcommand-dispatch pattern.
`collect_read_identifiers` gained Term/TermBang/Return arms (reads through
`term x`). `sync<group> node` parsing added (parser) so group-barrier
classification is expressible in source. Parser bug fixed: `async node` prefix
now preserves the async flag (was dropped, so explicitly-async nodes were
never classified). Benchmarks audited per §4.6: async_counters + async_counters_sym
→ `async node`; async_counters_runtime → `sync<counters>` (sequential/barrier
intent, matches its "not thread pool" comment). 4 gate unit tests.

### Phase 4 — Flat-scripting plugin (one-shot opening node)

| File | Change |
|------|--------|
| `src/plugin/script_plugin.rs` (new) | §4.5 synthesis; `defn main` wiring to `briv_main` |
| `src/plugin/mod.rs`, `src/compile.rs`, `config/targets.toml` | Register `ScriptPlugin` (priority after entry) |
| `src/parser/definitions.rs:1210` | Remove the placeholder `wrap_implicit_entry` (replaced by the plugin) |

**Tests:** bare-statement script compiles to one-shot node; runs exactly once;
`[true]` never present in generated preconditions (assert on emitted IR);
`defn main` runs once; collision with `__script_done` errors.

**Status (2026-08-01): IMPLEMENTED.** New `src/plugin/script_plugin.rs`
(Parsed stage, registered after entry): a script-style program (no reactive
node/txn, no `sync<group>`, no non-`main` defn, no explicit `entry!`) gets a
synthesized one-shot opening node:
`node __script_main [__script_done == false][__script_done] { ...;
__script_done = true; }`. Two cases: (1) `defn main()` → the node calls
`main()` once (fixes the dead-code gap — `briv_main` was defined but never
invoked); (2) bare top-level lets/consts → the node runs them in order.
`[true]` is never emitted; `__script_main`/`__script_done` are
compiler-reserved (collision = error). Supporting fixes: `emit_user_call`
now maps a call to `main` → `briv_main` (the defn rename), and the
`wrap_implicit_entry` parser placeholder was removed. Verified:
`.smoke/main_script.bv` (defn main runs once), `.smoke/script_let.bv`
(one-shot bare-let script). 6 script plugin tests; the `sync<group>`/entry!
programs are correctly NOT wrapped.

### Phase B2 — Bits model: content-view casts + encoding door (gated — B-D6)

**Goal:** `#String → #Bit` yields the buffer pointer (content view);
`#Bit → #String` default = UTF8 wrap via the existing `CastFrom(#Bit)`
machinery; `!> encoding` metadata removed.

| File | Change |
|------|--------|
| `src/backend/llvm/emit_expr.rs:2900,2966` | `#String → #Bit` lane (`ExtractData`) → buffer-pointer content view |
| `src/backend/llvm/emit_expr.rs:2880,2927` | `#Bit → #String` lane (`Bitcast`) → UTF8 wrap default via `CastFrom(#Bit)` |
| `src/backend/llvm/mod.rs:1020`, `graph.rs:97` | confirm/complete the `CastFrom(#Bit)` override wiring |
| `lib/std/types/bootstrap.bv` | remove `!> encoding` (behavior-neutral; unread) |
| new example subtype | `Latin1String: #String { CastFrom(#Bit) ...; op Bytes ...; }` overriding the lane + `Bytes` |

**Tests:** `#String → #Bit` content view (header excluded); `#Bit → #String`
default wrap; subtype override path; no `encoding` metadata remains in the
universe or emitted IR.

**Status (2026-08-01): IMPLEMENTED.** `#String → #Bit` is the CONTENT VIEW
(`PtrToInt` — a String value is a ptr to `[len][bytes]`, so the cast yields the
buffer address; was `ExtractData` on the dead `{i64,i64}`). `#Bit → #String` is
the ENCODING DOOR (`CastFromBitCallback` — default UTF8 wrap via
`briv_bits_to_str`, which re-materializes the `[len][bytes]` header by
construction from the bits; registered `CastFrom(#Bit)` overrides are called).
`!> encoding` removed from bootstrap.bv (unread). Two supporting fixes: (1)
`type_to_protocol` strips the `#` from HashWord categories so casts to/from
`#Bit` resolve base lanes (was `"#Bit"` → no lanes → silent LLVM-coercion
fallthrough); (2) `type_to_protocol` follows a type's declared `base` parent so
`type Latin1String: #String` resolves to the String category (subtypes get the
protocol lanes). Parser now accepts `op CastFrom(#Bit) = fn` (variant parsed as
a type; `=` accepted like proto CastFrom). Verified: `.smoke/bit_let.bv`
(content view → buffer address), `.smoke/bit_to_str.bv` (encoding door
round-trip "hello"). 3 new tests. Note: a `let r: Latin1String = <String
literal>` still requires an implicit subtype cast (typechecker does not coerce
String literals to #String subtypes) — the override path fires on explicit
casts; literal coercion is a separate concern not in B2 scope.

### Phase B3 — Bits model: length-op dispatch (gated — B-D6)

**Goal:** wire `prop Size` / `prop Bytes` to real dispatch; `#String` default =
O(1) header read (`Bytes`) / UTF8 char count via runtime (`Size`); overloadable
by subtypes.

| File | Change |
|------|--------|
| prop dispatch (`typechecker`/backend) | resolve `prop Size` / `prop Bytes` bodies to codegen instead of treating them as documentation |
| `src/backend/llvm/intrinsics.rs:700-716` (`emit_len`) | String length routes through the op default; list length untouched |
| runtime | `briv_len` / `briv_char_len` helpers for the `#String` defaults (via `briv_str_to_c`) |

**Tests:** default `Bytes` O(1); `Size` UTF8 correctness (ASCII + multibyte);
subtype override (the B2 example); no `Length#` regression for lists.

**Status (2026-08-01): IMPLEMENTED.** `x.^Len` on a #String → the `Size` prop
default = UTF8 char count via `briv_char_len` (runtime counts codepoints);
`x.^^Bytes` → the `Bytes` prop default = O(1) header read (the `[0]` length
prefix). Interpreter parity in `eval.rs` Reflect (Len = char count, Bytes =
byte length). Bug fixed (BUGS.md): `collect_identifiers` did not handle
`Expr::Reflect`, so a String `let` used ONLY via reflection was eliminated as a
dead state field — `s.^Len` emitted `load i64, ptr @s` with `@s` undefined.
Reflect now keeps its receiver live. Verified: `.smoke/len_demo.bv` prints
`chars=5 bytes=6` for "héllo". 2 new tests.

### Phase 5 — Docs, SPEC, highlighter, full-suite verification

| File | Change |
|------|--------|
| `spec/SPEC.md` | §3.24 → `entry!`/`args!` (one-shot semantics); §3.28 scripting (one-shot node); macro-naming table |
| `docs/architecture/macro-system.md` | `!` macro naming convention, `entry!`/`args!` reference |
| `docs/architecture/concurrency-and-modifiers.md` | Gate is now enforced; update "Implementation notes" |
| `docs/architecture/agent-reference.md` | Naming convention + plugin surface |
| `docs/architecture/hash-words.md` | If it references `[#]` |
| `syntax-highlighter/` | Grammar for `entry!`/`args!`/`print!`/`println!`/`get_env!`/`get_env_int!` |
| `learn-briv/` | Tutorials: scripting, CLI, printing |
| `benchmarks/results/2026-08-01-plugin-rework-final.md` | Post-change full runtime table (rule #11) |

**Final verification:** `cargo test --lib` green; `cargo build` no new warnings;
Praetor on changed files (complexity ≤ 15, lines ≤ 100, params ≤ 6);
`compare_baseline.sh` on the FFI regression target; no benchmark regressed
without a documented A/B result.

### Phase B4 — Bits model: legacy retirement + docs + benchmarks (mandatory; closes Part B)

**Goal:** delete the dead fat-pointer/SSO machinery and the legacy string
types; document the bits model; record the Part B bugs; run the rule #11
before/after benchmark A/B.

| File | Change |
|------|--------|
| SSO layer | delete `feature_sso_strings` (`mod.rs:672,778,845-853`; `main.rs:324,373`), `emit_sso_literal`/`emit_sso_heap_literal`/SSO branches (`emit_expr.rs:1139-1144,1151-1220`), SSO String→C shim (`emit_expr.rs:1856-1883`) |
| `is_string_like` (`type_universe/mod.rs:289`) | delete; `Cast.#String` protocol membership only (rule #18) |
| legacy types | delete `StaticString`; retire `UTF8View`/`SmallString64` + their modules (`utf8view.bv`, `small_string.bv`); migrate any surviving ops into `#String` defaults |
| `lib/std/types/bootstrap.bv` | final `type String: #String;` (fields/encoding already dropped in B0) |
| docs | `docs/architecture/casting-protocol.md`, `backend-type-dispatch.md`, `hash-words.md`, `agent-reference.md`, `spec/SPEC.md` — bits model, `#Bit` content view, encoding door, length ops |
| `BUGS.md` | record root causes + fixes: address-based `Eq`; `i128` String state slot; `{ i64, i64 }` declare from dead frgn |
| benchmarks | rule #11 final A/B (baseline before B0 vs after B4); fasta documented as String-free |

**Acceptance:** `grep` over `src/` returns zero for `is_string_like`,
`feature_sso_strings`, and any `{ i64, i64 }`/`i128` String claim; `cargo test
--lib` green; benchmark table in `benchmarks/results/2026-08-01-plugin-rework-final.md`.

**Tests:** full suite green; `--no-stdlib` still type-checks
`let x: Int = 5`; no legacy-type callers regress (verified none exist).

**Status (2026-08-01): B4a (SSO + is_string_like retirement) IMPLEMENTED.**
`feature_sso_strings` (flag, setter, plumbing in main.rs/compile.rs/mod.rs) and
`is_string_like` (the 2-int-field structural heuristic) deleted; `grep` over
`src/` returns zero for both. The dead SSO branches were removed:
`emit_sso_literal`/`emit_sso_heap_literal`/`emit_sso_concat`/the SSO String→C
shim/the `{i64,i64}` string-type claims. `is_string_like` call sites migrated
to `#String`/`#Data` protocol membership (`protocol_llvm_type`,
`type_is_heap_allocated`, `is_string_identifier`, trigger adapters,
`push_field_type`). Bug fixed (BUGS.md): `emit_box_concat_result` tagged the
concat result with the legacy OR-2 temp bit and returned i64 — a String is an
UNTAGGED ptr under B0, so consumers expecting `ptr` (`__print_str`) failed;
it now returns the untagged ptr as a String-typed register (concat demo prints
"foobar"). 1359 tests green.

### Phase B4 (cont.) — B4b: legacy types + B4c: docs/benchmarks

**B4b (2026-08-01): IMPLEMENTED.** Deleted `lib/std/types/utf8view.bv` +
`small_string.bv` and their type declarations from bootstrap.bv; removed the
hardcoded `%SmallString64/%StaticString/%UTF8View` struct decls and the
UTF8View `{i64,i64}` special-case. grep over `src/` + `lib/std` returns zero
for `UTF8View`/`SmallString64`/`StaticString`.

**B4c (2026-08-01): IMPLEMENTED.** Docs updated to the bits model:
`backend-type-dispatch.md` (String = ptr, `!>` metadata, protocol membership
rows), `casting-protocol.md` (ptr concat examples), `agent-reference.md`
(String = ptr table rows), `hash-words.md` (`op InsertAt:` strategy bindings),
`spec/SPEC.md` §5.6 rewritten (`!> key: value;` — the `<~` metadata form was
removed; the type_property grammar and all metadata examples converted),
`learn-briv/12-pragmas.md` + `15-custom-types.md` (the `<~` → `!>` syntax).
`bits-thesis.md` is a historical thesis — its `<~` references are "old
mechanism" comparisons and the removal note, intentionally preserved. The
final benchmark table (`2026-08-01-plugin-rework-final.md`) got a B4 note:
the SSO/legacy-type removals are compile-time-only (the flag was always off),
so the recorded runtime numbers are unchanged.

---

## 7. Commit order (continuous commits, rule "Continuous commits")

Working-tree state at plan approval: Part A Phase 1 core (lowercase macros +
format expansion) plus two B0-aligned fixes already applied (String state slot
→ `i64` in `push_field_type`; `PrintStr#` → `i64` handle). These two fixes are
entangled with Phase 1 changes in the same files (`mod.rs`, `intrinsics.rs`),
so they were committed together with Phase 1 as one green unit (tests 1323
green, release build clean). B0 proceeds from that commit.

1. `docs/plans/2026-08-01-plugin-macro-rework.md` (this file) — on `main`.
2. Phase 0: baseline results + audit findings.
3. **Phase B0: String representation coherence** (includes the two working-tree
   fixes; tests green; `format_demo` links). *Before* Phase 1 completion.
4. Phase 1 completion: format-demo end-to-end + remaining Phase 1 tests.
5. **Phase B1: deref operands + content equality** (interpreter + backend).
6. Phase 2: `[#]`/`is_entry` removal (mechanical; tests green).
7. Phase 3: CLI runtime → stdlib → entry plugin → gate (each step green).
8. Phase 4: script plugin.
9. **Phase B2 (gated): content-view casts + encoding door.**
10. **Phase B3 (gated): length-op dispatch.**
11. Phase 5 + **Phase B4**: docs/highlighter/learn-briv, legacy retirement,
    final benchmark A/B.

Each commit: `git add` only intended files; `cargo test --lib` before commit;
`cargo build` no new warnings; Praetor on changed files.

## 8. Undo / rollback

- Every phase is a self-contained commit on `feat/plugin-macro-rework` (new
  branch in the worktree). Rollback = `git revert <commit>` (never
  `git checkout --` / `git restore` — rule #7).
- `is_entry` removal is preceded by a commit that archives the field's semantics
  in this plan; nothing consumes it, so reversion is trivial.
- **B0 is a prerequisite for Phase 1 completion and Phases 3-4.** Reverting B0
  blocks those phases; the plan does not commit Phase 1 completion until B0 is
  green. B0 is a self-contained commit (representation + tests) so it can be
  reverted cleanly if a hidden dependency surfaces.
- **B2/B3 are gated.** If the §6 checkpoint is not met, they are simply not
  committed; Part A and B0/B1/B4 are unaffected.
- The baseline worktree at `d6c6c818` remains the controlled A/B reference; it
  is not modified further during execution.

## 9. Risks

| Risk | Mitigation |
|------|-----------|
| `main` signature change (10 sites) breaks a backend path | Phase 3 is isolated; additive global capture; full `cargo test --lib` + benchmark suite per step |
| Concurrency gate denies existing valid programs | Explicit audit + reclassification step with per-change review (Phase 3) |
| `println!` formatting changes emitted IR → FFI benchmark deltas | Phase 0 guard test pins the FFI call sequence; compare_baseline.sh on target |
| Interpreter drift (rule #4) | Interpreter gains native handling in the same phase as the plugin (incl. B1's content-`Eq`) |
| Stale `benchmarks/*.ll` in repo confuse verification | Rebuild from clean source; never trust committed `.ll` |
| String type-claim changes break an existing String frgn declare | B0 compiles a real String frgn (base64, `ffi/encoding.bv`) end-to-end; frgn declares were already broken — B0 fixes them |
| Literal register type (ptr vs i64) still mismatches declares | B0 pins one type and asserts on emitted IR (no `{ i64, i64 }`, no `i128` for String) |
| Content-`Eq` changes semantics a program relied on | Contract-first: content equality *is* the contract; examples/benchmarks audited for address-`Eq` reliance (none expected) |
| Retiring legacy types breaks a caller | Verified no callers outside their own modules; full suite catches regressions |
| B0 scope creep (dead SSO layer) | SSO removal is deferred to B4 as a mechanical deletion of unreachable code; tests green before/after |
