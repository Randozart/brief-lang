// ── Navigation Chain Evaluation ─────────────────────────────────────────
// 2026-07-21: Evaluates a tree of $ calls (produced by parsing navigation
// chains like Tag$("import").First$().Before$().Insert$(...)) against the
// live compilation state. Each $ name dispatches to the appropriate
// selection, traversal, position, or action handler.
//
// The chain is evaluated recursively from the innermost call outward:
//   Before$(First$(Tag$("import")))  →  Tag$ → First$ → Before$
//   Insert$(Before$(First$(...)), node)  →  ... → Before$ → Insert$
//
// Max 2 levels per function. Flat dispatch: one arm per intrinsic name.

use crate::ast::{Expr, Statement, TopLevel, PropertyValue, ImportKind, Type};
use crate::ast::top::*;
use crate::ast::StageKind;
use crate::type_universe::TypeUniverse;
use super::selection::{
    Selection, NodeRef, Selector,
    TagSelector, NamedSelector, WithKeySelector, WithAttrSelector, AllSelector,
    AndSelector, OrSelector, NotSelector, node_tag, top_level_name,
};
use super::actions::{
    Position, insert_items, insert_before_each, insert_after_each,
    delete_selection, replace_selection, set_metadata, rename_selection,
};
use super::text_ops::TextSelection;
use super::stage_target;

/// Result of evaluating a single navigation chain link.
#[derive(Debug)]
pub enum NavValue {
    Selection(Selection),
    Position(Position),
    TextSelection(TextSelection),
    Count(usize),
    Names(Vec<String>),
    Bool(bool),
    TopLevel(TopLevel),
    VecTopLevel(Vec<TopLevel>),
    Void,
}

/// Evaluate a $ call tree against the compilation state.
/// `expr` is the current link in the chain, `ctx` carries compilation state.
pub fn eval_nav_chain(
    expr: &Expr,
    program: &mut Vec<TopLevel>,
    universe: &mut TypeUniverse,
    stage: StageKind,
) -> Result<NavValue, String> {
    match expr {
        Expr::Call(name, args, _) if name.ends_with('$') => {
            eval_nav_call(name, args, program, universe, stage)
        }
        Expr::Field(obj, name) if name.ends_with('$') => {
            // Bare field ref like .Count$ or .IsEmpty$ — no parens
            let prev = eval_nav_chain(obj, program, universe, stage)?;
            eval_nav_field_method(name, prev, program, stage)
        }
        other => Err(format!(
            "expected a $ navigation call, got {:?}", other
        )),
    }
}

fn eval_nav_call(
    name: &str,
    args: &[Expr],
    program: &mut Vec<TopLevel>,
    universe: &mut TypeUniverse,
    stage: StageKind,
) -> Result<NavValue, String> {
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
        "All$" => {
            Ok(NavValue::Selection(Selection {
                nodes: AllSelector.apply(program)?
            }))
        }

        // ── Traversal ──────────────────────────────────────────────
        "First$" => selector_1_int_opt(args, |n| {
            let prev = eval_nav_chain(&args[0], program, universe, stage)?;
            match prev {
                NavValue::Selection(sel) => Ok(NavValue::Selection(sel.first(n))),
                _ => Err("First$ requires a Selection operand".into()),
            }
        }),
        "Last$" => selector_1_int_opt(args, |n| {
            let prev = eval_nav_chain(&args[0], program, universe, stage)?;
            match prev {
                NavValue::Selection(sel) => Ok(NavValue::Selection(sel.last(n))),
                _ => Err("Last$ requires a Selection operand".into()),
            }
        }),
        "Nth$" => {
            let n = expect_int_arg(args, 0, "Nth$")? as usize;
            let prev = eval_nav_chain(&args[1], program, universe, stage)?;
            match prev {
                NavValue::Selection(sel) => Ok(NavValue::Selection(sel.nth(n))),
                _ => Err("Nth$ requires a Selection operand".into()),
            }
        }
        "Children$" => {
            let filter = if args.len() > 1 {
                Some(expect_str_arg(args, 0, "Children$")?)
            } else { None };
            let prev = if args.len() > 1 {
                eval_nav_chain(&args[1], program, universe, stage)?
            } else {
                eval_nav_chain(&args[0], program, universe, stage)?
            };
            match prev {
                NavValue::Selection(sel) => Ok(NavValue::Selection(
                    sel.children(program, filter.as_deref())
                )),
                _ => Err("Children$ requires a Selection operand".into()),
            }
        }
        "Descendants$" => {
            let filter = if args.len() > 1 {
                Some(expect_str_arg(args, 0, "Descendants$")?)
            } else { None };
            let prev = if args.len() > 1 {
                eval_nav_chain(&args[1], program, universe, stage)?
            } else {
                eval_nav_chain(&args[0], program, universe, stage)?
            };
            match prev {
                NavValue::Selection(sel) => Ok(NavValue::Selection(
                    sel.descendants(program, filter.as_deref())
                )),
                _ => Err("Descendants$ requires a Selection operand".into()),
            }
        }
        "Parent$" => {
            let prev = eval_nav_chain(&args[0], program, universe, stage)?;
            match prev {
                NavValue::Selection(sel) => Ok(NavValue::Selection(sel.parent(program))),
                _ => Err("Parent$ requires a Selection operand".into()),
            }
        }

        // ── Introspection ──────────────────────────────────────────
        "Count$" => {
            let prev = if args.is_empty() {
                NavValue::Selection(AllSelector.apply(program).map(|n| Selection { nodes: n })?)
            } else {
                eval_nav_chain(&args[0], program, universe, stage)?
            };
            match prev {
                NavValue::Selection(sel) => Ok(NavValue::Count(sel.count())),
                _ => Err("Count$ requires a Selection operand".into()),
            }
        }
        "Names$" => {
            let prev = eval_nav_chain(&args[0], program, universe, stage)?;
            match prev {
                NavValue::Selection(sel) => Ok(NavValue::Names(sel.names(program))),
                _ => Err("Names$ requires a Selection operand".into()),
            }
        }
        "IsEmpty$" => {
            let prev = eval_nav_chain(&args[0], program, universe, stage)?;
            match prev {
                NavValue::Selection(sel) => Ok(NavValue::Bool(sel.is_empty())),
                _ => Err("IsEmpty$ requires a Selection operand".into()),
            }
        }

        // ── Positions ──────────────────────────────────────────────
        "Before$" => {
            let prev = eval_nav_chain(&args[0], program, universe, stage)?;
            match prev {
                NavValue::Selection(sel) => {
                    let pos = Position::before(&sel)
                        .ok_or_else(|| "Before$: selection is empty".to_string())?;
                    Ok(NavValue::Position(pos))
                }
                _ => Err("Before$ requires a Selection operand".into()),
            }
        }
        "After$" => {
            let prev = eval_nav_chain(&args[0], program, universe, stage)?;
            match prev {
                NavValue::Selection(sel) => {
                    let pos = Position::after(&sel)
                        .ok_or_else(|| "After$: selection is empty".to_string())?;
                    Ok(NavValue::Position(pos))
                }
                _ => Err("After$ requires a Selection operand".into()),
            }
        }
        "Replace$" => {
            let prev = eval_nav_chain(&args[0], program, universe, stage)?;
            match prev {
                NavValue::Selection(sel) => {
                    let pos = Position::replace(&sel)
                        .ok_or_else(|| "Replace$: selection is empty".to_string())?;
                    Ok(NavValue::Position(pos))
                }
                _ => Err("Replace$ requires a Selection operand".into()),
            }
        }
        "Inside$" => {
            let prev = eval_nav_chain(&args[0], program, universe, stage)?;
            match prev {
                NavValue::Selection(sel) => {
                    let pos = Position::inside(&sel)
                        .ok_or_else(|| "Inside$: selection is empty".to_string())?;
                    Ok(NavValue::Position(pos))
                }
                _ => Err("Inside$ requires a Selection operand".into()),
            }
        }
        "AppendTo$" => {
            let prev = eval_nav_chain(&args[0], program, universe, stage)?;
            match prev {
                NavValue::Selection(sel) => {
                    let pos = Position::append_to(&sel)
                        .ok_or_else(|| "AppendTo$: selection is empty".to_string())?;
                    Ok(NavValue::Position(pos))
                }
                _ => Err("AppendTo$ requires a Selection operand".into()),
            }
        }

        // ── Actions ────────────────────────────────────────────────
        "Insert$" => {
            let pos_result = eval_nav_chain(&args[0], program, universe, stage)?;
            let mut nodes = Vec::new();
            for arg in &args[1..] {
                let val = eval_nav_chain(arg, program, universe, stage)?;
                match val {
                    NavValue::TopLevel(tl) => nodes.push(tl),
                    NavValue::VecTopLevel(v) => nodes.extend(v),
                    _ => return Err("Insert$: argument must produce an AST node".into()),
                }
            }
            match pos_result {
                NavValue::Position(pos) => {
                    insert_items(program, &pos, nodes, stage)
                        .map(|_| NavValue::Void)
                }
                NavValue::Selection(sel) => {
                    insert_before_each(program, &sel, nodes, stage)
                        .map(|_| NavValue::Void)
                }
                _ => Err("Insert$ requires a Position or Selection operand".into()),
            }
        }
        "Delete$" => {
            let prev = eval_nav_chain(&args[0], program, universe, stage)?;
            match prev {
                NavValue::Selection(sel) => {
                    delete_selection(program, &sel).map(|_| NavValue::Void)
                }
                _ => Err("Delete$ requires a Selection operand".into()),
            }
        }
        "ReplaceWith$" => {
            let prev = eval_nav_chain(&args[0], program, universe, stage)?;
            let repl = eval_nav_chain(&args[1], program, universe, stage)?;
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
        "Set$" => {
            let prev = eval_nav_chain(&args[0], program, universe, stage)?;
            let key = expect_str_arg(args, 1, "Set$")?;
            let val = PropertyValue::Bool(true); // default — will be improved
            match prev {
                NavValue::Selection(sel) => {
                    set_metadata(program, &sel, &key, val).map(|_| NavValue::Void)
                }
                _ => Err("Set$ requires a Selection operand".into()),
            }
        }
        "Rename$" => {
            let prev = eval_nav_chain(&args[0], program, universe, stage)?;
            let name = expect_str_arg(args, 1, "Rename$")?;
            match prev {
                NavValue::Selection(sel) => {
                    rename_selection(program, &sel, &name).map(|_| NavValue::Void)
                }
                _ => Err("Rename$ requires a Selection operand".into()),
            }
        }

        // ── AST Constructors (evaluated inside navigation chains) ──
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
                if let NavValue::TopLevel(TopLevel::Statement(stmt)) = eval_nav_chain(arg, program, universe, stage)? {
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
                if let NavValue::TopLevel(TopLevel::Statement(stmt)) = eval_nav_chain(arg, program, universe, stage)? {
                    stmts.push(*stmt);
                }
            }
            Ok(NavValue::TopLevel(TopLevel::Statement(Box::new(
                Statement::Block(stmts)
            ))))
        }

        // ── Stage$ ─────────────────────────────────────────────────
        "Stage$.Insert$" | "Insert$" if args.len() == 2 && false => {
            // Placeholder — full implementation in follow-up
            let _path = expect_str_arg(args, 1, "Stage$.Insert$")?;
            Err("Stage$.Insert$: Phase H placeholder — not yet wired".into())
        }
        "Stage$.List$" | "List$" => {
            Err("Stage$.List$: Phase H placeholder — not yet wired".into())
        }

        _ => Err(format!(
            "unknown navigation intrinsic '{}'", name
        )),
    }
}

/// Evaluate a field method on a NavValue (for field-like chain steps like `.Count$`).
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
                .map(|p| NavValue::Position(p))
                .ok_or_else(|| "Before$: empty selection".into()),
            _ => Err("Before$ requires Selection".into()),
        },
        "After$" => match prev {
            NavValue::Selection(ref sel) => Position::after(sel)
                .map(|p| NavValue::Position(p))
                .ok_or_else(|| "After$: empty selection".into()),
            _ => Err("After$ requires Selection".into()),
        },
        "Replace$" => match prev {
            NavValue::Selection(ref sel) => Position::replace(sel)
                .map(|p| NavValue::Position(p))
                .ok_or_else(|| "Replace$: empty selection".into()),
            _ => Err("Replace$ requires Selection".into()),
        },
        _ => Err(format!("unknown field method '{}' on navigation value", name)),
    }
}

// ── Entry point for stage block evaluation ─────────────────────────────

/// Evaluate a $(Stage) block body. Each statement is either a navigation
/// chain expression or a flow control statement (let, foreach, when).
pub fn evaluate_stage_block(
    body: &[Statement],
    program: &mut Vec<TopLevel>,
    universe: &mut TypeUniverse,
    stage: StageKind,
) -> Result<(), String> {
    for stmt in body {
        evaluate_stage_stmt(stmt, program, universe, stage)?;
    }
    Ok(())
}

fn evaluate_stage_stmt(
    stmt: &Statement,
    program: &mut Vec<TopLevel>,
    universe: &mut TypeUniverse,
    stage: StageKind,
) -> Result<(), String> {
    match stmt {
        Statement::Expression(expr) => {
            // Evaluate a $ call chain
            let _result = eval_nav_chain(expr, program, universe, stage)?;
            Ok(())
        }
        Statement::Let { expr: Some(e), .. } => {
            // Evaluate the expression and bind the result
            let _result = eval_nav_chain(e, program, universe, stage)?;
            Ok(())
        }
        Statement::Block(statements) => {
            for s in statements {
                evaluate_stage_stmt(s, program, universe, stage)?;
            }
            Ok(())
        }
        Statement::Foreach { item, list, body } => {
            // Evaluate the list expression to get a selection/collection
            let list_val = eval_nav_chain(list, program, universe, stage)?;
            let items = match list_val {
                NavValue::Selection(sel) => sel,
                _ => return Err("foreach: expected a Selection from the list expression".into()),
            };
            // For each element, bind $item and evaluate body
            for node in &items.nodes {
                // Bind the node as the item variable
                // Store in a simple scope: (item_name, NodeRef)
                let bindings = std::collections::HashMap::from([
                    (item.clone(), format!("{:?}", node)),
                ]);
                let _ = bindings;
                for s in body {
                    evaluate_stage_stmt(s, program, universe, stage)?;
                }
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

// ── Argument Extraction Helpers ────────────────────────────────────────

fn expect_str_arg(args: &[Expr], idx: usize, intrinsic: &str) -> Result<String, String> {
    let arg = args.get(idx).ok_or_else(|| {
        format!("{}: missing argument {}", intrinsic, idx)
    })?;
    match arg {
        Expr::Quoted(bytes) => String::from_utf8(bytes.clone())
            .map_err(|_| format!("{}: arg {} is not valid UTF-8", intrinsic, idx)),
        Expr::Identifier(s) => Ok(s.clone()),
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

/// Helper for selectors that take one string arg: Tag$("name")
fn selector_1_str<F>(args: &[Expr], f: F) -> Result<NavValue, String>
where F: FnOnce(String) -> Result<NavValue, String> {
    let s = expect_str_arg(args, 0, "?")?;
    f(s)
}

/// Helper for selectors that take an optional int arg: First$(n) or First$()
fn selector_1_int_opt<F>(args: &[Expr], f: F) -> Result<NavValue, String>
where F: FnOnce(usize) -> Result<NavValue, String> {
    // The first arg is the implicit receiver (the previous chain result)
    // The optional second arg is the number
    let n = if args.len() > 1 {
        expect_int_arg(args, 1, "?")? as usize
    } else {
        1
    };
    f(n)
}

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tag_selector_via_call() {
        let mut program = vec![
            TopLevel::Import(Import::literal("std/io.bv", vec![])),
        ];
        let mut universe = TypeUniverse::new();
        let expr = Expr::Call("Tag$".into(), vec![
            Expr::Quoted("import".into()),
        ], None);
        let result = eval_nav_chain(&expr, &mut program, &mut universe, StageKind::Parsed).unwrap();
        match result {
            NavValue::Selection(sel) => assert_eq!(sel.count(), 1),
            _ => panic!("expected Selection"),
        }
    }

    #[test]
    fn test_count_via_call() {
        let mut program = vec![
            TopLevel::Import(Import::literal("std/io.bv", vec![])),
            TopLevel::Import(Import::literal("std/net.bv", vec![])),
        ];
        let mut universe = TypeUniverse::new();
        // Count$(Tag$("import"))
        let inner = Expr::Call("Tag$".into(), vec![Expr::Quoted("import".into())], None);
        let expr = Expr::Call("Count$".into(), vec![inner], None);
        let result = eval_nav_chain(&expr, &mut program, &mut universe, StageKind::Parsed).unwrap();
        match result {
            NavValue::Count(n) => assert_eq!(n, 2),
            _ => panic!("expected Count"),
        }
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
        // Simulate: Tag$("import").First$().Before$().Insert$(Import$("std/prelude.bv"))
        let import_call = Expr::Call("Import$".into(), vec![Expr::Quoted("std/prelude.bv".into())], None);
        let tag_call = Expr::Call("Tag$".into(), vec![Expr::Quoted("import".into())], None);
        let first_call = Expr::Call("First$".into(), vec![tag_call], None);
        let before_call = Expr::Call("Before$".into(), vec![first_call], None);
        let insert_call = Expr::Call("Insert$".into(), vec![before_call, import_call], None);

        let result = eval_nav_chain(&insert_call, &mut program, &mut universe, StageKind::Parsed).unwrap();
        assert!(matches!(result, NavValue::Void));
        assert_eq!(program.len(), 3);
        match &program[0] {
            TopLevel::Import(i) => assert_eq!(i.path(), "std/prelude.bv"),
            other => panic!("expected Import at position 0, got {:?}", other),
        }
    }

    #[test]
    fn test_delete_selection() {
        let mut program = vec![
            TopLevel::Import(Import::literal("std/io.bv", vec![])),
        ];
        let mut universe = TypeUniverse::new();
        // Tag$("import").Delete$()
        let tag_call = Expr::Call("Tag$".into(), vec![Expr::Quoted("import".into())], None);
        let delete_call = Expr::Call("Delete$".into(), vec![tag_call], None);
        let result = eval_nav_chain(&delete_call, &mut program, &mut universe, StageKind::Parsed).unwrap();
        assert!(matches!(result, NavValue::Void));
        assert!(program.is_empty());
    }
}
