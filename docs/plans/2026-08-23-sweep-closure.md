# Track A — Sweep Closure + CI Wiring

**Date:** 2026-08-23
**Parent:** `docs/plans/2026-08-22-spec-conformance.md` Phase 10
**Status:** active

## Goal

Flip the conformance sweep's `#[ignore]` off and wire it into the standard
test path so regressions find themselves immediately.

## Steps

1. **Exclude glue.dbv from sweep** — 9 files are load-bearing for the glue
   config system and validated by `glue::config::*` tests via their own
   quoted-mode parser. The sweep's non-quoted dbv parser can't handle them.
   Filter by path prefix in `discover_active_sources` or the sweep test.
2. **Fix remaining misc singles** (~8 files): physics.bv `var`, main.bv /
   stdlib_usage.bv / simple-counter (nonexistent module imports),
   fn-ptr-demo remnants.
3. **Tamer WIP**: leave lib/compiler/*.bv failing — foreign track owns
   these. Note in BUGS.md; coordinate before migrating.
4. **Flip `#[ignore]`** once non-tamer count is stable at ≤5.
5. **CI wiring**: add `cargo test --lib conformance_sweep` as a named gate.

## Then: Track C sub-plan

Open `docs/plans/2026-08-23-async-scheduler.md`.
