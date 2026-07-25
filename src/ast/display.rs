// ── Display Impls for AST Types ────────────────────────────────────────
// 2026-07-12: Phase 0.2 — Format AST types as valid Brief source text.
// 2026-07-22: Phase 8 — Round-trip tests verify Rust<->Brief pp parity.

use crate::ast::*;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// Display implementations
// ═══════════════════════════════════════════════════════════════════════

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Expr::Quoted(bytes) => write!(f, "\"{}\"", String::from_utf8_lossy(bytes)),
            Expr::Decimal(n) | Expr::TaggedLiteral(n, _) => write!(f, "{}", n),
            Expr::Bool(b) => write!(f, "{}", if *b { "true" } else { "false" }),
            Expr::Float(n) => write!(f, "{}", n),
            Expr::Identifier(name) => write!(f, "{}", name),
            Expr::Call(name, args, _) => {
                write!(f, "{}(", name)?;
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", arg)?;
                }
                write!(f, ")")
            }
            Expr::BinaryOp(kind, lhs, rhs) => {
                write!(f, "({} {} {})", lhs, kind, rhs)
            }
            Expr::UnaryOp(kind, expr) => {
                write!(f, "({}{})", kind, expr)
            }
            Expr::Field(obj, name) => write!(f, "{}.{}", obj, name),
            Expr::Index(obj, index) => write!(f, "{}[{}]", obj, index),
            Expr::Exists(name) => write!(f, "{}?", name),
            Expr::Block(stmts) => {
                write!(f, "{{ ")?;
                for stmt in stmts {
                    write!(f, "{} ", stmt)?;
                }
                write!(f, "}}")
            }
            Expr::If(cond, then, else_) => {
                write!(f, "if {} then {}", cond, then)?;
                if let Some(else_) = else_ {
                    write!(f, " else {}", else_)?;
                }
                Ok(())
            }
            Expr::Match(expr, arms) => {
                write!(f, "match {} {{ ", expr)?;
                for arm in arms {
                    write!(f, "{} => {}", arm.pattern, arm.body)?;
                }
                write!(f, " }}")
            }
            Expr::Tuple(elems) => {
                write!(f, "(")?;
                for (i, elem) in elems.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", elem)?;
                }
                write!(f, ")")
            }
            Expr::List(elems) => {
                write!(f, "[")?;
                for (i, elem) in elems.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", elem)?;
                }
                write!(f, "]")
            }
            Expr::Lambda(params, body) => {
                write!(f, "(")?;
                for (i, p) in params.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", p)?;
                }
                write!(f, ") => {}", body)
            }
            Expr::Cast(expr, ty) => write!(f, "{} as {}", expr, ty),
            Expr::IsType(expr, ty) => write!(f, "{} is {}", expr, ty),
            Expr::Within(expr, scope) => write!(f, "{} within {}", expr, scope),
            Expr::DerivationBlock(block) => fmt::Display::fmt(block, f),
            Expr::Deref(inner) => write!(f, "*{}", inner),
            Expr::AddrOf(inner) => write!(f, "&{}", inner),
            Expr::PluginIntercept { name, args, type_args: _ } => {
                write!(f, "{}!(", name)?;
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}", arg)?;
                }
                write!(f, ")")
            }
            Expr::PropertyGet(name) => write!(f, "property '{}'", name),
            Expr::FormattingAnnotation(fmt_) => write!(f, "formatting <~ {}", fmt_.name()),
            Expr::StructLiteral { type_name, .. } => write!(f, "{} {{ ... }}", type_name),
        }
    }
}

impl fmt::Display for BinaryOpKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BinaryOpKind::Add => write!(f, "+"),
            BinaryOpKind::Sub => write!(f, "-"),
            BinaryOpKind::Mul => write!(f, "*"),
            BinaryOpKind::Div => write!(f, "/"),
            BinaryOpKind::Mod => write!(f, "%"),
            BinaryOpKind::Eq => write!(f, "=="),
            BinaryOpKind::Neq => write!(f, "!="),
            BinaryOpKind::Lt => write!(f, "<"),
            BinaryOpKind::Gt => write!(f, ">"),
            BinaryOpKind::Le => write!(f, "<="),
            BinaryOpKind::Ge => write!(f, ">="),
            BinaryOpKind::And => write!(f, "&&"),
            BinaryOpKind::Or => write!(f, "||"),
            BinaryOpKind::BitAnd => write!(f, "&"),
            BinaryOpKind::BitOr => write!(f, "|"),
            BinaryOpKind::BitXor => write!(f, "^"),
            BinaryOpKind::Shl => write!(f, "<<"),
            BinaryOpKind::Shr => write!(f, ">>"),
            BinaryOpKind::Concat => write!(f, "++"),
        }
    }
}

impl fmt::Display for UnaryOpKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UnaryOpKind::Neg => write!(f, "-"),
            UnaryOpKind::Not => write!(f, "!"),
            UnaryOpKind::BitNot => write!(f, "~"),
        }
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::Bits(n) => write!(f, "Bits<{}>", n * 8),
            Type::Void => write!(f, "void"),
            Type::Custom(name) => write!(f, "{}", name),
            Type::HashWord(name) => write!(f, "{}", name),
            Type::HashWordVariant(name, var) => write!(f, "{}<{}>", name, var),
            Type::Generic(name, args) => {
                write!(f, "{}<", name)?;
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", arg)?;
                }
                write!(f, ">")
            }
            Type::Applied(name, args) => {
                write!(f, "{}<", name)?;
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", arg)?;
                }
                write!(f, ">")
            }
            Type::Union(types) => {
                for (i, t) in types.iter().enumerate() {
                    if i > 0 {
                        write!(f, " | ")?;
                    }
                    write!(f, "{}", t)?;
                }
                Ok(())
            }
            Type::Tuple(types) => {
                write!(f, "(")?;
                for (i, t) in types.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", t)?;
                }
                write!(f, ")")
            }
            Type::TypeVar(name) => write!(f, "{}", name),
            Type::Ptr(inner) => write!(f, "Ptr<{}>", inner),
            Type::PtrConst(inner) => write!(f, "Ptr<const {}>", inner),
            Type::Function(params, ret) => {
                write!(f, "(")?;
                for (i, p) in params.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", p)?;
                }
                write!(f, ") -> {}", ret)
            }
            Type::Width(n) => write!(f, "{}", n),
            Type::Vector(ty, dims) => {
                write!(f, "{}", ty)?;
                for dim in dims {
                    write!(f, "[{}]", dim)?;
                }
                Ok(())
            }
            Type::Constrained(ty, range) => write!(f, "{} @/ {:?}", ty, range),
            Type::LayoutPtr(c) => write!(f, "Ptr<Bits @/{}>", c.bytes),
        }
    }
}

impl fmt::Display for Dimension {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Dimension::Anonymous(n) => write!(f, "{}", n),
            Dimension::Named(name, n) => write!(f, "{} = {}", name, n),
        }
    }
}

impl fmt::Display for Statement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Statement::Let { name, ty, expr, .. } => {
                if let Some(ty) = ty {
                    if let Some(expr) = expr {
                        write!(f, "let {}: {} = {};", name, ty, expr)
                    } else {
                        write!(f, "let {}: {};", name, ty)
                    }
                } else if let Some(expr) = expr {
                    write!(f, "let {} = {};", name, expr)
                } else {
                    write!(f, "let {};", name)
                }
            }
            Statement::Assign(lhs, rhs) => write!(f, "{} = {};", lhs, rhs),
            Statement::Term(val) => {
                if let Some(val) = val {
                    write!(f, "term {};", val)
                } else {
                    write!(f, "term;")
                }
            }
            Statement::TermBang(val) => {
                if let Some(val) = val {
                    write!(f, "term! {};", val)
                } else {
                    write!(f, "term!;")
                }
            }
            Statement::Return(val) => {
                if let Some(val) = val {
                    write!(f, "return {};", val)
                } else {
                    write!(f, "return;")
                }
            }
            Statement::Guarded(cond, body) => {
                write!(f, "[{}] {{ ", cond)?;
                for stmt in body {
                    write!(f, "{} ", stmt)?;
                }
                write!(f, "}}")
            }
            Statement::Expression(expr) => write!(f, "{};", expr),
            Statement::If(cond, then, else_) => {
                write!(f, "if {} {{ ", cond)?;
                for stmt in then {
                    write!(f, "{} ", stmt)?;
                }
                write!(f, "}}")?;
                if !else_.is_empty() {
                    write!(f, " else {{ ")?;
                    for stmt in else_ {
                        write!(f, "{} ", stmt)?;
                    }
                    write!(f, "}}")?;
                }
                Ok(())
            }
            Statement::Block(stmts) => {
                write!(f, "{{ ")?;
                for stmt in stmts {
                    write!(f, "{} ", stmt)?;
                }
                write!(f, "}}")
            }
            Statement::MetadataAssignment(key, val) => {
                write!(f, "{} <~ {:?};", key, val)
            }
            Statement::Escape(val) => {
                if let Some(val) = val {
                    write!(f, "escape {};", val)
                } else {
                    write!(f, "escape;")
                }
            }
            Statement::Foreach { item, list, body } => {
                write!(f, "foreach({} in {}) {{ ", item, list)?;
                for stmt in body {
                    write!(f, "{} ", stmt)?;
                }
                write!(f, "}}")
            }
            Statement::TrgBinding {
                name,
                instance,
                port,
            } => {
                write!(f, "trg {} @ {}.{};", name, instance, port)
            }
            Statement::InlineAsm { asm_string, .. } => {
                write!(f, "asm \"{}\";", asm_string)
            }
            Statement::SyncBlock(body) => {
                write!(f, "sync {{ ")?;
                for stmt in body {
                    write!(f, "{} ", stmt)?;
                }
                write!(f, "}}")
            }
            Statement::InlineDefn(d) => write!(f, "$defn {}", d.name),
            Statement::InlineTxn(t) => write!(f, "$txn {}", t.name),
            Statement::Match { .. } => write!(f, "match {{ ... }}"),
        }
    }
}

impl fmt::Display for TopLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TopLevel::Definition(defn) => {
                write!(f, "defn {}(", defn.name)?;
                for (i, (name, ty)) in defn.parameters.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}: {}", name, ty)?;
                }
                write!(f, ")")?;
                if let Some(oty) = &defn.output_type {
                    write!(f, " -> {}", oty)?;
                }
                write!(f, " {}", defn.contract)?;
                if let Some(deriv) = &defn.derivation {
                    write!(f, " {}", deriv)?;
                }
                write!(f, " {{ ")?;
                for stmt in &defn.body {
                    write!(f, "{} ", stmt)?;
                }
                write!(f, "}};")
            }
            TopLevel::Transaction(txn) => {
                let prefix = if txn.is_reactive { "node" } else { "txn" };
                write!(f, "{} {}", prefix, txn.name)?;
                if !txn.parameters.is_empty() {
                    write!(f, "(")?;
                    for (i, (name, ty)) in txn.parameters.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{}: {}", name, ty)?;
                    }
                    write!(f, ")")?;
                }
                write!(f, " {}", txn.contract)?;
                write!(f, " {{ ")?;
                for stmt in &txn.body {
                    write!(f, "{} ", stmt)?;
                }
                write!(f, "}};")
            }
            TopLevel::Cell(cell) => {
                write!(f, "cell {} {{ ... }};", cell.name)
            }
            TopLevel::Import(import) => {
                match &import.kind {
                    ImportKind::Literal(path) => {
                        if import.symbols.is_empty() {
                            write!(f, "import \"{}\";", path)
                        } else {
                            write!(f, "import {{ {} }} from \"{}\";", import.symbols.join(", "), path)
                        }
                    }
                    ImportKind::Registry(name) => {
                        if import.symbols.is_empty() {
                            write!(f, "import <{}>;", name)
                        } else {
                            write!(f, "import {{ {} }} from <{}>;", import.symbols.join(", "), name)
                        }
                    }
                }
            }
            TopLevel::Export(export) => {
                write!(f, "export {}", export.inner)
            }
            TopLevel::Meld(meld) => {
                write!(f, "meld {} -> {};", meld.name, meld.target)
            }
            TopLevel::Trigger(trg) => {
                write!(f, "trg {} @ {}.{};", trg.name, trg.instance, trg.port)
            }
            _ => write!(f, "<definition>"),
        }
    }
}

impl fmt::Display for OutputType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OutputType::Single(ty) => write!(f, "{}", ty),
            OutputType::Union(types) => {
                for (i, t) in types.iter().enumerate() {
                    if i > 0 {
                        write!(f, " | ")?;
                    }
                    write!(f, "{}", t)?;
                }
                Ok(())
            }
            OutputType::Tuple(types) => {
                write!(f, "(")?;
                for (i, t) in types.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", t)?;
                }
                write!(f, ")")
            }
            OutputType::Array(inner) => write!(f, "[]{}", inner),
            OutputType::Named(name, inner) => write!(f, "{}: {}", name, inner),
        }
    }
}

impl fmt::Display for Contract {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_entry {
            write!(f, "[#]")?;
            write!(f, " [{}]", self.post_condition)?;
            return Ok(());
        }
        write!(f, "[{}]", self.pre_condition)?;
        write!(f, "[{}]", self.post_condition)
    }
}

impl fmt::Display for Pattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Pattern::Wildcard => write!(f, "_"),
            Pattern::Literal(expr) => write!(f, "{}", expr),
            Pattern::Binding(name) => write!(f, "{}", name),
            Pattern::EnumVariant(name, fields) => {
                write!(f, "{}", name)?;
                if !fields.is_empty() {
                    write!(f, "(")?;
                    for (i, field) in fields.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{}", field)?;
                    }
                    write!(f, ")")?;
                }
                Ok(())
            }
            Pattern::Tuple(elems) => {
                write!(f, "(")?;
                for (i, elem) in elems.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", elem)?;
                }
                write!(f, ")")
            }
            Pattern::Range(start, end) => write!(f, "{}..{}", start, end),
        }
    }
}

impl fmt::Display for DerivationBlock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, ":= {{ ")?;
        for example in &self.examples {
            write!(f, "{}", example)?;
        }
        write!(f, "}}")
    }
}

impl fmt::Display for DerivationExample {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, input) in self.inputs.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}", input)?;
        }
        write!(f, " -> {}", self.output)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Round-trip tests (Rust Display vs Brief pretty-printer via GLUE bridge)
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    /// Snapshot test: verify Rust Display output for all Type variants.
    #[test]
    fn test_display_type_snapshots() {
        assert_eq!(format!("{}", Type::Bits(8)), "Bits<64>");
        assert_eq!(format!("{}", Type::Void), "void");
        assert_eq!(format!("{}", Type::Custom("Int".into())), "Int");
        assert_eq!(format!("{}", Type::HashWord("L".into())), "L");
        assert_eq!(format!("{}", Type::HashWordVariant("String".into(), "utf8".into())), "String<utf8>");
        assert_eq!(
            format!("{}", Type::Generic("List".into(), vec![Type::Custom("Int".into())])),
            "List<Int>"
        );
        assert_eq!(
            format!("{}", Type::Applied("Result".into(), vec![Type::Custom("Int".into()), Type::Custom("Error".into())])),
            "Result<Int, Error>"
        );
        assert_eq!(format!("{}", Type::Union(vec![Type::Custom("A".into()), Type::Custom("B".into())])), "A | B");
        assert_eq!(format!("{}", Type::Tuple(vec![Type::Custom("Int".into()), Type::Custom("Bool".into())])), "(Int, Bool)");
        assert_eq!(format!("{}", Type::TypeVar("T".into())), "T");
        assert_eq!(format!("{}", Type::Ptr(Box::new(Type::Custom("Int".into())))), "Ptr<Int>");
        assert_eq!(format!("{}", Type::PtrConst(Box::new(Type::Custom("Int".into())))), "Ptr<const Int>");
        assert_eq!(
            format!("{}", Type::Function(vec![Type::Custom("Int".into())], Box::new(Type::Custom("Bool".into())))),
            "(Int) -> Bool"
        );
        assert_eq!(format!("{}", Type::Width(64)), "64");
        assert_eq!(
            format!("{}", Type::Vector(Box::new(Type::Custom("Float".into())), vec![Dimension::Anonymous(4)])),
            "Float[4]"
        );
    }

    /// Snapshot test: verify Rust Display output for BinaryOpKind.
    #[test]
    fn test_display_binop_snapshots() {
        assert_eq!(format!("{}", BinaryOpKind::Add), "+");
        assert_eq!(format!("{}", BinaryOpKind::Sub), "-");
        assert_eq!(format!("{}", BinaryOpKind::Mul), "*");
        assert_eq!(format!("{}", BinaryOpKind::Div), "/");
        assert_eq!(format!("{}", BinaryOpKind::Eq), "==");
        assert_eq!(format!("{}", BinaryOpKind::Neq), "!=");
        assert_eq!(format!("{}", BinaryOpKind::Lt), "<");
        assert_eq!(format!("{}", BinaryOpKind::Gt), ">");
        assert_eq!(format!("{}", BinaryOpKind::Le), "<=");
        assert_eq!(format!("{}", BinaryOpKind::Ge), ">=");
        assert_eq!(format!("{}", BinaryOpKind::And), "&&");
        assert_eq!(format!("{}", BinaryOpKind::Or), "||");
        assert_eq!(format!("{}", BinaryOpKind::BitAnd), "&");
        assert_eq!(format!("{}", BinaryOpKind::BitOr), "|");
        assert_eq!(format!("{}", BinaryOpKind::BitXor), "^");
        assert_eq!(format!("{}", BinaryOpKind::Shl), "<<");
        assert_eq!(format!("{}", BinaryOpKind::Shr), ">>");
        assert_eq!(format!("{}", BinaryOpKind::Concat), "++");
    }

    /// Snapshot test: verify Rust Display output for UnaryOpKind.
    #[test]
    fn test_display_unary_op_snapshots() {
        assert_eq!(format!("{}", UnaryOpKind::Neg), "-");
        assert_eq!(format!("{}", UnaryOpKind::Not), "!");
        assert_eq!(format!("{}", UnaryOpKind::BitNot), "~");
    }
}
