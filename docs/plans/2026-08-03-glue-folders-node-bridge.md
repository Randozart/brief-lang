# Per-language GLUE folders + the Node probe (Python ↔ Node bridge)

**Date:** 2026-08-03
**Status:** Active plan
**Branch:** `glue-host-callable` (worktree `../brief-compiler-glue-host`)
**Related:** `docs/plans/2026-08-03-native-python-meld-composite.md`,
`docs/guides/ffi-and-export.md`

---

## Goal

Rearchitect language binding generation into per-language glue folders —
`lib/glue/<lang>/` referenced as config, found by name on invocation — and
validate the generic system against **Node** (a language with no mature binding
to Python), culminating in a live **Python ↔ Node bridge** test. This is a
probe of the *generic system*: can it capture any random interaction without
per-language knowledge in the compiler?

Take the best of all three eras that exist in the tree:
- **Era 1 (compile-time metaprogramming, legacy/unwired):** `lib/ffi/gen_*.bv`
  — Brief `$defn` generators + `FileWrite$` + `ShellCmd$` (Turing-complete
  generation, plugins can run the toolchain).
- **Era 2 (generic plugin folders):** `plugins/{front,mid,post,back}/` — for
  compiler phases, **not** language bindings.
- **Era 3 (config + Rust renderer, current P1–P3):** monolithic
  `config/glue.dbvl` + `render_native_shim` + `brief extension`. Proven, but the
  toolchain recipe is Python-hardcoded in Rust (`run_extension_cli:537-555`) —
  the hole the Node probe exposes.

**Hybrid split (most efficient per part):**
- Declarative contract → **config** (`glue.dbvl` per language).
- Template-shaped emission → **generic Rust renderer**.
- Imperative generation beyond templates → **`gen.bv` compile-time plugin**
  escape hatch (designed now, staged later).
- Toolchain recipe → **in `glue.dbvl`** (closes the hole).

## Decisions locked with the author

1. **Best ideas from all three eras** — the hybrid split above.
2. **Found by name on invocation** — the command resolves `lib/glue/<lang>/`
   by language name.
3. **Most efficient approach per mechanism/target, kept** — evaluated per
   target as we migrate, tested each.
4. **`glue.dbvl` per folder.**
5. **Node FFI path = generated `.node` addon** (config-driven, no npm).

## 1. Per-language glue folders

```
lib/glue/<lang>/
  types.bv      boundary declarations (exists)
  glue.dbvl     the language config: target entry + templates + bindings + native.* + toolchain recipe
  gen.bv        OPTIONAL compile-time plugin (imperative generation escape hatch)
```

- `brief export|bindings|extension <bridge> <lang>` resolves
  `lib/glue/<lang>/glue.dbvl` **by name** (reusing lib/ resolution), loads it,
  and emits whatever files the project's further compilation needs (header,
  crate, wrapper, extension).
- **Registry mode preserved:** `load_glue_config(None)` scans
  `lib/glue/*/glue.dbvl` and merges by folder name, so
  `find_language_by_extension` (compile.rs:575, frgn_dispatch.rs) and
  `ConfigGet$` (macros/eval.rs:1241) keep working.
  `load_glue_config(Some(path))` + `--glue-config` override stay for single-file
  compat.
- Update `tests/glue_test.rs` baked-shape tests to the folder-scan source
  (behavioral: python/rust entries present, `"rs"` → rust).

## 2. Toolchain recipe in `glue.dbvl`

New config fields: `native.include_cmd`, `native.suffix` (literal) or
`native.suffix_cmd`, `native.link_cmd`, `native.cc` (default `cc`).
`run_extension_cli` becomes generic: resolve language folder → load target →
render shim → run the target's recipe commands → `cc -fPIC -c` + `cc -shared`.
Zero language knowledge in Rust.

- python: `python3-config --includes` / `--extension-suffix` / `--ldflags`.
- node: `node -p "…"` emits `-I<…>/include/node` (verified
  `/usr/local/include/node`), literal suffix `.node`, empty link_cmd (symbols
  resolve at load).

## 3. The `gen.bv` compile-time plugin escape hatch (designed, staged later)

If `lib/glue/<lang>/gen.bv` exists, the command invokes it instead of the
renderer: the pipeline writes a `bridge.dbvl` contract next to the output (the
`extract_bridge_info` data, Data Brief), the plugin reads it via
`FileRead$`/`ConfigGet$`, generates files via `FileWrite$`, runs the toolchain
via `ShellCmd$` (both already wired: macros/audit.rs severity, macros/eval.rs).
Turing-complete generation for anything the templates can't express.
**Documented now; implemented after the config-driven path is proven.**

## 4. Node target — generated `.node` addon

`lib/glue/node/glue.dbvl`: NAPI templates —
- `native.module`: `NAPI_MODULE_INIT()` + `g_state` + `napi_property_descriptor`
  method table.
- `native.method`: `napi_callback_info` → `argv[]` via `napi_get_cb_info`.
- per-category `native.parse.#Int/#Float/#String`:
  `napi_get_value_int64` / `napi_get_value_double` / `napi_get_value_string_utf8`.
- `native.build.#*`: `napi_create_int64` / `napi_create_double` /
  `napi_create_string_utf8` — String out reads the composite's NUL-invariant
  data zero-copy: `napi_create_string_utf8(env, (const char*)r,
  NAPI_AUTO_LENGTH, …)`.
- `native.c_type.#*` / `native.ret.#*` + toolchain recipe.

The generic `render_native_shim` renders it unchanged (per-category snippets,
module/method/method_def templates, per-export extern protos).

## 5. The bridge + Python ↔ Node round-trip

`examples/glue-host/node_bridge.bv`:
```
import "glue/c.bv";
frgn brief_read_file_raw(path: String) -> Int as __read_file__ from "lib/runtime/brief_rt.c" fallback 0;
frgn brief_write_file(path: String, data: String) -> Int as __write_file__ from "lib/runtime/brief_rt.c" fallback 0;
state saved: String = "";
state count: Int = 0;
txn store_text(name: CStr) -> CStr { saved = name; term saved; };
export defn save(name: CStr) -> CStr { term store_text(name); };
export defn read() -> CStr { term saved; };
export defn bump(delta: Int) -> Int { count = count + delta; term count; };
export defn persist(path: CStr) -> CStr { ... __write_file__ ... term saved; };
export defn load(path: CStr) -> CStr { ... __read_file__ + cstr_to_brief ... term saved; };
```
- The CStr ↔ String meld (c.bv) carries the boundary cast-free.
- `load` wraps the bare `__read_file__` buffer with `cstr_to_brief` (correctness,
  not the fragile str_to_c heuristic).
- Cross-process constraint: Node and Python are separate processes (no
  shared live state, unlike Rust-embedding-Python). In-process round-trips are
  live; the cross-language exchange uses Brief's own runtime file I/O — the
  only interface both languages share.

`tests/c_driver_node.rs` (toolchain-guarded on node/python3-config/cc):
1. build bridge `.a` → `brief extension … node` → `.node`.
2. Node: `save("node→brief")`, `read()` assert, `bump(5)`, `persist(x.dat)`.
3. `brief extension … python` → `.so`.
4. Python: `load(x.dat)` asserts a Node-originated composite, `save("python→brief")`,
   `persist(y.dat)`, `bump(3)`.
5. Node: `load(y.dat)` asserts the reverse.

## 6. Migration — all languages, each tested

1. **python** → `lib/glue/python/glue.dbvl` (+ toolchain recipe): re-run
   `c_driver_python`, `c_driver_boundary`, extension build/benchmark unchanged.
2. **node** → new: the probe (`c_driver_node`).
3. **c** → `lib/glue/c/glue.dbvl` (bindings/header): header assertions in
   `c_driver_boundary` stay green.
4. **rust** → `lib/glue/rust/glue.dbvl` (crate): `rust-host` still builds.
5. **web** → `lib/glue/web/glue.dbvl`: config loads + templates render (wasm
   target, no active test).

## 7. Docs

- `docs/guides/ffi-and-export.md`: glue-folder model, toolchain recipe, §11
  Node addon + Python ↔ Node bridge.
- This plan (with the `gen.bv` design and the composite/cross-process
  rationale).
- Config references (`config/glue.dbvl` → `lib/glue/<lang>/glue.dbvl`) in code
  comments.

## Verification

`cargo test --lib` green; all glue integration tests green; Praetor on changed
dirs; python extension benchmark re-checked (migration is organizational —
numbers must hold); BUGS.md updated if anything surfaces.

## Sequencing

1. Config migration + loader + tests.
2. Toolchain-recipe generalization (python still green, now config-driven).
3. Node target + bridge + Python ↔ Node test.
4. c/rust/web migration, each verified.
5. Docs.
The `gen.bv` escape hatch stays documented-but-deferred.
