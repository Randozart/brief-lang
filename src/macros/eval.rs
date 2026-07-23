// ── Navigation Chain Evaluation ─────────────────────────────────────────
// 2026-07-21: Evaluates a tree of $ calls against the live compilation state.
// Each $ name dispatches to the appropriate selection, traversal, position,
// or action handler.
// 2026-07-23: Added capability sandboxing — each intrinsic checks its
// capability requirement before executing. Side-effect intrinsics require
// explicit --allow-* flags.
//
// Max 2 levels per function. Flat dispatch: one arm per intrinsic name.

use std::collections::HashMap;
use crate::ast::{Expr, Statement, TopLevel, PropertyValue, ImportKind, Type};
use crate::ast::top::*;
use crate::ast::StageKind;
use crate::type_universe::TypeUniverse;
use crate::plugin::PluginManager;
use super::selection::{
    Selection, NodeRef, Selector,
    TagSelector, NamedSelector, WithKeySelector, WithAttrSelector, AllSelector,
    node_tag, top_level_name,
};
use super::actions::{
    Position, insert_items, insert_before_each, insert_after_each,
    delete_selection, replace_selection, set_metadata, rename_selection,
};
use super::text_ops::TextSelection;
use super::stage_target;

/// Sandbox permissions for compile-time $ intrinsics.
/// Each capability corresponds to a --allow-* CLI flag.
/// By default, only pure AST/string intrinsics are allowed.
#[derive(Debug, Clone, Default)]
pub struct Sandbox {
    /// Read files via FileRead$
    pub allow_read: bool,
    /// Write files via FileWrite$
    pub allow_write: bool,
    /// Execute shell commands via ShellCmd$
    pub allow_run: bool,
    /// Query host hardware via SysQuery$
    pub allow_sys_query: bool,
    /// Access network via HttpFetch$
    pub allow_net: bool,
    /// Instruction budget (gas limit), 0 = unlimited
    pub budget: u64,
    /// Remaining instructions this macro can execute
    pub remaining: u64,
}

impl Sandbox {
    /// Create a new sandbox with all capabilities granted (for backward compat).
    pub fn permissive() -> Self {
        Self {
            allow_read: true, allow_write: true, allow_run: true,
            allow_sys_query: true, allow_net: true,
            budget: 0, remaining: 0,
        }
    }

    /// Consume one unit of the instruction budget.
    /// Returns an error if the budget is exhausted.
    pub fn consume(&mut self, intrinsic: &str) -> Result<(), String> {
        if self.budget > 0 {
            if self.remaining == 0 {
                return Err(format!(
                    "instruction budget exceeded (limit {}): called '{}'",
                    self.budget, intrinsic
                ));
            }
            self.remaining -= 1;
        }
        Ok(())
    }

    /// Check that a capability is granted.
    pub fn check(&self, cap: Capability, intrinsic: &str) -> Result<(), String> {
        let granted = match cap {
            Capability::Pure => true,
            Capability::DiskRead => self.allow_read,
            Capability::DiskWrite => self.allow_write,
            Capability::Shell => self.allow_run,
            Capability::SysQuery => self.allow_sys_query,
            Capability::Network => self.allow_net,
        };
        if !granted {
            Err(format!(
                "intrinsic '{}' requires capability '{:?}'. Run with the corresponding --allow-* flag.",
                intrinsic, cap
            ))
        } else {
            Ok(())
        }
    }
}

/// Capability categories for $ intrinsics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Capability {
    Pure,
    DiskRead,
    DiskWrite,
    Shell,
    SysQuery,
    Network,
}

/// Result of evaluating a single navigation chain link.
#[derive(Debug, Clone)]
pub enum NavValue {
    Selection(Selection),
    Position(Position),
    TextSelection(TextSelection),
    Count(usize),
    Names(Vec<String>),
    Bool(bool),
    Int(i64),
    Str(String),
    TopLevel(TopLevel),
    VecTopLevel(Vec<TopLevel>),
    List(Vec<NavValue>),
    Void,
}

/// Extract a reference to the Sandbox from the optional PluginManager.
/// When no PluginManager is available (tests), returns a permissive sandbox.

type Scope = HashMap<String, NavValue>;

// ── Main Entry Points ──────────────────────────────────────────────────

/// Evaluate a $(Stage) block body.
pub fn evaluate_stage_block(
    body: &[Statement],
    program: &mut Vec<TopLevel>,
    universe: &mut TypeUniverse,
    stage: StageKind,
    mut pm: &mut Option<&mut PluginManager>,
) -> Result<(), String> {
    let mut scope = Scope::new();
    let sandbox = pm.as_ref().map(|p| p.sandbox.clone()).unwrap_or_else(Sandbox::permissive);
    let mut sandbox = sandbox;
    for stmt in body {
        evaluate_stage_stmt(stmt, program, universe, stage, &mut scope, &mut sandbox, &mut pm)?;
    }
    Ok(())
}

/// Evaluate a $ call tree. `pm` is a mutable ref to Option for reborrowing.
pub fn eval_nav_chain(
    expr: &Expr,
    program: &mut Vec<TopLevel>,
    universe: &mut TypeUniverse,
    stage: StageKind,
    scope: &Scope,
    sandbox: &mut Sandbox,
    pm: &mut Option<&mut PluginManager>,
) -> Result<NavValue, String> {
    match expr {
        Expr::Call(name, args, _) if name.ends_with('$') => {
            eval_nav_call(name, args, program, universe, stage, scope, sandbox, pm)
        }
        Expr::Field(obj, name) if name.ends_with('$') => {
            let prev = eval_nav_chain(obj, program, universe, stage, scope, sandbox, pm)?;
            eval_nav_field_method(name, prev, program, stage)
        }
        // Fix 1: Resolve identifiers from the compile-time scope
        Expr::Identifier(name) => {
            if let Some(val) = scope.get(name) {
                Ok(val.clone())
            } else {
                Err(format!("undefined compile-time variable '{}'", name))
            }
        }
        Expr::Decimal(n) => Ok(NavValue::Int(*n)),
        Expr::Float(n) => Ok(NavValue::Int(*n as i64)),
        Expr::Bool(b) => Ok(NavValue::Bool(*b)),
        other => Err(format!(
            "expected a $ navigation call, got {:?}", other
        )),
    }
}

// ── Statement Evaluation ───────────────────────────────────────────────

/// Extract an i64 value from a NavValue for comparison operators.
fn nav_to_i64(val: &NavValue) -> i64 {
    match val {
        NavValue::Count(n) => *n as i64,
        NavValue::Int(n) => *n,
        NavValue::Bool(b) => if *b { 1 } else { 0 },
        _ => 0,
    }
}

fn evaluate_stage_stmt(
    stmt: &Statement,
    program: &mut Vec<TopLevel>,
    universe: &mut TypeUniverse,
    stage: StageKind,
    scope: &mut Scope,
    sandbox: &mut Sandbox,
    pm: &mut Option<&mut PluginManager>,
) -> Result<(), String> {
    match stmt {
        Statement::Expression(expr) => {
            let _result = eval_nav_chain(expr, program, universe, stage, scope, sandbox, pm)?;
            Ok(())
        }
        Statement::Let { name, expr: Some(e), .. } => {
            let result = eval_nav_chain(e, program, universe, stage, scope, sandbox, pm)?;
            scope.insert(name.clone(), result);
            Ok(())
        }
        Statement::Block(statements) => {
            for s in statements {
                evaluate_stage_stmt(s, program, universe, stage, scope, sandbox, pm)?;
            }
            Ok(())
        }
        Statement::Foreach { item, list, body } => {
            let list_val = eval_nav_chain(list, program, universe, stage, scope, sandbox, pm)?;
            let items = match list_val {
                NavValue::Selection(sel) => sel,
                _ => return Err("foreach: expected a Selection from the list expression".into()),
            };
            for node in &items.nodes {
                // Bind the loop variable to a single-element selection
                let selection = Selection::single(node.clone());
                scope.insert(item.clone(), NavValue::Selection(selection));
                for s in body {
                    evaluate_stage_stmt(s, program, universe, stage, scope, sandbox, pm)?;
                }
            }
            scope.remove(item);
            Ok(())
        }
        // 2026-07-21: Fix A — evaluate when guards in stage blocks.
        // Without this, the prelude plugin's `when anchor.Count$() > 0 { ... }`
        // is silently skipped, never inserting stdlib imports.
        Statement::Guarded(guard, body) => {
            // Handle BinaryOp comparisons (Count$() > 0, etc.) which
            // eval_nav_chain doesn't support directly.
            let result = match guard {
                Expr::BinaryOp(op, left, right) => {
                    let l = eval_nav_chain(left, program, universe, stage, scope, sandbox, pm)?;
                    let r = eval_nav_chain(right, program, universe, stage, scope, sandbox, pm)?;
                    // Extract integer values from both sides for comparison
                    let lv = nav_to_i64(&l);
                    let rv = nav_to_i64(&r);
                    let cmp = match op {
                        crate::ast::BinaryOpKind::Eq => lv == rv,
                        crate::ast::BinaryOpKind::Neq => lv != rv,
                        crate::ast::BinaryOpKind::Gt => lv > rv,
                        crate::ast::BinaryOpKind::Lt => lv < rv,
                        crate::ast::BinaryOpKind::Ge => lv >= rv,
                        crate::ast::BinaryOpKind::Le => lv <= rv,
                        _ => return Err(format!("unsupported operator in when guard: {:?}", op)),
                    };
                    NavValue::Bool(cmp)
                }
                _ => eval_nav_chain(guard, program, universe, stage, scope, sandbox, pm)?,
            };
            let is_truthy = match &result {
                NavValue::Selection(sel) => !sel.nodes.is_empty(),
                NavValue::Count(n) => *n > 0,
                NavValue::Bool(b) => *b,
                NavValue::Int(n) => *n != 0,
                NavValue::Str(s) => !s.is_empty(),
                NavValue::Void => false,
                _ => true,
            };
            if is_truthy {
                for s in body {
                    evaluate_stage_stmt(s, program, universe, stage, scope, sandbox, pm)?;
                }
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

// ── Intrinsic Dispatch ─────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn eval_nav_call(
    name: &str,
    args: &[Expr],
    program: &mut Vec<TopLevel>,
    universe: &mut TypeUniverse,
    stage: StageKind,
    scope: &Scope,
    sandbox: &mut Sandbox,
    pm: &mut Option<&mut PluginManager>,
) -> Result<NavValue, String> {
    // 2026-07-23: Every $ intrinsic call consumes one unit of the gas budget.
    sandbox.consume(name)?;
    match name {
        // ── Selectors ──────────────────────────────────────────────
        "Tag$" => selector_1_str(args, |s| {
            Ok(NavValue::Selection(Selection { nodes: TagSelector { tag: s }.apply(program)? }))
        }),
        "Named$" => selector_1_str(args, |s| {
            Ok(NavValue::Selection(Selection { nodes: NamedSelector { name: s }.apply(program)? }))
        }),
        "WithKey$" => selector_1_str(args, |s| {
            Ok(NavValue::Selection(Selection { nodes: WithKeySelector { key: s }.apply(program)? }))
        }),
        "WithAttr$" => {
            let key = expect_str_arg(args, 0, "WithAttr$")?;
            let val = expect_str_arg(args, 1, "WithAttr$")?;
            Ok(NavValue::Selection(Selection {
                nodes: WithAttrSelector { key, val }.apply(program)?
            }))
        }
        "All$" => Ok(NavValue::Selection(Selection {
            nodes: AllSelector.apply(program)?
        })),

        // ── Traversal ──────────────────────────────────────────────
        "First$" => selector_1_int_opt(args, |n| {
            let prev = eval_nav_chain(&args[0], program, universe, stage, scope, sandbox, pm)?;
            match prev {
                NavValue::Selection(sel) => Ok(NavValue::Selection(sel.first(n))),
                _ => Err("First$ requires a Selection operand".into()),
            }
        }),
        "Last$" => selector_1_int_opt(args, |n| {
            let prev = eval_nav_chain(&args[0], program, universe, stage, scope, sandbox, pm)?;
            match prev {
                NavValue::Selection(sel) => Ok(NavValue::Selection(sel.last(n))),
                _ => Err("Last$ requires a Selection operand".into()),
            }
        }),
        "Nth$" => {
            let n = expect_int_arg(args, 0, "Nth$")? as usize;
            let prev = eval_nav_chain(&args[1], program, universe, stage, scope, sandbox, pm)?;
            match prev {
                NavValue::Selection(sel) => Ok(NavValue::Selection(sel.nth(n))),
                _ => Err("Nth$ requires a Selection operand".into()),
            }
        }
        "Children$" => {
            let filter = args.first().and_then(|a| extract_str_lit(a));
            let prev_arg = if filter.is_some() { args.get(1) } else { args.first() };
            let Some(prev_arg) = prev_arg else {
                return Err("Children$: missing operand".into());
            };
            let prev = eval_nav_chain(prev_arg, program, universe, stage, scope, sandbox, pm)?;
            match prev {
                NavValue::Selection(sel) => Ok(NavValue::Selection(
                    sel.children(program, filter.as_deref())
                )),
                _ => Err("Children$ requires a Selection operand".into()),
            }
        }
        "Descendants$" => {
            let filter = args.first().and_then(|a| extract_str_lit(a));
            let prev_arg = if filter.is_some() { args.get(1) } else { args.first() };
            let Some(prev_arg) = prev_arg else {
                return Err("Descendants$: missing operand".into());
            };
            let prev = eval_nav_chain(prev_arg, program, universe, stage, scope, sandbox, pm)?;
            match prev {
                NavValue::Selection(sel) => Ok(NavValue::Selection(
                    sel.descendants(program, filter.as_deref())
                )),
                _ => Err("Descendants$ requires a Selection operand".into()),
            }
        }
        "Parent$" => {
            let prev = eval_nav_chain(&args[0], program, universe, stage, scope, sandbox, pm)?;
            match prev {
                NavValue::Selection(sel) => Ok(NavValue::Selection(sel.parent(program))),
                _ => Err("Parent$ requires a Selection operand".into()),
            }
        }

        // ── Introspection ──────────────────────────────────────────
        "Count$" => {
            let prev = if args.is_empty() {
                NavValue::Selection(Selection { nodes: AllSelector.apply(program)? })
            } else {
                eval_nav_chain(&args[0], program, universe, stage, scope, sandbox, pm)?
            };
            match prev {
                NavValue::Selection(sel) => Ok(NavValue::Count(sel.count())),
                _ => Err("Count$ requires a Selection operand".into()),
            }
        }
        "Names$" => {
            let prev = eval_nav_chain(&args[0], program, universe, stage, scope, sandbox, pm)?;
            match prev {
                NavValue::Selection(sel) => Ok(NavValue::Names(sel.names(program))),
                _ => Err("Names$ requires a Selection operand".into()),
            }
        }
        "IsEmpty$" => {
            let prev = eval_nav_chain(&args[0], program, universe, stage, scope, sandbox, pm)?;
            match prev {
                NavValue::Selection(sel) => Ok(NavValue::Bool(sel.is_empty())),
                _ => Err("IsEmpty$ requires a Selection operand".into()),
            }
        }

        // ── Positions ──────────────────────────────────────────────
        "Before$" => {
            let prev = eval_nav_chain(&args[0], program, universe, stage, scope, sandbox, pm)?;
            match prev {
                NavValue::Selection(ref sel) => Position::before(sel)
                    .map(|p| NavValue::Position(p))
                    .ok_or_else(|| "Before$: selection is empty".into()),
                _ => Err("Before$ requires a Selection operand".into()),
            }
        }
        "After$" => {
            let prev = eval_nav_chain(&args[0], program, universe, stage, scope, sandbox, pm)?;
            match prev {
                NavValue::Selection(ref sel) => Position::after(sel)
                    .map(|p| NavValue::Position(p))
                    .ok_or_else(|| "After$: selection is empty".into()),
                _ => Err("After$ requires a Selection operand".into()),
            }
        }
        "Replace$" => {
            let prev = eval_nav_chain(&args[0], program, universe, stage, scope, sandbox, pm)?;
            match prev {
                NavValue::Selection(ref sel) => Position::replace(sel)
                    .map(|p| NavValue::Position(p))
                    .ok_or_else(|| "Replace$: selection is empty".into()),
                _ => Err("Replace$ requires a Selection operand".into()),
            }
        }
        "Inside$" => {
            let prev = eval_nav_chain(&args[0], program, universe, stage, scope, sandbox, pm)?;
            match prev {
                NavValue::Selection(ref sel) => Position::inside(sel)
                    .map(|p| NavValue::Position(p))
                    .ok_or_else(|| "Inside$: selection is empty".into()),
                _ => Err("Inside$ requires a Selection operand".into()),
            }
        }
        "AppendTo$" => {
            let prev = eval_nav_chain(&args[0], program, universe, stage, scope, sandbox, pm)?;
            match prev {
                NavValue::Selection(ref sel) => Position::append_to(sel)
                    .map(|p| NavValue::Position(p))
                    .ok_or_else(|| "AppendTo$: selection is empty".into()),
                _ => Err("AppendTo$ requires a Selection operand".into()),
            }
        }

        // Fix 2: Stage$.Insert$ vs regular Insert$
        // Stage$.Insert$("path") → first arg is Identifier("Stage$")
        // Tag$("x").Insert$(y) → first arg is a Call chain (receiver)
        "Insert$" if is_stage_receiver(args) => {
            let Some(pm) = pm else {
                return Err("Stage$.Insert$: no PluginManager available".into());
            };
            let path = expect_str_arg(args, 1, "Stage$.Insert$")?;
            stage_target::insert_plugin_from_file(pm, &path, stage)
                .map(|_| NavValue::Void)
        }
        // ── Actions ────────────────────────────────────────────────
        "Insert$" => {
            let pos_result = eval_nav_chain(&args[0], program, universe, stage, scope, sandbox, pm)?;
            let mut nodes = Vec::new();
            for arg in &args[1..] {
                let val = eval_nav_chain(arg, program, universe, stage, scope, sandbox, pm)?;
                match val {
                    NavValue::TopLevel(tl) => nodes.push(tl),
                    NavValue::VecTopLevel(v) => nodes.extend(v),
                    _ => return Err("Insert$: argument must produce an AST node".into()),
                }
            }
            match pos_result {
                NavValue::Position(pos) => {
                    insert_items(program, &pos, nodes, stage).map(|_| NavValue::Void)
                }
                NavValue::Selection(sel) => {
                    insert_before_each(program, &sel, nodes, stage).map(|_| NavValue::Void)
                }
                _ => Err("Insert$ requires a Position or Selection operand".into()),
            }
        }
        "Delete$" => {
            let prev = eval_nav_chain(&args[0], program, universe, stage, scope, sandbox, pm)?;
            match prev {
                NavValue::Selection(sel) => delete_selection(program, &sel).map(|_| NavValue::Void),
                _ => Err("Delete$ requires a Selection operand".into()),
            }
        }
        "ReplaceWith$" => {
            let prev = eval_nav_chain(&args[0], program, universe, stage, scope, sandbox, pm)?;
            let repl = eval_nav_chain(&args[1], program, universe, stage, scope, sandbox, pm)?;
            let repl_node = match repl {
                NavValue::TopLevel(tl) => tl,
                _ => return Err("ReplaceWith$: second arg must produce a TopLevel node".into()),
            };
            match prev {
                NavValue::Selection(sel) => {
                    replace_selection(program, &sel, repl_node).map(|_| NavValue::Void)
                }
                _ => Err("ReplaceWith$ requires a Selection operand".into()),
            }
        }
        // Fix 3: Set$ — parse second arg as PropertyValue
        "Set$" => {
            let prev = eval_nav_chain(&args[0], program, universe, stage, scope, sandbox, pm)?;
            let key = expect_str_arg(args, 1, "Set$")?;
            let val = expect_prop_arg(args, 2, "Set$")?;
            match prev {
                NavValue::Selection(sel) => {
                    set_metadata(program, &sel, &key, val).map(|_| NavValue::Void)
                }
                _ => Err("Set$ requires a Selection operand".into()),
            }
        }
        "Rename$" => {
            let prev = eval_nav_chain(&args[0], program, universe, stage, scope, sandbox, pm)?;
            let name = expect_str_arg(args, 1, "Rename$")?;
            match prev {
                NavValue::Selection(sel) => {
                    rename_selection(program, &sel, &name).map(|_| NavValue::Void)
                }
                _ => Err("Rename$ requires a Selection operand".into()),
            }
        }

        // ── AST Constructors ───────────────────────────────────────
        "Import$" => {
            let path = expect_str_arg(args, 0, "Import$")?;
            Ok(NavValue::TopLevel(TopLevel::Import(Import::literal(path, vec![]))))
        }
        "Defn$" => {
            let name = expect_str_arg(args, 0, "Defn$")?;
            Ok(NavValue::TopLevel(TopLevel::Definition(Definition {
                name, type_params: vec![], parameters: vec![],
                output_type: None, outputs: vec![],
                contract: Contract::new(Expr::Bool(true), Expr::Bool(true)),
                body: vec![], metadata: Default::default(),
                derivation: None, modifiers: vec![], annotations: vec![], span: None,
            })))
        }
        "Call$" => {
            let fn_name = expect_str_arg(args, 0, "Call$")?;
            let mut call_args = Vec::new();
            for arg in &args[1..] {
                let val = eval_nav_chain(arg, program, universe, stage, scope, sandbox, pm)?;
                if let NavValue::TopLevel(TopLevel::Statement(stmt)) = val {
                    if let Statement::Expression(expr) = stmt.as_ref() {
                        call_args.push(expr.clone());
                    }
                }
            }
            Ok(NavValue::TopLevel(TopLevel::Statement(Box::new(
                Statement::Expression(Expr::Call(fn_name, call_args, None))
            ))))
        }
        "Block$" => {
            let mut stmts = Vec::new();
            for arg in args {
                if let NavValue::TopLevel(TopLevel::Statement(stmt)) =
                    eval_nav_chain(arg, program, universe, stage, scope, sandbox, pm)?
                {
                    stmts.push(*stmt);
                }
            }
            Ok(NavValue::TopLevel(TopLevel::Statement(Box::new(
                Statement::Block(stmts)
            ))))
        }
        // 2026-07-23: Quote$ — structural quasiquoting.
        // Template string parsed as Brief source; $ident references resolved
        // from compile-time scope variables. $$escapes to literal $.
        // Returns TopLevel (single item) or VecTopLevel (multiple items).
        "Quote$" => {
            let template = expect_str_arg(args, 0, "Quote$")?;
            let tokens = crate::lexer::tokenize(&template)
                .map_err(|e| format!("Quote$: tokenization error: {}", e))?;
            let mut parser = crate::parser::Parser::new(tokens, &template);
            let mut items = parser.parse_program()
                .map_err(|e| format!("Quote$: parse error: {}", e))?;
            // Resolve $$ → $ escapes and $ident references from scope.
            // $$handled at AST level by resolve_dollar_refs_in_expr.
            for item in items.iter_mut() {
                resolve_dollar_refs_in_toplevel(item, scope)?;
            }
            if items.len() == 1 {
                Ok(NavValue::TopLevel(items.into_iter().next().unwrap()))
            } else {
                Ok(NavValue::VecTopLevel(items))
            }
        }

        // Fix 2: Stage$.List$ — list registered plugins
        "List$" if is_stage_receiver(args) => {
            let Some(pm) = pm else {
                return Err("Stage$.List$: no PluginManager available".into());
            };
            let names = stage_target::list_plugins(pm);
            Ok(NavValue::Names(names))
        }
        // Stage$.Remove$("name") — disable a plugin
        "Remove$" if is_stage_receiver(args) => {
            let Some(pm) = pm else {
                return Err("Stage$.Remove$: no PluginManager available".into());
            };
            let name = expect_str_arg(args, 1, "Stage$.Remove$")?;
            stage_target::remove_plugin(pm, &name);
            Ok(NavValue::Void)
        }

        // ── String Operations (2026-07-22) ───────────────────────────
        "StrLen$" => {
            let s = expect_str_arg(args, 0, "StrLen$")?;
            Ok(NavValue::Int(s.len() as i64))
        }
        "StrReplace$" => {
            let s = expect_str_arg(args, 0, "StrReplace$")?;
            let from = expect_str_arg(args, 1, "StrReplace$")?;
            let to = expect_str_arg(args, 2, "StrReplace$")?;
            Ok(NavValue::Str(s.replace(&from, &to)))
        }
        "StrJoin$" => {
            let list_val = eval_nav_chain(&args[0], program, universe, stage, scope, sandbox, pm)?;
            let parts: Vec<String> = match &list_val {
                NavValue::List(items) => items.iter().map(|v| nav_to_string(v)).collect(),
                NavValue::Names(names) => names.clone(),
                _ => return Err("StrJoin$: first arg must be a List or Names".into()),
            };
            let sep = expect_str_arg(args, 1, "StrJoin$")?;
            Ok(NavValue::Str(parts.join(&sep)))
        }
        "StrSplit$" => {
            let s = expect_str_arg(args, 0, "StrSplit$")?;
            let pat = expect_str_arg(args, 1, "StrSplit$")?;
            let parts: Vec<NavValue> = s.split(&pat).map(|p| NavValue::Str(p.to_string())).collect();
            Ok(NavValue::List(parts))
        }
        "StrSubstr$" => {
            let s = expect_str_arg(args, 0, "StrSubstr$")?;
            let start = expect_int_arg(args, 1, "StrSubstr$")? as usize;
            let end = expect_int_arg(args, 2, "StrSubstr$")? as usize;
            let end = end.min(s.len());
            if start > s.len() || start > end {
                return Err(format!("StrSubstr$: invalid range {}..{} for string of length {}", start, end, s.len()));
            }
            Ok(NavValue::Str(s[start..end].to_string()))
        }

        // ── File I/O (2026-07-22) ────────────────────────────────────
        "FileWrite$" => {
            sandbox.check(Capability::DiskWrite, "FileWrite$")?;
            let path = expect_str_arg(args, 0, "FileWrite$")?;
            let content = expect_str_arg(args, 1, "FileWrite$")?;
            // Third argument: `{ persist: true }` to flush to physical disk
            let persist = args.get(2).map(|a| matches!(a, Expr::Bool(true))).unwrap_or(false);

            if path.starts_with("virtual://") || !persist {
                // Default: write to in-memory virtual filesystem
                let vfs_path = path.strip_prefix("virtual://").unwrap_or(&path).to_string();
                if let Some(pm_inner) = pm.as_mut() {
                    pm_inner.vfs.insert(vfs_path, content);
                }
            } else {
                // Explicit persist: write to physical disk
                if let Some(parent) = std::path::Path::new(&path).parent() {
                    if !parent.as_os_str().is_empty() {
                        std::fs::create_dir_all(parent)
                            .map_err(|e| format!("FileWrite$: cannot create dir '{}': {}", parent.display(), e))?;
                    }
                }
                std::fs::write(&path, &content)
                    .map_err(|e| format!("FileWrite$: cannot write '{}': {}", path, e))?;
            }
            Ok(NavValue::Void)
        }
        "FileRead$" => {
            sandbox.check(Capability::DiskRead, "FileRead$")?;
            let path = expect_str_arg(args, 0, "FileRead$")?;
            if path.starts_with("virtual://") {
                let vfs_path = path.strip_prefix("virtual://").unwrap_or(&path).to_string();
                match pm.as_ref().and_then(|p| p.vfs.get(&vfs_path)) {
                    Some(content) => Ok(NavValue::Str(content.clone())),
                    None => Err(format!("FileRead$: '{}' not found in virtual filesystem", path)),
                }
            } else {
                // Check VFS first, then physical disk
                if let Some(content) = pm.as_ref().and_then(|p| p.vfs.get(&path)) {
                    return Ok(NavValue::Str(content.clone()));
                }
                let content = std::fs::read_to_string(&path)
                    .map_err(|e| format!("FileRead$: cannot read '{}': {}", path, e))?;
                Ok(NavValue::Str(content))
            }
        }

        // ── Configuration (2026-07-22) ───────────────────────────────
        "ConfigGet$" => {
            let section = expect_str_arg(args, 0, "ConfigGet$")?;
            let key = expect_str_arg(args, 1, "ConfigGet$")?;
            let targets = crate::glue::config::load_glue_config(None)
                .map_err(|e| format!("ConfigGet$: {}", e))?;
            let target = targets.get(&section)
                .ok_or_else(|| format!("ConfigGet$: no section '{}' in lib/glue.toml", section))?;
            // Resolve dotted key (e.g., "templates.fn_template" → target.templates["fn_template"])
            if let Some(dot) = key.find('.') {
                let (sub, field) = key.split_at(dot);
                let field = &field[1..];
                match sub {
                    "templates" => {
                        let val = target.templates.get(field)
                            .ok_or_else(|| format!("ConfigGet$: no template '{}'", field))?;
                        Ok(NavValue::Str(val.clone()))
                    }
                    "protocols" => {
                        let proto_key = format!("#{}", field);
                        let entry = target.protocols.get(&proto_key);
                        match entry {
                            Some(e) => Ok(NavValue::Str(format!("{}/{}", e.native, e.c_abi))),
                            None => Err(format!("ConfigGet$: no protocol '{}' in section '{}'", field, section)),
                        }
                    }
                    _ => Err(format!("ConfigGet$: unknown sub-section '{}'", sub)),
                }
            } else {
                Err("ConfigGet$: need dotted key like 'templates.fn_template'".into())
            }
        }

        // ── Universe Queries (2026-07-22) ────────────────────────────
        "DocRead$" => {
            let type_name = expect_str_arg(args, 0, "DocRead$")?;
            let prop = expect_str_arg(args, 1, "DocRead$")?;
            let rt = universe.get(&type_name)
                .ok_or_else(|| format!("DocRead$: type '{}' not in universe", type_name))?;
            match prop.as_str() {
                "properties" => {
                    let props: Vec<String> = rt.properties.keys().cloned().collect();
                    Ok(NavValue::Names(props))
                }
                "bytes" => Ok(NavValue::Int(rt.bytes as i64)),
                "fields" => {
                    let field_names: Vec<String> = rt.fields.iter().map(|(n, _)| n.clone()).collect();
                    Ok(NavValue::Names(field_names))
                }
                _ => match rt.properties.get(&prop) {
                    Some(v) => Ok(NavValue::Str(format!("{:?}", v))),
                    None => Err(format!("DocRead$: no property '{}' on type '{}'", prop, type_name)),
                }
            }
        }
        "CastPath$" => {
            let src = expect_str_arg(args, 0, "CastPath$")?;
            let tgt = expect_str_arg(args, 1, "CastPath$")?;
            let path = crate::analysis::layout_optimizer::find_cast_path(universe, &src, &tgt);
            match path {
                Some(types) => {
                    let steps: Vec<NavValue> = types.into_iter()
                        .map(|t| NavValue::Str(t)).collect();
                    Ok(NavValue::List(steps))
                }
                None => Ok(NavValue::List(vec![])),
            }
        }

        // ── Type Information (2026-07-22) ────────────────────────────
        "TypeInfo$" => {
            let sel_val = eval_nav_chain(&args[0], program, universe, stage, scope, sandbox, pm)?;
            let field = expect_str_arg(args, 1, "TypeInfo$")?;
            let tl = match &sel_val {
                NavValue::Selection(sel) => {
                    let node = sel.nodes.first()
                        .ok_or_else::<String, _>(|| "TypeInfo$: empty selection".into())?;
                    match node {
                        crate::macros::selection::NodeRef::TopLevel(i) => {
                            program.get(*i).ok_or_else::<String, _>(|| "TypeInfo$: invalid node index".into())?
                        }
                        _ => return Err("TypeInfo$: only top-level items supported".into()),
                    }
                }
                NavValue::TopLevel(tl) => tl,
                _ => return Err("TypeInfo$: first arg must be a Selection or TopLevel".into()),
            };
            let result = type_info_from_toplevel(tl, &field)?;
            Ok(NavValue::Str(result))
        }

        // ── External Commands (2026-07-22) ───────────────────────────
        "ShellCmd$" => {
            sandbox.check(Capability::Shell, "ShellCmd$")?;
            let cmd = expect_str_arg(args, 0, "ShellCmd$")?;
            let cmd_args: Vec<String> = args[1..].iter()
                .map(|a| eval_nav_chain(a, program, universe, stage, scope, sandbox, pm))
                .map(|r| r.and_then(|v| match v {
                    NavValue::Str(s) => Ok(s),
                    NavValue::Int(n) => Ok(n.to_string()),
                    _ => Err("ShellCmd$: arg must be a string or integer".into()),
                }))
                .collect::<Result<Vec<_>, _>>()?;
            let output = std::process::Command::new(&cmd)
                .args(&cmd_args)
                .output()
                .map_err(|e| format!("ShellCmd$: failed to execute '{}': {}", cmd, e))?;
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(format!("ShellCmd$: '{}' failed: {}", cmd, stderr));
            }
            Ok(NavValue::Str(String::from_utf8_lossy(&output.stdout).to_string()))
        }

        // ── Diagnostics (2026-07-23) ────────────────────────────────
        "EmitInfo$" => {
            let msg = expect_str_arg(args, 0, "EmitInfo$")?;
            println!("info: {}", msg);
            Ok(NavValue::Void)
        }
        "EmitWarning$" => {
            let msg = expect_str_arg(args, 0, "EmitWarning$")?;
            eprintln!("warning: {}", msg);
            Ok(NavValue::Void)
        }
        "EmitError$" => {
            let msg = expect_str_arg(args, 0, "EmitError$")?;
            Err(msg)
        }

        // ── System Queries (2026-07-23) ────────────────────────────────
        // SysQuery$ provides structured host hardware introspection.
        // Requires --allow-sys-query. Supports:
        //   cpu.cores           → number of logical cores
        //   cpu.arch            → CPU architecture string (x86_64, aarch64, etc.)
        //   os                  → OS type string (linux, macos, windows, etc.)
        //   os.version          → OS kernel/release version
        //   hostname            → machine hostname
        //   memory.total        → total physical RAM in bytes (Linux only)
        //   memory.free         → available RAM in bytes (Linux only)
        //   pagesize            → system page size in bytes
        "SysQuery$" => {
            sandbox.check(Capability::SysQuery, "SysQuery$")?;
            let query = expect_str_arg(args, 0, "SysQuery$")?;
            match query.as_str() {
                "cpu.cores" => {
                    let cores = std::thread::available_parallelism()
                        .map(|n| n.get() as i64)
                        .unwrap_or(1);
                    Ok(NavValue::Int(cores))
                }
                "cpu.arch" => {
                    Ok(NavValue::Str(std::env::consts::ARCH.to_string()))
                }
                "os" => {
                    Ok(NavValue::Str(std::env::consts::OS.to_string()))
                }
                "os.version" => {
                    // 2026-07-23: On Linux, read /proc/sys/kernel/osrelease.
                    // On other platforms, fall back to `uname -r` or "unknown".
                    #[cfg(target_os = "linux")]
                    {
                        let version = std::fs::read_to_string("/proc/sys/kernel/osrelease")
                            .unwrap_or_else(|_| "unknown".into());
                        return Ok(NavValue::Str(version.trim().to_string()));
                    }
                    #[cfg(not(target_os = "linux"))]
                    {
                        let version = std::process::Command::new("uname")
                            .arg("-r")
                            .output()
                            .ok()
                            .and_then(|o| {
                                if o.status.success() {
                                    String::from_utf8(o.stdout).ok()
                                } else {
                                    None
                                }
                            })
                            .unwrap_or_else(|| "unknown".into());
                        return Ok(NavValue::Str(version.trim().to_string()));
                    }
                }
                "hostname" => {
                    // 2026-07-23: On Linux, read /proc/sys/kernel/hostname.
                    // Fall back to `hostname` command on other platforms.
                    #[cfg(target_os = "linux")]
                    {
                        let hostname = std::fs::read_to_string("/proc/sys/kernel/hostname")
                            .unwrap_or_else(|_| "unknown".into());
                        return Ok(NavValue::Str(hostname.trim().to_string()));
                    }
                    #[cfg(not(target_os = "linux"))]
                    {
                        let hostname = std::process::Command::new("hostname")
                            .output()
                            .ok()
                            .and_then(|o| {
                                if o.status.success() {
                                    String::from_utf8(o.stdout).ok()
                                } else {
                                    None
                                }
                            })
                            .unwrap_or_else(|| "unknown".into());
                        return Ok(NavValue::Str(hostname.trim().to_string()));
                    }
                }
                "memory.total" | "memory.free" => {
                    // 2026-07-23: Parse /proc/meminfo on Linux for total/available RAM.
                    // Returns bytes (parsed from kB values).
                    #[cfg(target_os = "linux")]
                    {
                        let meminfo = std::fs::read_to_string("/proc/meminfo")
                            .map_err(|e| format!("SysQuery$: cannot read /proc/meminfo: {}", e))?;
                        let prefix = if query == "memory.total" { "MemTotal:" } else { "MemAvailable:" };
                        for line in meminfo.lines() {
                            if let Some(rest) = line.strip_prefix(prefix) {
                                let val = rest.trim().strip_suffix(" kB").unwrap_or(rest.trim());
                                if let Ok(kb) = val.trim().parse::<i64>() {
                                    return Ok(NavValue::Int(kb * 1024));
                                }
                            }
                        }
                        Err(format!("SysQuery$: could not parse {} from /proc/meminfo", query))
                    }
                    #[cfg(not(target_os = "linux"))]
                    {
                        Err(format!("SysQuery$: '{}' is only supported on Linux", query))
                    }
                }
                "pagesize" => {
                    // 2026-07-23: Return the system page size.
                    // Uses `getconf PAGE_SIZE` on POSIX, defaults to 4096.
                    #[cfg(not(windows))]
                    {
                        let pagesize = std::process::Command::new("getconf")
                            .arg("PAGE_SIZE")
                            .output()
                            .ok()
                            .and_then(|o| {
                                if o.status.success() {
                                    String::from_utf8(o.stdout).ok()
                                } else {
                                    None
                                }
                            })
                            .and_then(|s| s.trim().parse::<i64>().ok())
                            .unwrap_or(4096);
                        Ok(NavValue::Int(pagesize))
                    }
                    #[cfg(windows)]
                    {
                        // Windows page size is always 4096
                        Ok(NavValue::Int(4096))
                    }
                }
                _ => Err(format!("SysQuery$: unknown query '{}'", query)),
            }
        }

        // ── Timestamp (2026-07-23) ──────────────────────────────────────
        // TimeNow$ returns a deterministic timestamp for reproducibility.
        // Uses the most recent git commit timestamp if available, falling
        // back to the current system time. Pure intrinsic — no sandbox check.
        "TimeNow$" => {
            // 2026-07-23: Try git commit timestamp first (reproducible per commit),
            // then fall back to current wall-clock time.
            let ts = std::process::Command::new("git")
                .args(["log", "-1", "--format=%ct"])
                .output()
                .ok()
                .and_then(|o| {
                    if o.status.success() {
                        let s = String::from_utf8_lossy(&o.stdout);
                        s.trim().parse::<i64>().ok()
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| {
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs() as i64
                });
            Ok(NavValue::Int(ts))
        }

        // ── Environment Variables (2026-07-23) ──────────────────────────
        // EnvGet$ reads a named environment variable. Returns empty string
        // if the variable is not set. Requires --allow-sys-query.
        "EnvGet$" => {
            sandbox.check(Capability::SysQuery, "EnvGet$")?;
            let name = expect_str_arg(args, 0, "EnvGet$")?;
            match std::env::var(&name) {
                Ok(val) => Ok(NavValue::Str(val)),
                Err(_) => Ok(NavValue::Str(String::new())),
            }
        }

        // ── HTTP Fetch (2026-07-23) ─────────────────────────────────────
        // HttpFetch$ fetches a URL via HTTP GET and returns the response
        // body as a string. Requires --allow-net.
        "HttpFetch$" => {
            sandbox.check(Capability::Network, "HttpFetch$")?;
            let url = expect_str_arg(args, 0, "HttpFetch$")?;
            let resp = ureq::get(&url).call()
                .map_err(|e| format!("HttpFetch$: failed to fetch '{}': {}", url, e))?;
            let body = resp.into_string()
                .map_err(|e| format!("HttpFetch$: failed to read body from '{}': {}", url, e))?;
            Ok(NavValue::Str(body))
        }

        _ => Err(format!("unknown navigation intrinsic '{}'", name)),
    }
}

// ── Field Method Dispatch ──────────────────────────────────────────────

fn eval_nav_field_method(
    name: &str,
    prev: NavValue,
    program: &mut Vec<TopLevel>,
    stage: StageKind,
) -> Result<NavValue, String> {
    match name {
        "Count$" => match prev {
            NavValue::Selection(sel) => Ok(NavValue::Count(sel.count())),
            _ => Err("Count$ requires Selection".into()),
        },
        "IsEmpty$" => match prev {
            NavValue::Selection(sel) => Ok(NavValue::Bool(sel.is_empty())),
            _ => Err("IsEmpty$ requires Selection".into()),
        },
        "Names$" => match prev {
            NavValue::Selection(sel) => Ok(NavValue::Names(sel.names(program))),
            _ => Err("Names$ requires Selection".into()),
        },
        "Before$" => match prev {
            NavValue::Selection(ref sel) => Position::before(sel)
                .map(NavValue::Position)
                .ok_or_else(|| "Before$: empty selection".into()),
            _ => Err("Before$ requires Selection".into()),
        },
        "After$" => match prev {
            NavValue::Selection(ref sel) => Position::after(sel)
                .map(NavValue::Position)
                .ok_or_else(|| "After$: empty selection".into()),
            _ => Err("After$ requires Selection".into()),
        },
        "Replace$" => match prev {
            NavValue::Selection(ref sel) => Position::replace(sel)
                .map(NavValue::Position)
                .ok_or_else(|| "Replace$: empty selection".into()),
            _ => Err("Replace$ requires Selection".into()),
        },
        _ => Err(format!("unknown field method '{}' on navigation value", name)),
    }
}

// ── Helpers ────────────────────────────────────────────────────────────

fn expect_str_arg(args: &[Expr], idx: usize, intrinsic: &str) -> Result<String, String> {
    let arg = args.get(idx).ok_or_else(|| {
        format!("{}: missing argument {}", intrinsic, idx)
    })?;
    match arg {
        Expr::Quoted(bytes) => String::from_utf8(bytes.clone())
            .map_err(|_| format!("{}: arg {} is not valid UTF-8", intrinsic, idx)),
        Expr::Identifier(s) => Ok(s.clone()),
        Expr::Decimal(n) => Ok(n.to_string()),
        _ => Err(format!("{}: arg {} must be a string", intrinsic, idx)),
    }
}

fn expect_int_arg(args: &[Expr], idx: usize, intrinsic: &str) -> Result<i64, String> {
    let arg = args.get(idx).ok_or_else(|| {
        format!("{}: missing argument {}", intrinsic, idx)
    })?;
    match arg {
        Expr::Decimal(n) => Ok(*n),
        _ => Err(format!("{}: arg {} must be an integer", intrinsic, idx)),
    }
}

// Fix 3: Parse an Expr argument as a PropertyValue
fn expect_prop_arg(args: &[Expr], idx: usize, intrinsic: &str) -> Result<PropertyValue, String> {
    let arg = args.get(idx).ok_or_else(|| {
        format!("{}: missing argument {}", intrinsic, idx)
    })?;
    match arg {
        Expr::Bool(b) => Ok(PropertyValue::Bool(*b)),
        Expr::Decimal(n) => Ok(PropertyValue::Int(*n)),
        Expr::Quoted(bytes) => {
            let s = String::from_utf8(bytes.clone())
                .map_err(|_| format!("{}: arg {} is not valid UTF-8", intrinsic, idx))?;
            // Quoted could be a string or identifier
            Ok(PropertyValue::String(s))
        }
        Expr::Identifier(s) => Ok(PropertyValue::Identifier(s.clone())),
        _ => Err(format!("{}: arg {} must be a property value (bool, int, string)", intrinsic, idx)),
    }
}

fn extract_str_lit(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Quoted(bytes) => String::from_utf8(bytes.clone()).ok(),
        _ => None,
    }
}

fn selector_1_str<F>(args: &[Expr], f: F) -> Result<NavValue, String>
where F: FnOnce(String) -> Result<NavValue, String> {
    let s = expect_str_arg(args, 0, "?")?;
    f(s)
}

fn selector_1_int_opt<F>(args: &[Expr], f: F) -> Result<NavValue, String>
where F: FnOnce(usize) -> Result<NavValue, String> {
    let n = if args.len() > 1 {
        expect_int_arg(args, 1, "?")? as usize
    } else {
        1
    };
    f(n)
}

/// Check if the first arg of a $ call is Identifier("Stage$"),
/// indicating a Stage$.Foo$ method call rather than a navigation chain.
fn is_stage_receiver(args: &[Expr]) -> bool {
    matches!(args.first(), Some(Expr::Identifier(s)) if s == "Stage$")
}

/// Convert a NavValue to a string for StrJoin$ and similar operations.
fn nav_to_string(val: &NavValue) -> String {
    match val {
        NavValue::Str(s) => s.clone(),
        NavValue::Int(n) => n.to_string(),
        NavValue::Count(n) => n.to_string(),
        NavValue::Bool(b) => b.to_string(),
        NavValue::Names(names) => names.join(", "),
        NavValue::Selection(sel) => format!("Selection({})", sel.count()),
        _ => format!("{:?}", val),
    }
}

/// Extract type information from a TopLevel AST node by field path.
fn type_info_from_toplevel(tl: &TopLevel, field: &str) -> Result<String, String> {
    match (tl, field) {
        (TopLevel::Definition(d), "name") => Ok(d.name.clone()),
        (TopLevel::Definition(d), "params.count") => Ok(d.parameters.len().to_string()),
        (TopLevel::Definition(d), f) if f.starts_with("params.") => {
            let rest = &f[7..]; // strip "params."
            let parts: Vec<&str> = rest.split('.').collect();
            let idx: usize = parts[0].parse()
                .map_err(|_| format!("TypeInfo$: invalid param index '{}'", parts[0]))?;
            let param = d.parameters.get(idx)
                .ok_or_else(|| format!("TypeInfo$: param index {} out of bounds (max {})", idx, d.parameters.len().saturating_sub(1)))?;
            match parts.get(1) {
                Some(&"name") => Ok(param.0.clone()),
                Some(&"type") => Ok(format!("{}", param.1)),
                _ => Err(format!("TypeInfo$: unknown param field '{:?}'", parts.get(1))),
            }
        }
        (TopLevel::Definition(d), "output_type") => {
            Ok(format!("{:?}", d.output_type))
        }
        (TopLevel::Definition(d), "outputs.count") => Ok(d.outputs.len().to_string()),
        (TopLevel::ForeignBinding(fb), "name") => Ok(fb.foreign_name.clone()),
        (TopLevel::ForeignBinding(fb), "brief_name") => Ok(fb.effective_brief_name().to_string()),
        (TopLevel::Import(i), "path") => Ok(i.path().to_string()),
        _ => Err(format!("TypeInfo$: unknown field '{}' for this item type", field)),
    }
}

// ── Quote$ Helpers ──────────────────────────────────────────────────────
// 2026-07-23: AST-level quasiquoting — resolve $identifier references from
// scope, convert NavValues to Exprs, and handle $$ escaping.

/// Parse a string as a single Brief expression.
fn parse_expr_from_string(s: &str) -> Result<Expr, String> {
    let tokens = crate::lexer::tokenize(s)
        .map_err(|e| format!("cannot tokenize expression '{}': {}", s, e))?;
    let mut parser = crate::parser::Parser::new(tokens, s);
    parser.parse_expression()
        .map_err(|e| format!("cannot parse '{}' as expression: {}", s, e))
}

/// Human-readable name for a NavValue variant.
fn nav_type_name(val: &NavValue) -> &'static str {
    match val {
        NavValue::Selection(_) => "Selection",
        NavValue::Position(_) => "Position",
        NavValue::TextSelection(_) => "TextSelection",
        NavValue::Count(_) => "Count",
        NavValue::Names(_) => "Names",
        NavValue::Bool(_) => "Bool",
        NavValue::Int(_) => "Int",
        NavValue::Str(_) => "Str",
        NavValue::TopLevel(_) => "TopLevel",
        NavValue::VecTopLevel(_) => "VecTopLevel",
        NavValue::List(_) => "List",
        NavValue::Void => "Void",
    }
}

/// Convert a NavValue to an Expr for substitution into a quasiquote template.
fn nav_value_to_expr(val: &NavValue) -> Result<Expr, String> {
    match val {
        NavValue::Int(n) => Ok(Expr::Decimal(*n)),
        NavValue::Bool(b) => Ok(Expr::Bool(*b)),
        NavValue::Count(n) => Ok(Expr::Decimal(*n as i64)),
        NavValue::Str(s) => parse_expr_from_string(s),
        NavValue::Names(names) => {
            let exprs: Vec<Expr> = names.iter()
                .map(|n| Expr::Quoted(n.as_bytes().to_vec()))
                .collect();
            Ok(Expr::List(exprs))
        }
        NavValue::TopLevel(TopLevel::Statement(stmt)) => {
            match stmt.as_ref() {
                Statement::Expression(expr) => Ok(expr.clone()),
                _ => Err(format!(
                    "cannot substitute a non-expression statement '{}' as an expression",
                    stmt
                )),
            }
        }
        other => Err(format!(
            "cannot substitute {} as an expression (use Str, Int, Bool, Count, Names, or an expression statement)",
            nav_type_name(other)
        )),
    }
}

/// Recursively resolve $ident references in an Expr tree.
/// $$ident → $ident (literal, escape). $ident → scope lookup.
fn resolve_dollar_refs_in_expr(expr: &mut Expr, scope: &Scope) -> Result<(), String> {
    match expr {
        Expr::Identifier(name) => {
            // $$escape → produce literal $ident (no interpolation). The leading
            // $ is preserved but won't be re-matched by $ident because we return.
            if let Some(rest) = name.strip_prefix("$$") {
                *expr = Expr::Identifier(format!("${}", rest));
                return Ok(());
            }
            // $ident → scope lookup
            if let Some(var_name) = name.strip_prefix('$') {
                if !var_name.is_empty() {
                    if let Some(val) = scope.get(var_name) {
                        let replacement = nav_value_to_expr(val)?;
                        *expr = replacement;
                    }
                }
            }
            Ok(())
        }
        Expr::Call(_, args, _) => {
            for arg in args.iter_mut() {
                resolve_dollar_refs_in_expr(arg, scope)?;
            }
            Ok(())
        }
        Expr::BinaryOp(_, lhs, rhs) => {
            resolve_dollar_refs_in_expr(lhs, scope)?;
            resolve_dollar_refs_in_expr(rhs, scope)
        }
        Expr::UnaryOp(_, operand) => resolve_dollar_refs_in_expr(operand, scope),
        Expr::Field(obj, _) => resolve_dollar_refs_in_expr(obj, scope),
        Expr::Index(obj, index) => {
            resolve_dollar_refs_in_expr(obj, scope)?;
            resolve_dollar_refs_in_expr(index, scope)
        }
        Expr::If(cond, then, else_) => {
            resolve_dollar_refs_in_expr(cond, scope)?;
            resolve_dollar_refs_in_expr(then, scope)?;
            if let Some(el) = else_ {
                resolve_dollar_refs_in_expr(el, scope)?;
            }
            Ok(())
        }
        Expr::Block(stmts) => {
            for stmt in stmts.iter_mut() {
                resolve_dollar_refs_in_stmt(stmt, scope)?;
            }
            Ok(())
        }
        Expr::Match(scrutinee, arms) => {
            resolve_dollar_refs_in_expr(scrutinee, scope)?;
            for arm in arms.iter_mut() {
                if let Some(ref mut g) = arm.guard {
                    resolve_dollar_refs_in_expr(g, scope)?;
                }
                resolve_dollar_refs_in_expr(&mut arm.body, scope)?;
            }
            Ok(())
        }
        Expr::Tuple(items) | Expr::List(items) => {
            for item in items.iter_mut() {
                resolve_dollar_refs_in_expr(item, scope)?;
            }
            Ok(())
        }
        Expr::Lambda(_, body) => resolve_dollar_refs_in_expr(body, scope),
        Expr::Cast(inner, _)
        | Expr::Within(inner, _)
        | Expr::IsType(inner, _)
        | Expr::Deref(inner)
        | Expr::AddrOf(inner) => resolve_dollar_refs_in_expr(inner, scope),
        Expr::PluginIntercept { args: pargs, .. } => {
            for arg in pargs.iter_mut() {
                resolve_dollar_refs_in_expr(arg, scope)?;
            }
            Ok(())
        }
        Expr::DerivationBlock(db) => {
            if let Some(ref mut syn) = db.synthesized {
                resolve_dollar_refs_in_expr(syn, scope)?;
            }
            Ok(())
        }
        // Literals and simple values — no nested identifiers
        Expr::Quoted(_) | Expr::Decimal(_) | Expr::Float(_) | Expr::Bool(_)
        | Expr::TaggedLiteral(_, _) | Expr::PropertyGet(_)
        | Expr::FormattingAnnotation(_) => Ok(()),
    }
}

/// Recursively resolve $ident references in a Statement tree.
fn resolve_dollar_refs_in_stmt(stmt: &mut Statement, scope: &Scope) -> Result<(), String> {
    match stmt {
        Statement::Let { expr, .. } => {
            if let Some(e) = expr {
                resolve_dollar_refs_in_expr(e, scope)?;
            }
            Ok(())
        }
        Statement::Assign(target, value) => {
            resolve_dollar_refs_in_expr(target, scope)?;
            resolve_dollar_refs_in_expr(value, scope)
        }
        Statement::Term(expr) | Statement::TermBang(expr)
        | Statement::Return(expr) | Statement::Escape(expr) => {
            if let Some(e) = expr {
                resolve_dollar_refs_in_expr(e, scope)?;
            }
            Ok(())
        }
        Statement::Expression(expr) => resolve_dollar_refs_in_expr(expr, scope),
        Statement::If(cond, then, else_) => {
            resolve_dollar_refs_in_expr(cond, scope)?;
            for s in then.iter_mut() {
                resolve_dollar_refs_in_stmt(s, scope)?;
            }
            for s in else_.iter_mut() {
                resolve_dollar_refs_in_stmt(s, scope)?;
            }
            Ok(())
        }
        Statement::Guarded(guard, body) => {
            resolve_dollar_refs_in_expr(guard, scope)?;
            for s in body.iter_mut() {
                resolve_dollar_refs_in_stmt(s, scope)?;
            }
            Ok(())
        }
        Statement::Block(stmts) | Statement::SyncBlock(stmts) => {
            for s in stmts.iter_mut() {
                resolve_dollar_refs_in_stmt(s, scope)?;
            }
            Ok(())
        }
        Statement::Foreach { list, body, .. } => {
            resolve_dollar_refs_in_expr(list, scope)?;
            for s in body.iter_mut() {
                resolve_dollar_refs_in_stmt(s, scope)?;
            }
            Ok(())
        }
        Statement::TrgBinding { instance, .. } => {
            resolve_dollar_refs_in_expr(instance, scope)
        }
        Statement::InlineAsm { .. } | Statement::MetadataAssignment(..) => Ok(()),
    }
}

/// Recursively resolve $ident references in a Type tree (minimal — handles
/// Custom name lookup from Str scope values).
fn resolve_dollar_refs_in_type(ty: &mut Type, scope: &Scope) -> Result<(), String> {
    match ty {
        Type::Custom(name) => {
            if let Some(var_name) = name.strip_prefix('$') {
                if let Some(val) = scope.get(var_name) {
                    match val {
                        NavValue::Str(s) => {
                            *ty = Type::Custom(s.clone());
                        }
                        _ => return Err(format!(
                            "cannot substitute {} as a type name (use a Str scope variable)",
                            nav_type_name(val)
                        )),
                    }
                }
            }
            Ok(())
        }
        Type::Generic(_, args) | Type::Applied(_, args)
        | Type::Tuple(args) | Type::Union(args) => {
            for a in args.iter_mut() {
                resolve_dollar_refs_in_type(a, scope)?;
            }
            Ok(())
        }
        Type::Ptr(inner) | Type::PtrConst(inner) => {
            resolve_dollar_refs_in_type(inner, scope)
        }
        Type::Vector(inner, _) | Type::Constrained(inner, _) => {
            resolve_dollar_refs_in_type(inner, scope)
        }
        Type::Void | Type::Bits(_) | Type::Width(_)
        | Type::TypeVar(_) | Type::HashWord(_) | Type::HashWordVariant(_, _)
        | Type::LayoutPtr(_) | Type::Function(_, _) => Ok(()),
    }
}

/// Recursively resolve $ident references in a TopLevel AST node.
/// Handles common TopLevel variants; less common ones (Meld, Trigger, etc.)
/// are skipped conservatively.
fn resolve_dollar_refs_in_toplevel(tl: &mut TopLevel, scope: &Scope) -> Result<(), String> {
    match tl {
        TopLevel::Statement(stmt) => resolve_dollar_refs_in_stmt(stmt, scope),
        TopLevel::Definition(def) => {
            for stmt in &mut def.body {
                resolve_dollar_refs_in_stmt(stmt, scope)?;
            }
            Ok(())
        }
        TopLevel::Transaction(txn) => {
            for stmt in &mut txn.body {
                resolve_dollar_refs_in_stmt(stmt, scope)?;
            }
            Ok(())
        }
        TopLevel::StageBlock(sb) => {
            for stmt in &mut sb.body {
                resolve_dollar_refs_in_stmt(stmt, scope)?;
            }
            Ok(())
        }
        TopLevel::ForeignBinding(fb) => {
            for (_, ty) in &mut fb.inputs {
                resolve_dollar_refs_in_type(ty, scope)?;
            }
            for (_, ty) in &mut fb.success_output {
                resolve_dollar_refs_in_type(ty, scope)?;
            }
            Ok(())
        }
        TopLevel::Struct(s) => {
            for field in &mut s.fields {
                resolve_dollar_refs_in_type(&mut field.ty, scope)?;
            }
            Ok(())
        }
        TopLevel::Enum(e) => {
            for variant in &mut e.variants {
                match variant {
                    crate::ast::top::EnumVariant::Unit(_) => {}
                    crate::ast::top::EnumVariant::Tuple(_, types) => {
                        for ty in types.iter_mut() {
                            resolve_dollar_refs_in_type(ty, scope)?;
                        }
                    }
                    crate::ast::top::EnumVariant::Struct(_, fields) => {
                        for (_, ty) in fields.iter_mut() {
                            resolve_dollar_refs_in_type(ty, scope)?;
                        }
                    }
                }
            }
            Ok(())
        }
        TopLevel::Constant(c) => {
            resolve_dollar_refs_in_expr(&mut c.expr, scope)?;
            Ok(())
        }
        TopLevel::Import(_) | TopLevel::Export(_) | TopLevel::Cell(_)
        | TopLevel::Meld(_) | TopLevel::Trigger(_) | TopLevel::Signature(_)
        | TopLevel::StateDecl(_) | TopLevel::TriggerBinding { .. }
        | TopLevel::LinkDependency(_) | TopLevel::ResourceDecl(_)
        | TopLevel::RStruct(_) | TopLevel::TypeDef(_) | TopLevel::Codec(_)
        | TopLevel::Assertion { .. } | TopLevel::Fuzzed { .. }
        | TopLevel::RenderBlock(_) | TopLevel::Stylesheet(_)
        | TopLevel::SvgComponent { .. } | TopLevel::SyncGroup { .. }
        | TopLevel::Cfg(_) => Ok(()),
    }
}

/// Restore $$ → $ in all identifiers within a TopLevel AST node.
/// Called after parse-time sentinel replacement to handle identifiers
/// that contain the sentinel byte.
fn restore_double_dollar_in_toplevel(tl: &mut TopLevel) {
    // For the initial implementation, this is a no-op at the AST level
    // because $$ is not a valid identifier start in Brief's lexer —
    // it would be lexed as two separate tokens: $ and $identifier.
    // The sentinel replacement at the string level handles this correctly.
    // This function is a hook for future $$-in-identifier support.
    let _ = tl;
}

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_scope() -> Scope { Scope::new() }

    fn test_sandbox() -> Sandbox { Sandbox::permissive() }

    #[test]
    fn test_tag_selector_via_call() {
        let mut program = vec![
            TopLevel::Import(Import::literal("std/io.bv", vec![])),
        ];
        let mut universe = TypeUniverse::new();
        let expr = Expr::Call("Tag$".into(), vec![Expr::Quoted("import".into())], None);
        let result = eval_nav_chain(&expr, &mut program, &mut universe,
            StageKind::Parsed, &empty_scope(), &mut test_sandbox(), &mut None).unwrap();
        match result { NavValue::Selection(sel) => assert_eq!(sel.count(), 1), _ => panic!() }
    }

    #[test]
    fn test_count_via_call() {
        let mut program = vec![
            TopLevel::Import(Import::literal("std/io.bv", vec![])),
            TopLevel::Import(Import::literal("std/net.bv", vec![])),
        ];
        let mut universe = TypeUniverse::new();
        let inner = Expr::Call("Tag$".into(), vec![Expr::Quoted("import".into())], None);
        let expr = Expr::Call("Count$".into(), vec![inner], None);
        let result = eval_nav_chain(&expr, &mut program, &mut universe,
            StageKind::Parsed, &empty_scope(), &mut test_sandbox(), &mut None).unwrap();
        match result { NavValue::Count(n) => assert_eq!(n, 2), _ => panic!() }
    }

    #[test]
    fn test_insert_before_first_import() {
        let mut program = vec![
            TopLevel::Import(Import::literal("std/io.bv", vec![])),
            TopLevel::Definition(Definition {
                name: "main".into(), type_params: vec![], parameters: vec![],
                output_type: None, outputs: vec![Type::Custom("Int".into())],
                contract: Contract::new(Expr::Bool(true), Expr::Bool(true)),
                body: vec![], metadata: Default::default(),
                derivation: None, modifiers: vec![], annotations: vec![], span: None,
            }),
        ];
        let mut universe = TypeUniverse::new();
        let import_call = Expr::Call("Import$".into(), vec![Expr::Quoted("std/prelude.bv".into())], None);
        let tag_call = Expr::Call("Tag$".into(), vec![Expr::Quoted("import".into())], None);
        let first_call = Expr::Call("First$".into(), vec![tag_call], None);
        let before_call = Expr::Call("Before$".into(), vec![first_call], None);
        let insert_call = Expr::Call("Insert$".into(), vec![before_call, import_call], None);
        let result = eval_nav_chain(&insert_call, &mut program, &mut universe,
            StageKind::Parsed, &empty_scope(), &mut test_sandbox(), &mut None).unwrap();
        assert!(matches!(result, NavValue::Void));
        assert_eq!(program.len(), 3);
        match &program[0] {
            TopLevel::Import(i) => assert_eq!(i.path(), "std/prelude.bv"),
            other => panic!("expected Import, got {:?}", other),
        }
    }

    #[test]
    fn test_delete_selection() {
        let mut program = vec![TopLevel::Import(Import::literal("std/io.bv", vec![]))];
        let mut universe = TypeUniverse::new();
        let tag_call = Expr::Call("Tag$".into(), vec![Expr::Quoted("import".into())], None);
        let delete_call = Expr::Call("Delete$".into(), vec![tag_call], None);
        let result = eval_nav_chain(&delete_call, &mut program, &mut universe,
            StageKind::Parsed, &empty_scope(), &mut test_sandbox(), &mut None).unwrap();
        assert!(matches!(result, NavValue::Void));
        assert!(program.is_empty());
    }

    // Fix 1: Test ForEach loop variable binding
    #[test]
    fn test_foreach_binds_variable() {
        let mut program = vec![
            TopLevel::Import(Import::literal("std/a.bv", vec![])),
            TopLevel::Import(Import::literal("std/b.bv", vec![])),
        ];
        let mut universe = TypeUniverse::new();
        let foreach_stmt = Statement::Foreach {
            item: "imp".into(),
            list: Box::new(Expr::Call("Tag$".into(), vec![Expr::Quoted("import".into())], None)),
            body: vec![
                Statement::Expression(Expr::Call("Count$".into(), vec![
                    Expr::Identifier("imp".into()),
                ], None)),
            ],
        };
        let body = vec![foreach_stmt];
        let result = evaluate_stage_block(&body, &mut program, &mut universe,
            StageKind::Parsed, &mut None);
        assert!(result.is_ok());
    }

    // Fix 3: Test Set$ with bool value
    #[test]
    fn test_set_with_bool_value() {
        let mut program = vec![
            TopLevel::Definition(Definition {
                name: "main".into(), type_params: vec![], parameters: vec![],
                output_type: None, outputs: vec![Type::Custom("Int".into())],
                contract: Contract::new(Expr::Bool(true), Expr::Bool(true)),
                body: vec![], metadata: Default::default(),
                derivation: None, modifiers: vec![], annotations: vec![], span: None,
            }),
        ];
        let mut universe = TypeUniverse::new();
        // Tag$("defn").Set$("entry", true)
        let tag_call = Expr::Call("Tag$".into(), vec![Expr::Quoted("defn".into())], None);
        let set_call = Expr::Call("Set$".into(), vec![
            tag_call,
            Expr::Quoted("entry".into()),
            Expr::Bool(true),
        ], None);
        let result = eval_nav_chain(&set_call, &mut program, &mut universe,
            StageKind::Parsed, &empty_scope(), &mut test_sandbox(), &mut None).unwrap();
        assert!(matches!(result, NavValue::Void));
        if let TopLevel::Definition(d) = &program[0] {
            assert_eq!(d.metadata.get("entry"), Some(&PropertyValue::Bool(true)));
        } else {
            panic!("expected Definition");
        }
    }

    // ── SysQuery$ Tests (2026-07-23) ──────────────────────────────

    #[test]
    fn test_sys_query_cpu_cores() {
        let mut program = vec![];
        let mut universe = TypeUniverse::new();
        let expr = Expr::Call("SysQuery$".into(), vec![Expr::Quoted("cpu.cores".into())], None);
        let result = eval_nav_chain(&expr, &mut program, &mut universe,
            StageKind::Parsed, &empty_scope(), &mut test_sandbox(), &mut None).unwrap();
        match result {
            NavValue::Int(n) => assert!(n >= 1, "cpu.cores should be >= 1, got {}", n),
            other => panic!("expected Int, got {:?}", other),
        }
    }

    #[test]
    fn test_sys_query_cpu_arch() {
        let mut program = vec![];
        let mut universe = TypeUniverse::new();
        let expr = Expr::Call("SysQuery$".into(), vec![Expr::Quoted("cpu.arch".into())], None);
        let result = eval_nav_chain(&expr, &mut program, &mut universe,
            StageKind::Parsed, &empty_scope(), &mut test_sandbox(), &mut None).unwrap();
        match result {
            NavValue::Str(s) => assert!(!s.is_empty(), "cpu.arch should not be empty"),
            other => panic!("expected Str, got {:?}", other),
        }
    }

    #[test]
    fn test_sys_query_os() {
        let mut program = vec![];
        let mut universe = TypeUniverse::new();
        let expr = Expr::Call("SysQuery$".into(), vec![Expr::Quoted("os".into())], None);
        let result = eval_nav_chain(&expr, &mut program, &mut universe,
            StageKind::Parsed, &empty_scope(), &mut test_sandbox(), &mut None).unwrap();
        match result {
            NavValue::Str(s) => assert!(!s.is_empty(), "os should not be empty"),
            other => panic!("expected Str, got {:?}", other),
        }
    }

    #[test]
    fn test_sys_query_hostname() {
        let mut program = vec![];
        let mut universe = TypeUniverse::new();
        let expr = Expr::Call("SysQuery$".into(), vec![Expr::Quoted("hostname".into())], None);
        let result = eval_nav_chain(&expr, &mut program, &mut universe,
            StageKind::Parsed, &empty_scope(), &mut test_sandbox(), &mut None).unwrap();
        match result {
            NavValue::Str(s) => assert!(!s.is_empty(), "hostname should not be empty"),
            other => panic!("expected Str, got {:?}", other),
        }
    }

    #[test]
    fn test_sys_query_unknown_query() {
        let mut program = vec![];
        let mut universe = TypeUniverse::new();
        let expr = Expr::Call("SysQuery$".into(), vec![Expr::Quoted("nonexistent".into())], None);
        let result = eval_nav_chain(&expr, &mut program, &mut universe,
            StageKind::Parsed, &empty_scope(), &mut test_sandbox(), &mut None);
        assert!(result.is_err(), "unknown query should error");
    }

    #[test]
    fn test_sys_query_rejects_without_capability() {
        let mut program = vec![];
        let mut universe = TypeUniverse::new();
        let mut restricted = Sandbox::default(); // all false
        let expr = Expr::Call("SysQuery$".into(), vec![Expr::Quoted("cpu.cores".into())], None);
        let result = eval_nav_chain(&expr, &mut program, &mut universe,
            StageKind::Parsed, &empty_scope(), &mut restricted, &mut None);
        assert!(result.is_err(), "SysQuery$ should error without --allow-sys-query");
        let err = result.unwrap_err();
        assert!(err.contains("SysQuery"), "error should mention capability, got: {}", err);
    }

    // ── TimeNow$ Tests (2026-07-23) ──────────────────────────────

    #[test]
    fn test_time_now_returns_positive() {
        let mut program = vec![];
        let mut universe = TypeUniverse::new();
        let expr = Expr::Call("TimeNow$".into(), vec![], None);
        let result = eval_nav_chain(&expr, &mut program, &mut universe,
            StageKind::Parsed, &empty_scope(), &mut test_sandbox(), &mut None).unwrap();
        match result {
            NavValue::Int(ts) => {
                // Should be a reasonable Unix timestamp (> 2020-01-01 = 1577836800)
                assert!(ts > 1577836800, "TimeNow$: timestamp {} seems too old", ts);
            }
            other => panic!("expected Int, got {:?}", other),
        }
    }

    #[test]
    fn test_time_now_no_sandbox_check_needed() {
        // TimeNow$ is Pure — should work even with a restricted sandbox
        let mut program = vec![];
        let mut universe = TypeUniverse::new();
        let mut restricted = Sandbox::default(); // all false
        let expr = Expr::Call("TimeNow$".into(), vec![], None);
        let result = eval_nav_chain(&expr, &mut program, &mut universe,
            StageKind::Parsed, &empty_scope(), &mut restricted, &mut None);
        assert!(result.is_ok(), "TimeNow$ should work without any capabilities");
    }

    // ── EnvGet$ Tests (2026-07-23) ───────────────────────────────

    #[test]
    fn test_env_get_var() {
        let mut program = vec![];
        let mut universe = TypeUniverse::new();
        // Set a test env var
        // SAFETY: single-threaded test environment — no concurrent reads
        unsafe { std::env::set_var("BRIEF_TEST_VAR", "hello_world"); }
        let expr = Expr::Call("EnvGet$".into(), vec![Expr::Quoted("BRIEF_TEST_VAR".into())], None);
        let result = eval_nav_chain(&expr, &mut program, &mut universe,
            StageKind::Parsed, &empty_scope(), &mut test_sandbox(), &mut None).unwrap();
        match result {
            NavValue::Str(s) => assert_eq!(s, "hello_world"),
            other => panic!("expected Str, got {:?}", other),
        }
    }

    #[test]
    fn test_env_get_missing_var() {
        let mut program = vec![];
        let mut universe = TypeUniverse::new();
        let expr = Expr::Call("EnvGet$".into(), vec![Expr::Quoted("BRIEF_VAR_THAT_DOES_NOT_EXIST_XYZ".into())], None);
        let result = eval_nav_chain(&expr, &mut program, &mut universe,
            StageKind::Parsed, &empty_scope(), &mut test_sandbox(), &mut None).unwrap();
        match result {
            NavValue::Str(s) => assert!(s.is_empty(), "missing env var should return empty string"),
            other => panic!("expected Str, got {:?}", other),
        }
    }

    #[test]
    fn test_env_get_rejects_without_capability() {
        let mut program = vec![];
        let mut universe = TypeUniverse::new();
        let mut restricted = Sandbox::default(); // all false
        let expr = Expr::Call("EnvGet$".into(), vec![Expr::Quoted("HOME".into())], None);
        let result = eval_nav_chain(&expr, &mut program, &mut universe,
            StageKind::Parsed, &empty_scope(), &mut restricted, &mut None);
        assert!(result.is_err(), "EnvGet$ should error without --allow-sys-query");
        let err = result.unwrap_err();
        assert!(err.contains("SysQuery"), "error should mention capability, got: {}", err);
    }

    // ── HttpFetch$ Tests (2026-07-23) ────────────────────────────

    #[test]
    fn test_http_fetch_rejects_without_capability() {
        let mut program = vec![];
        let mut universe = TypeUniverse::new();
        let mut restricted = Sandbox::default(); // all false
        let expr = Expr::Call("HttpFetch$".into(), vec![Expr::Quoted("http://example.com".into())], None);
        let result = eval_nav_chain(&expr, &mut program, &mut universe,
            StageKind::Parsed, &empty_scope(), &mut restricted, &mut None);
        assert!(result.is_err(), "HttpFetch$ should error without --allow-net");
        let err = result.unwrap_err();
        assert!(err.contains("Network"), "error should mention Network capability, got: {}", err);
    }

    // ── Quote$ Tests (2026-07-23) ───────────────────────────────────

    fn scope_with(pairs: Vec<(&str, NavValue)>) -> Scope {
        let mut s = Scope::new();
        for (k, v) in pairs {
            s.insert(k.to_string(), v);
        }
        s
    }

    fn eval_quote(template: &str, scope: &Scope) -> Result<NavValue, String> {
        let mut program = vec![];
        let mut universe = TypeUniverse::new();
        let expr = Expr::Call("Quote$".into(), vec![Expr::Quoted(template.as_bytes().to_vec())], None);
        eval_nav_chain(&expr, &mut program, &mut universe,
            StageKind::Parsed, &scope, &mut test_sandbox(), &mut None)
    }

    #[test]
    fn test_quote_basic_int_interpolation() {
        let scope = scope_with(vec![("val", NavValue::Int(42))]);
        let result = eval_quote("let x = $val;", &scope).unwrap();
        match result {
            NavValue::TopLevel(TopLevel::Statement(stmt)) => {
                match stmt.as_ref() {
                    Statement::Let { name, expr: Some(e), .. } => {
                        assert_eq!(name, "x");
                        assert!(matches!(e, Expr::Decimal(42)), "expected Decimal(42), got {:?}", e);
                    }
                    other => panic!("expected Let statement, got {:?}", other),
                }
            }
            other => panic!("expected TopLevel, got {:?}", other),
        }
    }

    #[test]
    fn test_quote_str_as_expression() {
        let scope = scope_with(vec![("expr", NavValue::Str("a + b".into()))]);
        let result = eval_quote("let x = $expr;", &scope).unwrap();
        match result {
            NavValue::TopLevel(TopLevel::Statement(stmt)) => {
                match stmt.as_ref() {
                    Statement::Let { expr: Some(e), .. } => {
                        // Should parse "a + b" as BinaryOp(Add, Identifier("a"), Identifier("b"))
                        match e {
                            Expr::BinaryOp(kind, _, _) => {
                                assert_eq!(*kind, crate::ast::BinaryOpKind::Add,
                                    "expected Add, got {:?}", kind);
                            }
                            other => panic!("expected BinaryOp, got {:?}", other),
                        }
                    }
                    other => panic!("expected Let with expr, got {:?}", other),
                }
            }
            other => panic!("expected TopLevel, got {:?}", other),
        }
    }

    #[test]
    fn test_quote_bool_interpolation() {
        let scope = scope_with(vec![("flag", NavValue::Bool(true))]);
        let result = eval_quote("let x = $flag;", &scope).unwrap();
        match result {
            NavValue::TopLevel(TopLevel::Statement(stmt)) => {
                match stmt.as_ref() {
                    Statement::Let { expr: Some(Expr::Bool(b)), .. } => {
                        assert!(b);
                    }
                    other => panic!("expected Let with Bool(true), got {:?}", other),
                }
            }
            other => panic!("expected TopLevel, got {:?}", other),
        }
    }

    #[test]
    fn test_quote_multiple_variables() {
        // Use $name in expression position (not the LET name field, which is a String)
        let scope = scope_with(vec![
            ("name", NavValue::Str("counter".into())),
            ("val", NavValue::Int(0)),
        ]);
        let result = eval_quote("let x = $name; let y = $val;", &scope).unwrap();
        match result {
            NavValue::VecTopLevel(items) => {
                assert_eq!(items.len(), 2);
                if let TopLevel::Statement(stmt) = &items[0] {
                    match stmt.as_ref() {
                        Statement::Let { expr: Some(Expr::Identifier(s)), .. } => {
                            assert_eq!(s, "counter", "$name should resolve to 'counter'");
                        }
                        other => panic!("expected Let with Identifier(counter), got {:?}", other),
                    }
                } else {
                    panic!("expected first item to be a Statement");
                }
                if let TopLevel::Statement(stmt) = &items[1] {
                    match stmt.as_ref() {
                        Statement::Let { expr: Some(Expr::Decimal(0)), .. } => {
                            // OK
                        }
                        other => panic!("expected second Let = Decimal(0), got {:?}", other),
                    }
                } else {
                    panic!("expected second item to be a Statement");
                }
            }
            other => panic!("expected VecTopLevel, got {:?}", other),
        }
    }

    #[test]
    fn test_quote_double_dollar_escape() {
        // $$myvar inside a let expression should produce literal $myvar
        let scope = scope_with(vec![("myvar", NavValue::Str("secret".into()))]);
        let result = eval_quote("let x = $$myvar;", &scope).unwrap();
        match result {
            NavValue::TopLevel(TopLevel::Statement(stmt)) => {
                match stmt.as_ref() {
                    Statement::Let { expr: Some(Expr::Identifier(name)), .. } => {
                        assert_eq!(name, "$myvar", "$$ should escape to literal $");
                    }
                    other => panic!("expected Let with Identifier($myvar), got {:?}", other),
                }
            }
            other => panic!("expected TopLevel, got {:?}", other),
        }
    }

    #[test]
    fn test_quote_ident_not_in_scope() {
        // $unknown not in scope — should be left as-is in expression position
        let scope = empty_scope();
        let result = eval_quote("let x = $unknown;", &scope).unwrap();
        match result {
            NavValue::TopLevel(TopLevel::Statement(stmt)) => {
                match stmt.as_ref() {
                    Statement::Let { expr: Some(Expr::Identifier(name)), .. } => {
                        assert_eq!(name, "$unknown");
                    }
                    other => panic!("expected Let with Identifier($unknown), got {:?}", other),
                }
            }
            other => panic!("expected TopLevel, got {:?}", other),
        }
    }

    #[test]
    fn test_quote_no_substitution() {
        let scope = empty_scope();
        let result = eval_quote("let x = 42;", &scope).unwrap();
        match result {
            NavValue::TopLevel(TopLevel::Statement(stmt)) => {
                match stmt.as_ref() {
                    Statement::Let { name, expr: Some(Expr::Decimal(42)), .. } => {
                        assert_eq!(name, "x");
                    }
                    other => panic!("expected Let x = Decimal(42), got {:?}", other),
                }
            }
            other => panic!("expected TopLevel, got {:?}", other),
        }
    }

    #[test]
    fn test_quote_multiple_top_level_items() {
        let scope = scope_with(vec![("a", NavValue::Int(1)), ("b", NavValue::Int(2))]);
        let result = eval_quote("let x = $a; let y = $b;", &scope).unwrap();
        match result {
            NavValue::VecTopLevel(items) => {
                assert_eq!(items.len(), 2);
                if let TopLevel::Statement(stmt) = &items[0] {
                    match stmt.as_ref() {
                        Statement::Let { name, expr: Some(Expr::Decimal(1)), .. } => {
                            assert_eq!(name, "x");
                        }
                        other => panic!("expected first Let x = 1, got {:?}", other),
                    }
                } else {
                    panic!("expected first item to be a Statement");
                }
                if let TopLevel::Statement(stmt) = &items[1] {
                    match stmt.as_ref() {
                        Statement::Let { name, expr: Some(Expr::Decimal(2)), .. } => {
                            assert_eq!(name, "y");
                        }
                        other => panic!("expected second Let y = 2, got {:?}", other),
                    }
                } else {
                    panic!("expected second item to be a Statement");
                }
            }
            other => panic!("expected VecTopLevel, got {:?}", other),
        }
    }

    #[test]
    fn test_quote_nested_in_call() {
        let scope = scope_with(vec![
            ("a", NavValue::Str("x".into())),
            ("b", NavValue::Int(42)),
        ]);
        let result = eval_quote("let result = foo($a, $b);", &scope).unwrap();
        match result {
            NavValue::TopLevel(TopLevel::Statement(stmt)) => {
                match stmt.as_ref() {
                    Statement::Let { expr: Some(Expr::Call(name, args, _)), .. } => {
                        assert_eq!(name, "foo");
                        assert_eq!(args.len(), 2);
                        assert!(matches!(&args[0], Expr::Identifier(s) if s == "x"));
                        assert!(matches!(&args[1], Expr::Decimal(42)));
                    }
                    other => panic!("expected Let with Call, got {:?}", other),
                }
            }
            other => panic!("expected TopLevel, got {:?}", other),
        }
    }

    #[test]
    fn test_quote_empty_template() {
        let scope = empty_scope();
        let result = eval_quote("", &scope).unwrap();
        match result {
            NavValue::VecTopLevel(items) => {
                assert!(items.is_empty(), "empty template should produce empty VecTopLevel");
            }
            NavValue::TopLevel(_) => panic!("empty template should produce VecTopLevel, not TopLevel"),
            other => panic!("expected VecTopLevel, got {:?}", other),
        }
    }

    #[test]
    fn test_quote_template_with_block() {
        let scope = scope_with(vec![("val", NavValue::Int(99))]);
        let result = eval_quote("defn main() { let x = $val; }", &scope).unwrap();
        match result {
            NavValue::TopLevel(TopLevel::Definition(def)) => {
                assert_eq!(def.name, "main");
                assert_eq!(def.body.len(), 1);
                match &def.body[0] {
                    Statement::Let { name, expr: Some(Expr::Decimal(99)), .. } => {
                        assert_eq!(name, "x");
                    }
                    other => panic!("expected Let x = 99 in defn body, got {:?}", other),
                }
            }
            other => panic!("expected TopLevel::Definition, got {:?}", other),
        }
    }

    #[test]
    fn test_quote_pure_intrinsic_no_sandbox_check() {
        // Quote$ is pure — should work without any capabilities
        let scope = scope_with(vec![("val", NavValue::Int(7))]);
        let mut program = vec![];
        let mut universe = TypeUniverse::new();
        let mut restricted = Sandbox::default(); // all false
        let expr = Expr::Call("Quote$".into(), vec![Expr::Quoted("let x = $val;".as_bytes().to_vec())], None);
        let result = eval_nav_chain(&expr, &mut program, &mut universe,
            StageKind::Parsed, &scope, &mut restricted, &mut None);
        assert!(result.is_ok(), "Quote$ should work without any capabilities");
    }
}
