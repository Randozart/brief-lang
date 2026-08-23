# Phase 9 — Ownership Algebra Design

**Date:** 2026-08-20
**Status:** Design locked; implementation pending
**Normative source:** `spec/SPEC.md` §14.1–§14.4
**Supersedes:** tracker row "ownership algebra + `.s` enforcement (Phase 9)" in
`docs/plans/2026-08-15-spec-implementation-status.md`

## Scope

Everything in §14 is in scope for Phase 9:

- Parse the five ownership strategy keywords at parameter and return positions
- Resolve them through the mechanism registry
- Enforce consistency (proof-vs-decision hierarchy)
- `.s` enforcement (severity mapping)
- Wire existing `free`/`keep` (§12, already implemented) to the resolved algebra
- Backend realization (Layer 4) — the DropInjector and lifetime analysis already
  exist; they consume the resolved ownership constraints instead of guessing

## Architecture: four layers

### Layer 1 — Parse (mechanical, no semantics)

- `parse_parameter_list` recognizes an optional ownership prefix before the
  param name: `borrow` / `consume` / `owned` / `shared` / `borrowed<source>`.
- Same for return position: `-> owned Node`, `-> borrowed<source> Slice`.
- AST change: `Vec<(String, Type)>` becomes `Vec<Param>` where
  `Param { name, ty, ownership: Option<OwnershipCategory> }`.
  Same for `OutputType`.
- No resolution, no enforcement, no codegen in this layer.

### Layer 2 — Resolve (frontend, registry-driven)

- A resolver pass (sibling to the typechecker, or a phase within it) takes the
  parsed ownership categories and resolves them through the mechanism registry.
- The registry answers: for this category, at this boundary kind
  (frgn vs. defn vs. txn), what are the constraints? (retain-after-call
  allowed? who frees? exclusivity obligation? lifetime bound source?)
- Output: a set of **ownership constraints** attached to the call graph edges —
  not code, not IR, just the semantic facts. Same shape as the effect set in
  §14.4.
- The five categories are **program-independent** (compile with `--no-stdlib`,
  no config). The resolver has built-in defaults for each category; the
  registry only *refines* (e.g., a custom `shared` policy with a specific
  retain/release mechanism).

### Layer 3 — Enforce (frontend, profile-gated)

- A verification pass checks the resolved constraints against the actual call
  graph.
- Runs in all profiles. Severity is profile-dependent (see `.s` below).
- Ownership violations (e.g., a `borrow` param retained past the call) are
  **hard errors in all profiles** — contract violations, not lifetime warnings.

### Layer 4 — Realize (backend, per-target)

- The backend consumes the resolved ownership constraints and chooses the
  physical realization: where to emit the release, whether to elide it (proven
  last-use), how to encode exclusivity (atomic RMW, cell boundary, etc.).
- §14.2/§14.3 territory — pointer safety, `free`/`keep` scheduling,
  dangling-pointer detection.
- The backend already has the machinery (DropInjector pass, lifetime analysis);
  Phase 9 feeds it the ownership constraints instead of letting it guess.
- `free`/`keep` (already implemented, §12 2026-08-09) become *wired to the
  algebra*: `free` checks the ownership category before emitting a release.

## Registry

**Sibling file** `config/ownership-strategies.dbvl` — not mixed into
`alloc-strategies.dbvl`.

Reason: the two registries answer different questions.

- `alloc-strategies.dbvl` — "how do I allocate/free this strategy?" (IR templates)
- `ownership-strategies.dbvl` — "what are the constraints for this ownership
  category at this boundary?" (semantic rules)

The SPEC keeps them separate: "Allocation and destruction policy is configured
rather than hardcoded into the ownership keyword. Read/write permission belongs
to effects." The resolver consults both; they stay orthogonal.

## Three-tier resolution (proof-vs-decision hierarchy)

### Tier 1 — Prove

The compiler derives a single best category from context (type + boundary kind
+ usage: read? written? retained past the call? aliased?). If the derivation is
unique, it's **silent**. The keyword is never needed.

- Normal profile: silent
- `.s` profile: silent

### Tier 2 — Guardrail with request for disambiguation

The derivation is genuinely ambiguous. The compiler picks a **reasonable
default** (the most conservative option that keeps the program correct) and
emits a **warning** naming the ambiguity, the derived default, and the
alternative(s) considered.

- Normal profile: warning
- `.s` profile: **hard error** (this is the `.s` enforcement — it raises the
  severity of the "I had to guess" tier, it doesn't add new checks)

**Diagnostic format (locked 2026-08-20):**

The warning names the specific alternative(s) and why they're consistent with
usage. Example:

```
warning: ownership category ambiguous for param `input` — derived `borrow` (conservative);
         `consume` also consistent with usage. Use `consume input: Ptr<Byte>` if the callee
         takes ownership.
```

Not the shorter form that omits the alternative. The helpful-messages rule
(Phase 6) applies: name the specific choice and why.

**"Reasonable default" = most conservative option that keeps the program
correct** — never the most permissive:

- `borrow` over `consume` (narrower claim: "I won't retain this" is safer to
  assume than "I'm taking this")
- `owned` over `borrowed<source>` for returns (the return owns its result
  unless the author says the lifetime is bounded by an input)
- `shared` over exclusive for mutable access (shared is the conservative claim;
  exclusive requires proof)

The conservative default is always *safe* (no use-after-free, no data race);
it might be *suboptimal* (misses an optimization the author intended). The
warning says "I kept it safe, but you might want to tell me the stronger
claim."

### Tier 3 — Error

The author *declared* a category that's provably inconsistent with usage
(a `borrow` param retained past the call, a `consume` param used after the
call). **Hard error in all profiles.** Not a warning, not `.s`-only. The
compiler never ships a provably wrong program.

## `.s` enforcement

- `.s` is a **frontend** gate (the enforcement pass runs after typecheck,
  before codegen). The backend doesn't know about `.s`.
- `.s` changes **acceptance criteria, not runtime semantics or grammar**
  (§3.2). It doesn't add checks; it raises the severity of Tier 2
  (warning → error).
- Tier 1: silent in all profiles.
- Tier 2: warning in normal, hard error in `.s`.
- Tier 3: hard error in all profiles.

## Derived path vs. declared path

The five categories are **strategy keywords** — transparent, ordinary words,
never a speed win. The default (no keyword) must be the efficient path
(zero knowledge tax).

- **Derived:** no keyword → compiler infers from type + boundary kind + usage.
  This is the "prove the best default" part. Most of the real work is here.
- **Declared:** keyword present → compiler uses the declared category and
  *verifies* it's consistent with usage. A `borrow` param that's actually
  retained is an error, not a silent override.

## Interaction with `free`/`keep` (§14.3)

Already implemented (§12, 2026-08-09). Phase 9 wires them to the algebra in
Layer 4:

- "Proven last use may be scheduled automatically" → Tier 1 (silent).
- "Unresolved lifetime is a warning in normal profiles and a hard error in
  `.s`" → Tier 2.
- `free` checks the ownership category before emitting a release.
- `keep` transfers the value to boundary/owner lifetime.

Same three-tier shape as the ownership algebra itself.

## Interaction with effects (§14.4)

The frontend already infers one effect set (reads/writes, alloc/release,
spawn/await, FFI/IO, blocking, purity, cancellation). Ownership constraints are
a **refinement** of that effect set — they add the boundary-specific facts
(who retains, who frees, exclusivity obligation) that the general effect set
doesn't capture. `.^^Effects` reflection can expose both.

## Implementation checklist

- [ ] Layer 1: parse the five keywords at param + return positions
- [ ] Layer 1: AST `Param` / `OutputType` gain `ownership` field
- [ ] Layer 2: `config/ownership-strategies.dbvl` (sibling to alloc-strategies)
- [ ] Layer 2: resolver pass — derived path (Tier 1) + declared path (Tier 3)
- [ ] Layer 2: conservative-default selection (Tier 2)
- [ ] Layer 3: enforcement pass — consistency check on call graph
- [ ] Layer 3: `.s` severity mapping (Tier 2 warning → error)
- [ ] Layer 3: diagnostic format — name the alternative(s) + why consistent
- [ ] Layer 4: wire `free`/`keep` to resolved ownership category
- [ ] Layer 4: backend consumes ownership constraints (DropInjector, lifetime)
- [ ] Tests: Tier 1 (proven, silent), Tier 2 (ambiguous, warning + `.s` error),
      Tier 3 (declared inconsistent, hard error)
- [ ] Tests: conservative-default selection (borrow > consume, owned > borrowed,
      shared > exclusive)
- [ ] Tests: `free`/`keep` wired to algebra
- [ ] Tests: `.s` enforcement (Tier 2 → error)
