# Self-Hosted (Briv-in-Briv) LLVM Backend

## Overview

The self-hosted LLVM backend lives at `lib/compiler/backends/llvm.bv`. It mirrors the Rust backend's logic but emits `.ll` text using Briv's `StringBuilder` pattern.

## Key Translations

| Rust Feature | Briv Equivalent |
|--------------|------------------|
| `String` (`push_str`, `push`) | `StringBuilder` (`sb.append`, `sb.append_line`) |
| `writeln!(output, "...")` | `&sb = sb.append_line("...");` |
| `format!("val={}", x)` | `let s = "val=" ++ x.to_string();` |
| `match expr { ... }` | `match expr { ... }` (self-hosted!) |
| `Vec<T>` / iteration | Recursive definitions on `List<T>` |
| `todo!()` / `unimplemented!()` | `term Err("not implemented")` |
| `HashMap<String, Value>` | `let map: HashMap<String, Value> = new_map();` |
| `Option<T>` | `option::Some(val)` / `option::None` |

## Module Structure

```
lib/compiler/backends/
├── mod.bv              # Backend router (dispatch by name)
├── llvm.bv             # LLVM IR emitter (this file)
├── lowering.bv         # Pre-emission AST lowering (shared with other backends)
├── abi.bv              # Type → LLVM type mapping
└── README.md
```

## `llvm.bv` Function Signatures

```briv
import string_builder from "std/string_builder.bv";
import option from "std/option.bv";
import result from "std/result.bv";
import list from "std/list.bv";
import call_graph from "../call_graph.bv";

// ── Entry Point ────────────────────────────────────────────────────

defn compile_to_llvm(program: Program, state: CompilerState, cg: CallGraphResult) -> Result<String, String> {
    let sb = new_string_builder();
    let sb = emit_module_header(sb, program, state);
    let sb = emit_state_type(sb, program);
    let sb = emit_global_state(sb);
    let sb = emit_functions(sb, program, cg);
    let sb = emit_reactor_loop(sb, program, cg);
    let sb = emit_intrinsics(sb);
    term Ok(sb.to_string());
};

// ── Module Header ──────────────────────────────────────────────────

defn emit_module_header(sb: StringBuilder, program: Program, state: CompilerState) -> StringBuilder {
    let sb = sb.append_line("; ModuleID = '" ++ state.file_path ++ "'");
    let sb = sb.append_line("source_filename = \"" ++ state.file_path ++ "\"");
    let sb = sb.append_line("target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"");
    let sb = sb.append_line("target triple = \"x86_64-unknown-linux-gnu\"");
    term sb;
};

// ── Type Emission ──────────────────────────────────────────────────

defn emit_struct_type(sb: StringBuilder, name: String, fields: List<String>) -> StringBuilder {
    let sb = sb.append_line("%struct." ++ name ++ " = type { " ++ list.join(fields, ", ") ++ " }");
    term sb;
};

defn emit_enum_type(sb: StringBuilder, name: String, variants: List<EnumVariant>) -> StringBuilder {
    // Emit: %struct.EnumName = type { i64, [largest variant payload] }
    // discriminant at offset 0, variant data at offset 8
    let sb = sb.append_line("; enum " ++ name ++ " — todo: full variant payload union");
    term sb;
};

// ── Function Emission ─────────────────────────────────────────────

defn emit_txn_function(sb: StringBuilder, txn: TopTxn, cg: CallGraphResult) -> StringBuilder {
    let name = txn.name;
    let has_cycle = cg.has_cycle();

    // Function signature
    let attrs = if has_cycle { "" } else { " noalias nocapture" };
    let sb = sb.append_line("");
    let sb = sb.append_line("define void @" ++ name ++ "(%struct.State*" ++ attrs ++ " %state) local_unnamed_addr #0 {");
    let sb = sb.append_line("entry:");

    // Load fields from state
    let sb = emit_state_loads(sb, txn);

    // Precondition: !range metadata
    let sb = emit_precondition_metadata(sb, txn);

    // Body
    let sb = emit_statements(sb, txn.body, 1);

    // Postcondition
    let sb = sb.append_line("  call void @llvm.assume(i1 true)");

    // Commit stores
    let sb = emit_state_stores(sb, txn);

    let sb = sb.append_line("  ret void");
    let sb = sb.append_line("}");
    term sb;
};

// ── Pattern Match → switch ───────────────────────────────────────

defn emit_match(sb: StringBuilder, match_expr: MatchExpr, indent: Int) -> StringBuilder {
    // match scrutinee { arm1 => body1, _ => default }
    // → switch i64 %disc, label %default [ i64 0, label %arm0 ... ]
    let sb = sb.append_line(indent_string(indent) ++ "switch i64 %discriminant, label %default [");
    let sb = emit_match_arms(sb, match_expr.arms, indent + 1);
    let sb = sb.append_line(indent_string(indent) ++ "]");
    term sb;
};

// ── Reactor Loop ─────────────────────────────────────────────────

defn emit_reactor(sb: StringBuilder, program: Program, cg: CallGraphResult) -> StringBuilder {
    let has_cycle = cg.has_cycle();
    let attrs = if has_cycle { "" } else { " norecurse" };

    let sb = sb.append_line("");
    let sb = sb.append_line("define i32 @main() local_unnamed_addr #0 {");
    let sb = sb.append_line("  call void @init_state()");
    let sb = sb.append_line("  br label %tick");
    let sb = sb.append_line("tick:");
    let sb = sb.append_line("  call void @reactor_tick()");
    let sb = sb.append_line("  br label %tick");
    let sb = sb.append_line("}");
    let sb = sb.append_line("");

    let sb = sb.append_line("define void @reactor_tick()" ++ attrs ++ " #0 {");
    let sb = emit_tick_body(sb, program, has_cycle, 1);
    let sb = sb.append_line("  ret void");
    let sb = sb.append_line("}");
    term sb;
};

// ── FFI Declarations ─────────────────────────────────────────────

defn emit_ffi_decl(sb: StringBuilder, frgn: ForeignBinding) -> StringBuilder {
    let ret_type = if frgn.ret_type == "Void" { "void" } else { frgn.ret_type };
    let param_types = list.join(frgn.param_types, ", ");
    let sb = sb.append_line("declare " ++ ret_type ++ " @" ++ frgn.name ++ "(" ++ param_types ++ ") #1");
    term sb;
};

// ── Intrinsics ───────────────────────────────────────────────────

defn emit_intrinsics(sb: StringBuilder) -> StringBuilder {
    let sb = sb.append_line("");
    let sb = sb.append_line("declare void @llvm.assume(i1) #1");
    let sb = sb.append_line("");
    let sb = sb.append_line("attributes #0 = { mustprogress nofree norecurse nosync nounwind willreturn memory(argmem: readwrite) }");
    let sb = sb.append_line("attributes #1 = { nocallback nofree nosync nounwind willreturn memory(inaccessiblemem: write) }");
    term sb;
};
```

## Wire into `main.bv`

In `lib/compiler/main.bv`, add to the backend dispatch (around line 95-106):

```briv
[state.backend == "llvm"] {
    let sb_result = compile_to_llvm(program, state, cg_result);
    [is_ok(sb_result)] {
        let output = unwrap(sb_result);
        // Write to output file (same as other backends)
        let write_result = __write_file(state.file_path ++ ".ll", output);
        ...[write_result] { ... };
    };
    [is_err(sb_result)] {
        term Err(unwrap_err(sb_result));
    };
};
```

## Implementation Priority for Self-Hosted

1. **Parse-only mode** — `Briv-compiler selfhost counter.bv --target llvm` runs without crashing
2. **Module header + `%State` type** — basic `.ll` structure
3. **load/store for transactions** — one `txn` with a single `Int` field
4. **Match → `switch`** — pattern matching in self-hosted code
5. **Reactor loop + acyclic detection** — wire call graph analysis into codegen
6. **`noalias` + `!range`** — contract metadata injection
7. **FFI `declare`** — foreign function declarations
8. **Parity with Rust backend** — full `.ll` emission for any `bv` file