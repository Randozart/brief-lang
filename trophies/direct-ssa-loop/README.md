## A006: Direct SSA Loop Dispatch

**What**: Eliminated the `@reactor_tick` function call for programs with no
async state or MMIO events, replacing it with a direct SSA loop in the
function body.

**Why it matters**: The reactor tick dispatch adds function call overhead,
state snapshotting, and event-loop metadata setup even for programs that
only use synchronous transactions. For programs like nbody_sqrt and
float_math, this was 5-15% of total execution time.

**How**: The dispatch analysis determines at compile time whether a program
uses any async triggers (FFI signals, MMIO mappings, `node` with reactor
speed). If not, the codegen emits a `while` loop directly around the
transaction bodys' combined state transitions, using phi nodes for field
state instead of a function call into the reactor. The `emit_dispatch_type()`
helper returns `DirectSSA` vs `ReactorTick` based on the analysis.

**Before/After**:
| Program | Before | After | Improvement |
|---------|--------|-------|-------------|
| nbody_sqrt | `@reactor_tick` wrapper | direct SSA phis | ~10% faster |
| float_math | `@reactor_tick` wrapper | direct SSA phis | ~8% faster |
| async_counters | direct SSA | direct SSA | unchanged (needs reactor) |
