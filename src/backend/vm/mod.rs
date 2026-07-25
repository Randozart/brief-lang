// 2026-07-25: VM backend — emits .lair bytecode from typed AST.
// This backend implements BackendKind::Vm, compiling Brief programs
// to portable stack-based bytecode for the tamer VM interpreter.
//
// The VM backend follows the same structure as the LLVM backend:
// emit_expr, emit_stmt, emit_toplevel, assembler.
// Currently: assembler is functional, expr/stmt/toplevel emission
// is built incrementally as the tamer source code expands.

pub mod assembler;

pub use assembler::Assembler;
