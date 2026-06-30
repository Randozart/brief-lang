use crate::ast::*;

pub struct Desugarer {
    generated_signatures: Vec<Signature>,
    generated_state: Vec<StateDecl>,
    pipe_counter: usize,
}

impl Desugarer {
    pub fn new() -> Self {
        Desugarer {
            generated_signatures: Vec::new(),
            generated_state: Vec::new(),
            pipe_counter: 0,
        }
    }

    fn extract_vars_from_expr(&self, expr: &Expr) -> Vec<String> {
        let mut vars = Vec::new();
        self.collect_vars(expr, &mut vars);
        vars
    }

    fn collect_vars(&self, expr: &Expr, vars: &mut Vec<String>) {
        match expr {
            Expr::Identifier(name) => {
                if !vars.contains(name) {
                    vars.push(name.clone());
                }
            }
            Expr::PriorState(name) => {
                // Don't create state for prior state references - that's just reading
            }
            Expr::Add(l, r) | Expr::Sub(l, r) | Expr::Mul(l, r) | Expr::Div(l, r) => {
                self.collect_vars(l, vars);
                self.collect_vars(r, vars);
            }
            Expr::Eq(l, r)
            | Expr::Ne(l, r)
            | Expr::Lt(l, r)
            | Expr::Le(l, r)
            | Expr::Gt(l, r)
            | Expr::Ge(l, r) => {
                self.collect_vars(l, vars);
                self.collect_vars(r, vars);
            }
            Expr::And(l, r) | Expr::Or(l, r) => {
                self.collect_vars(l, vars);
                self.collect_vars(r, vars);
            }
            Expr::Not(inner) => self.collect_vars(inner, vars),
            Expr::Call(_, args) => {
                // Function calls in postcondition - don't extract as state vars
                for arg in args {
                    self.collect_vars(arg, vars);
                }
            }
            Expr::Bool(_) | Expr::Term | Expr::Integer(_) | Expr::Float(_) | Expr::String(_) => {}
            _ => {}
        }
    }

    fn infer_type_from_expr(&self, expr: &Expr, var_name: &str) -> Type {
        match expr {
            Expr::Identifier(name) if name == var_name => Type::Bool,
            Expr::PriorState(name) if name == var_name => Type::Bool,
            Expr::Eq(l, r)
            | Expr::Ne(l, r)
            | Expr::Lt(l, r)
            | Expr::Le(l, r)
            | Expr::Gt(l, r)
            | Expr::Ge(l, r) => {
                let left_type = self.infer_type_from_expr(l, var_name);
                let right_type = self.infer_type_from_expr(r, var_name);
                if right_type != Type::Bool {
                    right_type
                } else {
                    left_type
                }
            }
            Expr::Add(l, r) | Expr::Sub(l, r) | Expr::Mul(l, r) | Expr::Div(l, r) => {
                let left_type = self.infer_type_from_expr(l, var_name);
                let right_type = self.infer_type_from_expr(r, var_name);
                if right_type != Type::Int {
                    right_type
                } else {
                    left_type
                }
            }
            Expr::And(_, _) | Expr::Or(_, _) => Type::Bool,
            Expr::Not(_) => Type::Bool,
            Expr::Integer(_) => Type::Int,
            Expr::Float(_) => Type::Float,
            Expr::String(_) => Type::String,
            Expr::Bool(_) => Type::Bool,
            Expr::Call(_, args) => {
                for arg in args {
                    let ty = self.infer_type_from_expr(arg, var_name);
                    if ty != Type::Bool {
                        return ty;
                    }
                }
                Type::Bool
            }
            _ => Type::Bool,
        }
    }

    pub fn desugar(&mut self, program: &Program) -> Program {
        let mut items = Vec::new();

        // First pass: collect all existing state declarations, constants, and triggers
        let existing_state: std::collections::HashSet<String> = program
            .items
            .iter()
            .filter_map(|item| match item {
                TopLevel::StateDecl(s) => Some(s.name.clone()),
                TopLevel::Constant(c) => Some(c.name.clone()),
                TopLevel::Trigger(t) => Some(t.name.clone()),
                _ => None,
            })
            .collect();

        let mut struct_defs: std::collections::HashMap<String, &StructDefinition> =
            std::collections::HashMap::new();
        for item in &program.items {
            if let TopLevel::Struct(s) = item {
                struct_defs.insert(s.name.clone(), s);
            }
        }

        // Flatten struct derivation chains: prepend parent fields to child fields.
        // This runs before any item processing so all backends see flat structs.
        let mut flat_structs: std::collections::HashMap<String, Vec<StructField>> =
            std::collections::HashMap::new();

        fn collect_parent_fields<'a>(
            struct_name: &str,
            struct_defs: &std::collections::HashMap<String, &'a StructDefinition>,
            flat_structs: &std::collections::HashMap<String, Vec<StructField>>,
            visited: &mut std::collections::HashSet<String>,
        ) -> Result<Vec<StructField>, String> {
            if !visited.insert(struct_name.to_string()) {
                return Err(format!("Circular derivation detected involving '{}'", struct_name));
            }
            let s = struct_defs.get(struct_name).ok_or_else(|| {
                format!("Parent struct '{}' not found", struct_name)
            })?;
            let parent_fields = if let Some(ref parent) = s.parent {
                let parent_name = match parent {
                    Type::Custom(n) => n.clone(),
                    Type::Applied(n, _) => n.clone(),
                    _ => return Err(format!("Invalid parent type for '{}'", struct_name)),
                };
                collect_parent_fields(&parent_name, struct_defs, flat_structs, visited)?
            } else {
                Vec::new()
            };
            // Check for field collisions between parent and child
            let parent_names: std::collections::HashSet<&str> =
                parent_fields.iter().map(|f| f.name.as_str()).collect();
            for field in &s.fields {
                if parent_names.contains(field.name.as_str()) {
                    return Err(format!(
                        "Field '{}' in '{}' collides with field in parent struct",
                        field.name, struct_name
                    ));
                }
            }
            let mut merged = parent_fields;
            merged.extend(s.fields.clone());
            Ok(merged)
        }

        for item in &program.items {
            if let TopLevel::Struct(s) = item {
                if s.parent.is_some() {
                    let mut visited = std::collections::HashSet::new();
                    match collect_parent_fields(&s.name, &struct_defs, &flat_structs, &mut visited) {
                        Ok(fields) => {
                            flat_structs.insert(s.name.clone(), fields);
                        }
                        Err(e) => {
                            panic!("{}", e);
                        }
                    }
                }
            }
        }

        for item in &program.items {
            match item {
                TopLevel::Transaction(txn) => {
                    // Infer state from postcondition variables
                    let post_vars = self.extract_vars_from_expr(&txn.contract.post_condition);
                    for var_name in post_vars {
                        if !existing_state.contains(&var_name)
                            && !txn.parameters.iter().any(|(n, _)| n == &var_name)
                            && !self
                                .generated_state
                                .iter()
                                .any(|s: &StateDecl| s.name == var_name)
                        {
                            // Infer type from postcondition expression
                            let ty =
                                self.infer_type_from_expr(&txn.contract.post_condition, &var_name);
                            let default_val = match &ty {
                                Type::Int => Expr::Integer(0),
                                Type::Float => Expr::Float(0.0),
                                Type::Bool => Expr::Bool(false),
                                Type::String => Expr::String(String::new()),
                                _ => Expr::Bool(false),
                            };
                            self.generated_state.push(StateDecl {
                                name: var_name,
                                ty,
                                expr: Some(default_val),
                                address: None,
                                bit_range: None,
                                is_override: false,
                                os_mode: false,
                                span: None,
                                attrs: Vec::new(),
                            constraint: None,
                            });
                        }
                    }

                    if txn.is_reactive && self.needs_desugaring(txn) {
                        let (new_txn, sigs, state) = self.desugar_reactive_txn(txn);
                        items.extend(state.into_iter().map(TopLevel::StateDecl));
                        items.extend(sigs.into_iter().map(TopLevel::Signature));
                        items.push(TopLevel::Transaction(
                            self.expand_implicit_terms_txn(&new_txn),
                        ));
                    } else {
                        items.push(TopLevel::Transaction(self.expand_implicit_terms_txn(txn)));
                    }
                }
                TopLevel::Definition(defn) => {
                    items.push(TopLevel::Definition(self.expand_implicit_terms_defn(defn)));
                }
                TopLevel::Struct(s) => {
                    // Use flattened fields if struct has a parent derivation
                    let fields: &[StructField] = flat_structs.get(&s.name)
                        .map(|f| f.as_slice())
                        .unwrap_or(&s.fields);
                    // Only generate state for struct fields if the struct has transactions
                    // (i.e., it's a hardware component, not a pure data type)
                    if !s.transactions.is_empty() {
                        for field in fields {
                            let ty = match &field.ty {
                                Type::Int => Type::Int,
                                Type::Float => Type::Float,
                                Type::Bool => Type::Bool,
                                Type::String => Type::String,
                                other => other.clone(),
                            };
                            let initial_expr = match &ty {
                                Type::Int => Some(Expr::Integer(0)),
                                Type::Float => Some(Expr::Integer(0)),
                                Type::Bool => Some(Expr::Bool(false)),
                                Type::String => Some(Expr::String("".to_string())),
                                Type::Applied(name, _) if name == "List" => {
                                    Some(Expr::ListLiteral(vec![]))
                                }
                                _ => Some(Expr::Bool(false)),
                            };
                            self.generated_state.push(StateDecl {
                        attrs: Vec::new(),
                                name: field.name.clone(),
                                ty,
                                expr: initial_expr,
                                address: None,
                                bit_range: None,
                                is_override: false,
                                os_mode: false,
                                span: None,
                            constraint: None,
                            });
                        }
                    }
                    for txn in &s.transactions {
                        let txn_name = if txn.name.contains('.') {
                            txn.name.clone()
                        } else {
                            format!("{}.{}", s.name, txn.name)
                        };
                        items.push(TopLevel::Transaction(Transaction {
                            name: txn_name,
                            ..txn.clone()
                        }));
                    }
                    items.push(item.clone());
                }
                TopLevel::RStruct(rs) => {
                    // Only generate state for struct fields if the struct has transactions
                    if !rs.transactions.is_empty() {
                        for field in &rs.fields {
                            let ty = match &field.ty {
                                Type::Int => Type::Int,
                                Type::Float => Type::Float,
                                Type::Bool => Type::Bool,
                                Type::String => Type::String,
                                other => other.clone(),
                            };
                            let initial_expr = match &ty {
                                Type::Int => Some(Expr::Integer(0)),
                                Type::Float => Some(Expr::Integer(0)),
                                Type::Bool => Some(Expr::Bool(false)),
                                Type::String => Some(Expr::String("".to_string())),
                                Type::Applied(name, _) if name == "List" => {
                                    Some(Expr::ListLiteral(vec![]))
                                }
                                _ => Some(Expr::Bool(false)),
                            };
                            self.generated_state.push(StateDecl {
                        attrs: Vec::new(),
                                name: field.name.clone(),
                                ty,
                                expr: initial_expr,
                                address: None,
                                bit_range: None,
                                is_override: false,
                                os_mode: false,
                                span: None,
                            constraint: None,
                            });
                        }
                    }
                    for txn in &rs.transactions {
                        let txn_name = if txn.name.contains('.') {
                            txn.name.clone()
                        } else {
                            format!("{}.{}", rs.name, txn.name)
                        };
                        items.push(TopLevel::Transaction(Transaction {
                            name: txn_name,
                            ..txn.clone()
                        }));
                    }
                    items.push(TopLevel::Struct(StructDefinition {
                        name: rs.name.clone(),
                        type_params: Vec::new(),
                        parent: None,
                        fields: rs.fields.clone(),
                        transactions: rs.transactions.clone(),
                        view_html: Some(rs.view_html.clone()),
                        span: rs.span,
                        modifiers: Vec::new(),
                        variants: Vec::new(),
                    }));
                    items.push(TopLevel::RenderBlock(RenderBlock {
                        struct_name: rs.name.clone(),
                        view_html: rs.view_html.clone(),
                        span: rs.span,
                    }));
                }
                TopLevel::StateDecl(state) => {
                    let elem_type = Self::resolve_element_type(&state.ty);
                    if let Some(expr) = &state.expr {
                        let new_expr = self.transform_object_literals(
                            expr.clone(),
                            &struct_defs,
                            elem_type.as_deref(),
                        );
                        items.push(TopLevel::StateDecl(StateDecl {
                            expr: Some(new_expr),
                            constraint: None,
                            ..state.clone()
                        }));
                    } else {
                        items.push(item.clone());
                    }
                }
                _ => {
                    items.push(self.desugar_toplevel(item));
                }
            }
        }

        if !self.generated_state.is_empty() {
            for state in self.generated_state.drain(..) {
                if !items.iter().any(|i| {
                    if let TopLevel::StateDecl(s) = i {
                        s.name == state.name
                    } else {
                        false
                    }
                }) {
                    items.insert(0, TopLevel::StateDecl(state));
                }
            }
        }

        if !self.generated_signatures.is_empty() {
            for sig in self.generated_signatures.drain(..) {
                if !items.iter().any(|i| {
                    if let TopLevel::Signature(s) = i {
                        s.name == sig.name
                    } else {
                        false
                    }
                }) {
                    items.insert(0, TopLevel::Signature(sig));
                }
            }
        }

        // Final pass: desugar any remaining pipe chains in the program
        let items = items.into_iter().map(|item| self.desugar_toplevel(&item)).collect();

        Program {
                    attrs: Vec::new(),
            items,
            comments: program.comments.clone(),
            reactor_speed: program.reactor_speed,
            ffi: program.ffi.clone(),
            strict_mode: StrictMode::Off,
            dispatch_mode: program.dispatch_mode,
            exit_condition: program.exit_condition.clone().map(|e| Box::new(self.desugar_expr(*e))),
            out_pragmas: program.out_pragmas.clone(),
            default_sig_modifier: program.default_sig_modifier.clone(),
                watchdog_defaults: (None, None),
        }
    }

    /// Desugar pipe chains in a single TopLevel item.
    fn desugar_toplevel(&mut self, item: &TopLevel) -> TopLevel {
        match item {
            TopLevel::Transaction(txn) => {
                let mut new_txn = txn.clone();
                new_txn.body = self.desugar_body(&txn.body);
                new_txn.contract = self.desugar_contract(&txn.contract);
                TopLevel::Transaction(new_txn)
            }
            TopLevel::Definition(defn) => {
                let mut new_defn = defn.clone();
                new_defn.body = self.desugar_body(&defn.body);
                new_defn.contract = self.desugar_contract(&defn.contract);
                TopLevel::Definition(new_defn)
            }
            TopLevel::StateDecl(state) => {
                let mut new_state = state.clone();
                new_state.expr = state.expr.as_ref().map(|e| self.desugar_expr(e.clone()));
                TopLevel::StateDecl(new_state)
            }
            TopLevel::Constant(c) => {
                let mut new_c = c.clone();
                new_c.expr = self.desugar_expr(c.expr.clone());
                TopLevel::Constant(new_c)
            }
            TopLevel::Signature(_sig) => {
                // Signature has no expression field to desugar
                item.clone()
            }
            TopLevel::Trigger(trig) => {
                let mut new_trig = trig.clone();
                new_trig.condition = trig.condition.as_ref().map(|e| self.desugar_expr(e.clone()));
                TopLevel::Trigger(new_trig)
            }
            TopLevel::Struct(s) => {
                let mut new_s = s.clone();
                new_s.transactions = s.transactions.iter().map(|txn| {
                    let mut new_txn = txn.clone();
                    new_txn.body = self.desugar_body(&txn.body);
                    new_txn.contract = self.desugar_contract(&txn.contract);
                    new_txn
                }).collect();
                TopLevel::Struct(new_s)
            }
            TopLevel::Cell(cell) => {
                let mut c = cell.as_ref().clone();
                c.transactions = c.transactions.into_iter()
                    .map(|t| {
                        let mut new_t = t.clone();
                        new_t.body = self.desugar_body(&t.body);
                        new_t.contract = self.desugar_contract(&t.contract);
                        new_t
                    })
                    .collect();
                c.definitions = c.definitions.into_iter()
                    .map(|d| {
                        let mut new_d = d.clone();
                        new_d.body = self.desugar_body(&d.body);
                        new_d.contract = self.desugar_contract(&d.contract);
                        new_d
                    })
                    .collect();
                c.internal_triggers = c.internal_triggers.into_iter()
                    .map(|t| TriggerDeclaration {
                        condition: t.condition.map(|e| self.desugar_expr(e)),
                        ..t
                    })
                    .collect();
                TopLevel::Cell(Box::new(c))
            }
            other => other.clone(),
        }
    }

    fn needs_desugaring(&self, txn: &Transaction) -> bool {
        if let Expr::Not(inner) = &txn.contract.pre_condition {
            if let Expr::Identifier(name) = &**inner {
                if name == "done"
                    && matches!(&txn.contract.post_condition, Expr::Identifier(n) if n == "done")
                {
                    return self.has_term_with_expression(&txn.body);
                }
            }
        }
        false
    }

    fn has_term_with_expression(&self, body: &[Statement]) -> bool {
        for stmt in body {
            if let Statement::Term { values: outputs, .. } | Statement::TermBang { values: outputs, .. } = stmt {
                if let Some(Some(_)) = outputs.first() {
                    return true;
                }
            }
        }
        false
    }

    fn desugar_reactive_txn(
        &mut self,
        txn: &Transaction,
    ) -> (Transaction, Vec<Signature>, Vec<StateDecl>) {
        let mut sigs = Vec::new();
        let mut state = Vec::new();

        state.push(StateDecl {
                    attrs: Vec::new(),
            name: "done".to_string(),
            ty: Type::Bool,
            expr: Some(Expr::Bool(false)),
            address: None,
            bit_range: None,
            is_override: false,
            os_mode: false,
            span: None,
            constraint: None,
        });

        let mut new_body_items = Vec::new();
        for stmt in &txn.body {
            if let Statement::Term { values: outputs, .. } = stmt {
                if let Some(Some(expr)) = outputs.first() {
                    let fn_sigs = self.extract_function_call(expr);
                    sigs.extend(fn_sigs);

                    new_body_items.push(Statement::Expression(expr.clone()));
                    new_body_items.push(Statement::Assignment {
                        lhs: Expr::OwnedRef("done".to_string()),
                        expr: Expr::Bool(true),
                        timeout: None,
                        modifiers: vec![],
                    });
                    new_body_items.push(Statement::Term { values: vec![], modifiers: vec![], swan_song: None });
                    continue;
                }
            }
            new_body_items.push(stmt.clone());
        }

        let contract = Contract {
            pre_condition: Expr::Not(Box::new(Expr::Identifier("done".to_string()))),
            post_condition: Expr::Identifier("done".to_string()),
            watchdog: None,
            span: None,
        };

        let dependencies = contract
            .pre_condition
            .extract_dependencies()
            .into_iter()
            .collect();

        let new_txn = Transaction {
                    annotations: vec![],
            is_async: txn.is_async,
            is_reactive: txn.is_reactive,
            name: txn.name.clone(),
            parameters: txn.parameters.clone(),
            contract,
            body: new_body_items,
            reactor_speed: txn.reactor_speed,
            span: None,
            is_lambda: txn.is_lambda,
            dependencies,
            modifiers: Vec::new(),
            variant_bodies: Vec::new(),
                 outputs: Vec::new(),
         output_type: None,
     };

        (new_txn, sigs, state)
    }

    fn extract_function_call(&mut self, expr: &Expr) -> Vec<Signature> {
        if let Expr::Call(name, args) = expr {
            let input_types: Vec<Type> =
                args.iter().map(|_| Type::Custom("_".to_string())).collect();

            if !self.generated_signatures.iter().any(|s| s.name == *name) {
                let sig = Signature {
                    name: name.clone(),
                    params: input_types.iter().map(|t| ("".to_string(), t.clone())).collect(),
                    result_type: ResultType::Projection(vec![Type::Bool]),
                    source: None,
                    alias: None,
                    bound_defn: None,
                    modifier: None,
                    output_type: None,
                };
                self.generated_signatures.push(Signature {
                    name: name.clone(),
                    params: input_types.into_iter().map(|t| ("".to_string(), t)).collect(),
                    result_type: ResultType::TrueAssertion,
                    source: None,
                    alias: None,
                    bound_defn: None,
                    modifier: None,
                    output_type: None,
                });
                return vec![sig];
            }
        }
        vec![]
    }

    /// Expand implicit term statements:
    /// - `term;` with no outputs becomes `term true;` when the postcondition is a Bool expression
    fn expand_implicit_terms_defn(&mut self, defn: &Definition) -> Definition {
        let postcond_is_bool = matches!(defn.contract.post_condition, Expr::Bool(_));

        let new_body: Vec<Statement> = defn
            .body
            .iter()
            .map(|stmt| {
            if let Statement::Term { values: outputs, .. } | Statement::TermBang { values: outputs, .. } = stmt {
                    if outputs.is_empty() && postcond_is_bool {
                        return Statement::Term { values: vec![Some(Expr::Bool(true))], modifiers: vec![], swan_song: None }
                    }
                }
                stmt.clone()
            })
            .collect();

        Definition {
            body: new_body,
            ..defn.clone()
        }
    }

    fn expand_implicit_terms_txn(&mut self, txn: &Transaction) -> Transaction {
        Transaction {
            body: txn.body.clone(),
            ..txn.clone()
        }
    }
    fn resolve_element_type(ty: &Type) -> Option<String> {
        match ty {
            Type::Applied(name, inner) if name == "List" || name == "Set" => {
                if let Some(inner_ty) = inner.first() {
                    match inner_ty {
                        Type::Custom(n) => Some(n.clone()),
                        _ => None,
                    }
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn transform_object_literals(
        &self,
        expr: Expr,
        struct_defs: &std::collections::HashMap<String, &StructDefinition>,
        element_type: Option<&str>,
    ) -> Expr {
        match expr {
            Expr::ObjectLiteral(fields) => {
                if let Some(type_name) = element_type {
                    if let Some(struct_def) = struct_defs.get(type_name) {
                        let mut all_fields = Vec::new();
                        for struct_field in &struct_def.fields {
                            let value = fields
                                .iter()
                                .find(|(name, _)| name == &struct_field.name)
                                .map(|(_, v)| v.clone())
                                .unwrap_or_else(|| {
                                    struct_field.default.clone().unwrap_or(Expr::Integer(0))
                                });
                            all_fields.push((struct_field.name.clone(), value));
                        }
                        Expr::StructInstance(type_name.to_string(), all_fields)
                    } else {
                        Expr::ObjectLiteral(fields)
                    }
                } else {
                    Expr::ObjectLiteral(fields)
                }
            }
            Expr::ListLiteral(elements) => {
                let new_elements = elements
                    .into_iter()
                    .map(|e| self.transform_object_literals(e, struct_defs, element_type))
                    .collect();
                Expr::ListLiteral(new_elements)
            }
            other => other,
        }
    }

    // ── Pipe Chain Desugaring ──────────────────────────────────────────

    /// Desugar `Expr::PipeChain` into `Expr::Block` with let-bound temporaries.
    fn desugar_pipe_chain(&mut self, pipe: &crate::ast::PipeChain) -> Expr {
        let crate::ast::PipeChain { initial, steps } = pipe;
        let mut stmts: Vec<Statement> = Vec::with_capacity(steps.len() + 1);

        // First binding: __pipe_0 = <initial expression>
        let p0_name = "__pipe_0".to_string();
        stmts.push(Statement::Let {
            name: p0_name,
            ty: None,
            expr: Some(self.desugar_expr(*initial.clone())),
            address: None,
            address_expr: None,
            bit_range: None,
            constraint: None,
            is_override: false,
            modifiers: Vec::new(),
        });

        // Subsequent bindings: __pipe_i = <target>(__pipe_{pos-1-skip})
        for (i, step) in steps.iter().enumerate() {
            let pos = i + 1; // 1-indexed pipe position
            assert!(
                step.skip <= pos - 1,
                "PipeStep skip ({}) exceeds pipeline position ({}): \
                 cannot skip past the initial value",
                step.skip, pos
            );
            let read_idx = pos - 1 - step.skip;
            let read_name = format!("__pipe_{}", read_idx);
            let read_expr = Expr::Identifier(read_name);

            // Build the call expression: prepend pipeline value as first arg
            let call_expr = self.prepend_pipeline_arg(&step.target, read_expr);

            let binding_name = format!("__pipe_{}", pos);
            stmts.push(Statement::Let {
                name: binding_name,
                ty: None,
                expr: Some(self.desugar_expr(call_expr)),
                address: None,
                address_expr: None,
                bit_range: None,
                constraint: None,
                is_override: false,
                modifiers: Vec::new(),
            });
        }

        let final_expr = if steps.is_empty() {
            self.desugar_expr(*initial.clone())
        } else {
            Expr::Identifier(format!("__pipe_{}", steps.len()))
        };

        Expr::Block(stmts, Box::new(final_expr))
    }

    /// Prepend the pipeline value as the first argument to a call expression.
    /// If target is a bare `Identifier`, wrap it as `f(pipeline_val)`.
    fn prepend_pipeline_arg(&self, target: &Expr, pipeline_val: Expr) -> Expr {
        match target {
            Expr::Call(name, args) => {
                let mut new_args = vec![pipeline_val];
                new_args.extend(args.iter().cloned());
                Expr::Call(name.clone(), new_args)
            }
            Expr::CellCall(callee, args) => {
                let mut new_args = vec![pipeline_val];
                new_args.extend(args.iter().cloned());
                Expr::CellCall(callee.clone(), new_args)
            }
            Expr::Identifier(name) => {
                // Auto-wrap bare identifier as function call
                Expr::Call(name.clone(), vec![pipeline_val])
            }
            other => {
                // Non-callable: wrap as generic call for the typechecker to reject
                // This should be a parse error, but belt-and-suspenders:
                // also emit Call so it survives to typechecking.
                Expr::Identifier("__pipe_error_non_callable".to_string())
            }
        }
    }

    /// Recursively desugar pipe chains in an expression tree.
    fn desugar_expr(&mut self, expr: Expr) -> Expr {
        match expr {
            Expr::PipeChain(pc) => {
                // Desugar the pipe chain itself
                self.desugar_pipe_chain(&pc)
            }
            // Binary ops — recurse into children
            Expr::Add(l, r) => Expr::Add(
                Box::new(self.desugar_expr(*l)),
                Box::new(self.desugar_expr(*r)),
            ),
            Expr::Sub(l, r) => Expr::Sub(
                Box::new(self.desugar_expr(*l)),
                Box::new(self.desugar_expr(*r)),
            ),
            Expr::Mul(l, r) => Expr::Mul(
                Box::new(self.desugar_expr(*l)),
                Box::new(self.desugar_expr(*r)),
            ),
            Expr::Div(l, r) => Expr::Div(
                Box::new(self.desugar_expr(*l)),
                Box::new(self.desugar_expr(*r)),
            ),
            Expr::Mod(l, r) => Expr::Mod(
                Box::new(self.desugar_expr(*l)),
                Box::new(self.desugar_expr(*r)),
            ),
            Expr::Eq(l, r) => Expr::Eq(
                Box::new(self.desugar_expr(*l)),
                Box::new(self.desugar_expr(*r)),
            ),
            Expr::Ne(l, r) => Expr::Ne(
                Box::new(self.desugar_expr(*l)),
                Box::new(self.desugar_expr(*r)),
            ),
            Expr::Lt(l, r) => Expr::Lt(
                Box::new(self.desugar_expr(*l)),
                Box::new(self.desugar_expr(*r)),
            ),
            Expr::Le(l, r) => Expr::Le(
                Box::new(self.desugar_expr(*l)),
                Box::new(self.desugar_expr(*r)),
            ),
            Expr::Gt(l, r) => Expr::Gt(
                Box::new(self.desugar_expr(*l)),
                Box::new(self.desugar_expr(*r)),
            ),
            Expr::Ge(l, r) => Expr::Ge(
                Box::new(self.desugar_expr(*l)),
                Box::new(self.desugar_expr(*r)),
            ),
            Expr::And(l, r) => Expr::And(
                Box::new(self.desugar_expr(*l)),
                Box::new(self.desugar_expr(*r)),
            ),
            Expr::Or(l, r) => Expr::Or(
                Box::new(self.desugar_expr(*l)),
                Box::new(self.desugar_expr(*r)),
            ),
            Expr::Concat(l, r) => Expr::Concat(
                Box::new(self.desugar_expr(*l)),
                Box::new(self.desugar_expr(*r)),
            ),
            Expr::BitAnd(l, r) => Expr::BitAnd(
                Box::new(self.desugar_expr(*l)),
                Box::new(self.desugar_expr(*r)),
            ),
            Expr::BitOr(l, r) => Expr::BitOr(
                Box::new(self.desugar_expr(*l)),
                Box::new(self.desugar_expr(*r)),
            ),
            Expr::BitXor(l, r) => Expr::BitXor(
                Box::new(self.desugar_expr(*l)),
                Box::new(self.desugar_expr(*r)),
            ),
            Expr::Shl(l, r) => Expr::Shl(
                Box::new(self.desugar_expr(*l)),
                Box::new(self.desugar_expr(*r)),
            ),
            Expr::Shr(l, r) => Expr::Shr(
                Box::new(self.desugar_expr(*l)),
                Box::new(self.desugar_expr(*r)),
            ),
            // Unary ops
            Expr::Not(e) => Expr::Not(Box::new(self.desugar_expr(*e))),
            Expr::Neg(e) => Expr::Neg(Box::new(self.desugar_expr(*e))),
            Expr::BitNot(e) => Expr::BitNot(Box::new(self.desugar_expr(*e))),
            // Calls — recurse into args and callee
            Expr::Call(name, args) => Expr::Call(
                name,
                args.into_iter().map(|a| self.desugar_expr(a)).collect(),
            ),
            Expr::CellCall(callee, args) => {
                let callee = Box::new(self.desugar_expr(*callee));
                let args = args.into_iter().map(|a| self.desugar_expr(a)).collect();
                Expr::CellCall(callee, args)
            }
            Expr::IntrinsicCall { intrinsic, args } => Expr::IntrinsicCall {
                intrinsic,
                args: args.into_iter().map(|a| self.desugar_expr(a)).collect(),
            },
            // Lists, tuples, maps, sets
            Expr::ListLiteral(items) => {
                Expr::ListLiteral(items.into_iter().map(|i| self.desugar_expr(i)).collect())
            }
            Expr::Tuple(items) => {
                Expr::Tuple(items.into_iter().map(|i| self.desugar_expr(i)).collect())
            }
            Expr::MapLiteral(pairs) => Expr::MapLiteral(
                pairs
                    .into_iter()
                    .map(|(k, v)| (self.desugar_expr(k), self.desugar_expr(v)))
                    .collect(),
            ),
            Expr::SetLiteral(items) => {
                Expr::SetLiteral(items.into_iter().map(|i| self.desugar_expr(i)).collect())
            }
            // Blocks — recurse into statements and trailing expr
            Expr::Block(stmts, trailing) => {
                let new_stmts = stmts.into_iter().map(|s| self.desugar_stmt(s)).collect();
                Expr::Block(new_stmts, Box::new(self.desugar_expr(*trailing)))
            }
            // Field access, list index — recurse
            Expr::FieldAccess(obj, name) => {
                Expr::FieldAccess(Box::new(self.desugar_expr(*obj)), name)
            }
            Expr::ListIndex(obj, idx) => Expr::ListIndex(
                Box::new(self.desugar_expr(*obj)),
                Box::new(self.desugar_expr(*idx)),
            ),
            Expr::StructInstance(name, fields) => Expr::StructInstance(
                name,
                fields
                    .into_iter()
                    .map(|(n, v)| (n, self.desugar_expr(v)))
                    .collect(),
            ),
            Expr::ObjectLiteral(fields) => Expr::ObjectLiteral(
                fields
                    .into_iter()
                    .map(|(n, v)| (n, self.desugar_expr(v)))
                    .collect(),
            ),
            Expr::Slice {
                value,
                start,
                end,
                stride,
                mask,
            } => Expr::Slice {
                value: Box::new(self.desugar_expr(*value)),
                start: start.map(|s| Box::new(self.desugar_expr(*s))),
                end: end.map(|e| Box::new(self.desugar_expr(*e))),
                stride: stride.map(|s| Box::new(self.desugar_expr(*s))),
                mask: mask.map(|m| Box::new(self.desugar_expr(*m))),
            },
            Expr::MultiSlice { value, ops } => Expr::MultiSlice {
                value: Box::new(self.desugar_expr(*value)),
                ops,
            },
            Expr::Match { value, arms } => {
                let new_arms = arms
                    .into_iter()
                    .map(|arm| MatchArm {
                        pattern: arm.pattern,
                        guard: arm.guard.map(|g| Box::new(self.desugar_expr(*g))),
                        body: Box::new(self.desugar_expr(*arm.body)),
                    })
                    .collect();
                Expr::Match {
                    value: Box::new(self.desugar_expr(*value)),
                    arms: new_arms,
                }
            }
            Expr::Cast(e, ty) => Expr::Cast(Box::new(self.desugar_expr(*e)), ty),
            Expr::IsType(e, target) => Expr::IsType(Box::new(self.desugar_expr(*e)), target),
            Expr::FromCheck(e, ty) => Expr::FromCheck(Box::new(self.desugar_expr(*e)), ty),
            Expr::Like(l, r) => Expr::Like(
                Box::new(self.desugar_expr(*l)),
                Box::new(self.desugar_expr(*r)),
            ),
            Expr::Projection { source, target } => Expr::Projection {
                source: Box::new(self.desugar_expr(*source)),
                target,
            },
            Expr::TemplateCall {
                name,
                args,
                block,
                span,
            } => Expr::TemplateCall {
                name,
                args: args.into_iter().map(|a| self.desugar_expr(a)).collect(),
                block,
                span,
            },
            Expr::MacroCall {
                name,
                args,
                block,
                span,
            } => Expr::MacroCall {
                name,
                args: args.into_iter().map(|a| self.desugar_expr(a)).collect(),
                block,
                span,
            },
            Expr::SubtypeProjection { source, ops } => Expr::SubtypeProjection {
                source: Box::new(self.desugar_expr(*source)),
                ops,
            },
            // Arrow ops — recurse
            Expr::ArrowMut {
                dir,
                target,
                index,
                value,
            } => Expr::ArrowMut {
                dir,
                target: Box::new(self.desugar_expr(*target)),
                index: Box::new(self.desugar_expr(*index)),
                value: value.map(|v| Box::new(self.desugar_expr(*v))),
            },
            Expr::ArrowDiscard { target, index } => Expr::ArrowDiscard {
                target: Box::new(self.desugar_expr(*target)),
                index: Box::new(self.desugar_expr(*index)),
            },
            Expr::ArrowTransfer {
                dest,
                source,
                filter,
            } => Expr::ArrowTransfer {
                dest: Box::new(self.desugar_expr(*dest)),
                source: Box::new(self.desugar_expr(*source)),
                filter: filter.map(|f| Box::new(self.desugar_expr(*f))),
            },
            Expr::QuoteBlock {
                statements,
                trailing_expr,
            } => Expr::QuoteBlock {
                statements: statements.into_iter().map(|s| self.desugar_stmt(s)).collect(),
                trailing_expr: trailing_expr.map(|e| Box::new(self.desugar_expr(*e))),
            },
            Expr::InterpolateExpr(e) => {
                Expr::InterpolateExpr(Box::new(self.desugar_expr(*e)))
            }
            Expr::PatternMatch {
                value,
                variant,
                fields,
            } => Expr::PatternMatch {
                value: Box::new(self.desugar_expr(*value)),
                variant,
                fields,
            },
            Expr::TupleDestructure(names, e) => {
                Expr::TupleDestructure(names, Box::new(self.desugar_expr(*e)))
            }
            Expr::SigCall { modifier, expr } => Expr::SigCall {
                modifier,
                expr: Box::new(self.desugar_expr(*expr)),
            },
            // Pattern B variants — recurse into inner expressions
            Expr::BinaryOp(bop) => Expr::BinaryOp(Box::new(crate::features::binary_op::BinaryOpExpr {
                kind: bop.kind,
                left: Box::new(self.desugar_expr(*bop.left)),
                right: Box::new(self.desugar_expr(*bop.right)),
            })),
            Expr::UnaryOp(uop) => {
                Expr::UnaryOp(Box::new(crate::features::unary_op::UnaryOpExpr {
                    kind: uop.kind,
                    operand: Box::new(self.desugar_expr(*uop.operand)),
                }))
            }
            Expr::CallExpr(ce) => {
                let crate::features::call::CallExpr { name, args } = ce;
                Expr::CallExpr(crate::features::call::CallExpr {
                    name,
                    args: args.into_iter().map(|a| self.desugar_expr(a)).collect(),
                })
            }
            // Pass through: no child expressions with pipe chains
            other => other,
        }
    }

    /// Recursively desugar pipe chains in statements.
    fn desugar_stmt(&mut self, stmt: Statement) -> Statement {
        match stmt {
            Statement::Let {
                name,
                ty,
                expr,
                address,
                address_expr,
                bit_range,
                constraint,
                is_override,
                modifiers,
            } => Statement::Let {
                name,
                ty,
                expr: expr.map(|e| self.desugar_expr(e)),
                address,
                address_expr: address_expr.map(|a| Box::new(self.desugar_expr(*a))),
                bit_range,
                constraint: constraint.map(|c| Box::new(self.desugar_expr(*c))),
                is_override,
                modifiers,
            },
            Statement::Assignment {
                lhs,
                expr,
                timeout,
                modifiers,
            } => Statement::Assignment {
                lhs: self.desugar_expr(lhs),
                expr: self.desugar_expr(expr),
                timeout,
                modifiers,
            },
            Statement::Expression(e) => Statement::Expression(self.desugar_expr(e)),
            Statement::Term {
                values,
                swan_song,
                modifiers,
            } => Statement::Term {
                values: values
                    .into_iter()
                    .map(|v| v.map(|e| self.desugar_expr(e)))
                    .collect(),
                swan_song: swan_song.map(|s| Box::new(self.desugar_stmt(*s))),
                modifiers,
            },
            Statement::TermBang {
                values,
                swan_song,
                modifiers,
            } => Statement::TermBang {
                values: values
                    .into_iter()
                    .map(|v| v.map(|e| self.desugar_expr(e)))
                    .collect(),
                swan_song: swan_song.map(|s| Box::new(self.desugar_stmt(*s))),
                modifiers,
            },
            Statement::Guarded {
                condition,
                statements,
            } => Statement::Guarded {
                condition: self.desugar_expr(condition),
                statements: statements.into_iter().map(|s| self.desugar_stmt(s)).collect(),
            },
            Statement::Escape(e) => Statement::Escape(e.map(|e| self.desugar_expr(e))),
            Statement::SyncBlock { body } => {
                Statement::SyncBlock { body: body.into_iter().map(|s| self.desugar_stmt(s)).collect() }
            }
            Statement::Foreach {
                item,
                list,
                body,
                modifiers,
            } => Statement::Foreach {
                item,
                list: Box::new(self.desugar_expr(*list)),
                body: body.into_iter().map(|s| self.desugar_stmt(s)).collect(),
                modifiers,
            },
            Statement::Unification {
                name,
                variant,
                fields,
                expr,
            } => Statement::Unification {
                name,
                variant,
                fields,
                expr: self.desugar_expr(expr),
            },
            Statement::LocalTrigger {
                name,
                ty,
                expr,
                span,
            } => Statement::LocalTrigger {
                name,
                ty,
                expr: expr.map(|e| self.desugar_expr(e)),
                span,
            },
            Statement::OnExit { body, span } => {
                Statement::OnExit { body: body.into_iter().map(|s| self.desugar_stmt(s)).collect(), span }
            }
            Statement::Oracle { handler, body, span } => Statement::Oracle {
                handler: handler.into_iter().map(|s| self.desugar_stmt(s)).collect(),
                body: body.into_iter().map(|s| self.desugar_stmt(s)).collect(),
                span,
            },
            Statement::Await { expr, modifiers } => Statement::Await {
                expr: self.desugar_expr(expr),
                modifiers,
            },
            Statement::Async { body, modifiers } => Statement::Async {
                body: Box::new(self.desugar_stmt(*body)),
                modifiers,
            },
            Statement::AsyncAwait { body, lhs, modifiers } => Statement::AsyncAwait {
                body: Box::new(self.desugar_stmt(*body)),
                lhs,
                modifiers,
            },
            Statement::InlineAsm { .. } | Statement::Alka(_) => stmt,
            Statement::TrgBinding { name, ty, instance, port, modifiers } => Statement::TrgBinding {
                name,
                ty,
                instance: self.desugar_expr(instance),
                port,
                modifiers,
            },
        }
    }

    /// Desugar pipe chains in contracts (pre/post conditions).
    fn desugar_contract(&mut self, contract: &Contract) -> Contract {
        Contract {
            pre_condition: self.desugar_expr(contract.pre_condition.clone()),
            post_condition: self.desugar_expr(contract.post_condition.clone()),
            watchdog: contract.watchdog.clone(),
            span: contract.span,
        }
    }

    /// Desugar pipe chains in a block of statements (e.g. txn body, defn body).
    fn desugar_body(&mut self, body: &[Statement]) -> Vec<Statement> {
        body.iter().map(|s| self.desugar_stmt(s.clone())).collect()
    }
}

impl Default for Desugarer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_implicit_term_true_in_defn() {
        let defn = Definition {
            name: "test".to_string(),
            type_params: vec![],
            parameters: vec![("x".to_string(), Type::Int)],
            outputs: vec![],
            output_type: None,
            output_names: vec![],
            contract: Contract {
                pre_condition: Expr::Bool(true),
                post_condition: Expr::Bool(true),
                watchdog: None,
                span: None,
            },
            body: vec![Statement::Term { values: vec![], modifiers: vec![], swan_song: None }],
            is_lambda: false,
            annotations: vec![],
            modifiers: vec![],
            variant_bodies: vec![],
        };

        let mut desugarer = Desugarer::new();
        let result = desugarer.expand_implicit_terms_defn(&defn);

        if let Statement::Term { values: outputs, .. } = &result.body[0] {
            assert_eq!(outputs.len(), 1, "Should have 1 output after desugaring");
            if let Some(Expr::Bool(true)) = &outputs[0] {
                println!("✓ Implicit term true correctly added");
            } else {
                panic!("Expected Bool(true)");
            }
        } else {
            panic!("Expected Term statement");
        }
    }

    #[test]
    fn test_expand_implicit_term_true_in_txn() {
        let txn = Transaction {
            is_async: false,
            is_reactive: false,
            name: "test".to_string(),
            parameters: vec![],
            contract: Contract::new(Expr::Bool(true), Expr::Bool(true)),
            body: vec![Statement::Term { values: vec![], modifiers: vec![], swan_song: None }],
            reactor_speed: None,
            span: None,
            is_lambda: false,
            dependencies: vec![],
            annotations: vec![],
            modifiers: vec![],
            variant_bodies: vec![],
                 outputs: Vec::new(),
         output_type: None,
     };

        let mut desugarer = Desugarer::new();
        let result = desugarer.expand_implicit_terms_txn(&txn);

        if let Statement::Term { values: outputs, .. } = &result.body[0] {
            assert!(outputs.is_empty(), "transaction term should NOT be expanded");
        } else {
            panic!("Expected Term statement");
        }
    }

    #[test]
    fn test_expansion_preserves_nontrivial_body() {
        let defn = Definition {
            name: "test".to_string(),
            type_params: vec![],
            parameters: vec![("x".to_string(), Type::Int)],
            outputs: vec![],
            output_type: None,
            output_names: vec![],
            contract: Contract {
                pre_condition: Expr::Bool(true),
                post_condition: Expr::Integer(42),
                watchdog: None,
                span: None,
            },
            body: vec![Statement::Term { values: vec![Some(Expr::Integer(1))], modifiers: vec![], swan_song: None }],
            is_lambda: true,
            annotations: vec![],
            modifiers: vec![],
            variant_bodies: vec![],
        };

        let mut desugarer = Desugarer::new();
        let result = desugarer.expand_implicit_terms_defn(&defn);

        assert!(!result.body.is_empty(), "Should preserve existing body");
    }

    // ── Pipe Chain Desugaring Tests ──────────────────────────────────

    #[test]
    fn test_desugar_basic_pipe() {
        // x |> f() → { let __pipe_0 = x; let __pipe_1 = f(__pipe_0); __pipe_1 }
        let pipe = PipeChain {
            initial: Box::new(Expr::Identifier("x".to_string())),
            steps: vec![PipeStep {
                target: Box::new(Expr::Call("f".to_string(), vec![])),
                skip: 0,
            }],
        };
        let mut desugarer = Desugarer::new();
        let result = desugarer.desugar_expr(Expr::PipeChain(pipe));

        // Should desugar to a Block expression
        match &result {
            Expr::Block(stmts, trailing) => {
                assert_eq!(stmts.len(), 2, "Should have 2 let statements");
                // First let: __pipe_0 = x
                if let Statement::Let { name, expr: Some(expr), .. } = &stmts[0] {
                    assert_eq!(name, "__pipe_0");
                    assert_eq!(expr, &Expr::Identifier("x".to_string()));
                } else {
                    panic!("Expected Let statement");
                }
                // Second let: __pipe_1 = f(__pipe_0)
                if let Statement::Let { name, expr: Some(expr), .. } = &stmts[1] {
                    assert_eq!(name, "__pipe_1");
                    assert!(matches!(expr, Expr::Call(name, args)
                        if name == "f" && args.len() == 1
                        && args[0] == Expr::Identifier("__pipe_0".to_string())
                    ));
                } else {
                    panic!("Expected Let statement");
                }
                // Trailing expr should be __pipe_1
                assert_eq!(trailing.as_ref(), &Expr::Identifier("__pipe_1".to_string()));
            }
            _ => panic!("Expected Block, got {:?}", result),
        }
    }

    #[test]
    fn test_desugar_pipe_chaining() {
        // x |> f() |> g() → two let bindings
        let pipe = PipeChain {
            initial: Box::new(Expr::Identifier("x".to_string())),
            steps: vec![
                PipeStep { target: Box::new(Expr::Call("f".to_string(), vec![])), skip: 0 },
                PipeStep { target: Box::new(Expr::Call("g".to_string(), vec![])), skip: 0 },
            ],
        };
        let mut desugarer = Desugarer::new();
        let result = desugarer.desugar_expr(Expr::PipeChain(pipe));

        match &result {
            Expr::Block(stmts, trailing) => {
                assert_eq!(stmts.len(), 3);
                // __pipe_0 = x
                // __pipe_1 = f(__pipe_0)
                if let Statement::Let { name, expr: Some(expr), .. } = &stmts[1] {
                    assert_eq!(name, "__pipe_1");
                    assert!(matches!(expr, Expr::Call(name, args)
                        if name == "f" && args.len() == 1
                        && args[0] == Expr::Identifier("__pipe_0".to_string())
                    ));
                }
                // __pipe_2 = g(__pipe_1) — adjacent read
                if let Statement::Let { name, expr: Some(expr), .. } = &stmts[2] {
                    assert_eq!(name, "__pipe_2");
                    assert!(matches!(expr, Expr::Call(name, args)
                        if name == "g" && args.len() == 1
                        && args[0] == Expr::Identifier("__pipe_1".to_string())
                    ));
                }
                assert_eq!(trailing.as_ref(), &Expr::Identifier("__pipe_2".to_string()));
            }
            _ => panic!("Expected Block"),
        }
    }

    #[test]
    fn test_desugar_pipe_dot_skip() {
        // x |> f() .|> g() → second binding reads from __pipe_0 (skip=1)
        let pipe = PipeChain {
            initial: Box::new(Expr::Identifier("x".to_string())),
            steps: vec![
                PipeStep { target: Box::new(Expr::Call("f".to_string(), vec![])), skip: 0 },
                PipeStep { target: Box::new(Expr::Call("g".to_string(), vec![])), skip: 1 },
            ],
        };
        let mut desugarer = Desugarer::new();
        let result = desugarer.desugar_expr(Expr::PipeChain(pipe));

        match &result {
            Expr::Block(stmts, trailing) => {
                assert_eq!(stmts.len(), 3);
                // __pipe_1 = f(__pipe_0) — step 0, adjacent
                if let Statement::Let { name, expr: Some(expr), .. } = &stmts[1] {
                    assert_eq!(name, "__pipe_1");
                    assert!(matches!(expr, Expr::Call(name, args)
                        if name == "f" && args[0] == Expr::Identifier("__pipe_0".to_string())
                    ));
                }
                // __pipe_2 = g(__pipe_0) — step 1, skip=1 reads __pipe_{2-1-1}=__pipe_0
                if let Statement::Let { name, expr: Some(expr), .. } = &stmts[2] {
                    assert_eq!(name, "__pipe_2");
                    assert!(matches!(expr, Expr::Call(name, args)
                        if name == "g" && args.len() == 1
                        && args[0] == Expr::Identifier("__pipe_0".to_string())
                    ));
                }
                assert_eq!(trailing.as_ref(), &Expr::Identifier("__pipe_2".to_string()));
            }
            _ => panic!("Expected Block"),
        }
    }

    #[test]
    fn test_desugar_pipe_auto_wrap_identifier() {
        // x |> f — bare identifier target, auto-wrapped to f(__pipe_0)
        let pipe = PipeChain {
            initial: Box::new(Expr::Identifier("x".to_string())),
            steps: vec![PipeStep {
                target: Box::new(Expr::Identifier("f".to_string())),
                skip: 0,
            }],
        };
        let mut desugarer = Desugarer::new();
        let result = desugarer.desugar_expr(Expr::PipeChain(pipe));

        match &result {
            Expr::Block(stmts, _) => {
                assert_eq!(stmts.len(), 2);
                if let Statement::Let { expr: Some(expr), .. } = &stmts[1] {
                    assert!(matches!(expr, Expr::Call(name, args)
                        if name == "f" && args.len() == 1
                        && args[0] == Expr::Identifier("__pipe_0".to_string())
                    ), "Auto-wrap should produce Call(f, [__pipe_0]), got {:?}", expr);
                }
            }
            _ => panic!("Expected Block"),
        }
    }

    #[test]
    fn test_desugar_pipe_three_step() {
        // x |> f() |> g() .|> h() ..|> i()
        let pipe = PipeChain {
            initial: Box::new(Expr::Identifier("x".to_string())),
            steps: vec![
                PipeStep { target: Box::new(Expr::Call("f".to_string(), vec![])), skip: 0 },
                PipeStep { target: Box::new(Expr::Call("g".to_string(), vec![])), skip: 0 },
                PipeStep { target: Box::new(Expr::Call("h".to_string(), vec![])), skip: 1 },
                PipeStep { target: Box::new(Expr::Call("i".to_string(), vec![])), skip: 2 },
            ],
        };
        let mut desugarer = Desugarer::new();
        let result = desugarer.desugar_expr(Expr::PipeChain(pipe));

        match &result {
            Expr::Block(stmts, trailing) => {
                // 4 steps + initial = 5 let bindings
                assert_eq!(stmts.len(), 5);
                // Step 0: __pipe_1 = f(__pipe_0)
                assert!(matches!(&stmts[1], Statement::Let { name, expr: Some(Expr::Call(n, args)), .. }
                    if name == "__pipe_1" && n == "f" && args[0] == Expr::Identifier("__pipe_0".to_string())
                ));
                // Step 1: __pipe_2 = g(__pipe_1)
                assert!(matches!(&stmts[2], Statement::Let { name, expr: Some(Expr::Call(n, args)), .. }
                    if name == "__pipe_2" && n == "g" && args[0] == Expr::Identifier("__pipe_1".to_string())
                ));
                // Step 2 (skip=1): __pipe_3 = h(__pipe_1)
                assert!(matches!(&stmts[3], Statement::Let { name, expr: Some(Expr::Call(n, args)), .. }
                    if name == "__pipe_3" && n == "h" && args[0] == Expr::Identifier("__pipe_1".to_string())
                ));
                // Step 3 (skip=2): __pipe_4 = i(__pipe_{4-1-2}=__pipe_1)
                assert!(matches!(&stmts[4], Statement::Let { name, expr: Some(Expr::Call(n, args)), .. }
                    if name == "__pipe_4" && n == "i" && args[0] == Expr::Identifier("__pipe_1".to_string())
                ));
                assert_eq!(trailing.as_ref(), &Expr::Identifier("__pipe_4".to_string()));
            }
            _ => panic!("Expected Block"),
        }
    }
}
