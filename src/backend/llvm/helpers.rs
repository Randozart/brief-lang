// ── Expression Codegen Helper Functions ─────────────────────────
//
// 2026-06-29: Extracted from emit_expr.rs to enable submodule extraction.
// These are additional `impl LlvmBackend` methods used by expression
// codegen. Split via Rust's "impl block split" pattern — multiple files
// can define `impl Type { ... }` within the same module as long as they
// don't duplicate method signatures.
//
// Visibility convention:
//   `pub(crate)`  — visible to entire crate (for functions that become
//                    part of LlvmBackend's semi-public API)
//   `pub(super)`  — visible to parent `llvm` module and all its children
//                   (for functions that should stay backend-internal)
//   (private)     — visible only within this file (for internal helpers)

use crate::ast::{ArrowDir, BracketOp, Expr, Intrinsic, MatchArm, MatchPattern, OpDeclaration, OpRune, OutputType, Pattern, PipeChain, PipeStep, ProjectionTarget, SliceCoordinate, Statement, Type};
use crate::backend::llvm::*;
use crate::features::arrow::{ArrowMutExpr, ArrowDiscardExpr, ArrowTransferExpr};
use crate::features::binary_op::BinaryOpExpr;
use crate::features::block::BlockExpr;
use crate::features::call::CallExpr;
use crate::features::collection::{ListLiteralExpr, MapLiteralExpr, MultiSliceExpr, SetLiteralExpr, SliceExpr};
use crate::features::ellipsis::EllipsisExpr;
use crate::features::field::{FieldAccessExpr, ObjectLiteralExpr, StructInstanceExpr};
use crate::features::pattern::{MatchExpr, PatternMatchExpr};
use crate::features::projection::ProjectionExpr;
use crate::features::sigcall::SigCallExpr;
use crate::features::subtype::SubtypeProjectionExpr;
use crate::features::traits::{ExprCodegenLLVM, ExprDispatch};
use crate::features::tuple::{TupleDestructureExpr, TupleExpr};
use crate::features::unary_op::UnaryOpExpr;
use std::collections::HashMap;
use std::fmt::Write;

impl LlvmBackend {
    pub(super) fn rewrite_cell_identifiers(expr: &Expr, cell_name: &str) -> Expr {
        let p = |name: &str| -> String { format!("cell${}${}", cell_name, name) };
        match expr {
            // Leaf nodes — no identifiers
            Expr::Integer(_) | Expr::IntegerSuffixed(_, _) | Expr::Float(_) | Expr::Float64(_) | Expr::String(_) | Expr::RegexLiteral(_)
                | Expr::Char(_) | Expr::Bool(_) | Expr::Term | Expr::Ellipsis
                | Expr::SharedMem(_) => expr.clone(),
            Expr::Literal(lit) => Expr::Literal(lit.clone()),
            // Identifier variants — rewrite to prefixed form
            Expr::Identifier(name) => Expr::Identifier(p(name)),
            Expr::AddrOf(inner) => Expr::AddrOf(Box::new(Self::rewrite_cell_identifiers(inner, cell_name))),
            Expr::Deref(inner) => Expr::Deref(Box::new(Self::rewrite_cell_identifiers(inner, cell_name))),
            Expr::PriorState(name) => Expr::PriorState(p(name)),
            Expr::EllipsisExpr(e) => Expr::EllipsisExpr(e.clone()),
            Expr::TypeRef(name) => Expr::TypeRef(name.clone()),
            // Arrow variants
            Expr::ArrowMut { dir, target, index, value, consume } => Expr::ArrowMut { consume: *consume, 
                dir: dir.clone(), target: Box::new(Self::rewrite_cell_identifiers(target, cell_name)),
                index: Box::new(Self::rewrite_cell_identifiers(index, cell_name)),
                value: value.as_ref().map(|v| Box::new(Self::rewrite_cell_identifiers(v, cell_name))),
            },
            Expr::ArrowDiscard { target, index } => Expr::ArrowDiscard {
                target: Box::new(Self::rewrite_cell_identifiers(target, cell_name)),
                index: Box::new(Self::rewrite_cell_identifiers(index, cell_name)),
            },
            Expr::ArrowTransfer { dest, source, filter, consume } => Expr::ArrowTransfer { consume: *consume, dest: Box::new(Self::rewrite_cell_identifiers(dest, cell_name)),
                source: Box::new(Self::rewrite_cell_identifiers(source, cell_name)),
                filter: filter.as_ref().map(|f| Box::new(Self::rewrite_cell_identifiers(f, cell_name))),
            },
            Expr::ArrowMutExpr(e) => Expr::ArrowMutExpr(ArrowMutExpr {
                dir: e.dir.clone(),
                consume: e.consume,
                target: Box::new(Self::rewrite_cell_identifiers(&e.target, cell_name)),
                index: Box::new(Self::rewrite_cell_identifiers(&e.index, cell_name)),
                value: e.value.as_ref().map(|v| Box::new(Self::rewrite_cell_identifiers(v, cell_name))),
            }),
            Expr::ArrowDiscardExpr(e) => Expr::ArrowDiscardExpr(ArrowDiscardExpr {
                target: Box::new(Self::rewrite_cell_identifiers(&e.target, cell_name)),
                index: Box::new(Self::rewrite_cell_identifiers(&e.index, cell_name)),
            }),
            Expr::ArrowTransferExpr(e) => Expr::ArrowTransferExpr(ArrowTransferExpr {
                consume: e.consume,
                dest: Box::new(Self::rewrite_cell_identifiers(&e.dest, cell_name)),
                source: Box::new(Self::rewrite_cell_identifiers(&e.source, cell_name)),
                filter: e.filter.as_ref().map(|f| Box::new(Self::rewrite_cell_identifiers(f, cell_name))),
            }),
            // Binary ops — two children
            Expr::Add(l, r) => Expr::Add(Box::new(Self::rewrite_cell_identifiers(l, cell_name)), Box::new(Self::rewrite_cell_identifiers(r, cell_name))),
            Expr::Sub(l, r) => Expr::Sub(Box::new(Self::rewrite_cell_identifiers(l, cell_name)), Box::new(Self::rewrite_cell_identifiers(r, cell_name))),
            Expr::Mul(l, r) => Expr::Mul(Box::new(Self::rewrite_cell_identifiers(l, cell_name)), Box::new(Self::rewrite_cell_identifiers(r, cell_name))),
            Expr::Div(l, r) => Expr::Div(Box::new(Self::rewrite_cell_identifiers(l, cell_name)), Box::new(Self::rewrite_cell_identifiers(r, cell_name))),
            Expr::Mod(l, r) => Expr::Mod(Box::new(Self::rewrite_cell_identifiers(l, cell_name)), Box::new(Self::rewrite_cell_identifiers(r, cell_name))),
            Expr::Eq(l, r) => Expr::Eq(Box::new(Self::rewrite_cell_identifiers(l, cell_name)), Box::new(Self::rewrite_cell_identifiers(r, cell_name))),
            Expr::Ne(l, r) => Expr::Ne(Box::new(Self::rewrite_cell_identifiers(l, cell_name)), Box::new(Self::rewrite_cell_identifiers(r, cell_name))),
            Expr::Lt(l, r) => Expr::Lt(Box::new(Self::rewrite_cell_identifiers(l, cell_name)), Box::new(Self::rewrite_cell_identifiers(r, cell_name))),
            Expr::Le(l, r) => Expr::Le(Box::new(Self::rewrite_cell_identifiers(l, cell_name)), Box::new(Self::rewrite_cell_identifiers(r, cell_name))),
            Expr::Gt(l, r) => Expr::Gt(Box::new(Self::rewrite_cell_identifiers(l, cell_name)), Box::new(Self::rewrite_cell_identifiers(r, cell_name))),
            Expr::Ge(l, r) => Expr::Ge(Box::new(Self::rewrite_cell_identifiers(l, cell_name)), Box::new(Self::rewrite_cell_identifiers(r, cell_name))),
            Expr::And(l, r) => Expr::And(Box::new(Self::rewrite_cell_identifiers(l, cell_name)), Box::new(Self::rewrite_cell_identifiers(r, cell_name))),
            Expr::Or(l, r) => Expr::Or(Box::new(Self::rewrite_cell_identifiers(l, cell_name)), Box::new(Self::rewrite_cell_identifiers(r, cell_name))),
            Expr::BitAnd(l, r) => Expr::BitAnd(Box::new(Self::rewrite_cell_identifiers(l, cell_name)), Box::new(Self::rewrite_cell_identifiers(r, cell_name))),
            Expr::BitOr(l, r) => Expr::BitOr(Box::new(Self::rewrite_cell_identifiers(l, cell_name)), Box::new(Self::rewrite_cell_identifiers(r, cell_name))),
            Expr::BitXor(l, r) => Expr::BitXor(Box::new(Self::rewrite_cell_identifiers(l, cell_name)), Box::new(Self::rewrite_cell_identifiers(r, cell_name))),
            Expr::Shl(l, r) => Expr::Shl(Box::new(Self::rewrite_cell_identifiers(l, cell_name)), Box::new(Self::rewrite_cell_identifiers(r, cell_name))),
            Expr::Shr(l, r) => Expr::Shr(Box::new(Self::rewrite_cell_identifiers(l, cell_name)), Box::new(Self::rewrite_cell_identifiers(r, cell_name))),
            Expr::Concat(l, r) => Expr::Concat(Box::new(Self::rewrite_cell_identifiers(l, cell_name)), Box::new(Self::rewrite_cell_identifiers(r, cell_name))),
            // Unary ops
            Expr::Not(e) => Expr::Not(Box::new(Self::rewrite_cell_identifiers(e, cell_name))),
            Expr::Neg(e) => Expr::Neg(Box::new(Self::rewrite_cell_identifiers(e, cell_name))),
            Expr::BitNot(e) => Expr::BitNot(Box::new(Self::rewrite_cell_identifiers(e, cell_name))),
            // IsType / FromCheck / Like
            Expr::IsType(e, target) => Expr::IsType(Box::new(Self::rewrite_cell_identifiers(e, cell_name)), target.clone()),
            Expr::FromCheck(e, ty) => Expr::FromCheck(Box::new(Self::rewrite_cell_identifiers(e, cell_name)), ty.clone()),
            Expr::Like(l, r) => Expr::Like(Box::new(Self::rewrite_cell_identifiers(l, cell_name)), Box::new(Self::rewrite_cell_identifiers(r, cell_name))),
            // Pattern B: BinaryOp / UnaryOp
            Expr::BinaryOp(e) => Expr::BinaryOp(Box::new(BinaryOpExpr {
                kind: e.kind,
                left: Box::new(Self::rewrite_cell_identifiers(&e.left, cell_name)),
                right: Box::new(Self::rewrite_cell_identifiers(&e.right, cell_name)),
            })),
            Expr::UnaryOp(e) => Expr::UnaryOp(Box::new(UnaryOpExpr {
                kind: e.kind,
                operand: Box::new(Self::rewrite_cell_identifiers(&e.operand, cell_name)),
            })),
            // Cast and Projection
            Expr::Cast(e, ty) => Expr::Cast(Box::new(Self::rewrite_cell_identifiers(e, cell_name)), ty.clone()),
            Expr::Projection { source, target } => Expr::Projection {
                source: Box::new(Self::rewrite_cell_identifiers(source, cell_name)),
                target: target.clone(),
            },
            Expr::ProjectionExpr(e) => Expr::ProjectionExpr(ProjectionExpr {
                source: Box::new(Self::rewrite_cell_identifiers(&e.source, cell_name)),
                target: e.target.clone(),
            }),
            // Calls
            Expr::Call(name, args) => Expr::Call(
                name.clone(),
                args.iter().map(|a| Self::rewrite_cell_identifiers(a, cell_name)).collect(),
            ),
            Expr::CallExpr(e) => Expr::CallExpr(CallExpr {
                name: e.name.clone(),
                args: e.args.iter().map(|a| Self::rewrite_cell_identifiers(a, cell_name)).collect(),
            }),
            Expr::CellCall(callee, args) => Expr::CellCall(
                Box::new(Self::rewrite_cell_identifiers(callee, cell_name)),
                args.iter().map(|a| Self::rewrite_cell_identifiers(a, cell_name)).collect(),
            ),
            // Template/Macro calls
            Expr::TemplateCall { name, args, block, span } => Expr::TemplateCall {
                name: name.clone(),
                args: args.iter().map(|a| Self::rewrite_cell_identifiers(a, cell_name)).collect(),
                block: block.clone(),
                span: *span,
            },
            Expr::MacroCall { name, args, block, span } => Expr::MacroCall {
                name: name.clone(),
                args: args.iter().map(|a| Self::rewrite_cell_identifiers(a, cell_name)).collect(),
                block: block.clone(),
                span: *span,
            },
            Expr::IntrinsicCall { intrinsic, args } => Expr::IntrinsicCall {
                intrinsic: intrinsic.clone(),
                args: args.iter().map(|a| Self::rewrite_cell_identifiers(a, cell_name)).collect(),
            },
            // Collections
            Expr::ListLiteral(items) => Expr::ListLiteral(
                items.iter().map(|a| Self::rewrite_cell_identifiers(a, cell_name)).collect(),
            ),
            Expr::ListLiteralExpr(e) => Expr::ListLiteralExpr(ListLiteralExpr {
                elements: e.elements.iter().map(|a| Self::rewrite_cell_identifiers(a, cell_name)).collect(),
            }),
            Expr::MapLiteral(pairs) => Expr::MapLiteral(
                pairs.iter().map(|(k, v)| (Self::rewrite_cell_identifiers(k, cell_name), Self::rewrite_cell_identifiers(v, cell_name))).collect(),
            ),
            Expr::MapLiteralExpr(e) => Expr::MapLiteralExpr(MapLiteralExpr {
                entries: e.entries.iter().map(|(k, v)| (Self::rewrite_cell_identifiers(k, cell_name), Self::rewrite_cell_identifiers(v, cell_name))).collect(),
            }),
            Expr::SetLiteral(items) => Expr::SetLiteral(
                items.iter().map(|a| Self::rewrite_cell_identifiers(a, cell_name)).collect(),
            ),
            Expr::SetLiteralExpr(e) => Expr::SetLiteralExpr(SetLiteralExpr {
                entries: e.entries.iter().map(|a| Self::rewrite_cell_identifiers(a, cell_name)).collect(),
            }),
            Expr::ListIndex(list, idx) => Expr::ListIndex(
                Box::new(Self::rewrite_cell_identifiers(list, cell_name)),
                Box::new(Self::rewrite_cell_identifiers(idx, cell_name)),
            ),
            // Slice / MultiSlice
            Expr::Slice { value, start, end, stride, mask } => Expr::Slice {
                value: Box::new(Self::rewrite_cell_identifiers(value, cell_name)),
                start: start.as_ref().map(|s| Box::new(Self::rewrite_cell_identifiers(s, cell_name))),
                end: end.as_ref().map(|e| Box::new(Self::rewrite_cell_identifiers(e, cell_name))),
                stride: stride.as_ref().map(|s| Box::new(Self::rewrite_cell_identifiers(s, cell_name))),
                mask: mask.as_ref().map(|m| Box::new(Self::rewrite_cell_identifiers(m, cell_name))),
            },
            Expr::SliceExpr(e) => Expr::SliceExpr(SliceExpr {
                value: Box::new(Self::rewrite_cell_identifiers(&e.value, cell_name)),
                start: e.start.as_ref().map(|s| Box::new(Self::rewrite_cell_identifiers(s, cell_name))),
                end: e.end.as_ref().map(|s| Box::new(Self::rewrite_cell_identifiers(s, cell_name))),
                stride: e.stride.as_ref().map(|s| Box::new(Self::rewrite_cell_identifiers(s, cell_name))),
                mask: e.mask.as_ref().map(|m| Box::new(Self::rewrite_cell_identifiers(m, cell_name))),
            }),
            Expr::MultiSlice { value, ops } => Expr::MultiSlice {
                value: Box::new(Self::rewrite_cell_identifiers(value, cell_name)),
                ops: ops.clone(),
            },
            Expr::MultiSliceExpr(e) => Expr::MultiSliceExpr(MultiSliceExpr {
                value: Box::new(Self::rewrite_cell_identifiers(&e.value, cell_name)),
                ops: e.ops.clone(),
            }),
            // Field access
            Expr::FieldAccess(obj, field) => Expr::FieldAccess(
                Box::new(Self::rewrite_cell_identifiers(obj, cell_name)),
                field.clone(),
            ),
            Expr::FieldAccessExpr(e) => Expr::FieldAccessExpr(FieldAccessExpr {
                obj: Box::new(Self::rewrite_cell_identifiers(&e.obj, cell_name)),
                field: e.field.clone(),
            }),
            // Struct / Object
            Expr::StructInstance(name, fields) => Expr::StructInstance(
                name.clone(),
                fields.iter().map(|(n, e)| (n.clone(), Self::rewrite_cell_identifiers(e, cell_name))).collect(),
            ),
            Expr::StructInstanceExpr(e) => Expr::StructInstanceExpr(StructInstanceExpr {
                typename: e.typename.clone(),
                fields: e.fields.iter().map(|(n, e)| (n.clone(), Self::rewrite_cell_identifiers(e, cell_name))).collect(),
            }),
            Expr::ObjectLiteral(fields) => Expr::ObjectLiteral(
                fields.iter().map(|(n, e)| (n.clone(), Self::rewrite_cell_identifiers(e, cell_name))).collect(),
            ),
            Expr::ObjectLiteralExpr(e) => Expr::ObjectLiteralExpr(ObjectLiteralExpr {
                fields: e.fields.iter().map(|(n, e)| (n.clone(), Self::rewrite_cell_identifiers(e, cell_name))).collect(),
            }),
            // Pattern / Match
            Expr::PatternMatch { value, variant, fields } => Expr::PatternMatch {
                value: Box::new(Self::rewrite_cell_identifiers(value, cell_name)),
                variant: variant.clone(),
                fields: fields.clone(),
            },
            Expr::PatternMatchExpr(e) => Expr::PatternMatchExpr(PatternMatchExpr {
                value: Box::new(Self::rewrite_cell_identifiers(&e.value, cell_name)),
                variant: e.variant.clone(),
                fields: e.fields.clone(),
            }),
            Expr::Match { value, arms } => Expr::Match {
                value: Box::new(Self::rewrite_cell_identifiers(value, cell_name)),
                arms: arms.clone(),
            },
            Expr::MatchExpr(e) => Expr::MatchExpr(MatchExpr {
                value: Box::new(Self::rewrite_cell_identifiers(&e.value, cell_name)),
                arms: e.arms.clone(),
            }),
            // Block
            Expr::Block(stmts, last) => Expr::Block(
                stmts.iter().map(|s| Self::rewrite_cell_stmt_identifiers(s, cell_name)).collect(),
                Box::new(Self::rewrite_cell_identifiers(last, cell_name)),
            ),
            Expr::BlockExpr(e) => Expr::BlockExpr(BlockExpr {
                stmts: e.stmts.iter().map(|s| Self::rewrite_cell_stmt_identifiers(s, cell_name)).collect(),
                last: Box::new(Self::rewrite_cell_identifiers(&e.last, cell_name)),
            }),
            // Quote / Interpolation
            Expr::Interpolate(name) => Expr::Interpolate(name.clone()),
            Expr::InterpolateExpr(e) => Expr::InterpolateExpr(Box::new(Self::rewrite_cell_identifiers(e, cell_name))),
            Expr::QuoteBlock { statements, trailing_expr } => Expr::QuoteBlock {
                statements: statements.iter().map(|s| Self::rewrite_cell_stmt_identifiers(s, cell_name)).collect(),
                trailing_expr: trailing_expr.as_ref().map(|e| Box::new(Self::rewrite_cell_identifiers(e, cell_name))),
            },
            // Tuple
            Expr::TupleDestructure(names, expr) => Expr::TupleDestructure(
                names.clone(),
                Box::new(Self::rewrite_cell_identifiers(expr, cell_name)),
            ),
            Expr::TupleDestructureExpr(e) => Expr::TupleDestructureExpr(TupleDestructureExpr {
                names: e.names.clone(),
                expr: Box::new(Self::rewrite_cell_identifiers(&e.expr, cell_name)),
            }),
            Expr::Tuple(items) => Expr::Tuple(
                items.iter().map(|a| Self::rewrite_cell_identifiers(a, cell_name)).collect(),
            ),
            Expr::TupleExpr(e) => Expr::TupleExpr(TupleExpr {
                exprs: e.exprs.iter().map(|a| Self::rewrite_cell_identifiers(a, cell_name)).collect(),
            }),
            // SigCall
            Expr::SigCall { modifier, expr } => Expr::SigCall {
                modifier: modifier.clone(),
                expr: Box::new(Self::rewrite_cell_identifiers(expr, cell_name)),
            },
            Expr::SigCallExpr(e) => Expr::SigCallExpr(SigCallExpr {
                modifier: e.modifier.clone(),
                expr: Box::new(Self::rewrite_cell_identifiers(&e.expr, cell_name)),
            }),
            // Subtype projection
            Expr::SubtypeProjection { source, ops } => Expr::SubtypeProjection {
                source: Box::new(Self::rewrite_cell_identifiers(source, cell_name)),
                ops: ops.clone(),
            },
            Expr::SubtypeProjectionExpr(e) => Expr::SubtypeProjectionExpr(SubtypeProjectionExpr {
                source: Box::new(Self::rewrite_cell_identifiers(&e.source, cell_name)),
                ops: e.ops.clone(),
            }),
            // DBVL
            Expr::DbvlTable { path, field_names, key_offsets, schema_name } => Expr::DbvlTable {
                path: path.clone(),
                field_names: field_names.clone(),
                key_offsets: key_offsets.clone(),
                schema_name: schema_name.clone(),
            },
            Expr::DbvlTableExpr(e) => Expr::DbvlTableExpr(e.clone()),
            // Pipe chain
            Expr::PipeChain(chain) => Expr::PipeChain(PipeChain {
                initial: Box::new(Self::rewrite_cell_identifiers(&chain.initial, cell_name)),
                steps: chain.steps.iter().map(|s| PipeStep {
                    target: Box::new(Self::rewrite_cell_identifiers(&s.target, cell_name)),
                    skip: s.skip,
                }).collect(),
            }),
            Expr::Within { body, fallback, .. } => Expr::Within {
                body: Box::new(Self::rewrite_cell_identifiers(body, cell_name)),
                bound: 0, retries: 0, unit: crate::ast::TimeUnit::Cycles,
                fallback: Box::new(Self::rewrite_cell_identifiers(fallback, cell_name)),
            },
        }
    }

    pub(super) fn rewrite_cell_stmt_identifiers(stmt: &Statement, cell_name: &str) -> Statement {
        match stmt {
            Statement::Assignment { lhs, expr, timeout, modifiers } => Statement::Assignment {
                lhs: Self::rewrite_cell_identifiers(lhs, cell_name),
                expr: Self::rewrite_cell_identifiers(expr, cell_name),
                timeout: timeout.clone(),
                modifiers: modifiers.clone(),
            },
            Statement::Unification { name, variant, fields, expr } => Statement::Unification {
                name: name.clone(),
                variant: variant.clone(),
                fields: fields.clone(),
                expr: Self::rewrite_cell_identifiers(expr, cell_name),
            },
            Statement::Guarded { condition, statements, .. } => Statement::Guarded {
                condition: Self::rewrite_cell_identifiers(condition, cell_name),
                statements: statements.iter().map(|s| Self::rewrite_cell_stmt_identifiers(s, cell_name)).collect(),
                metadata: HashMap::new(),
            },
            Statement::Term { values, swan_song, modifiers } => Statement::Term {
                values: values.iter().map(|v| v.as_ref().map(|e| Self::rewrite_cell_identifiers(e, cell_name))).collect(),
                swan_song: swan_song.as_ref().map(|s| Box::new(Self::rewrite_cell_stmt_identifiers(s, cell_name))),
                modifiers: modifiers.clone(),
            },
            Statement::TermBang { values, swan_song, modifiers } => Statement::TermBang {
                values: values.iter().map(|v| v.as_ref().map(|e| Self::rewrite_cell_identifiers(e, cell_name))).collect(),
                swan_song: swan_song.as_ref().map(|s| Box::new(Self::rewrite_cell_stmt_identifiers(s, cell_name))),
                modifiers: modifiers.clone(),
            },
            Statement::Escape(expr) => Statement::Escape(
                expr.as_ref().map(|e| Self::rewrite_cell_identifiers(e, cell_name)),
            ),
            Statement::Expression(expr) => Statement::Expression(
                Self::rewrite_cell_identifiers(expr, cell_name),
            ),
            Statement::Let { name, ty, expr, address, address_expr, bit_range, constraint, is_override, modifiers } => Statement::Let {
                name: name.clone(),
                ty: ty.clone(),
                expr: expr.as_ref().map(|e| Self::rewrite_cell_identifiers(e, cell_name)),
                address: *address,
                address_expr: address_expr.as_ref().map(|a| Box::new(Self::rewrite_cell_identifiers(a, cell_name))),
                bit_range: bit_range.clone(),
                constraint: constraint.as_ref().map(|c| Box::new(Self::rewrite_cell_identifiers(c, cell_name))),
                is_override: *is_override,
                modifiers: modifiers.clone(),
            },
            Statement::InlineAsm { asm_string, clobbers, span } => Statement::InlineAsm {
                asm_string: asm_string.clone(),
                clobbers: clobbers.clone(),
                span: *span,
            },
            Statement::TrgBinding { name, ty, instance, port, modifiers } => Statement::TrgBinding {
                name: name.clone(),
                ty: ty.clone(),
                instance: Self::rewrite_cell_identifiers(instance, cell_name),
                port: port.clone(),
                modifiers: modifiers.clone(),
            },
            Statement::SyncBlock { body } => Statement::SyncBlock {
                body: body.iter().map(|s| Self::rewrite_cell_stmt_identifiers(s, cell_name)).collect(),
            },
            Statement::Foreach { item, list, body, modifiers } => Statement::Foreach {
                item: item.clone(),
                list: Box::new(Self::rewrite_cell_identifiers(list, cell_name)),
                body: body.iter().map(|s| Self::rewrite_cell_stmt_identifiers(s, cell_name)).collect(),
                modifiers: modifiers.clone(),
            },
            Statement::Oracle { handler, body, span } => Statement::Oracle {
                handler: handler.iter().map(|s| Self::rewrite_cell_stmt_identifiers(s, cell_name)).collect(),
                body: body.iter().map(|s| Self::rewrite_cell_stmt_identifiers(s, cell_name)).collect(),
                span: *span,
            },
            Statement::Await { expr, modifiers } => Statement::Await {
                expr: Self::rewrite_cell_identifiers(expr, cell_name),
                modifiers: modifiers.clone(),
            },
            Statement::Async { body, modifiers } => Statement::Async {
                body: Box::new(Self::rewrite_cell_stmt_identifiers(body, cell_name)),
                modifiers: modifiers.clone(),
            },
            Statement::AsyncAwait { body, lhs, modifiers } => Statement::AsyncAwait {
                body: Box::new(Self::rewrite_cell_stmt_identifiers(body, cell_name)),
                lhs: lhs.clone(),
                modifiers: modifiers.clone(),
            },
        }
    }

    pub(super) fn extract_output_names_llvm(ot: &Option<OutputType>) -> Vec<String> {
        match ot {
            Some(OutputType::Named(name, inner)) => {
                let mut names = vec![name.clone()];
                names.extend(Self::extract_output_names_llvm(&Some(inner.as_ref().clone())));
                names
            }
            Some(OutputType::Tuple(types)) => {
                types.iter().flat_map(|t| Self::extract_output_names_llvm(&Some(t.clone()))).collect()
            }
            Some(OutputType::Union(types)) => {
                types.iter().flat_map(|t| Self::extract_output_names_llvm(&Some(t.clone()))).collect()
            }
            Some(OutputType::Single(_)) | Some(OutputType::Array(_)) | None => Vec::new(),
        }
    }

    /// Emit a main() that stores final precomputed values and returns.
    /// A000: no runtime loop, no iteration. The region analyzer simulated
    /// all transactions within --optimize-budget and produced final values.
    /// This is the most extreme optimization: zero runtime memory traffic.
    pub(crate) fn emit_precomputed_main(
        &mut self,
        out: &mut String,
        final_values: &[(Vec<String>, std::collections::HashMap<String, i64>)],
    ) {
        writeln!(out, "define i32 @main() local_unnamed_addr {} {{", self.slp_attr("main", "#0")).ok();
        writeln!(out, "  entry:").ok();
        writeln!(out, "  %state = alloca %State, align 8").ok();
        self.emit_inline_init_stores(out, "%state");
        let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for (_, bindings) in final_values {
            for (var, val) in bindings {
                if !seen.insert(var) { continue; }
                if let Some(&idx) = self.ctx.field_index_map.get(var) {
                    let ty = &self.ctx.field_types[idx];
                    writeln!(out, "  %gp_{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", var, idx).ok();
                    match ty.as_str() {
                        "float" => {
                            let bits = *val as i32 as u32;
                            writeln!(out, "  store float bitcast (i32 {} to float), ptr %gp_{}, align 4", bits, var).ok();
                        }
                        "i8" => {
                            writeln!(out, "  store i8 {}, ptr %gp_{}, align 1", val, var).ok();
                        }
                        _ => {
                            writeln!(out, "  store i64 {}, ptr %gp_{}, align 8", val, var).ok();
                        }
                    }
                } else if let Some(&addr) = self.ctx.mmio_fields.get(var) {
                    writeln!(out, "  %gp_{} = inttoptr i64 {} to ptr", var, addr).ok();
                    writeln!(out, "  store volatile i64 {}, ptr %gp_{}, align 1", val, var).ok();
                }
            }
        }
        writeln!(out, "  ret i32 0").ok();
        writeln!(out, "}}").ok();
        writeln!(out).ok();
    }

    // ── WAKE TRIGGER METADATA ─────────────────────────────────
    pub(crate) fn emit_wake_metadata(&self, out: &mut String) {
        let wake_symbols: Vec<&str> = self.ctx.triggers.values()
            .filter(|t| t.is_wake)
            .filter_map(|t| match &t.address {
                crate::ast::LinkRef::Linked(s) => Some(s.as_str()),
                _ => None,
            })
            .collect();
        if wake_symbols.is_empty() { return; }
        let count = wake_symbols.len();
        let sym_list = wake_symbols.iter().map(|s| format!("ptr @{}", s)).collect::<Vec<_>>().join(", ");
        writeln!(out, "@llvm.wake_triggers = constant [{} x ptr] [{}]", count, sym_list).ok();
        writeln!(out, "!llvm.wake_triggers = !{{!6}}").ok();
        write!(out, "!6 = !{{").ok();
        for (i, sym) in wake_symbols.iter().enumerate() {
            if i > 0 { write!(out, ", ").ok(); }
            write!(out, "!\"{}\"", sym).ok();
        }
        writeln!(out, "}}").ok();
    }

    // ── THREAD POOL METADATA ────────────────────────────────
    pub(crate) fn emit_thread_pool_metadata(&self, out: &mut String) {
        if !self.has_async_txns || self.is_lightweight_async { return; }
        let count = self.async_txn_names.len();
        let fn_list: Vec<String> = self.async_txn_names.iter()
            .map(|n| format!("i8* bitcast (void (ptr)* @async_body_{} to ptr)", n))
            .collect();
        writeln!(out, "@llvm.thread_pool = constant [{} x ptr] [{}]",
            count, fn_list.join(", ")).ok();
        // Emit a packed array of function pointers for brief_thread_pool_init
        writeln!(out, "@thread_pool_fns = private constant [{} x void (ptr)*] [{}]",
            count,
            self.async_txn_names.iter()
                .map(|n| format!("void (ptr)* @async_body_{}", n))
                .collect::<Vec<_>>().join(", "),
        ).ok();
    }

    /// Emit the async phase calls in main: set state for workers, release
    /// workers, wait for workers. Used by emit_main and emit_enum_main.
    ///
    /// 2026-07-01: reactor_tick is now a no-op when the thread pool is active.
    /// The worker threads execute async bodies on the correct state snapshot
    /// (set via __set_async_state__), synchronized by the dual barriers.
    pub(crate) fn emit_async_phase(&self, out: &mut String, state_var: &str) {
        if !self.has_async_txns || self.is_lightweight_async { return; }
        writeln!(out, "  call void @__set_async_state__(ptr {})", state_var).ok();
        writeln!(out, "  call void @__barrier_release__()").ok();
        // reactor_tick is a no-op (workers handle the work).
        writeln!(out, "  call void @reactor_tick(ptr noalias nocapture {})", state_var).ok();
        writeln!(out, "  call void @__barrier_wait__()").ok();
    }

    // ── FUSABLE PAIRS ────────────────────────────────────────
    pub(crate) fn resolve_fusable_pairs(&self, txns: &[(String, &crate::ast::Transaction)]) -> Vec<(String, String)> {
        let prg = crate::ast::Program {
            items: txns.iter().map(|(_, t)| crate::ast::TopLevel::Transaction((*t).clone())).collect(),
            comments: vec![], reactor_speed: None, attrs: vec![], ffi: None, strict_mode: crate::ast::StrictMode::Off, dispatch_mode: crate::ast::DispatchMode::Sequential, exit_condition: None, out_pragmas: vec![], default_sig_modifier: None, watchdog_defaults: (None, None),
        };
        let mut pairs = crate::backend::detect_fusable_pairs(&prg);
        pairs.retain(|(a, b)| {
            if let (Some((_, ta)), Some((_, tb))) = (txns.iter().find(|(n, _)| n == a), txns.iter().find(|(n, _)| n == b)) {
                if ta.is_async || tb.is_async { return false; }
                // Skip callable txns — they don't use ptr, can't be fused with reactive txns
                if !ta.is_reactive || !tb.is_reactive { return false; }
                let aw = crate::backend::collect_assigned_identifiers(&ta.body);
                let bw = crate::backend::collect_assigned_identifiers(&tb.body);
                if aw.iter().any(|w| bw.contains(w)) { return false; }
                if self.trg_in_pre(&tb.contract.pre_condition) { return false; }
                true
            } else { false }
        });
        pairs
    }

    pub(crate) fn trg_in_pre(&self, pre: &Expr) -> bool {
        let mut ids = std::collections::HashSet::new();
        crate::backend::collect_expr_identifiers(pre, &mut ids);
        ids.iter().any(|id| self.ctx.trigger_names.contains(id))
    }

    pub(crate) fn emit_cast_convert(&mut self, out: &mut String, indent: &str, dst: &str, src: &str, src_ty: Option<Type>, target: &Type) {
        let src_ty = match src_ty {
            Some(t) => t,
            None => {
                let _ = writeln!(out, "{}{} = add i64 0, {}", indent, dst, src);
                return;
            }
        };
        if &src_ty == target {
            let _ = writeln!(out, "{}{} = add i64 0, {}", indent, dst, src);
            return;
        }
        match (&src_ty, target) {
            (Type::Custom(__t), Type::Custom(__s)) if (__t == "Int" || __t == "UInt") && __s == "Float" => {
                // 2026-06-17: Native float, not boxed i64. Downstream
                // code (emit_binop, enum constructors) converts to/from
                // i64 as needed via ensure_float_reg / native_float_or_box.
                let _ = writeln!(out, "{}{} = sitofp i64 {} to float", indent, dst, src);
            }
            (Type::Custom(__t), Type::Custom(__s)) if __t == "Float" && (__s == "Int" || __s == "UInt") => {
                let tr = format!("%ctr{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                let fl = format!("%cfl{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                let _ = writeln!(out, "{}{} = trunc i64 {} to i32", indent, tr, src);
                let _ = writeln!(out, "{}{} = bitcast i32 {} to float", indent, fl, tr);
                let _ = writeln!(out, "{}{} = fptosi float {} to i64", indent, dst, fl);
            }
            (Type::Custom(__t), Type::Custom(__s)) if (__t == "Int" || __t == "UInt") && __s == "Bool" => {
                let ci = format!("%ccb{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                let _ = writeln!(out, "{}{} = icmp ne i64 {}, 0", indent, ci, src);
                let _ = writeln!(out, "{}{} = zext i1 {} to i64", indent, dst, ci);
            }
            (Type::Custom(__t), Type::Custom(__s)) if __t == "Bool" && (__s == "Int" || __s == "UInt") => {
                let _ = writeln!(out, "{}{} = add i64 0, {}", indent, dst, src);
            }
            (Type::Custom(__t), Type::Custom(__s)) if __t == "Float" && __s == "Bool" => {
                let tr = format!("%cfbtr{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                let fl = format!("%cfbfl{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                let ci = format!("%cfbci{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                let _ = writeln!(out, "{}{} = trunc i64 {} to i32", indent, tr, src);
                let _ = writeln!(out, "{}{} = bitcast i32 {} to float", indent, fl, tr);
                let _ = writeln!(out, "{}{} = fcmp fast une float {}, 0.0", indent, ci, fl);
                let _ = writeln!(out, "{}{} = zext i1 {} to i64", indent, dst, ci);
            }
            (Type::Custom(__t), Type::Custom(__s)) if __t == "Bool" && __s == "Float" => {
                let ci = format!("%cbfci{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                let fl = format!("%cbffl{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                let fi = format!("%cbffi{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                let _ = writeln!(out, "{}{} = icmp ne i64 {}, 0", indent, ci, src);
                let _ = writeln!(out, "{}{} = select i1 {}, float 1.000000e+00, float 0.000000e+00", indent, fl, ci);
                let _ = writeln!(out, "{}{} = bitcast float {} to i32", indent, fi, fl);
                let _ = writeln!(out, "{}{} = zext i32 {} to i64", indent, dst, fi);
            }
            // Char ↔ Bool
            (Type::Custom(__t), Type::Custom(__s)) if __t == "Char" && __s == "Bool" => {
                let ci = format!("%cci{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                let _ = writeln!(out, "{}{} = icmp ne i64 {}, 0", indent, ci, src);
                let _ = writeln!(out, "{}{} = zext i1 {} to i64", indent, dst, ci);
            }
            (Type::Custom(__t), Type::Custom(__s)) if __t == "Bool" && __s == "Char" => {
                let _ = writeln!(out, "{}{} = add i64 0, {}", indent, dst, src);
            }
            // Char ↔ Int
            (Type::Custom(__t), Type::Custom(__s)) if __t == "Char" && (__s == "Int" || __s == "UInt") => {
                let _ = writeln!(out, "{}{} = add i64 0, {}", indent, dst, src);
            }
            (Type::Custom(__t), Type::Custom(__s)) if (__t == "Int" || __t == "UInt") && __s == "Char" => {
                let _ = writeln!(out, "{}{} = trunc i64 {} to i32", indent, dst, src);
            }
            // Char ↔ String (construct Brief string struct {cap, len, data})
            (Type::Custom(__t), Type::Custom(__s)) if __t == "Char" && __s == "String" => {
                let tr = format!("%cctr{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                let _ = writeln!(out, "{}{} = trunc i64 {} to i32", indent, tr, src);
                let alloc = format!("%ccac{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                let _ = writeln!(out, "{}{} = call ptr @malloc(i64 24)", indent, alloc);
                let hp = format!("%cchp{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                let _ = writeln!(out, "{}{} = bitcast ptr {} to ptr", indent, hp, alloc);
                let base = format!("%ccba{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                let _ = writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, base, alloc);
                let dp = format!("%ccdp{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                let _ = writeln!(out, "{}{} = add i64 {}, 16", indent, dp, base);
                let _ = writeln!(out, "{}store i64 {}, ptr {}, align 8", indent, dp, hp);
                let ls = format!("%ccls{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                let _ = writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 1", indent, ls, hp);
                let _ = writeln!(out, "{}store i64 1, ptr {}, align 8", indent, ls);
                let cs = format!("%cccs{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                let _ = writeln!(out, "{}{} = getelementptr i8, ptr {}, i64 16", indent, cs, alloc);
                let tb = format!("%cctb{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                let _ = writeln!(out, "{}{} = trunc i32 {} to i8", indent, tb, tr);
                let _ = writeln!(out, "{}store i8 {}, ptr {}, align 1", indent, tb, cs);
                let nt = format!("%ccnt{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                let _ = writeln!(out, "{}{} = getelementptr i8, ptr {}, i64 17", indent, nt, alloc);
                let _ = writeln!(out, "{}store i8 0, ptr {}, align 1", indent, nt);
                let _ = writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, dst, alloc);
            }
            (Type::Custom(__t), Type::Custom(__s)) if __t == "String" && __s == "Char" => {
                let ip = format!("%csip{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                let _ = writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ip, src);
                let lb = format!("%cslb{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                let _ = writeln!(out, "{}{} = load i8, ptr {}, align 1", indent, lb, ip);
                let _ = writeln!(out, "{}{} = zext i8 {} to i64", indent, dst, lb);
            }
            // Int ↔ String (via existing __int_to_str)
            (Type::Custom(__t), Type::Custom(__s)) if (__t == "Int" || __t == "UInt") && __s == "String" => {
                let _ = writeln!(out, "{}{} = call i64 @__int_to_str__(i64 {})", indent, dst, src);
            }
            (Type::Custom(__t), Type::Custom(__s)) if __t == "String" && (__s == "Int" || __s == "UInt") => {
                let ip = format!("%csii{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                let _ = writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ip, src);
                let _ = writeln!(out, "{}{} = call i64 @__str_to_int(i8* {})", indent, dst, ip);
            }
            // String ↔ Bool (non-empty is true)
            (Type::Custom(__t), Type::Custom(__s)) if __t == "String" && __s == "Bool" => {
                let ip = format!("%csbi{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                let lb = format!("%csbl{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                let ci = format!("%csbc{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                let _ = writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ip, src);
                let _ = writeln!(out, "{}{} = load i8, ptr {}, align 1", indent, lb, ip);
                let _ = writeln!(out, "{}{} = icmp ne i8 {}, 0", indent, ci, lb);
                let _ = writeln!(out, "{}{} = zext i1 {} to i64", indent, dst, ci);
            }
            (Type::Custom(__t), Type::Custom(__s)) if __t == "Bool" && __s == "String" => {
                let ci = format!("%cbsc{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                let ip = format!("%cbsi{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                let _ = writeln!(out, "{}{} = icmp ne i64 {}, 0", indent, ci, src);
                let _ = writeln!(out, "{}{} = call i8* @__chr_to_str(i32 {})", indent, ip, ci);
                let _ = writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, dst, ip);
            }
            _ => {
                let _ = writeln!(out, "{}{} = add i64 0, {}", indent, dst, src);
            }
        }
    }

    pub(crate) fn i64_to_float_reg(&mut self, out: &mut String, reg: &str, indent: &str) -> String {
        // Check cache first: these are actual float registers from SSA extraction
        // or float literal caching. Do NOT check register_types here — that map
        // tracks Brief-level float semantics, not LLVM type (boxed as i64).
        if let Some(cached) = self.fun.reg_float_cache.get(reg) {
            return cached.clone();
        }
        let tr = format!("%ftr{}", self.fun.txn_counter); self.fun.txn_counter += 1;
        let fl = format!("%ffl{}", self.fun.txn_counter); self.fun.txn_counter += 1;
        writeln!(out, "{}{} = trunc i64 {} to i32", indent, tr, reg).ok();
        writeln!(out, "{}{} = bitcast i32 {} to float", indent, fl, tr).ok();
        fl
    }

    /// If `reg` is an i64 (Int), truncate to i1. Otherwise return its name as-is.
    pub(super) fn as_bool_reg(&mut self, out: &mut String, indent: &str, reg: &TypedRegister) -> String {
        if reg.ty == Type::Custom("Int".to_string()) {
            let t = format!("%tb{}", self.fun.txn_counter);
            self.fun.txn_counter += 1;
            writeln!(out, "{}{} = trunc i64 {} to i1", indent, t, reg.name).ok();
            t
        } else {
            reg.name.clone()
        }
    }

    /// Convert a String/Data typed register to i64 for C ABI calls.
    /// Int/Bool/Char/Float registers are passed through as-is.
    fn ptrtoint_if_string(&mut self, out: &mut String, indent: &str, reg: &TypedRegister) -> String {
        if reg.ty == Type::Custom("String".to_string()) || reg.ty == Type::Custom("Data".to_string()) {
            let p = format!("%ptri{}", self.fun.txn_counter); self.fun.txn_counter += 1;
            writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, p, reg.name).ok();
            p
        } else {
            reg.name.clone()
        }
    }

    /// Emit inline string concatenation: malloc + header setup + memcpy + free.
    /// Both operands are i8* (Brief header pointers). Returns i8*.
    ///
    /// Tag convention (2026-06-19):
    ///   bit 0 = static string constant (don't free, don't read header at -16)
    ///   bit 1 = temporary concat result (safe to free when consumed)
    /// State-loaded strings have both bits clear (heap, state-owned).
    /// Only concat results get bit 1 set.
    //
    // Why inline string concat instead of calling sprintf/strcat: the compiler
    // knows each operand's length at emit time (from header slot 1), so it can
    // compute the total allocation size and emit memcpy calls that LLVM can
    // lower to rep movsb or inline. sprintf would need to scan for null
    // terminators at runtime, losing the length information.
    //
    // Tag bits: bit 0 = static string constant (from .rodata, don't free),
    // bit 1 = temporary concat result (safe to free when consumed).
    // State-loaded strings have both bits clear. The tag convention avoids
    // separate tracking data structures.
    pub(crate) fn emit_inline_concat(&mut self, out: &mut String, indent: &str, a: &TypedRegister, b: &TypedRegister) -> TypedRegister {
        let a_boxed = self.adapt_to_i64(out, indent, a);
        let b_boxed = self.adapt_to_i64(out, indent, b);
        // Mask off tag bits (bit 0 = static, bit 1 = temp) before reading headers
        let a_clean = format!("%cam{}", self.fun.txn_counter); self.fun.txn_counter += 1;
        writeln!(out, "{}{} = and i64 {}, -4", indent, a_clean, a_boxed).ok();
        let b_clean = format!("%cbm{}", self.fun.txn_counter); self.fun.txn_counter += 1;
        writeln!(out, "{}{} = and i64 {}, -4", indent, b_clean, b_boxed).ok();
        let ha = format!("%cha{}", self.fun.txn_counter); self.fun.txn_counter += 1;
        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ha, a_clean).ok();
        let la_ptr = format!("%clp{}", self.fun.txn_counter); self.fun.txn_counter += 1;
        writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 1", indent, la_ptr, ha).ok();
        let la = format!("%cla{}", self.fun.txn_counter); self.fun.txn_counter += 1;
        writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, la, la_ptr).ok();
        let hb = format!("%chb{}", self.fun.txn_counter); self.fun.txn_counter += 1;
        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, hb, b_clean).ok();
        let lb_ptr = format!("%clq{}", self.fun.txn_counter); self.fun.txn_counter += 1;
        writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 1", indent, lb_ptr, hb).ok();
        let lb = format!("%clb{}", self.fun.txn_counter); self.fun.txn_counter += 1;
        writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, lb, lb_ptr).ok();
        let total = format!("%ctl{}", self.fun.txn_counter); self.fun.txn_counter += 1;
        writeln!(out, "{}{} = add i64 {}, {}", indent, total, la, lb).ok();
        // Tight packing: 16 byte header + total chars + 1 null byte
        let header_size = format!("%chs{}", self.fun.txn_counter); self.fun.txn_counter += 1;
        writeln!(out, "{}{} = add i64 16, {}", indent, header_size, total).ok();
        let alloc_size = format!("%cas{}", self.fun.txn_counter); self.fun.txn_counter += 1;
        writeln!(out, "{}{} = add i64 {}, 1", indent, alloc_size, header_size).ok();
        let result = self.emit_arena_alloc(out, indent, &alloc_size);
        let hp = format!("%chp{}", self.fun.txn_counter); self.fun.txn_counter += 1;
        writeln!(out, "{}{} = bitcast ptr {} to ptr", indent, hp, result).ok();
        let base = format!("%cba{}", self.fun.txn_counter); self.fun.txn_counter += 1;
        writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, base, result).ok();
        let dp = format!("%cdp{}", self.fun.txn_counter); self.fun.txn_counter += 1;
        writeln!(out, "{}{} = add i64 {}, 16", indent, dp, base).ok();
        writeln!(out, "{}store i64 {}, ptr {}, align 8", indent, dp, hp).ok();
        let len_slot = format!("%cls{}", self.fun.txn_counter); self.fun.txn_counter += 1;
        writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 1", indent, len_slot, hp).ok();
        writeln!(out, "{}store i64 {}, ptr {}, align 8", indent, total, len_slot).ok();
        let a_dp = format!("%cad{}", self.fun.txn_counter); self.fun.txn_counter += 1;
        writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, a_dp, ha).ok();
        let a_chars = format!("%cac{}", self.fun.txn_counter); self.fun.txn_counter += 1;
        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, a_chars, a_dp).ok();
        let dest_start = format!("%cds{}", self.fun.txn_counter); self.fun.txn_counter += 1;
        writeln!(out, "{}{} = getelementptr i8, ptr {}, i64 16", indent, dest_start, result).ok();
        writeln!(out, "{}call void @llvm.memcpy.p0i8.p0i8.i64(i8* {}, ptr {}, i64 {}, i1 false)", indent, dest_start, a_chars, la).ok();
        let dest_off = format!("%cdo{}", self.fun.txn_counter); self.fun.txn_counter += 1;
        writeln!(out, "{}{} = getelementptr i8, ptr {}, i64 {}", indent, dest_off, dest_start, la).ok();
        let b_dp = format!("%cbd{}", self.fun.txn_counter); self.fun.txn_counter += 1;
        writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, b_dp, hb).ok();
        let b_chars = format!("%cbc{}", self.fun.txn_counter); self.fun.txn_counter += 1;
        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, b_chars, b_dp).ok();
        writeln!(out, "{}call void @llvm.memcpy.p0i8.p0i8.i64(i8* {}, ptr {}, i64 {}, i1 false)", indent, dest_off, b_chars, lb).ok();
        // Null terminate
        let nt = format!("%cnt{}", self.fun.txn_counter); self.fun.txn_counter += 1;
        writeln!(out, "{}{} = getelementptr i8, ptr {}, i64 {}", indent, nt, dest_start, total).ok();
        writeln!(out, "{}store i8 0, ptr {}, align 1", indent, nt).ok();
        // Free heap-allocated operands that are temporaries (bit 1 set).
        // When arena is active, the arena owns all allocations — skip.
        // Static constants (bit 0=1) and state fields (bit 0=0,bit 1=0) are
        // always preserved regardless of arena mode.
        if self.fun.arena_slots.is_none() {
            let tag_a = format!("%cta{}", self.fun.txn_counter); self.fun.txn_counter += 1;
            writeln!(out, "{}{} = and i64 {}, 2", indent, tag_a, a_boxed).ok();
            let is_temp_a = format!("%cia{}", self.fun.txn_counter); self.fun.txn_counter += 1;
            writeln!(out, "{}{} = icmp ne i64 {}, 0", indent, is_temp_a, tag_a).ok();
            let free_a_label = format!("free_a_{}", self.fun.txn_counter);
            let after_free_a_label = format!("af_a_{}", self.fun.txn_counter);
            writeln!(out, "{}br i1 {}, label %{}, label %{}", indent, is_temp_a, free_a_label, after_free_a_label).ok();
            writeln!(out, "{}{}:", indent, free_a_label).ok();
            let a_clean_all = format!("%cca{}", self.fun.txn_counter); self.fun.txn_counter += 1;
            writeln!(out, "{}{} = and i64 {}, -4", indent, a_clean_all, a_boxed).ok();
            let a_free_ptr = format!("%cfp{}", self.fun.txn_counter); self.fun.txn_counter += 1;
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, a_free_ptr, a_clean_all).ok();
            writeln!(out, "{}call void @free(ptr {})", indent, a_free_ptr).ok();
            writeln!(out, "{}br label %{}", indent, after_free_a_label).ok();
            writeln!(out, "{}{}:", indent, after_free_a_label).ok();
            // Same for operand B
            let tag_b = format!("%ctb{}", self.fun.txn_counter); self.fun.txn_counter += 1;
            writeln!(out, "{}{} = and i64 {}, 2", indent, tag_b, b_boxed).ok();
            let is_temp_b = format!("%cib{}", self.fun.txn_counter); self.fun.txn_counter += 1;
            writeln!(out, "{}{} = icmp ne i64 {}, 0", indent, is_temp_b, tag_b).ok();
            let free_b_label = format!("free_b_{}", self.fun.txn_counter);
            let after_free_b_label = format!("af_b_{}", self.fun.txn_counter);
            writeln!(out, "{}br i1 {}, label %{}, label %{}", indent, is_temp_b, free_b_label, after_free_b_label).ok();
            writeln!(out, "{}{}:", indent, free_b_label).ok();
            let b_clean_all = format!("%ccb{}", self.fun.txn_counter); self.fun.txn_counter += 1;
            writeln!(out, "{}{} = and i64 {}, -4", indent, b_clean_all, b_boxed).ok();
            let b_free_ptr = format!("%cfq{}", self.fun.txn_counter); self.fun.txn_counter += 1;
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, b_free_ptr, b_clean_all).ok();
            writeln!(out, "{}call void @free(ptr {})", indent, b_free_ptr).ok();
            writeln!(out, "{}br label %{}", indent, after_free_b_label).ok();
            writeln!(out, "{}{}:", indent, after_free_b_label).ok();
        }

        // 2026-06-28: Use txn_counter to prevent %t{N} collision with
        // emit_expr's register allocation (which also uses txn_counter).
        let v = format!("%t{}", self.fun.txn_counter); self.fun.txn_counter += 1;
        writeln!(out, "{}{} = bitcast ptr {} to ptr", indent, v, result).ok();
        // Box to i64 — downstream code expects i64 (ptrtoint).
        let vi = format!("%t{}", self.fun.txn_counter); self.fun.txn_counter += 1;
        writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, vi, v).ok();
        // Tag as temporary (bit 1 = 1) so future concat calls can free it
        let vi_tagged = format!("%t{}", self.fun.txn_counter); self.fun.txn_counter += 1;
        writeln!(out, "{}{} = or i64 {}, 2", indent, vi_tagged, vi).ok();
        TypedRegister { name: vi_tagged, ty: Type::Custom("Int".to_string()) }
    }

    /// Map int_op/float_op strings to an OpRune for operator resolution.
    /// 2026-06-29: Phase 7B — bridges emit_binop's string dispatch to
    /// the universe's operator→intrinsic mapping.
    fn op_str_to_rune(int_op: &str) -> OpRune {
        // 2026-07-05: Strip "nsw " prefix added by expr/math.rs
        // 2026-07-06: nsw now comes after opcode (LLVM 18 syntax: "add nsw")
        let op = int_op.strip_suffix(" nsw").unwrap_or(int_op);
        match op {
            "add" => OpRune::Add,
            "sub" => OpRune::Sub,
            "mul" => OpRune::Mul,
            "sdiv" | "udiv" => OpRune::Div,
            "srem" | "urem" => OpRune::Mod,
            "shl" => OpRune::Shl,
            "lshr" | "ashr" => OpRune::Shr,
            "and" => OpRune::BitAnd,
            "or" => OpRune::BitOr,
            "xor" => OpRune::BitXor,
            _ => OpRune::Add, // fallback for comparison ops
        }
    }

    pub(crate) fn emit_binop(&mut self, out: &mut String, indent: &str, l: &Expr, r: &Expr, int_op: &str, float_op: &str) -> TypedRegister {
        // ── Phase 7B: Custom type operator dispatch ──────────────
        // If either operand has a universe-registered type, check for
        // operator→intrinsic mappings and emit them.
        // This runs BEFORE constant-folding so custom types get their
        // own dispatch even when operands are literals.
        //
        // 2026-07-01: Save emitted registers to prevent O(2^depth) fallthrough.
        // emit_expr emits IR for the operand subtrees. If the operator IS found,
        // we return early. If NOT (standard types like Int), we fall through to
        // the normal codegen which also calls emit_expr — re-emitting the whole
        // subtree. For deeply nested Add chains like acc + C00 + ... + C19, this
        // is O(2^depth). Saving the emitted registers and reusing them in the
        // normal path avoids the double emission. See BUGS.md.
        let mut phase7b_l: Option<TypedRegister> = None;
        let mut phase7b_r: Option<TypedRegister> = None;
        if let Some(ref universe) = self.ctx.type_universe.clone() {
            let l_reg = self.emit_expr(out, l, indent);
            let l_key = l_reg.ty.universe_key().to_string();
            // 2026-07-01: Only save/reuse for non-float types.
            // Float/Float64 registers depend on reg_float_cache for
            // ensure_float_reg lookups. A Phase 7B-emitted float register
            // may be defined in a scope that doesn't dominate its use
            // in the normal codegen path, causing "use of undefined value"
            // SSA violations (nbody %bfr errors). Integer types have no
            // such cache dependency — saving and reusing their registers
            // avoids the O(2^depth) double-emission (const_heavy fix).
            // 2026-07-08: Phase 2D — use universe storage check for
            // native type detection instead of hardcoded name matching.
            let l_is_native = universe.get(&l_key).map(|r| r.storage == "Native").unwrap_or(false);
            if !l_is_native {
                phase7b_l = Some(l_reg.clone());
            }
            if universe.types.contains_key(&l_key) {
                let r_reg = self.emit_expr(out, r, indent);
                let r_key = r_reg.ty.universe_key().to_string();
                let r_is_native = universe.get(&r_key).map(|r| r.storage == "Native").unwrap_or(false);
                if !r_is_native {
                    phase7b_r = Some(r_reg.clone());
                }
                let rune = Self::op_str_to_rune(int_op);
                if let Some(op) = universe.resolve_operator(&l_key, rune, Some(&r_key)) {
                    return self.emit_operator_call(out, indent, &l_reg, &r_reg, op);
                }
            }
        }

        // Peephole: constant-fold integer binops at compile time
        // 2026-07-05: Strip "nsw " prefix for matching (added by expr/math.rs)
        // 2026-07-06: nsw now comes after opcode (LLVM 18 syntax: "add nsw")
        let int_op_clean = int_op.strip_suffix(" nsw").unwrap_or(int_op);
        if let (Expr::Integer(li), Expr::Integer(ri)) = (l, r) {
            let result = match int_op_clean {
                "add" => Some(li.wrapping_add(*ri)),
                "sub" => Some(li.wrapping_sub(*ri)),
                "mul" => Some(li.wrapping_mul(*ri)),
                "sdiv" if *ri != 0 => Some(li / ri),
                "and" => Some(li & ri),
                "or"  => Some(li | ri),
                "xor" => Some(li ^ ri),
                "shl" => Some(li.wrapping_shl(*ri as u32)),
                "lshr" => Some((*li as u64).wrapping_shr(*ri as u32) as i64),
                _ => None,
            };
            if let Some(folded) = result {
                // 2026-06-28: Use txn_counter to prevent %t{N} collision with
                // emit_expr's register allocation (which also uses txn_counter).
                // Previously used glob_counter which caused duplicate %t{N} defs
                // in the same function — the deduplicator cannot fix uses that
                // reference the renamed register via internal maps (let_bindings).
                let v = format!("%t{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                writeln!(out, "{}{} = add i64 0, {}", indent, v, folded).ok();
                return TypedRegister { name: v, ty: Type::Custom("Int".to_string()) };
            }
        }
        // 2026-07-01: Use Phase 7B-emitted registers if available (avoids
        // O(2^depth) re-emission of deeply nested addition chains).
        // See comment at the Phase 7B block above and BUGS.md.
        let (a, b) = (
            phase7b_l.unwrap_or_else(|| self.emit_expr(out, l, indent)),
            phase7b_r.unwrap_or_else(|| self.emit_expr(out, r, indent)),
        );

        // 2026-07-08: Phase 2D — universe storage check for native types.
        // Falls back to name-based Float/Float64 check when universe is absent (tests).
        let a_is_native = self.ctx.type_universe.as_ref()
            .and_then(|u| u.get_by_type(&a.ty))
            .map(|r| r.storage == "Native")
            .unwrap_or_else(|| a.ty == Type::Custom("Float".to_string()) || a.ty == Type::Custom("Float64".to_string()));
        let b_is_native = self.ctx.type_universe.as_ref()
            .and_then(|u| u.get_by_type(&b.ty))
            .map(|r| r.storage == "Native")
            .unwrap_or_else(|| b.ty == Type::Custom("Float".to_string()) || b.ty == Type::Custom("Float64".to_string()));

        // ── Expression hash-consing dedup cache lookup ─────────────
        // 2026-07-01: Check if we already emitted this (op, lhs, rhs) within
        // the current body scope. ...
        // For float ops the benefit is in IR compactness: fewer instructions
        // means LLVM can find SIMD vectorization patterns more easily.
        // 2026-07-08: Phase 2D — use universe storage query instead of name matching.
        let dedup_op = if a_is_native || b_is_native {
            float_op
        } else {
            int_op
        };
        let dedup_key = if dedup_op.len() >= 3 {
            Some((dedup_op.to_string(), a.name.clone(), b.name.clone()))
        } else {
            None
        };
        if let Some(ref key) = dedup_key {
            if let Some(cached) = self.fun.expr_dedup_cache.get(key) {
                let result_ty = a.ty.clone();
                return TypedRegister { name: cached.clone(), ty: result_ty };
            }
        }

        // Preserve Ptr type through arithmetic operations
        let is_a_ptr = matches!(&a.ty, Type::Applied(n, _) if n == "Ptr")
            || matches!(&a.ty, Type::LayoutPtr(_));
        let is_b_ptr = matches!(&b.ty, Type::Applied(n, _) if n == "Ptr")
            || matches!(&b.ty, Type::LayoutPtr(_));
        let ptr_ty = if is_a_ptr { Some(a.ty.clone()) }
                     else if is_b_ptr { Some(b.ty.clone()) }
                     else { None };
        // Handle Native storage types (float/double) via universe query
        if a_is_native && b_is_native && a.ty == b.ty {
            let fa = self.ensure_float_reg(out, indent, &a);
            let fb = self.ensure_float_reg(out, indent, &b);
            let llvm_ty = self.operator_llvm_type(&a.ty);
            let fr = format!("%bfr{}", self.fun.txn_counter); self.fun.txn_counter += 1;
            writeln!(out, "{}{} = {} fast {} {}, {}", indent, fr, float_op, llvm_ty, fa, fb).ok();
            self.fun.reg_float_cache.insert(fr.clone(), fr.clone());
            if let Some(ref key) = dedup_key { self.fun.expr_dedup_cache.insert(key.clone(), fr.clone()); }
            return TypedRegister { name: fr, ty: a.ty.clone() };
        }
        if a_is_native || b_is_native {
            // Mixed: one operand is native float, the other is boxed.
            // Box both to i64 and emit integer operation.
            let v = format!("%t{}", self.fun.txn_counter); self.fun.txn_counter += 1;
            let a_i64 = self.adapt_to_i64(out, indent, &a);
            let b_i64 = self.adapt_to_i64(out, indent, &b);
            writeln!(out, "{}{} = {} i64 {}, {}", indent, v, int_op, a_i64, b_i64).ok();
            if let Some(ref key) = dedup_key { self.fun.expr_dedup_cache.insert(key.clone(), v.clone()); }
            return TypedRegister { name: v, ty: ptr_ty.unwrap_or(Type::Custom("Int".to_string())) };
        }
        // 2026-06-29: Fixed-width integer same-type arithmetic (native width, no boxing)
        if a.ty.is_integral() && a.ty == b.ty {
            let llvm_ty_str = self.llvm_type(&a.ty).to_string();
            let v = format!("%t{}", self.fun.txn_counter); self.fun.txn_counter += 1;
            writeln!(out, "{}{} = {} {} {}, {}", indent, v, int_op, llvm_ty_str, a.name, b.name).ok();
            if let Some(ref key) = dedup_key { self.fun.expr_dedup_cache.insert(key.clone(), v.clone()); }
            return TypedRegister { name: v, ty: a.ty.clone() };
        }
        {
            let v = format!("%t{}", self.fun.txn_counter); self.fun.txn_counter += 1;
            let a_i64 = self.adapt_to_i64(out, indent, &a);
            let b_i64 = self.adapt_to_i64(out, indent, &b);
            writeln!(out, "{}{} = {} i64 {}, {}", indent, v, int_op, a_i64, b_i64).ok();
            if let Some(ref key) = dedup_key { self.fun.expr_dedup_cache.insert(key.clone(), v.clone()); }
            TypedRegister { name: v, ty: ptr_ty.unwrap_or(Type::Custom("Int".to_string())) }
        }
    }

    /// Check if a type is `Ptr<T>` or a layout-constrained pointer (returns true for any).
    fn is_ptr_ty(ty: &Type) -> bool {
        if let Type::Applied(name, _) = ty { name == "Ptr" } else { matches!(ty, Type::LayoutPtr(_)) }
    }

    /// Check if an expression is a reference to a linked String trigger.
    fn is_linked_string_trigger(&self, expr: &Expr) -> bool {
        if let Expr::Identifier(name) = expr {
            if let Some(trg) = self.ctx.triggers.get(name) {
                return trg.ty == Type::Custom("String".to_string()) || trg.ty == Type::Custom("Data".to_string());
            }
        }
        false
    }

    // ── Phase 7B: Operator Call Emission ───────────────────────
    //
    // 2026-06-29: Emits the implementation of a resolved operator
    // declaration. The implementation can be:
    //   - An intrinsic call (inop/intrinsic name)
    //   - An identifier (defn function name)
    //   - A defn block (inlined at call site)
    // Falls back to identity (no-op) if unimplemented.

    /// Emit a call to a resolved operator's implementation.
    /// 2026-07-08: Phase 2D — handles Native storage (float/double) by
    /// calling ensure_float_reg on operands before emitting the opcode.
    fn emit_operator_call(&mut self, out: &mut String, indent: &str,
                          a: &TypedRegister, b: &TypedRegister,
                          op: &OpDeclaration) -> TypedRegister {
        let v = self.fun.next_reg();
        // 2026-07-08: Determine LLVM type from operand's type storage.
        // For Native storage (float/double), ensure operands are in native form.
        let is_native = self.ctx.type_universe.as_ref()
            .and_then(|u| u.get_by_type(&a.ty))
            .map(|r| r.storage == "Native")
            .unwrap_or(false);
        let (op_a, op_b) = if is_native {
            (self.ensure_float_reg(out, indent, a), self.ensure_float_reg(out, indent, b))
        } else {
            (a.name.clone(), b.name.clone())
        };
        let llvm_ty = self.operator_llvm_type(&a.ty);
        match &op.implementation.as_ref() {
            // Intrinsic call: emit via the intrinsic name
            Expr::IntrinsicCall { intrinsic, .. } => {
                writeln!(out, "{}{} = call i64 @{}(i64 {}, i64 {})",
                         indent, v, intrinsic.name(), op_a, op_b).ok();
            }
            // Identifier → function call: call i64 @name(i64, i64)
            Expr::Identifier(name) => {
                writeln!(out, "{}{} = call i64 @{}(i64 {}, i64 {})",
                         indent, v, name, op_a, op_b).ok();
            }
            // 2026-07-08: String literal → LLVM opcode, e.g. "add nsw" → add nsw i64 %a, %b
            // For Native storage: "fadd fast" + ensure_float_reg operands → fadd fast float %fa, %fb
            Expr::String(llvm_op) => {
                writeln!(out, "{}{} = {} {} {}, {}",
                         indent, v, llvm_op, llvm_ty, op_a, op_b).ok();
                if is_native {
                    self.fun.reg_float_cache.insert(v.clone(), v.clone());
                }
            }
            Expr::Literal(lit) if matches!(lit.as_ref(), crate::features::literal::LiteralExpr::String(_)) => {
                if let crate::features::literal::LiteralExpr::String(llvm_op) = lit.as_ref() {
                    writeln!(out, "{}{} = {} {} {}, {}",
                             indent, v, llvm_op, llvm_ty, op_a, op_b).ok();
                    if is_native {
                        self.fun.reg_float_cache.insert(v.clone(), v.clone());
                    }
                }
            }
            // Fallback: identity
            _ => {
                writeln!(out, "{}{} = {} 0, {}", indent, v, llvm_ty, op_a).ok();
            }
        }
        TypedRegister { name: v, ty: a.ty.clone() }
    }

    /// 2026-07-08: Look up the LLVM codegen type for a Brief type.
    /// Returns "float" for Native storage types ≤32 bits, "double" for >32,
    /// and "i64" for Boxed or unknown types (boxed to native register).
    /// Falls back to name-based Float/Float64 check when universe is absent.
    fn operator_llvm_type(&self, ty: &Type) -> &'static str {
        if let Some(ref universe) = self.ctx.type_universe {
            if let Some(rt) = universe.get_by_type(ty) {
                return match rt.storage.as_str() {
                    "Native" => match ty.bit_width() {
                        Some(w) if w <= 32 => "float",
                        _ => "double",
                    },
                    _ => "i64",
                };
            }
        }
        // Fallback for tests without universe
        if ty == &Type::Custom("Float".to_string()) { "float" }
        else if ty == &Type::Custom("Float64".to_string()) { "double" }
        else { "i64" }
    }

    pub(crate) fn emit_fcmp(&mut self, out: &mut String, indent: &str, l: &Expr, r: &Expr, cond: &str) -> TypedRegister {
        // Peephole: constant-fold integer comparisons at compile time
        if let (Expr::Integer(li), Expr::Integer(ri)) = (l, r) {
            let result = match cond {
                "oeq" => li == ri,
                "one" => li != ri,
                "olt" => li < ri,
                "ole" => li <= ri,
                "ogt" => li > ri,
                "oge" => li >= ri,
                _ => false,
            };
            let v = format!("%t{}", self.fun.txn_counter); self.fun.txn_counter += 1;
            if result {
                writeln!(out, "{}{} = and i1 true, true", indent, v).ok();
            } else {
                writeln!(out, "{}{} = xor i1 true, true", indent, v).ok();
            }
            return TypedRegister { name: v, ty: Type::Custom("Bool".to_string()) };
        }
        // String trigger vs string literal: dereference pointer and compare first byte
        if let Expr::String(s) = r {
            if self.is_linked_string_trigger(l) {
                let a = self.emit_expr(out, l, indent);
                let icmp_cond = match cond {
                    "oeq" => "eq", "one" => "ne", "olt" => "slt",
                    "ole" => "sle", "ogt" => "sgt", "oge" => "sge",
                    _ => cond,
                };
                let p = format!("%fp{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, p, a.name).ok();
                let b = format!("%fb{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                writeln!(out, "{}{} = load i8, ptr {}, align 1", indent, b, p).ok();
                let z = format!("%fz{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                writeln!(out, "{}{} = zext i8 {} to i64", indent, z, b).ok();
                let byte_val = s.as_bytes().first().copied().unwrap_or(0u8) as i64;
                let c = format!("%fc{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                writeln!(out, "{}{} = icmp {} i64 {}, {}", indent, c, icmp_cond, z, byte_val).ok();
                return TypedRegister { name: c, ty: Type::Custom("Bool".to_string()) };
            }
        }
        if let Expr::String(s) = l {
            if self.is_linked_string_trigger(r) {
                let b = self.emit_expr(out, r, indent);
                let icmp_cond = match cond {
                    "oeq" => "eq", "one" => "ne", "olt" => "slt",
                    "ole" => "sle", "ogt" => "sgt", "oge" => "sge",
                    _ => cond,
                };
                let p = format!("%fp{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, p, b.name).ok();
                let bv = format!("%fb{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                writeln!(out, "{}{} = load i8, ptr {}, align 1", indent, bv, p).ok();
                let z = format!("%fz{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                writeln!(out, "{}{} = zext i8 {} to i64", indent, z, bv).ok();
                let byte_val = s.as_bytes().first().copied().unwrap_or(0u8) as i64;
                let c = format!("%fc{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                writeln!(out, "{}{} = icmp {} i64 {}, {}", indent, c, icmp_cond, z, byte_val).ok();
                return TypedRegister { name: c, ty: Type::Custom("Bool".to_string()) };
            }
        }
        let (a, b) = (self.emit_expr(out, l, indent), self.emit_expr(out, r, indent));
        let c = format!("%c{}", self.fun.txn_counter); self.fun.txn_counter += 1;
        if a.ty == Type::Custom("Float".to_string()) || b.ty == Type::Custom("Float".to_string()) {
            let fa = self.ensure_float_reg(out, indent, &a);
            let fb = self.ensure_float_reg(out, indent, &b);
            writeln!(out, "{}{} = fcmp fast {} float {}, {}", indent, c, cond, fa, fb).ok();
        } else {
            let icmp_cond = match cond {
                "oeq" => "eq",
                "one" => "ne",
                "olt" => "slt",
                "ole" => "sle",
                "ogt" => "sgt",
                "oge" => "sge",
                _ => cond,
            };
            // Ensure both operands are i64 for icmp — Bool (i1) and Char (i32)
            // need zext to i64. Uses let mut to handle inline zext.
            let a_i64 = self.adapt_to_i64(out, indent, &a);
            let b_i64 = self.adapt_to_i64(out, indent, &b);
            writeln!(out, "{}{} = icmp {} i64 {}, {}", indent, c, icmp_cond, a_i64, b_i64).ok();
        }
        TypedRegister { name: c, ty: Type::Custom("Bool".to_string()) }
    }

    /// Recursively detect if an expression chain produces a String/Data value.
    /// Used by emit_inline_concat to determine whether to use the inline
    /// concat path or emit generic Add IR.
    ///
    /// Why this exists: a + b on Ints should emit `add i64`, but a + b on
    /// Strings should emit malloc+memcpy. The type tracker checks type
    /// bindings, defn return types, and cast targets.
    pub(crate) fn is_string_chain(&self, e: &Expr) -> bool {
        match e {
            Expr::String(_) => true,
            Expr::Literal(lit) => matches!(lit.as_ref(), crate::features::literal::LiteralExpr::String(_)),
            Expr::Identifier(name) => {
                matches!(self.fun.let_binding_types.get(name), Some(t) if *t == Type::Custom("String".to_string()) || *t == Type::Custom("Data".to_string()))
                || matches!(self.fun.let_original_types.get(name), Some(t) if *t == Type::Custom("String".to_string()) || *t == Type::Custom("Data".to_string()))
                || {
                    // Check state fields whose LLVM type is i8* (String/Data)
                    self.ctx.field_index_map.get(name)
                        .and_then(|&idx| self.ctx.field_types.get(idx))
                        .map(|ft| ft == "i8*" || ft == "ptr")
                        .unwrap_or(false)
                }
            }
            Expr::Add(l, r) | Expr::Concat(l, r) => {
                self.is_string_chain(l) || self.is_string_chain(r)
            }
            Expr::Cast(inner, target_ty) => {
                *target_ty == Type::Custom("String".to_string()) || *target_ty == Type::Custom("Data".to_string())
                    || self.is_string_chain(inner)
            }
            Expr::BinaryOp(bo) if bo.kind == crate::features::binary_op::BinaryOpKind::Add => {
                self.is_string_chain(&bo.left) || self.is_string_chain(&bo.right)
            }
            Expr::Call(name, _) => {
                self.ctx.defn_return_types.get(name.as_str())
                    .map(|types| types.iter().any(|t| *t == Type::Custom("String".to_string()) || *t == Type::Custom("Data".to_string())))
                    .unwrap_or(false)
            }
            _ => false,
        }
    }

    /// Emit native LLVM IR for well-known UserDefinedWithArg projections.
    /// 45+ operator/type pairs (Add/Sub/Mul/Div/Eq/Ne on Int/Float/Bool).
    /// Avoids boxing through i64 — native add/fadd/icmp instructions.
    ///
    /// Why this exists: Brief's projection system is generic (any operator
    /// on any type dispatches through UserDefinedWithArg). But for primitive
    /// types, the generic dispatch would: load i64, convert to native, exec
    /// op, convert back. The fast path emits native IR directly, skipping
    /// both conversions.
    /// Emit LLVM IR for function metadata projections (Address, Name, etc.).
    /// Returns Some(register) if the target is an Fn* variant and the source is a function name.
    pub(super) fn try_emit_fn_projection(&mut self, out: &mut String, source: &Expr, target: &ProjectionTarget, indent: &str) -> Option<TypedRegister> {
        use crate::ast::ProjectionTarget;
        let name = match source {
            Expr::Identifier(n) => n.clone(),
            _ => return None,
        };
        let is_fn = matches!(target,
            ProjectionTarget::Address | ProjectionTarget::Name |
            ProjectionTarget::Params | ProjectionTarget::Returns |
            ProjectionTarget::Arity | ProjectionTarget::Loc |
            ProjectionTarget::Doc | ProjectionTarget::Hash |
            ProjectionTarget::Contracts | ProjectionTarget::Module |
            ProjectionTarget::IsPure | ProjectionTarget::FnSpan);
        if !is_fn { return None; }

        let v = format!("%fnm{}", self.fun.txn_counter); self.fun.txn_counter += 1;

        // Dispatch without exhaustive match to avoid non-exhaustive pattern errors
        if matches!(target, ProjectionTarget::Address) {
            writeln!(out, "{}{} = ptrtoint @{} to i64", indent, v, name).ok();
            return Some(TypedRegister { name: v, ty: Type::Custom("Int".to_string()) });
        }
        if matches!(target, ProjectionTarget::Arity) {
            let arity = self.ctx.defn_params.get(&name)
                .map(|p| p.len() as i64)
                .or_else(|| self.ctx.inop_decls.get(&name).map(|i| i.params.len() as i64))
                .unwrap_or(0);
            writeln!(out, "{}{} = add i64 0, {}", indent, v, arity).ok();
            return Some(TypedRegister { name: v, ty: Type::Custom("Int".to_string()) });
        }
        if matches!(target, ProjectionTarget::Hash) {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            name.hash(&mut hasher);
            let h = hasher.finish() as i64;
            writeln!(out, "{}{} = add i64 0, {}", indent, v, h).ok();
            return Some(TypedRegister { name: v, ty: Type::Custom("Int".to_string()) });
        }
        if matches!(target, ProjectionTarget::IsPure) {
            let is_inop_bang = self.ctx.inop_decls.get(&name).map(|i| i.has_side_effects).unwrap_or(false);
            let val = if is_inop_bang { 0 } else { 1 };
            writeln!(out, "{}{} = add i64 0, {}", indent, v, val).ok();
            return Some(TypedRegister { name: v, ty: Type::Custom("Int".to_string()) });
        }
        // Default for string-valued and other Fn* targets
        writeln!(out, "{}{} = add i64 0, 0 ; {:?}", indent, v, target).ok();
        Some(TypedRegister { name: v, ty: Type::Custom("Int".to_string()) })
    }

    pub(super) fn try_projection_fast_path(
        &mut self,
        out: &mut String,
        src_val: &TypedRegister,
        name: &str,
        arg_expr: &Expr,
        indent: &str,
        v: &str,
    ) -> Option<TypedRegister> {
        let rhs = self.emit_expr(out, arg_expr, indent);
        let tr = match (src_val.ty.clone(), name) {
            // ── Int arithmetic ──
            
(Type::Custom(__t), "Add") if __t == "Int" => {
                writeln!(out, "{}{} = add i64 {}, {}", indent, v, src_val.name, rhs.name).ok();
                TypedRegister { name: v.to_string(), ty: Type::Custom("Int".to_string()) }
            }
            
(Type::Custom(__t), "Sub") if __t == "Int" => {
                writeln!(out, "{}{} = sub i64 {}, {}", indent, v, src_val.name, rhs.name).ok();
                TypedRegister { name: v.to_string(), ty: Type::Custom("Int".to_string()) }
            }
            
(Type::Custom(__t), "Mul") if __t == "Int" => {
                writeln!(out, "{}{} = mul i64 {}, {}", indent, v, src_val.name, rhs.name).ok();
                TypedRegister { name: v.to_string(), ty: Type::Custom("Int".to_string()) }
            }
            
(Type::Custom(__t), "Div") if __t == "Int" => {
                writeln!(out, "{}{} = sdiv i64 {}, {}", indent, v, src_val.name, rhs.name).ok();
                TypedRegister { name: v.to_string(), ty: Type::Custom("Int".to_string()) }
            }
            
(Type::Custom(__t), "Mod") if __t == "Int" => {
                writeln!(out, "{}{} = srem i64 {}, {}", indent, v, src_val.name, rhs.name).ok();
                TypedRegister { name: v.to_string(), ty: Type::Custom("Int".to_string()) }
            }
            // ── Int comparison ──
            
(Type::Custom(__t), "Eq") if __t == "Int" => {
                let cmp = format!("%pcmp{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                writeln!(out, "{}{} = icmp eq i64 {}, {}", indent, cmp, src_val.name, rhs.name).ok();
                writeln!(out, "{}{} = zext i1 {} to i64", indent, v, cmp).ok();
                TypedRegister { name: v.to_string(), ty: Type::Custom("Int".to_string()) }
            }
            
(Type::Custom(__t), "Ne") if __t == "Int" => {
                let cmp = format!("%pcmp{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                writeln!(out, "{}{} = icmp ne i64 {}, {}", indent, cmp, src_val.name, rhs.name).ok();
                writeln!(out, "{}{} = zext i1 {} to i64", indent, v, cmp).ok();
                TypedRegister { name: v.to_string(), ty: Type::Custom("Int".to_string()) }
            }
            
(Type::Custom(__t), "Lt") if __t == "Int" => {
                let cmp = format!("%pcmp{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                writeln!(out, "{}{} = icmp slt i64 {}, {}", indent, cmp, src_val.name, rhs.name).ok();
                writeln!(out, "{}{} = zext i1 {} to i64", indent, v, cmp).ok();
                TypedRegister { name: v.to_string(), ty: Type::Custom("Int".to_string()) }
            }
            
(Type::Custom(__t), "Le") if __t == "Int" => {
                let cmp = format!("%pcmp{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                writeln!(out, "{}{} = icmp sle i64 {}, {}", indent, cmp, src_val.name, rhs.name).ok();
                writeln!(out, "{}{} = zext i1 {} to i64", indent, v, cmp).ok();
                TypedRegister { name: v.to_string(), ty: Type::Custom("Int".to_string()) }
            }
            
(Type::Custom(__t), "Gt") if __t == "Int" => {
                let cmp = format!("%pcmp{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                writeln!(out, "{}{} = icmp sgt i64 {}, {}", indent, cmp, src_val.name, rhs.name).ok();
                writeln!(out, "{}{} = zext i1 {} to i64", indent, v, cmp).ok();
                TypedRegister { name: v.to_string(), ty: Type::Custom("Int".to_string()) }
            }
            
(Type::Custom(__t), "Ge") if __t == "Int" => {
                let cmp = format!("%pcmp{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                writeln!(out, "{}{} = icmp sge i64 {}, {}", indent, cmp, src_val.name, rhs.name).ok();
                writeln!(out, "{}{} = zext i1 {} to i64", indent, v, cmp).ok();
                TypedRegister { name: v.to_string(), ty: Type::Custom("Int".to_string()) }
            }
            // ── Int bitwise ──
            
(Type::Custom(__t), "BitAnd") if __t == "Int" => {
                writeln!(out, "{}{} = and i64 {}, {}", indent, v, src_val.name, rhs.name).ok();
                TypedRegister { name: v.to_string(), ty: Type::Custom("Int".to_string()) }
            }
            
(Type::Custom(__t), "BitOr") if __t == "Int" => {
                writeln!(out, "{}{} = or i64 {}, {}", indent, v, src_val.name, rhs.name).ok();
                TypedRegister { name: v.to_string(), ty: Type::Custom("Int".to_string()) }
            }
            
(Type::Custom(__t), "BitXor") if __t == "Int" => {
                writeln!(out, "{}{} = xor i64 {}, {}", indent, v, src_val.name, rhs.name).ok();
                TypedRegister { name: v.to_string(), ty: Type::Custom("Int".to_string()) }
            }
            
(Type::Custom(__t), "Shl") if __t == "Int" => {
                writeln!(out, "{}{} = shl i64 {}, {}", indent, v, src_val.name, rhs.name).ok();
                TypedRegister { name: v.to_string(), ty: Type::Custom("Int".to_string()) }
            }
            
(Type::Custom(__t), "Shr") if __t == "Int" => {
                writeln!(out, "{}{} = lshr i64 {}, {}", indent, v, src_val.name, rhs.name).ok();
                TypedRegister { name: v.to_string(), ty: Type::Custom("Int".to_string()) }
            }
            // ── Int/Char logical (treated as boolean in Brief) ──
            
(Type::Custom(__t), "And") if __t == "Int" => {
                writeln!(out, "{}{} = and i64 {}, {}", indent, v, src_val.name, rhs.name).ok();
                TypedRegister { name: v.to_string(), ty: Type::Custom("Int".to_string()) }
            }
            
(Type::Custom(__t), "Or") if __t == "Int" => {
                writeln!(out, "{}{} = or i64 {}, {}", indent, v, src_val.name, rhs.name).ok();
                TypedRegister { name: v.to_string(), ty: Type::Custom("Int".to_string()) }
            }
            // ── Float arithmetic ──
            
(Type::Custom(__t), "Add") if __t == "Float" => {
                writeln!(out, "{}{} = fadd float {}, {}", indent, v, src_val.name, rhs.name).ok();
                TypedRegister { name: v.to_string(), ty: Type::Custom("Float".to_string()) }
            }
            
(Type::Custom(__t), "Sub") if __t == "Float" => {
                writeln!(out, "{}{} = fsub float {}, {}", indent, v, src_val.name, rhs.name).ok();
                TypedRegister { name: v.to_string(), ty: Type::Custom("Float".to_string()) }
            }
            
(Type::Custom(__t), "Mul") if __t == "Float" => {
                writeln!(out, "{}{} = fmul float {}, {}", indent, v, src_val.name, rhs.name).ok();
                TypedRegister { name: v.to_string(), ty: Type::Custom("Float".to_string()) }
            }
            
(Type::Custom(__t), "Div") if __t == "Float" => {
                writeln!(out, "{}{} = fdiv float {}, {}", indent, v, src_val.name, rhs.name).ok();
                TypedRegister { name: v.to_string(), ty: Type::Custom("Float".to_string()) }
            }
            
(Type::Custom(__t), "Eq") if __t == "Float" => {
                let cmp = format!("%pcmp{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                writeln!(out, "{}{} = fcmp oeq float {}, {}", indent, cmp, src_val.name, rhs.name).ok();
                let ext = format!("%pce{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                writeln!(out, "{}{} = zext i1 {} to i64", indent, ext, cmp).ok();
                writeln!(out, "{}{} = sitofp i64 {} to float", indent, v, ext).ok();
                TypedRegister { name: v.to_string(), ty: Type::Custom("Float".to_string()) }
            }
            
(Type::Custom(__t), "Ne") if __t == "Float" => {
                let cmp = format!("%pcmp{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                writeln!(out, "{}{} = fcmp one float {}, {}", indent, cmp, src_val.name, rhs.name).ok();
                let ext = format!("%pce{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                writeln!(out, "{}{} = zext i1 {} to i64", indent, ext, cmp).ok();
                writeln!(out, "{}{} = sitofp i64 {} to float", indent, v, ext).ok();
                TypedRegister { name: v.to_string(), ty: Type::Custom("Float".to_string()) }
            }
            
(Type::Custom(__t), "Lt") if __t == "Float" => {
                let cmp = format!("%pcmp{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                writeln!(out, "{}{} = fcmp olt float {}, {}", indent, cmp, src_val.name, rhs.name).ok();
                let ext = format!("%pce{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                writeln!(out, "{}{} = zext i1 {} to i64", indent, ext, cmp).ok();
                writeln!(out, "{}{} = sitofp i64 {} to float", indent, v, ext).ok();
                TypedRegister { name: v.to_string(), ty: Type::Custom("Float".to_string()) }
            }
            
(Type::Custom(__t), "Le") if __t == "Float" => {
                let cmp = format!("%pcmp{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                writeln!(out, "{}{} = fcmp ole float {}, {}", indent, cmp, src_val.name, rhs.name).ok();
                let ext = format!("%pce{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                writeln!(out, "{}{} = zext i1 {} to i64", indent, ext, cmp).ok();
                writeln!(out, "{}{} = sitofp i64 {} to float", indent, v, ext).ok();
                TypedRegister { name: v.to_string(), ty: Type::Custom("Float".to_string()) }
            }
            
(Type::Custom(__t), "Gt") if __t == "Float" => {
                let cmp = format!("%pcmp{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                writeln!(out, "{}{} = fcmp ogt float {}, {}", indent, cmp, src_val.name, rhs.name).ok();
                let ext = format!("%pce{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                writeln!(out, "{}{} = zext i1 {} to i64", indent, ext, cmp).ok();
                writeln!(out, "{}{} = sitofp i64 {} to float", indent, v, ext).ok();
                TypedRegister { name: v.to_string(), ty: Type::Custom("Float".to_string()) }
            }
            
(Type::Custom(__t), "Ge") if __t == "Float" => {
                let cmp = format!("%pcmp{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                writeln!(out, "{}{} = fcmp oge float {}, {}", indent, cmp, src_val.name, rhs.name).ok();
                let ext = format!("%pce{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                writeln!(out, "{}{} = zext i1 {} to i64", indent, ext, cmp).ok();
                writeln!(out, "{}{} = sitofp i64 {} to float", indent, v, ext).ok();
                TypedRegister { name: v.to_string(), ty: Type::Custom("Float".to_string()) }
            }
            // ── Bool logical ──
            
(Type::Custom(__t), "And") if __t == "Bool" => {
                writeln!(out, "{}{} = and i1 {}, {}", indent, v, src_val.name, rhs.name).ok();
                TypedRegister { name: v.to_string(), ty: Type::Custom("Bool".to_string()) }
            }
            
(Type::Custom(__t), "Or") if __t == "Bool" => {
                writeln!(out, "{}{} = or i1 {}, {}", indent, v, src_val.name, rhs.name).ok();
                TypedRegister { name: v.to_string(), ty: Type::Custom("Bool".to_string()) }
            }
            
(Type::Custom(__t), "Eq") if __t == "Bool" => {
                writeln!(out, "{}{} = icmp eq i1 {}, {}", indent, v, src_val.name, rhs.name).ok();
                TypedRegister { name: v.to_string(), ty: Type::Custom("Bool".to_string()) }
            }
            
(Type::Custom(__t), "Ne") if __t == "Bool" => {
                writeln!(out, "{}{} = icmp ne i1 {}, {}", indent, v, src_val.name, rhs.name).ok();
                TypedRegister { name: v.to_string(), ty: Type::Custom("Bool".to_string()) }
            }
            // ── Unknown combination — not a fast-path ──
            _ => return None,
        };
        Some(tr)
    }

    /// Emit a cached projection: load valid flag, branch on hit/miss.
    /// Hit: load cached value. Miss: compute, store in cache, set flag.
    /// Phi merges hit/miss paths. Cache slots are appended to %State by
    /// dead-field elimination (apply_field_modes).
    pub(crate) fn try_cached_projection(&mut self, out: &mut String, source_expr: &Expr,
        src_val: &TypedRegister, target_name: &str, indent: &str) -> Option<TypedRegister>
    {
        // Extract the field name from the source expression (must be a state field identifier)
        let field_name = match source_expr {
            Expr::Identifier(n) => n.clone(),
            _ => return None,
        };
        // Check if this field has a cache slot for this projection target
        let &(cache_idx, valid_idx) = self.ctx.cache_slots.get(&field_name)
            .and_then(|targets| targets.get(target_name))?;

        // 2026-06-28: Use txn_counter to prevent %t{N} collision
        let v = format!("%t{}", self.fun.txn_counter);
        self.fun.txn_counter += 1;
        let valid_gep = format!("%cvp{}", self.fun.txn_counter);
        self.fun.txn_counter += 1;
        writeln!(out, "{}{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}",
            indent, valid_gep, valid_idx).ok();
        let valid_load = format!("%cvv{}", self.fun.txn_counter);
        self.fun.txn_counter += 1;
        writeln!(out, "{}{} = load i8, ptr {}, align 1", indent, valid_load, valid_gep).ok();
        let valid_cond = format!("%cvc{}", self.fun.txn_counter);
        self.fun.txn_counter += 1;
        writeln!(out, "{}{} = icmp ne i8 {}, 0", indent, valid_cond, valid_load).ok();

        let hit_label = format!(".chit{}", self.fun.txn_counter);
        let miss_label = format!(".cmiss{}", self.fun.txn_counter);
        let merge_label = format!(".cmerge{}", self.fun.txn_counter);
        self.fun.txn_counter += 1;
        writeln!(out, "{}br i1 {}, label %{}, label %{}", indent, valid_cond, hit_label, miss_label).ok();
        writeln!(out, "{}:", hit_label).ok();
        let cache_gep = format!("%cve{}", self.fun.txn_counter);
        self.fun.txn_counter += 1;
        writeln!(out, "{}{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}",
            indent, cache_gep, cache_idx).ok();
        let cache_val = format!("%cvv{}", self.fun.txn_counter);
        self.fun.txn_counter += 1;
        writeln!(out, "{}{} = load i64, ptr {}, align 8, !tbaa !1", indent, cache_val, cache_gep).ok();
        writeln!(out, "{}br label %{}", indent, merge_label).ok();
        writeln!(out, "{}:", miss_label).ok();
        // Compute the projection value — reuses the source value as-is
        writeln!(out, "{}{} = add i64 0, {}", indent, v, src_val.name).ok();
        // Store the computed value in the cache and set valid flag
        let store_gep = format!("%cse{}", self.fun.txn_counter);
        self.fun.txn_counter += 1;
        writeln!(out, "{}{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}",
            indent, store_gep, cache_idx).ok();
        writeln!(out, "{}store i64 {}, ptr {}, align 8, !tbaa !1", indent, v, store_gep).ok();
        let valid_store_gep = format!("%csve{}", self.fun.txn_counter);
        self.fun.txn_counter += 1;
        writeln!(out, "{}{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}",
            indent, valid_store_gep, valid_idx).ok();
        writeln!(out, "{}store i8 1, ptr {}, align 1", indent, valid_store_gep).ok();
        writeln!(out, "{}br label %{}", indent, merge_label).ok();
        writeln!(out, "{}:", merge_label).ok();
        let phi_reg = format!("%cp{}", self.fun.txn_counter);
        self.fun.txn_counter += 1;
        writeln!(out, "{}{} = phi i64 [ {}, %{} ], [ {}, %{} ]",
            indent, phi_reg, cache_val, hit_label, v, miss_label).ok();
        Some(TypedRegister { name: phi_reg, ty: Type::Custom("Int".to_string()) })
    }

    /// Phase 2: Check if the source type has a meld route for the given projection target.
    /// When a meld route exists, evaluates the route's destination expression to derive
    /// the projection result from the backing value. Handles:
    /// - `Expr::Identifier(name)` where name is a projection target → emit direct projection
    /// - `Expr::IntrinsicCall { intrinsic, args }` → emit intrinsic with args as projections
    /// - `Expr::Projection { source, target }` → emit projection with substituted source
    pub(crate) fn try_meld_projection(&mut self, out: &mut String, src_val: &TypedRegister,
        target_name: &str, indent: &str) -> Option<TypedRegister>
    {
        let custom_name = match &src_val.ty {
            crate::ast::Type::Custom(n) => n.clone(),
            _ => return None,
        };
        let universe = self.ctx.type_universe.as_ref()?;
        // Find meld — clone data to avoid borrow conflict with mutable self
        let meld_entry = universe.melds.iter().find(|((a, b), _decl)| {
            a == &custom_name || b == &custom_name
        });
        let ((name_a, name_b), meld_decl) = meld_entry?;
        let partner = if *name_a == custom_name { name_b.clone() } else { name_a.clone() };
        let route = meld_decl.routes.iter().find(|r| r.accessor == target_name)?;
        let route_dest = route.dest_expr.clone();

        let result = self.emit_route_expression(out, &route_dest, src_val, &partner, indent);
        if let Some(ref reg) = result {
            // The backing type is the meld partner — the source value is viewed through
            // the custom_name lens but the actual bits are the partner type's bits.
            self.mark_chimera(&reg.name, &partner);
        }
        result
    }

    /// Evaluate a meld route's destination expression, substituting the meld partner's
    /// type name with the actual source value and treating known projection target names
    /// as projections on the backing value.
    fn emit_route_expression(&mut self, out: &mut String, expr: &Expr,
        src_val: &TypedRegister, partner: &str, indent: &str) -> Option<TypedRegister>
    {
        match expr {
            // Pattern 1: identity projection — "Ptr" or "Size" on the backing value
            Expr::Identifier(name) if name == "Ptr" || name == "Size"
                || name == "Bytes" || name == "Alignment" || name == "Type" => {
                self.emit_direct_projection(out, src_val, name, indent)
            }
            // Pattern 2: intrinsic call — "strlen#(Ptr)" etc.
            Expr::IntrinsicCall { intrinsic, args } => {
                // 2026-06-28: Use txn_counter to prevent %t{N} collision
                let v = format!("%t{}", self.fun.txn_counter);
                self.fun.txn_counter += 1;
                // Handle strlen#(arg) — the common meld route for CString.Size
                if let crate::ast::Intrinsic::Strlen = intrinsic {
                    if args.len() == 1 {
                        let arg_name = match &args[0] {
                            Expr::Identifier(n) => Some(n.clone()),
                            _ => None,
                        };
                        if let Some(ref name) = arg_name {
                            if name == "Ptr" || name == "Size" || name == "Bytes" {
                                let proj_reg = self.emit_direct_projection(out, src_val, name, indent)?;
                                writeln!(out, "{}{} = call i64 @__strlen__(i64 {})", indent, v, proj_reg.name).ok();
                                return Some(TypedRegister { name: v, ty: Type::Custom("Int".to_string()) });
                            }
                        }
                    }
                }
                None
            }
            // Pattern 3: field access on the partner type — "CString.ptr"
            Expr::FieldAccess(obj, field) => {
                if let Expr::Identifier(n) = obj.as_ref() {
                    if n == partner {
                        // Substitute with the actual source value and emit the field
                        // as the corresponding projection (Ptr, Size, etc.)
                        self.emit_direct_projection(out, src_val, field, indent)
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            // Pattern 4: projection on the partner type — "CString :> Size"
            Expr::Projection { source: sub_source, target: sub_target } => {
                let sub_name = match sub_source.as_ref() {
                    Expr::Identifier(n) => Some(n.clone()),
                    _ => None,
                };
                if let Some(ref name) = sub_name {
                    if name == partner {
                        // Substitute with the actual source value and emit the projection
                        // without going through the meld check again (avoid recursion)
                        let target_name = crate::analysis::transition_graph::projection_target_name(sub_target);
                        self.emit_direct_projection(out, src_val, &target_name, indent)
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Phase 3: Decay a chimera value to its canonical type at a boundary.
    /// When `target_ty` is `None`, assumes decay to the backing type (identity).
    /// Real field-level materialization will be added per type pair.
    pub(crate) fn emit_decay(&mut self, out: &mut String, val: &TypedRegister,
        target_ty: Option<&Type>, indent: &str) -> TypedRegister
    {
        if !self.is_chimera(&val.name) {
            return val.clone();
        }
        let backing = match self.chimera_backing(&val.name) {
            Some(b) => b.to_string(),
            None => return val.clone(),
        };
        let target_name = match target_ty {
            Some(Type::Custom(n)) => n.clone(),
            _ => return val.clone(), // primitive target → identity (bits are valid)
        };
        if backing == target_name {
            // Decay to own backing type — identity
            return val.clone();
        }
        // Generic materialization: look up the meld between backing and target,
        // derive each field of the target type from the backing value via routes.
        // Clone all data first to avoid borrow conflicts with mutable self.
        let meld_routes: Vec<crate::ast::MeldRouteDef> = {
            let universe = match self.ctx.type_universe.as_ref() {
                Some(u) => u,
                None => return val.clone(),
            };
            match universe.find_meld(&backing, &target_name) {
                Some(m) => m.routes.clone(),
                None => return val.clone(),
            }
        };
        let target_fields = match self.ctx.struct_types.get(&target_name) {
            Some(f) => f.clone(),
            None => return val.clone(),
        };

        // Derive each field value from the backing via meld routes
        let mut field_results: Vec<(String, Type, String)> = Vec::new(); // name, ty, reg
        for (field_name, field_ty) in &target_fields {
            if let Some(route) = meld_routes.iter().find(|r| r.accessor == *field_name) {
                // Use backing as partner — the route evaluates from backing's perspective
                if let Some(reg) = self.emit_route_expression(out, &route.dest_expr, val, &backing, indent) {
                    field_results.push((field_name.clone(), field_ty.clone(), reg.name));
                } else {
                    // Route evaluation failed — emit 0 as placeholder
        // 2026-06-28: Use txn_counter to prevent %t{N} collision
        let v = format!("%t{}", self.fun.txn_counter);
        self.fun.txn_counter += 1;
                    writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
                    field_results.push((field_name.clone(), field_ty.clone(), v));
                }
            } else {
                // No route for this field — emit 0 as placeholder
                // 2026-06-28: Use txn_counter to prevent %t{N} collision
                let v = format!("%t{}", self.fun.txn_counter);
                self.fun.txn_counter += 1;
                writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
                field_results.push((field_name.clone(), field_ty.clone(), v));
            }
        }

        if field_results.is_empty() {
            return val.clone();
        }

        // For single-field types: return the field value directly
        if field_results.len() == 1 {
            let (_, ref ty, ref reg) = field_results[0];
            return TypedRegister { name: reg.clone(), ty: ty.clone() };
        }

        // For multi-field types: allocate a struct on the heap
        let total_size = field_results.len() * 8; // each field is i64
        // 2026-06-28: Use txn_counter to prevent %t{N} collision
        let alloc = format!("%t{}", self.fun.txn_counter);
        self.fun.txn_counter += 1;
        writeln!(out, "{}{} = call ptr @malloc(i64 {})", indent, alloc, total_size).ok();
        let struct_ptr = format!("%t{}", self.fun.txn_counter);
        self.fun.txn_counter += 1;
        writeln!(out, "{}{} = bitcast ptr {} to ptr", indent, struct_ptr, alloc).ok();

        for (i, (_name, _ty, reg)) in field_results.iter().enumerate() {
            // 2026-06-28: Use txn_counter to prevent %t{N} collision
            let gep = format!("%t{}", self.fun.txn_counter);
            self.fun.txn_counter += 1;
            writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 {}", indent, gep, struct_ptr, i).ok();
            writeln!(out, "{}store i64 {}, ptr {}, align 8, !tbaa !1", indent, reg, gep).ok();
        }

        // 2026-06-29: FIXED — return struct_ptr (base of allocated struct) instead of
        // struct_ptr_name (which was computed as txn_counter-1 after the loop, pointing
        // to the LAST field's GEP register). The old code returned a pointer to the last
        // field instead of the struct base, causing memory corruption in FFI consumers.
        TypedRegister { name: struct_ptr, ty: Type::Custom(target_name.clone()) }
    }

    /// Emit a direct projection on a value without going through the meld route check.
    /// This avoids infinite recursion when a meld route maps to the same projection target.
    fn emit_direct_projection(&mut self, out: &mut String, src_val: &TypedRegister,
        target_name: &str, indent: &str) -> Option<TypedRegister>
    {
        // 2026-06-28: Use txn_counter to prevent %t{N} collision
        let v = format!("%t{}", self.fun.txn_counter);
        self.fun.txn_counter += 1;
        match target_name {
            "Ptr" => {
                writeln!(out, "{}{} = add i64 0, {} ; ptr", indent, v, src_val.name).ok();
                Some(TypedRegister { name: v, ty: Type::Custom("Int".to_string()) })
            }
            "Size" => {
                let hp = format!("%drphp{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, hp, src_val.name).ok();
                let lp = format!("%drplp{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 1", indent, lp, hp).ok();
                writeln!(out, "{}{} = load i64, ptr {}, align 8, !tbaa !1", indent, v, lp).ok();
                Some(TypedRegister { name: v, ty: Type::Custom("Int".to_string()) })
            }
            "Bytes" => {
                writeln!(out, "{}{} = add i64 0, 8 ; bytes", indent, v).ok();
                Some(TypedRegister { name: v, ty: Type::Custom("Int".to_string()) })
            }
            "Alignment" => {
                writeln!(out, "{}{} = add i64 0, 8 ; alignment", indent, v).ok();
                Some(TypedRegister { name: v, ty: Type::Custom("Int".to_string()) })
            }
            "Type" => {
                writeln!(out, "{}{} = add i64 0, 6 ; type=custom", indent, v).ok();
                Some(TypedRegister { name: v, ty: Type::Custom("Int".to_string()) })
            }
            _ => None,
        }
    }

    /// 2026-07-03: Try to emit EOR-optimized cast: detects
    /// Cast(BinaryOp(Cast(a, T), Cast(b, T)), U) where U <:> T.
    /// If matched, emits the binary op directly without redundant casts.
    pub(super) fn try_emit_eor(
        &mut self,
        out: &mut String,
        v: &str,
        inner: &crate::ast::Expr,
        target_ty: &crate::ast::Type,
        indent: &str,
    ) -> Option<TypedRegister> {
        use crate::ast::Expr;
        let (lhs, rhs) = match inner {
            Expr::Add(l, r) | Expr::Sub(l, r) | Expr::Mul(l, r) | Expr::Div(l, r) => {
                (l.as_ref().clone(), r.as_ref().clone())
            }
            _ => return None,
        };
        let cast_ty = match (&lhs, &rhs) {
            (Expr::Cast(_, lt), Expr::Cast(_, rt)) if lt == rt => lt.clone(),
            _ => return None,
        };
        let has_meld = self.ctx.type_universe.as_ref()
            .and_then(|tu| {
                let tn = match target_ty {
                    crate::ast::Type::Custom(n) => Some(n.as_str()),
                    crate::ast::Type::Applied(n, _) => Some(n.as_str()),
                    _ => None,
                };
                tn.and_then(|n| tu.find_meld(n, cast_ty.universe_key()))
            })
            .is_some();
        if !has_meld {
            return None;
        }
        let a = self.emit_expr(out, &lhs, indent);
        let b = self.emit_expr(out, &rhs, indent);
        let fl_op = match inner {
            Expr::Add(_, _) => "fadd",
            Expr::Sub(_, _) => "fsub",
            Expr::Mul(_, _) => "fmul",
            Expr::Div(_, _) => "fdiv",
            _ => return None,
        };
        let i_op = match inner {
            Expr::Add(_, _) => "add",
            Expr::Sub(_, _) => "sub",
            Expr::Mul(_, _) => "mul",
            Expr::Div(_, _) => "sdiv",
            _ => return None,
        };
        if cast_ty == Type::Custom("Float".to_string()) || cast_ty == Type::Custom("Float64".to_string()) {
            let fl_a = self.ensure_float_reg(out, indent, &a);
            let fl_b = self.ensure_float_reg(out, indent, &b);
            writeln!(out, "{}{} = {} float {}, {}", indent, v, fl_op, fl_a, fl_b).ok();
            let bi = format!("%eor_bi{}", self.fun.txn_counter);
            self.fun.txn_counter += 1;
            let ze = format!("%eor_ze{}", self.fun.txn_counter);
            writeln!(out, "{}{} = bitcast float {} to i32", indent, bi, v).ok();
            writeln!(out, "{}{} = zext i32 {} to i64", indent, ze, bi).ok();
            self.fun.reg_float_cache.insert(ze.clone(), v.to_string());
            let ret_ty = if cast_ty == Type::Custom("Float64".to_string()) { Type::Custom("Float64".to_string()) } else { Type::Custom("Float".to_string()) };
            Some(TypedRegister { name: ze, ty: ret_ty })
        } else {
            writeln!(out, "{}{} = {} i64 {}, {}", indent, v, i_op, a, b).ok();
            Some(TypedRegister { name: v.to_string(), ty: cast_ty })
        }
    }
}
