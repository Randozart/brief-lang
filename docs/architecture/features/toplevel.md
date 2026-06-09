# TopLevel Features — Pattern B Architecture

**Date:** 2026-06-09  
**Phase:** 3  
**Status:** Feature files exist with stub structs; dispatch not yet migrated

## Design

17 TopLevel variants (plus TypeDef from Phase 1.5) are extracted into
individual feature files under `src/features/toplevel/`. Each file contains
a Pattern B struct that wraps the existing AST type.

## File Layout

```
src/features/toplevel/
  mod.rs              — Module declarations for all 19 modules
  typedef.rs          — TopLevel::TypeDef (Phase 1.5 — implemented)
  signature.rs        — TopLevel::Signature(Signature)
  definition.rs       — TopLevel::Definition(Definition)
  transaction.rs      — TopLevel::Transaction(Transaction)
  state_decl.rs       — TopLevel::StateDecl(StateDecl)
  trigger.rs          — TopLevel::Trigger(TriggerDeclaration)
  constant.rs         — TopLevel::Constant(Constant)
  import_lnk.rs       — TopLevel::Import(Import)
  foreign.rs          — TopLevel::ForeignBinding { name, toml_path, ... }
  resource.rs         — TopLevel::ResourceDecl(ResourceDeclaration)
  struct_def.rs       — TopLevel::Struct(StructDefinition)
  rstruct.rs          — TopLevel::RStruct(RStructDefinition)
  enum_def.rs         — TopLevel::Enum(EnumDefinition)
  render.rs           — TopLevel::RenderBlock(RenderBlock)
  svg.rs              — TopLevel::SvgComponent { name, content }
  sync_group.rs       — TopLevel::SyncGroup { domains, item }
  test.rs             — TestItem (pragmas)
  assertion.rs        — AssertionItem (pragmas)
```

## Trait Pattern

Unlike Expr and Statement features, TopLevel items do not yet have
dedicated dispatch traits. The pass files iterate `program.items` and
match on `TopLevel::Transaction`, `TopLevel::Struct`, etc. directly.

Future phases may introduce a `TopLevel*` trait family for codegen
dispatch, but currently each feature file is purely organizational —
holding the struct definition co-located with its documentation.

## TypeDef (Phase 1.5)

TypeDef is the most developed TopLevel feature. It has:
- 13-compiler-primitive kernel (Bytes, Alignment, Endian, etc.)
- Pass 1 resolver in `type_universe.rs`
- 5 stub trait implementations in `typedef.rs`
- Real Pass 1 freeze logic (chain resolution, inherit/override)

See `features/typedef.md` for the full design.
