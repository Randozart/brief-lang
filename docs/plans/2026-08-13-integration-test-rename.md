# Integration-test rename sweep (post-layout-keywords loose end)

**2026-08-13** — branch `feat/spec-layout-keywords`.

## Problem

BUGS.md:3 claims "7 stale integration-test files reference pre-rename `briefc`
bin + `briv_*` symbols". The actual scope is **20 files / ~169 references**:
`briefc`→`brievc`, `briv_compiler`→`briev_compiler` (lib is `briev_compiler`),
`briv_types.h`→`briev_types.h`, `BrivState`→`BrievState`,
`__briv_*`→`__briev_*`, `briv_rt(.c/.o)`→`briev_rt(.c/.o)`,
`briv-compiler`→`briev-compiler`, and `briv_*_test` temp-dir names. The
rename commit `62ae145d` ("Massive rename") never migrated these. They do not
compile / cannot find the binary / cannot find the generated header.

## Why it is a rename + a call-site fix, not an API rewrite

The C-ABI *shape* is unchanged (verified in `lib/glue/c/glue.dbv`):
`typedef struct BrievState BrievState;`, `extern BrievState*
__briev_init_state(void);`, `extern void __glue_release(BrievState*)`, every
export declared as `ret export(BrievState* state, ...)`. The driver snippets
call exports WITHOUT the `state` argument (`echo((int64_t)"hello")`), so each
export call needs `state, ` inserted — that is the second, per-file fix.

## Rename map (mechanical, scripted)

- `briefc` → `brievc` (includes `CARGO_BIN_EXE_briefc`, "failed briefc" msgs)
- `briv_compiler` → `briev_compiler`
- `briv-compiler` → `briev-compiler`
- `briv_types.h` → `briev_types.h`
- `BrivState` → `BrievState`
- `__briv_` → `__briev_` (init_state, set_cancel, clear_cancel)
- `briv_rt` → `briev_rt` (real `.c` path in termination_diagnostics_test is
  link-blocking; `briv_rt.o` cosmetic)
- `briv_*_test` → `briev_*_test` (cosmetic temp-dir names, same sweep)

## Driver call-site fix

For each driver snippet that calls an exported function `export(args...)`,
insert the state pointer: `export(st, args...)`. Keep `st`/`state` variable
naming consistent with the driver.

## Verification

1. `cargo test --no-run` — all test binaries compile.
2. Run toolchain-supported drivers on this machine (cc/gcc/g++/clang/llc/ar,
   java, node, python3 available; lua/go/dotnet absent → toolchain-guarded
   SKIP, compile-only).
3. `glue_integration.sh` — rename + review (needs the glue toolchain).
4. Update BUGS.md:3 with the real scope + status.
5. Gates: `cargo test --lib` green, Praetor zero new, commit.

## Risks

- Driver API drift beyond the rename (string-ABI marshalling, `brievc build
  --library` artifact names) — fix as discovered during verification.
- Some drivers may reference bindings that fail for non-rename reasons; those
  get diagnosed and fixed or documented.
