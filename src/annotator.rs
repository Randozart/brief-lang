use crate::ast::*;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct CallSite {
    pub caller: String,
    pub callee: String,
    pub line: usize,
}

#[derive(Debug)]
pub struct Annotation {
    pub paths_to: Vec<Vec<String>>,
    pub callers: Vec<String>,
    pub is_entry: bool,
    pub is_recursive: bool,
    pub call_sites: Vec<CallSite>,
}

pub struct Annotator {
    call_graph: HashMap<String, Vec<(String, usize)>>,
    reverse_graph: HashMap<String, Vec<String>>,
    annotations: HashMap<String, Annotation>,
    entry_points: Vec<String>,
}

impl Annotator {
    pub fn new() -> Self {
        Annotator {
            call_graph: HashMap::new(),
            reverse_graph: HashMap::new(),
            annotations: HashMap::new(),
            entry_points: Vec::new(),
        }
    }

    pub fn analyze(&mut self, program: &Program) {
        self.build_call_graph(program);
        self.find_entry_points(program);
        self.detect_recursion();
        self.compute_annotations();
    }

    fn build_call_graph(&mut self, program: &Program) {
        for item in &program.items {
            match item {
                TopLevel::Definition(defn) => {
                    let callees = self.extract_calls(&defn.body);
                    self.call_graph.insert(defn.name.clone(), callees);
                }
                TopLevel::Transaction(txn) => {
                    let callees = self.extract_calls(&txn.body);
                    self.call_graph.insert(txn.name.clone(), callees);
                }
                _ => {}
            }
        }

        for (caller, callees) in &self.call_graph {
            for (callee, _) in callees {
                self.reverse_graph
                    .entry(callee.clone())
                    .or_insert_with(Vec::new)
                    .push(caller.clone());
            }
        }
    }

    fn extract_calls(&self, statements: &[Statement]) -> Vec<(String, usize)> {
        let mut calls = Vec::new();
        self.collect_calls_from_stmts(statements, &mut calls);
        calls
    }

    fn collect_calls_from_stmts(&self, statements: &[Statement], calls: &mut Vec<(String, usize)>) {
        for stmt in statements {
            match stmt {
                Statement::Expression(Expr::Call(name, _)) => {
                    calls.push((name.clone(), 0));
                }
                Statement::Guarded { statements, .. } => {
                    self.collect_calls_from_stmts(statements, calls);
                }
                Statement::Let { expr, .. } => {
                    if let Some(e) = expr {
                        self.collect_calls_from_expr(e, calls);
                    }
                }
                Statement::Assignment { expr, .. } => {
                    self.collect_calls_from_expr(expr, calls);
                }
                Statement::Unification { expr, .. } => {
                    self.collect_calls_from_expr(expr, calls);
                }
                _ => {}
            }
        }
    }

    fn collect_calls_from_expr(&self, expr: &Expr, calls: &mut Vec<(String, usize)>) {
        match expr {
            Expr::Call(name, args) => {
                calls.push((name.clone(), 0));
                for arg in args {
                    self.collect_calls_from_expr(arg, calls);
                }
            }
            Expr::Add(l, r) | Expr::Sub(l, r) | Expr::Mul(l, r) | Expr::Div(l, r) => {
                self.collect_calls_from_expr(l, calls);
                self.collect_calls_from_expr(r, calls);
            }
            Expr::Eq(l, r)
            | Expr::Ne(l, r)
            | Expr::Lt(l, r)
            | Expr::Le(l, r)
            | Expr::Gt(l, r)
            | Expr::Ge(l, r)
            | Expr::Or(l, r)
            | Expr::And(l, r) => {
                self.collect_calls_from_expr(l, calls);
                self.collect_calls_from_expr(r, calls);
            }
            Expr::Not(e) | Expr::Neg(e) | Expr::BitNot(e) => {
                self.collect_calls_from_expr(e, calls);
            }
            _ => {}
        }
    }

    fn find_entry_points(&mut self, program: &Program) {
        for item in program.items.iter() {
            if let TopLevel::Transaction(txn) = item {
                if txn.is_reactive {
                    if let Expr::Bool(true) = &txn.contract.pre_condition {
                        self.entry_points.push(txn.name.clone());
                    }
                }
            }
        }
    }

    fn detect_recursion(&self) {
        // Simple recursion detection: if A calls B and B calls A (directly or indirectly)
        // This is a simplified version
    }

    fn compute_annotations(&mut self) {
        for name in self.call_graph.keys() {
            let paths = self.compute_paths_to(name);
            let callers = self.reverse_graph.get(name).cloned().unwrap_or_default();
            let is_entry = self.entry_points.contains(name);
            let is_recursive = self.detect_self_recursion(name);
            let call_sites = self.get_call_sites(name);

            self.annotations.insert(
                name.clone(),
                Annotation {
                    paths_to: paths,
                    callers,
                    is_entry,
                    is_recursive,
                    call_sites,
                },
            );
        }
    }

    fn compute_paths_to(&self, target: &str) -> Vec<Vec<String>> {
        let mut all_paths = Vec::new();

        for entry in &self.entry_points {
            let mut visited = HashSet::new();
            let mut path = Vec::new();
            self.dfs_paths(entry, target, &mut visited, &mut path, &mut all_paths);
        }

        all_paths
    }

    fn dfs_paths(
        &self,
        current: &str,
        target: &str,
        visited: &mut HashSet<String>,
        path: &mut Vec<String>,
        all_paths: &mut Vec<Vec<String>>,
    ) {
        if current == target {
            let mut full_path = path.clone();
            full_path.push(current.to_string());
            all_paths.push(full_path);
            return;
        }

        if visited.contains(current) {
            return;
        }

        visited.insert(current.to_string());
        path.push(current.to_string());

        if let Some(callees) = self.call_graph.get(current) {
            for (callee, _) in callees {
                self.dfs_paths(callee, target, visited, path, all_paths);
            }
        }

        path.pop();
        visited.remove(current);
    }

    fn detect_self_recursion(&self, name: &str) -> bool {
        if let Some(callees) = self.call_graph.get(name) {
            callees.iter().any(|(callee, _)| callee == name)
        } else {
            false
        }
    }

    fn get_call_sites(&self, target: &str) -> Vec<CallSite> {
        let mut sites = Vec::new();

        for (caller, callees) in &self.call_graph {
            for (callee, line) in callees {
                if callee == target {
                    sites.push(CallSite {
                        caller: caller.clone(),
                        callee: callee.clone(),
                        line: *line,
                    });
                }
            }
        }

        sites
    }

    pub fn get_annotation(&self, name: &str) -> Option<&Annotation> {
        self.annotations.get(name)
    }

    pub fn format_path(&self, path: &[String]) -> String {
        if path.is_empty() {
            "→".to_string()
        } else {
            path.join(" → ")
        }
    }

    pub fn annotate_program(&self, program: &Program) -> String {
        // First, strip any existing annotations from original comments
        let cleaned_comments: Vec<&Comment> = program
            .comments
            .iter()
            .filter(|c| {
                let text = &c.text;
                !text.contains("=== PATH ANALYSIS ===")
                    && !text.contains("=== END PATH ANALYSIS ===")
                    && !text.starts_with("// PATHS_TO:")
                    && !text.starts_with("// CALLERS:")
                    && !text.starts_with("// RESOLVES_TO:")
                    && !text.contains("[ENTRY_POINT]")
                    && !text.contains("[RECURSIVE]")
            })
            .collect();

        let mut output = String::new();

        // Output original (non-annotation) comments first
        for comment in cleaned_comments {
            output.push_str(&comment.text);
            output.push('\n');
        }

        // Add path analysis in a collapsible block
        output.push_str("\n// === PATH ANALYSIS ===\n");
        output.push_str("// Annotations generated by brief-compiler --annotate\n");
        output.push_str("// These comments can be hidden or filtered out\n");
        output.push_str("// Use: grep -v '^// ===' or editor folding\n");

        for item in &program.items {
            match item {
                TopLevel::Definition(defn) => {
                    if let Some(annot) = self.get_annotation(&defn.name) {
                        output.push_str(&format!(
                            "// PATHS_TO: {}\n",
                            if annot.paths_to.is_empty() {
                                "(unreachable)".to_string()
                            } else {
                                annot
                                    .paths_to
                                    .iter()
                                    .map(|p| self.format_path(p))
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            }
                        ));
                        output.push_str(&format!(
                            "// CALLERS: {}\n",
                            if annot.callers.is_empty() {
                                "(none)".to_string()
                            } else {
                                annot.callers.join(", ")
                            }
                        ));
                        if annot.is_entry {
                            output.push_str("// [ENTRY_POINT]\n");
                        }
                        if annot.is_recursive {
                            output.push_str("// [RECURSIVE]\n");
                        }
                    }
                    output.push_str(&self.format_definition(defn));
                }
                TopLevel::Transaction(txn) => {
                    if let Some(annot) = self.get_annotation(&txn.name) {
                        output.push_str(&format!(
                            "// PATHS_TO: {}\n",
                            if annot.paths_to.is_empty() {
                                "(unreachable)".to_string()
                            } else {
                                annot
                                    .paths_to
                                    .iter()
                                    .map(|p| self.format_path(p))
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            }
                        ));
                        output.push_str(&format!(
                            "// CALLERS: {}\n",
                            if annot.callers.is_empty() {
                                "(none)".to_string()
                            } else {
                                annot.callers.join(", ")
                            }
                        ));
                        if annot.is_entry {
                            output.push_str("// [ENTRY_POINT]\n");
                        }
                        if annot.is_recursive {
                            output.push_str("// [RECURSIVE]\n");
                        }
                    }
                    output.push_str(&self.format_transaction(txn));
                }
                TopLevel::Signature(sig) => {
                    output.push_str("// CALLERS: (external)\n");
                    output.push_str(&format!(
                        "// RESOLVES_TO: {}\n",
                        self.format_result_type(&sig.result_type)
                    ));
                    output.push_str(&self.format_signature(sig));
                }
                TopLevel::StateDecl(state) => {
                    output.push_str(&self.format_state_decl(state));
                }
                TopLevel::Constant(const_decl) => {
                    output.push_str(&self.format_constant(const_decl));
                }
                TopLevel::Import(import) => {
                    output.push_str(&format!("import {};\n", import.path.join(".")));
                }
                TopLevel::ForeignSig(frgn) => {
                    let inputs = frgn
                        .input_types
                        .iter()
                        .map(|t| self.type_to_string(t))
                        .collect::<Vec<_>>()
                        .join(", ");
                    let outputs = frgn
                        .outputs
                        .iter()
                        .map(|t| self.type_to_string(t))
                        .collect::<Vec<_>>()
                        .join(", ");
                    output.push_str(&format!(
                        "frgn sig {}: {} -> {};\n",
                        frgn.name, inputs, outputs
                    ));
                }
                TopLevel::ForeignBinding {
                    name, toml_path, ..
                } => {
                    output.push_str(&format!("frgn {} from \"{}\";\n", name, toml_path));
                }
                TopLevel::Struct(struct_def) => {
                    output.push_str(&format!("struct {} {{ ", struct_def.name));
                    let field_strs: Vec<String> = struct_def
                        .fields
                        .iter()
                        .map(|f| format!("{}: {}", f.name, self.type_to_string(&f.ty)))
                        .collect();
                    output.push_str(&field_strs.join(", "));
                    output.push_str(" }\n");
                }
                TopLevel::RStruct(rstruct_def) => {
                    output.push_str(&format!("rstruct {} {{ ", rstruct_def.name));
                    let field_strs: Vec<String> = rstruct_def
                        .fields
                        .iter()
                        .map(|f| format!("{}: {}", f.name, self.type_to_string(&f.ty)))
                        .collect();
                    output.push_str(&field_strs.join(", "));
                    output.push_str(" ...view... }\n");
                }
                TopLevel::RenderBlock(rb) => {
                    output.push_str(&format!("render {} {{ ... }}\n", rb.struct_name));
                }
                TopLevel::Stylesheet(css) => {
                    output.push_str(&format!("// Stylesheet ({} chars)\n", css.len()));
                }
                TopLevel::SvgComponent(svg) => {
                    output.push_str(&format!("// SvgComponent ({} chars)\n", svg.len()));
                }
            }
            output.push('\n');
        }

        output.push_str("// === END PATH ANALYSIS ===\n");

        output
    }

    fn type_to_string(&self, ty: &Type) -> String {
        match ty {
            Type::Int => "Int".to_string(),
            Type::Float => "Float".to_string(),
            Type::String => "String".to_string(),
            Type::Bool => "Bool".to_string(),
            Type::Data => "Data".to_string(),
            Type::Void => "Void".to_string(),
            Type::Custom(name) => name.clone(),
            Type::Sig(name) => format!("sig {}", name),
            Type::TypeVar(name) => name.clone(),
            Type::Union(types) => types
                .iter()
                .map(|t| self.type_to_string(t))
                .collect::<Vec<_>>()
                .join(" | "),
            Type::ContractBound(inner, _) => self.type_to_string(inner),
            Type::Generic(name, type_args) => {
                format!(
                    "{}<{}>",
                    name,
                    type_args
                        .iter()
                        .map(|t| self.type_to_string(t))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
            Type::Applied(name, type_args) => {
                format!(
                    "{}<{}>",
                    name,
                    type_args
                        .iter()
                        .map(|t| self.type_to_string(t))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
            Type::Option(inner) => {
                format!("Option<{}>", self.type_to_string(inner))
            }
        }
    }

    fn format_definition(&self, defn: &Definition) -> String {
        let params: Vec<String> = defn
            .parameters
            .iter()
            .map(|(n, t)| format!("{}: {}", n, self.type_to_string(t)))
            .collect();

        let params_str = if params.is_empty() {
            "()".to_string()
        } else {
            format!("({})", params.join(", "))
        };
        let outputs_str = if defn.outputs.is_empty() {
            String::new()
        } else {
            let outputs: Vec<String> = defn
                .outputs
                .iter()
                .map(|t| self.type_to_string(t))
                .collect();
            format!(": {}", outputs.join(", "))
        };

        let pre = self.format_expr(&defn.contract.pre_condition);
        let post = self.format_expr(&defn.contract.post_condition);

        let body = self.format_body(&defn.body);

        format!(
            "defn {}{}{} [{}][{}] {{\n{}}};\n",
            defn.name, params_str, outputs_str, pre, post, body
        )
    }

    fn format_transaction(&self, txn: &Transaction) -> String {
        let modifier = if txn.is_async { "async " } else { "" };
        let rct = if txn.is_reactive { "rct " } else { "" };

        let pre = self.format_expr(&txn.contract.pre_condition);
        let post = self.format_expr(&txn.contract.post_condition);

        let body = self.format_body(&txn.body);

        format!(
            "{}txn {}{} [{}][{}] {{\n{}}};\n",
            rct, modifier, txn.name, pre, post, body
        )
    }

    fn format_signature(&self, sig: &Signature) -> String {
        let inputs: Vec<String> = sig
            .input_types
            .iter()
            .map(|t| self.type_to_string(t))
            .collect();
        format!(
            "sig {}: {} -> {};\n",
            sig.name,
            inputs.join(" -> "),
            self.format_result_type(&sig.result_type)
        )
    }

    fn format_result_type(&self, rt: &ResultType) -> String {
        match rt {
            ResultType::Projection(types) => types
                .iter()
                .map(|t| self.type_to_string(t))
                .collect::<Vec<_>>()
                .join(", "),
            ResultType::TrueAssertion => "true".to_string(),
        }
    }

    fn format_state_decl(&self, state: &StateDecl) -> String {
        let init = if let Some(expr) = &state.expr {
            format!(" = {}", self.format_expr(expr))
        } else {
            String::new()
        };
        format!(
            "let {}: {}{};\n",
            state.name,
            self.type_to_string(&state.ty),
            init
        )
    }

    fn format_constant(&self, const_decl: &Constant) -> String {
        format!(
            "const {}: {} = {};\n",
            const_decl.name,
            self.type_to_string(&const_decl.ty),
            self.format_expr(&const_decl.expr)
        )
    }

    fn format_body(&self, stmts: &[Statement]) -> String {
        stmts
            .iter()
            .map(|s| self.format_statement(s, 1))
            .collect::<Vec<_>>()
            .join("")
    }

    fn format_statement(&self, stmt: &Statement, indent: usize) -> String {
        let spaces = "  ".repeat(indent);
        match stmt {
            Statement::Let { name, ty, expr } => {
                let ty_str = if let Some(t) = ty {
                    format!(": {}", self.type_to_string(t))
                } else {
                    String::new()
                };
                let expr_str = if let Some(e) = expr {
                    format!(" = {}", self.format_expr(e))
                } else {
                    String::new()
                };
                format!("{}let {}{}{};\n", spaces, name, ty_str, expr_str)
            }
            Statement::Assignment {
                is_owned,
                name,
                expr,
            } => {
                let prefix = if *is_owned { "&" } else { "" };
                format!(
                    "{}{}{} = {};\n",
                    spaces,
                    prefix,
                    name,
                    self.format_expr(expr)
                )
            }
            Statement::Expression(expr) => {
                format!("{}{};\n", spaces, self.format_expr(expr))
            }
            Statement::Term(outputs) => {
                let outputs_str = outputs
                    .iter()
                    .map(|e| match e {
                        Some(ex) => self.format_expr(ex),
                        None => String::new(),
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{}term {};\n", spaces, outputs_str)
            }
            Statement::Escape(expr) => {
                let e = if let Some(ex) = expr {
                    format!(" {}", self.format_expr(ex))
                } else {
                    String::new()
                };
                format!("{}escape{};\n", spaces, e)
            }
            Statement::Guarded {
                condition,
                statements,
            } => {
                if statements.len() == 1 {
                    // Single statement: flat syntax
                    format!(
                        "{}[{}] {}\n",
                        spaces,
                        self.format_expr(condition),
                        self.format_statement(&statements[0], 0).trim()
                    )
                } else {
                    // Multiple statements: block syntax
                    let mut result = format!("{}[{}] {{\n", spaces, self.format_expr(condition));
                    for stmt in statements {
                        result.push_str(&self.format_statement(stmt, indent + 2));
                    }
                    result.push_str(&format!("{}}}\n", spaces));
                    result
                }
            }
            Statement::Unification {
                name,
                pattern,
                expr,
            } => {
                format!(
                    "{}{}({}) = {};\n",
                    spaces,
                    name,
                    pattern,
                    self.format_expr(expr)
                )
            }
        }
    }

    fn format_expr(&self, expr: &Expr) -> String {
        match expr {
            Expr::Integer(n) => n.to_string(),
            Expr::Float(f) => f.to_string(),
            Expr::String(s) => format!("\"{}\"", s),
            Expr::Bool(true) => "true".to_string(),
            Expr::Bool(false) => "false".to_string(),
            Expr::Identifier(n) => n.clone(),
            Expr::OwnedRef(n) => format!("&{}", n),
            Expr::PriorState(n) => format!("@{}", n),
            Expr::Call(name, args) => {
                let args_str = args
                    .iter()
                    .map(|a| self.format_expr(a))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{}({})", name, args_str)
            }
            Expr::Add(l, r) => format!("({} + {})", self.format_expr(l), self.format_expr(r)),
            Expr::Sub(l, r) => format!("({} - {})", self.format_expr(l), self.format_expr(r)),
            Expr::Mul(l, r) => format!("({} * {})", self.format_expr(l), self.format_expr(r)),
            Expr::Div(l, r) => format!("({} / {})", self.format_expr(l), self.format_expr(r)),
            Expr::Eq(l, r) => format!("({} == {})", self.format_expr(l), self.format_expr(r)),
            Expr::Ne(l, r) => format!("({} != {})", self.format_expr(l), self.format_expr(r)),
            Expr::Lt(l, r) => format!("({} < {})", self.format_expr(l), self.format_expr(r)),
            Expr::Le(l, r) => format!("({} <= {})", self.format_expr(l), self.format_expr(r)),
            Expr::Gt(l, r) => format!("({} > {})", self.format_expr(l), self.format_expr(r)),
            Expr::Ge(l, r) => format!("({} >= {})", self.format_expr(l), self.format_expr(r)),
            Expr::Or(l, r) => format!("({} || {})", self.format_expr(l), self.format_expr(r)),
            Expr::And(l, r) => format!("({} && {})", self.format_expr(l), self.format_expr(r)),
            Expr::Not(e) => format!("!{}", self.format_expr(e)),
            Expr::Neg(e) => format!("-{}", self.format_expr(e)),
            Expr::BitNot(e) => format!("~{}", self.format_expr(e)),
            Expr::ListLiteral(elements) => {
                let elements_str = elements
                    .iter()
                    .map(|e| self.format_expr(e))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("[{}]", elements_str)
            }
            Expr::ListIndex(list, index) => {
                format!("{}[{}]", self.format_expr(list), self.format_expr(index))
            }
            Expr::ListLen(list) => {
                format!("{}.len()", self.format_expr(list))
            }
            Expr::FieldAccess(obj, field) => {
                format!("{}.{}", self.format_expr(obj), field)
            }
            Expr::StructInstance(typename, fields) => {
                let fields_str = fields
                    .iter()
                    .map(|(f, v)| format!("{}: {}", f, self.format_expr(v)))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{} {{{}}}", typename, fields_str)
            }
        }
    }
}
