# Macro System Architecture — Safety, Sandboxing, and Phase Ordering

**Date:** 2026-07-23
**Status:** Architecture plan

---

## The Five Phase-Ordering Questions Answered

The core tension in any compile-time macro system is: *when do macros run relative to type checking?*

### Phase 1: Syntactic Macros (Pre-Typecheck) — `$(Parsed)` and earlier

| What's available | What's restricted |
|-----------------|-------------------|
| `Tag$`, `Named$`, `Children$`, `Parent$`, `First$`, `Last$`, `Nth$` | `TypeInfo$` — types don't exist yet |
| `Insert$`, `Delete$`, `ReplaceWith$`, `Rename$`, `Set$` | `DocRead$` — universe not populated |
| `Import$`, `Defn$`, `Call$`, `Block$` | `CastPath$` — no protocol graph |
| `ConfigGet$`, `FileRead$`, `FileWrite$` | `DocRead$` on non-primitive types |
| `StrLen$`, `StrReplace$`, `StrJoin$`, `StrSplit$`, `StrSubstr$` | |
| `ShellCmd$` (if sandbox permits) | |

**Effect on type checking:** AST mutations at this phase happen BEFORE typechecking. The typechecker sees the FINAL tree — it's as if the developer wrote the expanded code by hand. No re-check cycles needed.

### Phase 2: Semantic Macros (Post-Typecheck) — `$(Typed)` and later

| What's available | What's restricted |
|-----------------|-------------------|
| `TypeInfo$` — types fully resolved | `Insert$` on the CURRENT AST — would invalidate types |
| `DocRead$` — universe populated | `Delete$` on the CURRENT AST |
| `CastPath$` — protocol paths computable | `ReplaceWith$` on the CURRENT AST |
| All string, file, config intrinsics | |
| `WriteFile$` — can still output generated files | |

**This is the Generator Pattern:** Semantic macros don't mutate the current compilation unit. They READ type information and WRITE new files (or new compilation units for a subsequent pass). The Generator Pattern solves the phase-ordering paradox: type info is available, but mutations don't invalidate it.

### Phase 3: Generation Output — New Compilation Units

Files written via `FileWrite$` at any phase become new source files that are compiled in a separate, clean compiler invocation. A single `brief build` orchestrates both passes transparently, with the guarantee that macro-generated code is NEVER compiled in the same pass as the macro that generated it.

---

## Security Architecture

### Capability Hierarchy

```
Pure (always allowed by default)
├── AST navigation: Tag$, Named$, Children$, Parent$, First$, Last$, Nth$, Count$, Names$, IsEmpty$
├── String operations: StrLen$, StrReplace$, StrJoin$, StrSplit$, StrSubstr$
├── Type queries: TypeInfo$, DocRead$, CastPath$

Disk Read (opt-in: --allow-read)
├── FileRead$(path)
├── ConfigGet$(section, key)

Disk Write (opt-in: --allow-write)
├── FileWrite$(path, content)

Network (opt-in: --allow-net)
├── HttpFetch$(url)

Shell (opt-in: --allow-run)
├── ShellCmd$(cmd, args...)
```

### Virtual Filesystem by Default

`FileWrite$` writes to an in-memory virtual filesystem (`virtual://`) by default. Physical disk writes require explicit `.Persist$()` call:

```brief
// Default: writes to virtual filesystem (in-memory, never touches disk)
FileWrite$("src/generated/bridge.rs", content);

// Explicit: mark for physical persistence (requires --allow-write at build time)
FileWrite$("src/generated/bridge.rs", content, { persist: true });
```

**The build pipeline:**
```
1. Parse + typecheck
2. Run macros (all stages)
   ├── FileWrite$("virtual://...", ...)  ← buffers in VFS
   ├── FileWrite$("virtual://...", ..., { persist: true })  ← marks for flush
   └── ...
3. ALL macros succeeded → flush marked files to disk (if --allow-write)
   ├── src/generated/bridge.rs  ← written now
   └── src/generated/ffi.rs     ← written now
4. Continue to codegen
```

**Key properties:**
- **Atomicity:** If any macro crashes, the flush never runs. No partial output.
- **Determinism:** VFS content is purely a function of macro source + inputs.
- **Caching:** Hash macro source + reads → VFS is cacheable.
- **Safety:** Without `--allow-write`, persist is a no-op. Generated files exist only in `--dump-vfs`.
- **Separation:** Macro writers test with `--dump-vfs`. Consumers approve with `--allow-write`.

```bash
# Development — inspect generated code without writing anything
brief build bridge.bv --dump-vfs

# CI — write generated files as part of the build
brief build bridge.bv --allow-write
```

### Directory Scoping (Chrooted File I/O)

`FileWrite$` and `FileRead$` operate relative to the project root ONLY. Absolute paths and `../` traversal are rejected:

```brief
FileWrite$("src/generated/bridge.rs", content);  // allowed
FileWrite$("/etc/passwd", content);               // rejected
FileWrite$("../../.ssh/id_rsa", content);         // rejected
```

### Capability Lockfile (`macro-lock.toml`)

Generated on first build with `--allow-*` flags. Records SHA-256 hash of every `.bv` plugin file and the capabilities it requested:

```toml
[plugin."glue-generator"]
hash = "a1b2c3d4e5..."
requested = ["disk-read", "disk-write"]

[plugin."linter"]
hash = "f6e7d8c9a0..."
requested = []
```

On subsequent builds, if a plugin's hash changes and it requests NEW capabilities, the build halts with a diff:

```
[!] Plugin 'glue-generator' has changed since last approved.
    Previously requested: disk-read, disk-write
    Now also requests: network
    Run `brief audit` to inspect changes, or update macro-lock.toml
```

### Static Capability Auditing

```bash
brief audit

# Scanning 3 macros...
#   deps/glue/generator.bv
#     [HIGH] ShellCmd$(curl, https://api.example.com/schema)
#     [MEDIUM] FileWrite$(src/generated/*.rs)
#
#   src/lint/macro.bv
#     [LOW] Tag$, Named$, Delete$
```

### Interactive Prompting

When no `--allow-*` flag is provided and a macro requests a capability:

```
[!] Plugin "glue-generator" wants to write files to disk.
    Path: src/generated/bridge.rs
    Allow? [y/N/save]:
```

---

## Build System Integrity

### Transactional Macro Execution

All side effects (AST modifications, file writes) are held in a buffer during macro evaluation. If the macro crashes, the buffer is discarded:

```
Macro starts → creates delta-branch of AST
  → writes files to virtual buffer
  → if error: discard buffer, report error
  → if success: commit buffer (flush persisted files to real disk)
```

### Determinism Mocking

Non-deterministic system inputs are mocked for reproducible builds:

```brief
TimeNow$()  // returns frozen timestamp (git commit time or fixed epoch)
EnvGet$("BUILD_NUMBER")  // returns "42"
EnvGet$("HOME")          // returns "" (blocked unless allowlisted)
```

### Resource Quotas (Gas Limits)

Every `$` intrinsic call costs 1 unit. Default budget: 1,000,000. When exceeded:

```
[Error] Macro 'bridge_generator' exceeded instruction limit (1,000,000)
  at: glue/generator.bv:14:5
    Tag$("export").Children$("Definition")
    ^-----------------------------------^
  Hint: Increase with --macro-budget 2000000 or optimize your macro
```

### AST Lineage (Macro Expansion Traces)

Every generated or mutated AST node carries a "lineage span" back to the source macro:

```
[Type Error] Generated function 'ffi_greet' has mismatched types

  Generated by:
    --> deps/glue/generator.bv:22:18
         | let name = TypeInfo$(export, "name");
         |            ^ 'export' resolved to definition 'greet'
```

---

## Code Safety

### AST-Safe Quasiquoting (`Quote$`)

Structural interpolation instead of string-based template substitution:

```brief
let node = Quote$({
    fn $fn_name($params) -> $return_type {
        body_content
    }
});
```

`$fn_name` is bound as an AST identifier, not raw text. The `Quote$` step rejects syntax-breaking characters at bind time.

### Tainted Nodes (Anti-Infinite-Recursion)

Generated AST nodes carry a taint flag. `All$()` and `Tag$("defn")` EXCLUDE generated nodes by default. To explicitly select generated nodes:

```brief
All$({ include_generated: true }).Count$()
```

### Semantic Diff/Dry-Run

```bash
brief build --diff

# Changes proposed by all macros:
# + src/generated/bridge.rs (new, by glue-generator)
# - src/legacy/main.bv (removed, by migration-plugin)
```

---

## Implementation Order

| Step | What | Depends on |
|------|------|------------|
| 1 | Implement remaining `$` intrinsics (`FileWrite$`, `FileRead$`, `ShellCmd$`, `ConfigGet$`, `DocRead$`, `CastPath$`, `TypeInfo$`) | None (7 done, need testing) |
| 2 | Add capability categories to intrinsics engine | 1 |
| 3 | Add `--allow-read`, `--allow-write`, `--allow-run` CLI flags | 2 |
| 4 | Add `virtual://` VFS namespace (default for FileWrite$) | 1 |
| 5 | Add `.Persist$()` marker for physical flush | 4 |
| 6 | Add `macro-lock.toml` with capability hashing | 3 |
| 7 | Add `brief audit` command (static capability scan) | 2 |
| 8 | Add resource quota / gas limit | 1 |
| 9 | Add transactional macro execution | 4 |
| 10 | Add AST lineage / macro expansion traces | 1 |
| 11 | Add `Quote$` structural quasiquoting | 1 |
| 12 | Add tainted node filtering | 11 |
| 13 | Add `--diff` / dry-run | 4 |
| 14 | Add `TimeNow$`, `EnvGet$`, `HttpFetch$` | 3 |
| 15 | Rewrite GLUE bridge generator as `.bv` plugin | All of the above |

Steps 1-5 are the minimum viable security layer. Steps 6-8 are the safety net. Steps 9-15 are production hardening.

---

## Layer 2: Hardware Introspection — `SysQuery$`

The `SysQuery$` intrinsic provides structured, auditable access to host hardware
topology — without raw shell commands.

### The Intrinsic

```brief
let cache_line = SysQuery$("cpu.cache_line_size");     // → 64
let simd_width = SysQuery$("cpu.simd_register_width"); // → 512 (AVX-512)
let gpu_addr   = SysQuery$("pci.device.gpu.bar[0]");   // → 0xFE000000
```

### Three Resolution Modes

Driven by `brief.toml` target profiles, NOT by the macro source code:

| Mode | Use case | Build flag | Portability |
|------|----------|------------|-------------|
| `"host"` | Local dev — probes real hardware | `--allow-sys-query` | Machine-specific |
| `"file://path"` | Cross-compilation — reads manifest | `FileRead$` + `--allow-read` | Portable (manifest in git) |
| `"inline"` | Declarative CI/CD — values in config | `ConfigGet$` (no special flag) | Fully hermetic |

The macro code doesn't change between modes. Only the build config changes.

### Host-Tainted Build Metadata

When `SysQuery$` runs in `"host"` mode, the compiler marks the output:

```
1. SysQuery$ with "host" mode → taint flag set on compilation unit
2. Remote build cache is SKIPPED for this artifact (local-only)
3. Binary metadata embedded: "Optimized for Build-Machine-42"
4. Runtime diagnostics: if hardware mismatch detected, emit warning
```

This gives local developers fast, optimized binaries while preventing corrupted
cached artifacts from being served to other machines.

### Capability

| Intrinsic | Capability | Flag |
|-----------|------------|------|
| `SysQuery$` | `host-introspection` | `--allow-sys-query` |

### The Contract → Optimization Pipeline

`SysQuery$` feeds directly into Brief's contract system to create optimizations
unavailable in any other language:

```
SysQuery$("cpu.simd_register_width") → 64
    │
    ▼
Macro injects: invariant(buffer.len % 64 == 0)
    │
    ▼
Compiler's VRP (Value Range Propagation) proves the tail loop is dead
    │
    ▼
Tail-loop eliminated. Vectorized kernel is 100% main loop, zero cleanup.
```

Same mechanism for:
- **Bounds-check elimination:** `invariant(register_index < gpu_bar_size)` →
  compiler strips MMIO bounds checks, producing raw `MOV`/`STR`
- **False-sharing prevention:** `invariant(alignof(ThreadState) == cache_line)` →
  struct layout tuned to actual cache line width, zero waste
- **SIMD tail-loop elimination:** `invariant(buffer.len % simd_width == 0)` →
  compiler deletes the scalar cleanup loop entirely

### Comparison With Other Languages

| Feature | C/C++ | Rust | **Brief** |
|---------|-------|------|-----------|
| System info | `#ifdef` preprocessor (fragile) | `build.rs` env vars (string-based) | `SysQuery$` (typed, structured) |
| Assumptions | `__builtin_assume(x)` (UB if wrong) | Hard to pass to optimizer | Safe invariants (debug=assert, release=opt) |
| Portability | Non-portable, OS-specific | Limited via `cfg` flags | Portable — macro+mock config |

---

## Layer 3: Multi-Target Compilation

### Build Configuration (`brief.toml`)

```toml
[package]
name = "tensor_core"
targets = ["local-dev", "aws-h100-node", "edge-jetson-nano"]

[target.local-dev]
introspection = "host"

[target.aws-h100-node]
introspection = "file://profiles/nvidia_h100.json"

[target.edge-jetson-nano]
introspection = "inline"
"cpu.cache_line_size" = 64
"cpu.simd_register_width" = 128
"pci.device.gpu.bar[0].size" = "0x4000000"
```

### Multi-Pass Execution

The compiler driver runs N passes, each with a different `SysQuery$` mock:

```
                      ┌───► [Pass 1: local-dev]        SysQuery$ reads PHYSICAL HOST
                      │
brief build ──────────┼───► [Pass 2: aws-h100-node]    SysQuery$ reads nvidia_h100.json
                      │
                      └───► [Pass 3: edge-jetson-nano] SysQuery$ reads inline config
```

Each pass produces a target-specialized binary:

```bash
bin/local-dev/tensor_core        # AVX-512, 64-byte cache lines
bin/aws-h100/tensor_core         # H100-tuned, CUDA
bin/edge-jetson-nano/tensor_core # ARM NEON, 128-bit SIMD
```

### Fat Binaries with `#[multi_target]`

```brief
#[multi_target(targets = ["aws-h100-node", "edge-jetson-nano"])]
fn process_tensors(buffer: &mut [f32]) {
    let simd_width = SysQuery$("cpu.simd_register_width");
    invariant(buffer.len % simd_width == 0);
    // ... performance-critical loop (compiled twice, specialized each time) ...
};
```

The compiler generates:
1. `process_tensors_h100` — optimized for AWS H100
2. `process_tensors_jetson` — optimized for Jetson Nano
3. A runtime dispatcher that detects hardware and branches at startup

---

## Implementation Order (Expanded)

| Step | What | Depends on |
|------|------|------------|
| 1 | Implement existing `$` intrinsics (`FileWrite$`, `FileRead$`, `ShellCmd$`, `ConfigGet$`, `DocRead$`, `CastPath$`, `TypeInfo$`) | None (7 done, in testing) |
| 2 | Add capability categories to intrinsics engine | 1 |
| 3 | Add `--allow-read`, `--allow-write`, `--allow-run` CLI flags | 2 |
| 4 | Add `virtual://` VFS namespace (default for FileWrite$) | 1 |
| 5 | Add `.Persist$()` marker for physical flush | 4 |
| 6 | Add `macro-lock.toml` with capability hashing | 3 |
| 7 | Add `brief audit` command (static capability scan) | 2 |
| 8 | Add resource quota / gas limit | 1 |
| 9 | Add transactional macro execution | 4 |
| 10 | Add AST lineage / macro expansion traces | 1 |
| 11 | Add `Quote$` structural quasiquoting | 1 |
| 12 | Add tainted node filtering | 11 |
| 13 | Add `--diff` / dry-run | 4 |
| 14 | Add `TimeNow$`, `EnvGet$`, `HttpFetch$` | 3 |
| **2a** | **Add `SysQuery$` + `host-introspection` capability** | **3** |
| **2b** | **Add host-taint tracking on compilation output** | **2a** |
| **2c** | **Add target profiles in `brief.toml`** | **2a** |
| **2d** | **Add `#[multi_target]` + dispatch stub generator** | **2c** |
| 15 | Rewrite GLUE bridge generator as `.bv` plugin | All of the above |

**Bold = Layer 2/3 additions (this document).** Other steps from the Layer 1 plan.

---

## Related Plan Documents

| Document | Covers |
|----------|--------|
| `docs/plans/2026-07-23-macro-system-safety.md` | (this file) Security, sandboxing, phase ordering, `SysQuery$`, multi-target |
| `docs/plans/2026-07-22-macro-system-extensions.md` | 7 original generic `$` intrinsics (StrLen$, FileWrite$, ConfigGet$, etc.) |
| `docs/plans/2026-07-22-complete-glue-fix-plan.md` | GLUE pipeline gaps (export, state param, protocol paths) |
| `docs/plans/2026-07-22-stdlib-frgn-cleanup.md` | Stdlib frgn declaration cleanup |
| `docs/plans/2026-07-22-ship-of-theseus-inop-removal.md` | Dead InopDeclaration removal |
| `docs/plans/2026-07-22-phase8-pp-port.md` | Phase 8: AST pretty-printer port (completed) |
| `docs/plans/2026-07-22-stress-test-glue-pipeline.md` | GLUE stress test plan (completed) |
| `docs/plans/2026-07-22-post-phase8-automation.md` | Post-Phase 8 optimization targets |
| `docs/architecture/macro-system.md` | Macro system architecture catalog |
| `docs/architecture/protocol-types.md` | Protocol type system documentation |
| `docs/architecture/glue-as-abi-generator.md` | GLUE as ABI generator documentation |
| `docs/architecture/frgn-export-glue-architecture.md` | Full frgn/export/GLUE architecture |
| `docs/plans/2026-07-22-fully-dynamic-glue-config.md` | Dynamic GLUE config removal of hardcoded languages |
| `docs/plans/2026-07-22-metropolitan-ffi-refactor.md` | Metropolitan FFI GLUE/Metropipe split |

---

## Summary of All Active and Completed Work

| Area | Status | Plan Document |
|------|--------|---------------|
| GLUE pipeline (export, protocol BFS, templates) | ✅ Complete | `phase8-pp-port`, `stress-test-glue-pipeline`, `complete-glue-fix` |
| Protocol-driven type mapping | ✅ Complete | `fully-dynamic-glue-config`, `protocol-types` |
| Metropolitan FFI refactoring | ✅ Complete | `metropolitan-ffi-refactor` |
| Macro system: 7 generic `$` intrinsics | ✅ Implemented (in testing) | `macro-system-extensions` |
| Macro system: security, VFS, capabilities | 📝 Plan (this document) | `macro-system-safety` |
| Macro system: `SysQuery$`, multi-target | 📝 Plan (this document) | `macro-system-safety` |
| Macro system: write GLUE as `.bv` plugin | 📝 Planned | `macro-system-safety` Step 15 |
| Arena allocator budget control | ✅ Complete | `post-phase8-automation` |
| Layout optimizer protocol integration | ✅ Complete | `post-phase8-automation` |
| Triple-concat string bug | ✅ Fixed | N/A (format change) |
| Prelude `Before$` fix | ✅ Complete (this session) | N/A (plugin fix) |

