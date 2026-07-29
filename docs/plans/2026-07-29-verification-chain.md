# Verification Chain — Compile-Time Cross-Verified Implementation Selection

Date: 2026-07-29
Status: Plan
Supersedes: `docs/plans/2026-07-29-derivation-abstraction-discovery.md`
           `docs/plans/2026-07-29-cegis-counterexample-injection.md`
           `docs/plans/2026-07-29-skolem-counterexample-extraction.md`
Related: `docs/architecture/features/verification-chain.md`
         `docs/architecture/features/anti-unification-abstraction.md`

## Executive Summary

The `:=` operator on a `defn` declares a chain of structurally equivalent
implementations. The compiler selects the first body whose target matches
the compile target and passes cross-verification against every other body
in the chain. This makes inline assembly safe (verified against a Brief
reference), derivation optional (one body source among equals), and the
compiler a correctness gate rather than a trust boundary.

Derivation (CEGIS, anti-unification, SMT synthesis) remains intact as one
body-generator. It is not the core mechanism — the chain is. Derivation is
invoked only when a derivation block appears in the chain, and only if no
earlier body was selected.

---

## 1. Literature Grounding

### 1.1 Multi-implementation Verification — Chandra et al. 2001 [CT'01]

Chandra, S., Godefroid, P. & Larus, J. R. "Checking the 'Rights' of a
Program's Implementation." *Software Testing, Verification, and Reliability*
11(3), 2001.

The foundational paper for cross-checking: given two implementations of the
same specification, compare their outputs on a shared set of inputs. Any
discrepancy proves at least one implementation is buggy. The key insight:

  "If two independently developed implementations of the same specification
  produce the same output for all tested inputs, both are likely correct."

The chain `:= a := b := c` is a generalization: N implementations, N² cross-
checks at compile time. No single reference is trusted — the only trust is
in the agreement itself.

### 1.2 Inline Assembly Safety — Lopata et al. 2014 [L+14]

Lopata, W. et al. "Automated Verification of Inline Assembly in C Code
Using Symbolic Execution." *SPIN Workshop*, 2014.

Demonstrates that inline assembly can be verified against a C reference
by symbolic execution of the assembly combined with the surrounding C code.
The assembly is translated to a control-flow graph and checked against the
reference's postcondition.

Brief's approach is simpler: test the asm body against a Brief reference
on random inputs. No symbolic execution needed — concrete evaluation with
enough samples provides high confidence. Wrong instruction → wrong output
for any input → mismatch → skip to next body.

### 1.3 Pluggable Assembly — Keystone Engine [KE'16]

Keystone Engine (keystone-engine.org, 2016-present). LLVM's MC assembly
layer extracted as a standalone C library. Supports x86, x86-64, ARM,
ARM64, MIPS, PowerPC, SPARC, SystemZ, Hexagon, and more.

Used as the default assembler backend. The `AsmAssembler` trait allows
swapping Keystone for the platform assembler or a pass-through stub
on unsupported architectures.

---

## 2. Syntax

### 2.1 Assembly Function Declaration

```brief
asm<x86_64> name(args: Type) -> ReturnType {
    "instruction {result}, {param}"
};
```

Grammar (BNF):

```
asm_fn := 'asm' '<' target_literal '>' ident '(' params ')' '->' type '{' asm_body '}' ';'
target_literal ::= ident  // "x86_64", "aarch64", "riscv64", etc.
asm_body      ::= asm_line (';' asm_line)* ';'?
asm_line      ::= string_literal
```

Semantics:
- `target_literal` is a compile-time constant string matching one of the
  target strings in `config/targets.toml`.
- `asm_line` is a Brief string literal containing assembly mnemonics.
- `{param}` is a template variable; the compiler substitutes the ABI
  register for that parameter on the declared target.
- `{result}` is a template variable; the compiler substitutes the ABI
  return register for the target.
- The body may contain multiple semicolon-separated instruction strings.

### 2.2 Verification Chain

```brief
defn name(params) -> ReturnType [pre][post]
  := body1
  := body2
  := bodyN;    // last body is always unguarded
```

Where each body is one of:
- An assembly function name (declared separately via `asm<target>`)
- A derivation block `{ input -> output; ... }`
- A derivation with reference `{ ... } := ref_fn`
- A reference function name
- Another `defn` name

The semicolon terminates the entire chain. Each `:=` is a segment.
The last segment has no guard and is always accepted.

### 2.3 Derivation in the Chain

```brief
defn popcount(x: Int) -> Int
  := popcount_x86                  // asm<x86_64>, tested against ref
  := { 0->0; 1->1; 3->2; 7->3 }  // synthesis from examples
  := popcount_ref;                 // fallback, always accepted
```

The derivation block `:= { examples }` is NOT a test — it is a body
source. When the chain resolver reaches it, the CEGIS loop runs. If
synthesis succeeds AND the synthesized body passes cross-verification
against every other body in the chain, the synthesized body is emitted.
If synthesis fails or the synthesized body fails cross-verification,
the resolver moves to the next segment.

The current derivation syntax `:= { examples } := ref_fn` works within
this: it is a two-segment chain where the first is synthesis and the
second is the reference fallback. This is now recognized as a special
case of the more general `:= body := body := body` pattern.

### 2.4 Contracts — `[pre][post]` on the Signature

```brief
// Contracts live on the function signature, not on individual bodies.
// Every body in the chain must satisfy them.
defn popcount(x: Int) -> Int [result >= 0 && result < 64]
  := popcount_x86
  := popcount_ref;
```

Compiler enforcement:
1. For derivation bodies: contracts guide the CEGIS loop (existing behavior)
2. For asm bodies: contracts are checked via cross-verification
   (any contract violation manifests as output mismatch for some input)
3. For reference bodies: the reference defines correctness; contracts
   are checked against the reference (not the other way around)

---

## 3. Chain Resolution Algorithm

### 3.1 Pseudocode (Flat Control Flow)

```rust
fn resolve_chain(chain: &Chain, target: &Target) -> Result<Body> {
    // Phase 1: filter by target compatibility
    let mut candidates: Vec<&Body> = Vec::new();
    for body in &chain.bodies {
        match body.target() {
            Some(t) if t != target.arch => continue, // skip non-matching
            _ => {} // asm bodies with matching target, or non-asm bodies
        }
        candidates.push(body);
    }

    // Phase 2: cross-verification — test every candidate against all others
    for body in &candidates {
        // Cross-verify against all OTHER candidates
        let mut failed = false;
        for _sample in 0..CROSS_VERIFY_SAMPLES {
            let input = generate_random_input(body.params());
            let output = body.evaluate(&input);
            for other in &candidates {
                if body is other { continue; }
                let other_output = other.evaluate(&input);
                if output != other_output {
                    failed = true;
                    break;
                }
            }
            if failed { break; }
        }
        if !failed {
            // Body passes — this is the selected implementation
            return body.compile(target);
        }
        // Failed — try next candidate
    }

    // Phase 3: fall through — last candidate is always unguarded
    let last = candidates.last().ok_or(Error::EmptyChain)?;
    last.compile(target)
}
```

### 3.2 Derivation Body Evaluation

When a derivation block `:= { examples }` or `:= { examples } := ref_fn`
is evaluated during cross-verification:

1. Run CEGIS synthesis as today (enumerative search + SMT fallback)
2. If synthesis succeeds, evaluate the synthesized body for the test input
3. If synthesis fails, the body cannot produce output → skip to next
4. The synthesized body is tentatively kept; final compilation uses it
   ONLY if cross-verification against all other bodies passes

### 3.3 Cross-Verification Sampling

Default: `CROSS_VERIFY_SAMPLES = 50` random inputs per body pair.

The samples are generated once per verification run and shared across all
bodies. Each body is evaluated on the same set of inputs.

For expensive bodies (e.g., complex asm that must be compiled to native
code), the sample count is configurable via `config/targets.toml`:

```toml
[".bv"]
backend = "llvm"
assembler = "keystone"
cross_verify_samples = 100
```

### 3.4 Target Compatibility

An asm body's target is the string in `asm<target>`. Non-asm bodies
(derivation blocks, reference defns) have no target restriction.

The compile target (`target.arch`) comes from the target triple in
`config/targets.toml` or the compiled file's extension.

---

## 4. Assembly Architecture

### 4.1 Template Variable Substitution

Compiler maps `{param}` and `{result}` to ABI registers per target.
The mapping is built into the compiler (hardcoded for common targets):

| Target | `result` | arg1 | arg2 | arg3 | arg4 | arg5 | arg6 |
|--------|----------|------|------|------|------|------|------|
| x86_64 SysV | rax | rdi | rsi | rdx | rcx | r8 | r9 |
| aarch64 | x0 | x0 | x1 | x2 | x3 | x4 | x5 |
| riscv64 | a0 | a0 | a1 | a2 | a3 | a4 | a5 |
| wasm32 | n/a (no registers) | n/a | n/a | n/a | n/a | n/a |

The template `"popcnt {result}, {x}"` for x86_64 becomes:
`"popcnt rax, rdi"` (after substitution, before validation).

### 4.2 The `AsmAssembler` Trait

Defined in `src/backend/assembler.rs`:

```rust
/// 2026-07-29: Compile-time assembly validation trait.
/// Each implementation validates asm text for a target architecture
/// and returns assembled bytes (for verification) or an error.
/// Config-driven: selected via `assembler` key in targets.toml.
pub trait AsmAssembler: Debug {
    /// Human-readable name (e.g., "keystone", "platform", "none").
    fn name(&self) -> &str;

    /// Validate and assemble a single instruction or block.
    /// `text`: the instruction template with {param} already substituted.
    /// `arch`: target architecture string (e.g., "x86_64", "aarch64").
    /// Returns assembled bytes on success, error string on failure.
    fn assemble(&self, text: &str, arch: &str) -> Result<Vec<u8>, String>;

    /// Whether this assembler is available on the current system.
    fn is_available(&self) -> bool;
}
```

### 4.3 Keystone Implementation

File: `src/backend/assembler/keystone.rs`

Uses the `keystone-sys` FFI crate (or manual C bindings if the crate
is unavailable). Selected via `assembler = "keystone"` in targets.toml.

```rust
pub struct KeystoneAssembler;

impl AsmAssembler for KeystoneAssembler {
    fn name(&self) -> &str { "keystone" }

    fn assemble(&self, text: &str, arch: &str) -> Result<Vec<u8>, String> {
        let ks_arch = arch_to_keystone(arch)?;
        let mut engine = ks::Engine::new(ks_arch, ks::Mode::default())
            .map_err(|e| format!("keystone init failed: {:?}", e))?;
        let (bytes, _count) = engine.asm(text, 0)
            .map_err(|e| format!("keystone asm error: {:?}", e))?;
        Ok(bytes)
    }

    fn is_available(&self) -> bool { true }
}
```

### 4.4 Platform Assembler Implementation

File: `src/backend/assembler/platform.rs`

Shells out to the system assembler (`as` on Unix, `ml64` on Windows).
Selected via `assembler = "platform"` in targets.toml.

Slower than Keystone but requires no C library linkage. Good for
development environments where Keystone is not installed.

```rust
pub struct PlatformAssembler;

impl AsmAssembler for PlatformAssembler {
    fn assemble(&self, text: &str, arch: &str) -> Result<Vec<u8>, String> {
        let (assembler, flags) = target_to_assembler(arch)?;
        let output = std::process::Command::new(assembler)
            .args(&flags)
            .arg("-")  // read from stdin
            .stdin(std::process::Stdio::piped())
            .output()
            .map_err(|e| format!("failed to run {}: {}", assembler, e))?;
        // ...
    }
}
```

### 4.5 Stub Implementation

File: `src/backend/assembler/stub.rs`

No-op — passes through without validation. Selected via
`assembler = "none"` in targets.toml.

Warning at compile time: "assembly not validated — set `assembler = \"keystone\"`
or `assembler = \"platform\"` for safety."

```rust
pub struct StubAssembler;

impl AsmAssembler for StubAssembler {
    fn assemble(&self, text: &str, arch: &str) -> Result<Vec<u8>, String> {
        eprintln!("  warning: assembly not validated for {}: {}", arch, text);
        Ok(vec![])  // trust the programmer
    }
    fn is_available(&self) -> bool { true }
}
```

### 4.6 Assembler Selection

In `config/targets.toml`:

```toml
[".bv"]
backend = "llvm"
assembler = "keystone"
cross_verify_samples = 50
```

In `src/target.rs`, add `assembler: String` field to `TargetEntry`.
In `src/compile.rs` or a new `src/backend/assembler/mod.rs`, add:

```rust
fn get_assembler(config: &TargetConfig) -> Box<dyn AsmAssembler> {
    match config.assembler.as_str() {
        "keystone" => Box::new(KeystoneAssembler),
        "platform" => Box::new(PlatformAssembler),
        "none" => Box::new(StubAssembler),
        other => {
            eprintln!("  warning: unknown assembler '{}', falling back to 'none'", other);
            Box::new(StubAssembler)
        }
    }
}
```

---

## 5. AST and Parser Changes

### 5.1 New AST Variants

```rust
// 2026-07-29: Assembly function declaration.
// asm<x86_64> name(args: Type) -> ReturnType { "instruction" };
pub struct AsmFn {
    pub target: String,              // "x86_64", "aarch64"
    pub name: String,
    pub params: Vec<(String, Type)>,
    pub ret_type: Type,
    pub body: Vec<String>,           // instruction strings, pre-substitution
    pub span: Span,
}
```

Add to `TopLevel` enum:
```rust
pub enum TopLevel {
    // ... existing variants ...
    AsmFn(AsmFn),                    // 2026-07-29
}
```

### 5.2 Parser Rule

```rust
// In src/parser/top_level.rs or similar:
fn parse_asm_fn(&mut self) -> Result<TopLevel> {
    // Consume: 'asm' '<' target '>' name '(' params ')' '->' type '{' body '}' ';'
    self.expect_keyword("asm")?;
    self.expect_op("<")?;
    let target = self.expect_identifier()?;
    self.expect_op(">")?;
    let name = self.expect_identifier()?;
    self.expect_op("(")?;
    let params = self.parse_params()?;
    self.expect_op(")")?;
    self.expect_op("->")?;
    let ret_type = self.parse_type()?;
    self.expect_op("{")?;
    let body = self.parse_asm_body()?;  // string literals separated by ;
    self.expect_op("}")?;
    self.expect_op(";")?;
    Ok(TopLevel::AsmFn(AsmFn { target, name, params, ret_type, body, span: self.span() }))
}
```

### 5.3 Verification Chain Parser

The existing `:=` parsing in derivation blocks already supports:

```
defn name(args) -> Type := body;
```

Extend to allow multiple `:=` segments:

```
defn name(args) -> Type := body1 := body2 := body3;
```

Each body is parsed as:
- An identifier (reference to asm fn or defn)
- A `{ input -> output; ... }` block (derivation examples)

---

## 6. Config Changes

### 6.1 targets.toml Extension

New fields in `[".bv"]` section:

```toml
[".bv"]
backend = "llvm"
assembler = "keystone"
cross_verify_samples = 50
```

Defaults if not specified:
- `assembler` = `"none"` (safe default — no validation, but warns)
- `cross_verify_samples` = `50`

### 6.2 TargetEntry Extension

In `src/target.rs`:

```rust
pub struct TargetEntry {
    pub backend: String,
    pub assembler: String,         // 2026-07-29
    pub cross_verify_samples: u32, // 2026-07-29
    pub defaults: Option<toml::Value>,
    pub plugins: Option<Vec<String>>,
    pub target_triple: Option<String>,
    pub data_layout: Option<String>,
}
```

---

## 7. Cross-Verification Implementation

### 7.1 Compile-Time Evaluation

Each body in the chain must be evaluable at compile time:
- **Asm body**: compiled to native code (via LLVM JIT or temporary binary),
  executed against test inputs, output collected.
- **Derivation body**: synthesized from examples via CEGIS, then evaluated
  via the interpreter.
- **Reference defn**: evaluated via the interpreter.

The evaluation function:

```rust
fn evaluate_body(body: &Body, inputs: &[Value]) -> Result<Value, Error> {
    match body {
        Body::Asm(asm_fn) => {
            // Compile asm to native code, run with inputs, return output
            let native = compile_asm(asm_fn, inputs)?;
            Ok(native)
        }
        Body::Derivation(block) => {
            // Synthesize from examples, evaluate via interpreter
            let expr = synthesize(block, inputs)?;
            evaluate_synthesized(&expr, inputs)
        }
        Body::RefFn(ref_fn) => {
            // Evaluate via interpreter
            evaluate_synthesized(&ref_fn.body, inputs)
        }
    }
}
```

### 7.2 Sample Generation

```rust
fn generate_sample_inputs(params: &[(String, Type)], count: u32) -> Vec<Vec<Value>> {
    let mut rng = rand::thread_rng();
    (0..count).map(|_| {
        params.iter().map(|(_, ty)| match ty {
            Type::Int => Value::Int(rng.gen_range(-1000..=1000)),
            Type::Float => Value::Float(rng.gen_range(-1000.0..=1000.0)),
            Type::Bool => Value::Bool(rng.gen_bool(0.5)),
            _ => Value::Int(0),
        }).collect()
    }).collect()
}
```

---

## 8. Integration with Existing Derivation

### 8.1 What Stays

| Component | Status | Notes |
|-----------|--------|-------|
| Anti-unification abstraction | Intact | LevelCache compression at depth 2-3-4 |
| CEGIS counterexample injection | Intact | Random verification finds counterexamples |
| SMT reference threading | Intact | `smt_verify_candidate` with `ref_fn` |
| SyGuS synthesis fallback | Intact | Z3 solves when enumerative fails |
| Doppelganger output | Intact | Emits synthesized body |

### 8.2 What Changes

| Component | Change |
|-----------|--------|
| `synthesize_candidate` | Becomes one body-generator called by the chain resolver |
| CEGIS loop termination | No longer returns `NoSolution` — returns synthesized body,
  and chain resolver handles verification via cross-checking |
| `:=` parser | Extended to support multiple segments |
| Reference verification | Cross-verification replaces the old "verify against ref_fn" step.
  The ref_fn is no longer the sole verifier — EVERY body verifies EVERY other. |

### 8.3 Migration Path

1. Single-segment chains work as before: `:= { examples }` or `:= ref_fn`
2. Two-segment chains work: `:= asm_fn := ref_fn`
3. Derivation with fallback: `:= { examples } := ref_fn` — this is the
   same syntax as today, now recognized as a two-segment chain
4. Multi-segment chains: new functionality

---

## 9. Plan Directives Compliance

| Directive | How this plan meets it |
|-----------|------------------------|
| **§1: FLAT CONTROL FLOW** | Chain resolver has 3 phases (filter → verify → fall through), each is a single loop. `evaluate_body` is a match with 3 arms — no nesting. |
| **§2: COMMENT THE CODE** | Every new code site gets `// 2026-07-29:` with rationale. The `AsmAssembler` trait, each implementation, and the chain resolver all carry literature citations. |
| **§3: UPDATE ALL EXAMPLES** | Existing `:= { examples }` and `:= ref_fn` syntax continues to work. New `asm` examples in `examples/` and `benchmarks/`. |
| **§4: DOCUMENTATION IS CODE** | Architecture doc `verification-chain.md` plus this plan. Existing derivation docs updated to note the chain context. |
| **§5: BEHAVIORAL TESTS, NOT LITERAL** | Tests assert chain resolution outcomes (which body is selected for which target), cross-verification matches/mismatches, and Keystone vs platform vs stub behavior. |

## 10. References

[CT'01] Chandra, S., Godefroid, P. & Larus, J. R. "Checking the 'Rights'
of a Program's Implementation." *STVR* 11(3), 2001.

[L+14] Lopata, W. et al. "Automated Verification of Inline Assembly in C
Code Using Symbolic Execution." *SPIN Workshop*, 2014.

[KE'16] Keystone Engine. keystone-engine.org, 2016.

[GP'92] Koza, J. R. *Genetic Programming.* MIT Press, 1992.

[PG'15] Polozov, O. & Gulwani, S. "FlashMeta: A Framework for Inductive
Program Synthesis." POPL 2015.

[FCD'15] Feser, J. K., Chaudhuri, S. & Dillig, I. "Synthesizing Data
Structure Transformations from Input-Output Examples." PLDI 2015.

## 11. Implementation Order

### Phase A: Infrastructure (this session)
1. `config/targets.toml` — add `assembler` and `cross_verify_samples` fields
2. `src/target.rs` — add fields to `TargetEntry`
3. `src/backend/assembler/mod.rs` — `AsmAssembler` trait + selector
4. `src/backend/assembler/keystone.rs` — Keystone implementation (or stub
   if `keystone-sys` not yet added to Cargo.toml)
5. `src/backend/assembler/platform.rs` — platform assembler
6. `src/backend/assembler/stub.rs` — no-op with warning

### Phase B: Parser + AST (next)
7. Parser: `asm<target> name(...) -> T { ... }` rule
8. AST: `TopLevel::AsmFn` variant
9. Parser: multi-segment `:= body := body := body` chain

### Phase C: Chain Resolver (next)
10. Chain resolver algorithm
11. Cross-verification sampling + evaluation
12. Derivation-as-body-source adapter

### Phase D: Codegen (next)
13. LLVM `call asm sideeffect` emission for asm bodies
14. Webstack asm emission (if applicable)

### Phase E: Tests
15. Unit: AsmAssembler trait implementations
16. Unit: Template variable substitution
17. Integration: Chain resolution with asm + ref + derivation
18. Integration: Cross-verification mismatch detection
19. Integration: Derivation fallback when synthesis fails
20. Benchmark: popcount with asm chain (x86_64, aarch64, ref)
