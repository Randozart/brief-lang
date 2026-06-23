// Copyright 2026 Randy Smits-Schreuder Goedheijt
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//
// Runtime Exception for Use as a Language:
// When the Work or any Derivative Work thereof is used to generate code
// ("generated code"), such generated code shall not be subject to the
// terms of this License, provided that the generated code itself is not
// a Derivative Work of the Work. This exception does not apply to code
// that is itself a compiler, interpreter, or similar tool that incorporates
// or embeds the Work.

use crate::ast::*;
use crate::errors::{Diagnostic, Severity, Span};
use crate::features::literal::LiteralExpr;
use crate::ffi;
use crate::symbolic;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;

pub use crate::errors::TypeError;

#[derive(Debug, Clone, PartialEq)]
pub enum ResultCheckStatus {
    Unchecked,
    CheckedOk,
    CheckedErr,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CompilationTarget {
    Interpreter,
    Wasm,
    Verilog,
    Embedded,
    Circuit,
}

pub struct TypeChecker {
    scopes: Vec<HashMap<String, Type>>,
    errors: RefCell<Vec<crate::errors::TypeError>>,
    diagnostics: RefCell<Vec<Diagnostic>>,
    source: String,
    current_file: PathBuf,
    no_stdlib: bool,
    custom_stdlib_path: Option<PathBuf>,
    signatures: HashMap<String, Signature>,
    definitions: HashMap<String, Definition>,
    transactions: HashMap<String, Transaction>,
    ffi_results: RefCell<HashMap<String, ResultCheckStatus>>,
    foreign_bindings: HashMap<String, ForeignSignature>,
    pub target: CompilationTarget,
    enum_variants: HashMap<String, String>,  // variant_name -> enum_name
    struct_fields: HashMap<String, HashMap<String, Type>>,  // struct_name -> {field_name -> type}
    struct_field_visibility: HashMap<String, HashMap<String, Visibility>>,  // struct_name -> {field_name -> visibility}
    struct_files: HashMap<String, PathBuf>,  // struct_name -> defining file
    struct_parents: HashMap<String, Option<Type>>,  // struct_name -> parent type (for derivation upcast)
    trigger_names: std::collections::HashSet<String>,  // names of declared @ link triggers (read-only)
    inop_decls: HashMap<String, InopDeclaration>,
    type_universe: Option<crate::type_universe::TypeUniverse>,
    cell_defs: HashMap<String, CellDef>,
}

impl TypeChecker {
    pub fn new() -> Self {
        TypeChecker {
            scopes: vec![HashMap::new()],
            errors: RefCell::new(Vec::new()),
            diagnostics: RefCell::new(Vec::new()),
            source: String::new(),
            current_file: PathBuf::from("main.bv"),
            no_stdlib: false,
            custom_stdlib_path: None,
            signatures: HashMap::new(),
            definitions: HashMap::new(),
            transactions: HashMap::new(),
            ffi_results: RefCell::new(HashMap::new()),
            foreign_bindings: HashMap::new(),
            target: CompilationTarget::Interpreter,
            enum_variants: HashMap::new(),
            struct_fields: HashMap::new(),
            struct_field_visibility: HashMap::new(),
            struct_files: HashMap::new(),
            struct_parents: HashMap::new(),
            trigger_names: std::collections::HashSet::new(),
            inop_decls: HashMap::new(),
            type_universe: None,
            cell_defs: HashMap::new(),
        }
    }

    pub fn with_target(mut self, target: CompilationTarget) -> Self {
        self.target = target;
        self
    }

    pub fn with_source(mut self, source: String) -> Self {
        self.source = source;
        self
    }

    pub fn with_file(mut self, file: PathBuf) -> Self {
        self.current_file = file;
        self
    }

    pub fn with_stdlib_config(mut self, no_stdlib: bool, custom_path: Option<PathBuf>) -> Self {
        self.no_stdlib = no_stdlib;
        self.custom_stdlib_path = custom_path;
        self
    }

    pub fn with_type_universe(mut self, tu: crate::type_universe::TypeUniverse) -> Self {
        self.type_universe = Some(tu);
        self
    }

    fn register_stdlib_signatures(&mut self) {
        // Add stdlib function signatures for type checking
        // to_json(value: Object) -> String
        self.signatures.insert(
            "to_json".to_string(),
            Signature {
                name: "to_json".to_string(),
                params: vec![("".to_string(), Type::Custom("Object".to_string()))], modifier: None, output_type: None,
                result_type: ResultType::Projection(vec![Type::String]),
                source: None,
                alias: None,
                bound_defn: None,
            },
        );

        // from_json(json_str: String) -> Result<Object, String>
        self.signatures.insert(
            "from_json".to_string(),
            Signature {
                name: "from_json".to_string(),
                params: vec![("".to_string(), Type::String)], modifier: None, output_type: None,
                result_type: ResultType::Projection(vec![Type::Applied(
                    "Result".to_string(),
                    vec![Type::Custom("Object".to_string()), Type::String],
                )]),
                source: None,
                alias: None,
                bound_defn: None,
            },
        );

        // StringBuilder functions
        self.signatures.insert(
            "new_builder".to_string(),
            Signature {
                name: "new_builder".to_string(),
                params: vec![], modifier: None, output_type: None,
                result_type: ResultType::Projection(vec![Type::Custom("StringBuilder".to_string())]),
                source: None,
                alias: None,
                bound_defn: None,
            },
        );

        self.signatures.insert(
            "append_str".to_string(),
            Signature {
                name: "append_str".to_string(),
                params: vec![("".to_string(), Type::Custom("StringBuilder".to_string())), ("".to_string(), Type::String)], modifier: None, output_type: None,
                result_type: ResultType::Projection(vec![Type::Custom("StringBuilder".to_string())]),
                source: None,
                alias: None,
                bound_defn: None,
            },
        );

        self.signatures.insert(
            "append_char".to_string(),
            Signature {
                name: "append_char".to_string(),
                params: vec![("".to_string(), Type::Custom("StringBuilder".to_string())), ("".to_string(), Type::Char)], modifier: None, output_type: None,
                result_type: ResultType::Projection(vec![Type::Custom("StringBuilder".to_string())]),
                source: None,
                alias: None,
                bound_defn: None,
            },
        );

        self.signatures.insert(
            "append_int".to_string(),
            Signature {
                name: "append_int".to_string(),
                params: vec![("".to_string(), Type::Custom("StringBuilder".to_string())), ("".to_string(), Type::Int)], modifier: None, output_type: None,
                result_type: ResultType::Projection(vec![Type::Custom("StringBuilder".to_string())]),
                source: None,
                alias: None,
                bound_defn: None,
            },
        );

        self.signatures.insert(
            "to_string".to_string(),
            Signature {
                name: "to_string".to_string(),
                params: vec![("".to_string(), Type::Custom("StringBuilder".to_string()))], modifier: None, output_type: None,
                result_type: ResultType::Projection(vec![Type::String]),
                source: None,
                alias: None,
                bound_defn: None,
            },
        );

        self.signatures.insert(
            "String".to_string(),
            Signature {
                name: "String".to_string(),
                params: vec![("".to_string(), Type::Int)], modifier: None, output_type: None,
                result_type: ResultType::Projection(vec![Type::String]),
                source: None,
                alias: None,
                bound_defn: None,
            },
        );

        self.signatures.insert(
            "char_to_string".to_string(),
            Signature {
                name: "char_to_string".to_string(),
                params: vec![("".to_string(), Type::Char)], modifier: None, output_type: None,
                result_type: ResultType::Projection(vec![Type::String]),
                source: None,
                alias: None,
                bound_defn: None,
            },
        );

        // Register stdlib enum variants for type checking
        // Option<T>
        self.enum_variants.insert("Some".to_string(), "Option".to_string());
        self.enum_variants.insert("None".to_string(), "Option".to_string());
        // Result<T, E>
        self.enum_variants.insert("Ok".to_string(), "Result".to_string());
        self.enum_variants.insert("Err".to_string(), "Result".to_string());
        // Token (compiler library)
        self.enum_variants.insert("TokenInt".to_string(), "Token".to_string());
        self.enum_variants.insert("TokenFloat".to_string(), "Token".to_string());
        self.enum_variants.insert("TokenString".to_string(), "Token".to_string());
        self.enum_variants.insert("TokenChar".to_string(), "Token".to_string());
        self.enum_variants.insert("TokenIdentifier".to_string(), "Token".to_string());
        self.enum_variants.insert("TokenEof".to_string(), "Token".to_string());
        self.enum_variants.insert("TokenError".to_string(), "Token".to_string());
        self.enum_variants.insert("KeywordLet".to_string(), "Token".to_string());
        self.enum_variants.insert("KeywordConst".to_string(), "Token".to_string());
        self.enum_variants.insert("KeywordTxn".to_string(), "Token".to_string());
        self.enum_variants.insert("KeywordRct".to_string(), "Token".to_string());
        self.enum_variants.insert("KeywordAsync".to_string(), "Token".to_string());
        self.enum_variants.insert("KeywordTerm".to_string(), "Token".to_string());
        self.enum_variants.insert("KeywordEscape".to_string(), "Token".to_string());
        self.enum_variants.insert("KeywordDefn".to_string(), "Token".to_string());
        self.enum_variants.insert("KeywordSig".to_string(), "Token".to_string());
        self.enum_variants.insert("KeywordFrgn".to_string(), "Token".to_string());
        self.enum_variants.insert("KeywordStruct".to_string(), "Token".to_string());
        self.enum_variants.insert("KeywordEnum".to_string(), "Token".to_string());
        self.enum_variants.insert("KeywordImport".to_string(), "Token".to_string());
        self.enum_variants.insert("KeywordFrom".to_string(), "Token".to_string());
        self.enum_variants.insert("KeywordAs".to_string(), "Token".to_string());
        self.enum_variants.insert("KeywordTrue".to_string(), "Token".to_string());
        self.enum_variants.insert("KeywordFalse".to_string(), "Token".to_string());
        self.enum_variants.insert("OpPlus".to_string(), "Token".to_string());
        self.enum_variants.insert("OpMinus".to_string(), "Token".to_string());
        self.enum_variants.insert("OpStar".to_string(), "Token".to_string());
        self.enum_variants.insert("OpSlash".to_string(), "Token".to_string());
        self.enum_variants.insert("OpPercent".to_string(), "Token".to_string());
        self.enum_variants.insert("OpEq".to_string(), "Token".to_string());
        self.enum_variants.insert("OpBang".to_string(), "Token".to_string());
        self.enum_variants.insert("OpAmp".to_string(), "Token".to_string());
        self.enum_variants.insert("OpPipe".to_string(), "Token".to_string());
        self.enum_variants.insert("OpCaret".to_string(), "Token".to_string());
        self.enum_variants.insert("OpTilde".to_string(), "Token".to_string());
        self.enum_variants.insert("OpQuestion".to_string(), "Token".to_string());
        self.enum_variants.insert("OpAt".to_string(), "Token".to_string());
        self.enum_variants.insert("OpDot".to_string(), "Token".to_string());
        self.enum_variants.insert("OpColon".to_string(), "Token".to_string());
        self.enum_variants.insert("OpSemicolon".to_string(), "Token".to_string());
        self.enum_variants.insert("OpComma".to_string(), "Token".to_string());
        self.enum_variants.insert("OpEqEq".to_string(), "Token".to_string());
        self.enum_variants.insert("OpNeq".to_string(), "Token".to_string());
        self.enum_variants.insert("OpLt".to_string(), "Token".to_string());
        self.enum_variants.insert("OpGt".to_string(), "Token".to_string());
        self.enum_variants.insert("OpLtEq".to_string(), "Token".to_string());
        self.enum_variants.insert("OpGtEq".to_string(), "Token".to_string());
        self.enum_variants.insert("OpAnd".to_string(), "Token".to_string());
        self.enum_variants.insert("OpOr".to_string(), "Token".to_string());
        self.enum_variants.insert("OpLtLt".to_string(), "Token".to_string());
        self.enum_variants.insert("OpGtGt".to_string(), "Token".to_string());
        self.enum_variants.insert("OpArrow".to_string(), "Token".to_string());
        self.enum_variants.insert("OpFatArrow".to_string(), "Token".to_string());
        self.enum_variants.insert("DelimLParen".to_string(), "Token".to_string());
        self.enum_variants.insert("DelimRParen".to_string(), "Token".to_string());
        self.enum_variants.insert("DelimLBrace".to_string(), "Token".to_string());
        self.enum_variants.insert("DelimRBrace".to_string(), "Token".to_string());
        self.enum_variants.insert("DelimLBracket".to_string(), "Token".to_string());
        self.enum_variants.insert("DelimRBracket".to_string(), "Token".to_string());

        // Character classification functions
        self.signatures.insert(
            "is_whitespace".to_string(),
            Signature {
                name: "is_whitespace".to_string(),
                params: vec![("".to_string(), Type::Char)], modifier: None, output_type: None,
                result_type: ResultType::Projection(vec![Type::Bool]),
                source: None,
                alias: None,
                bound_defn: None,
            },
        );

        self.signatures.insert(
            "is_digit".to_string(),
            Signature {
                name: "is_digit".to_string(),
                params: vec![("".to_string(), Type::Char)], modifier: None, output_type: None,
                result_type: ResultType::Projection(vec![Type::Bool]),
                source: None,
                alias: None,
                bound_defn: None,
            },
        );

        self.signatures.insert(
            "is_alpha".to_string(),
            Signature {
                name: "is_alpha".to_string(),
                params: vec![("".to_string(), Type::Char)], modifier: None, output_type: None,
                result_type: ResultType::Projection(vec![Type::Bool]),
                source: None,
                alias: None,
                bound_defn: None,
            },
        );

        self.signatures.insert(
            "is_alphanumeric".to_string(),
            Signature {
                name: "is_alphanumeric".to_string(),
                params: vec![("".to_string(), Type::Char)], modifier: None, output_type: None,
                result_type: ResultType::Projection(vec![Type::Bool]),
                source: None,
                alias: None,
                bound_defn: None,
            },
        );

        self.signatures.insert(
            "is_hex_digit".to_string(),
            Signature {
                name: "is_hex_digit".to_string(),
                params: vec![("".to_string(), Type::Char)], modifier: None, output_type: None,
                result_type: ResultType::Projection(vec![Type::Bool]),
                source: None,
                alias: None,
                bound_defn: None,
            },
        );

        // String conversion functions
        self.signatures.insert(
            "to_int".to_string(),
            Signature {
                name: "to_int".to_string(),
                params: vec![("".to_string(), Type::String)], modifier: None, output_type: None,
                result_type: ResultType::Projection(vec![Type::Int]),
                source: None,
                alias: None,
                bound_defn: None,
            },
        );

        self.signatures.insert(
            "to_float".to_string(),
            Signature {
                name: "to_float".to_string(),
                params: vec![("".to_string(), Type::String)], modifier: None, output_type: None,
                result_type: ResultType::Projection(vec![Type::Float]),
                source: None,
                alias: None,
                bound_defn: None,
            },
        );

        // String len
        self.signatures.insert(
            "len".to_string(),
            Signature {
                name: "len".to_string(),
                params: vec![("".to_string(), Type::String)], modifier: None, output_type: None,
                result_type: ResultType::Projection(vec![Type::Int]),
                source: None,
                alias: None,
                bound_defn: None,
            },
        );

        self.signatures.insert(
            "len".to_string(),
            Signature {
                name: "len".to_string(),
                params: vec![("".to_string(), Type::Applied("List".to_string(), vec![Type::TypeVar("T".to_string())]))], modifier: None, output_type: None,
                result_type: ResultType::Projection(vec![Type::Int]),
                source: None,
                alias: None,
                bound_defn: None,
            },
        );

        self.signatures.insert(
            "append".to_string(),
            Signature {
                name: "append".to_string(),
                params: vec![("".to_string(), Type::Applied("List".to_string(), vec![Type::TypeVar("T".to_string())])), ("".to_string(), Type::TypeVar("T".to_string()))], modifier: None, output_type: None,
                result_type: ResultType::Projection(vec![Type::Applied("List".to_string(), vec![Type::TypeVar("T".to_string())])]),
                source: None,
                alias: None,
                bound_defn: None,
            },
        );

        self.signatures.insert(
            "char_to_string".to_string(),
            Signature {
                name: "char_to_string".to_string(),
                params: vec![("".to_string(), Type::Char)], modifier: None, output_type: None,
                result_type: ResultType::Projection(vec![Type::String]),
                source: None,
                alias: None,
                bound_defn: None,
            },
        );

        self.signatures.insert(
            "String".to_string(),
            Signature {
                name: "String".to_string(),
                params: vec![("".to_string(), Type::Int)], modifier: None, output_type: None,
                result_type: ResultType::Projection(vec![Type::String]),
                source: None,
                alias: None,
                bound_defn: None,
            },
        );

        self.signatures.insert(
            "Some".to_string(),
            Signature {
                name: "Some".to_string(),
                params: vec![("".to_string(), Type::TypeVar("T".to_string()))], modifier: None, output_type: None,
                result_type: ResultType::Projection(vec![Type::Applied("Option".to_string(), vec![Type::TypeVar("T".to_string())])]),
                source: None,
                alias: None,
                bound_defn: None,
            },
        );

        self.signatures.insert(
            "None".to_string(),
            Signature {
                name: "None".to_string(),
                params: vec![], modifier: None, output_type: None,
                result_type: ResultType::Projection(vec![Type::Applied("Option".to_string(), vec![Type::TypeVar("T".to_string())])]),
                source: None,
                alias: None,
                bound_defn: None,
            },
        );

        self.signatures.insert(
            "Ok".to_string(),
            Signature {
                name: "Ok".to_string(),
                params: vec![("".to_string(), Type::TypeVar("T".to_string()))], modifier: None, output_type: None,
                result_type: ResultType::Projection(vec![Type::Applied("Result".to_string(), vec![Type::TypeVar("T".to_string()), Type::TypeVar("E".to_string())])]),
                source: None,
                alias: None,
                bound_defn: None,
            },
        );

        self.signatures.insert(
            "Err".to_string(),
            Signature {
                name: "Err".to_string(),
                params: vec![("".to_string(), Type::TypeVar("E".to_string()))], modifier: None, output_type: None,
                result_type: ResultType::Projection(vec![Type::Applied("Result".to_string(), vec![Type::TypeVar("T".to_string()), Type::TypeVar("E".to_string())])]),
                source: None,
                alias: None,
                bound_defn: None,
            },
        );
    }

    pub fn check_program(&mut self, program: &mut Program) -> Vec<TypeError> {
        self.source = String::new();
        self.scopes = vec![HashMap::new()];
        self.errors = RefCell::new(Vec::new());

        self.register_stdlib_signatures();

        // GPU (.abv) validation — enforce type and intrinsic restrictions
        if program.strict_mode.is_gpu() {
            self.validate_gpu_program(program);
        }

        // Pass 1: Collect all signatures and definitions for global visibility
        for item in &program.items {
            match item {
                TopLevel::Signature(sig) => {
                    let key = sig.name.clone();
                    self.signatures.insert(key, sig.clone());
                }
                TopLevel::Definition(defn) => {
                    self.definitions.insert(defn.name.clone(), defn.clone());
                }
                TopLevel::Transaction(txn) => {
                    self.transactions.insert(txn.name.clone(), txn.clone());
                }
                TopLevel::ForeignBinding {
                    name, signature, ..
                } => {
                    // Collect foreign binding signature for type inference
                    self.foreign_bindings
                        .insert(name.clone(), signature.clone());
                }
                TopLevel::Inop(inop) => {
                    self.inop_decls.insert(inop.name.clone(), inop.clone());
                    let verifier_errors = crate::analysis::bild_verifier::check_bild(inop);
                    for err in &verifier_errors {
                        let diag = err.to_diagnostic(&inop.name);
                        self.diagnostics.borrow_mut().push(diag);
                    }
                    // Symbolic verification of BILD body vs fallback
                    let sym_result = crate::analysis::bild_symexec::verify_inop(inop);
                    if let Some(bild_expr) = &sym_result.bild_expr {
                        if let Some(fb_expr) = &sym_result.fallback_expr {
                            if let Some(msg) = crate::analysis::bild_symexec::compare_bild_with_fallback(
                                &Some(bild_expr.clone()),
                                &Some(fb_expr.clone()),
                                &inop.name,
                            ) {
                                let diag = crate::errors::Diagnostic::new(
                                    "B005",
                                    if sym_result.has_opaque_ops {
                                        crate::errors::Severity::Warning
                                    } else {
                                        crate::errors::Severity::Error
                                    },
                                    "BILD body and fallback mismatch",
                                ).with_explanation(&msg);
                                self.diagnostics.borrow_mut().push(diag);
                            }
                        }
                    }
                    // %state access requires inop! (side-effecting) and contract
                    if inop.has_state_access {
                        if !inop.has_side_effects {
                            let diag = crate::errors::Diagnostic::new(
                                "B003",
                                crate::errors::Severity::Error,
                                "%%state access requires inop!",
                            ).with_explanation(&format!(
                                "inop `{}`: (%state) marker requires `inop!` (side-effecting), \
                                 use `inop! {}` instead of `inop {}`",
                                inop.name, inop.name, inop.name
                            ));
                            self.diagnostics.borrow_mut().push(diag);
                        }
                        if matches!(inop.contract.pre_condition, Expr::Bool(true))
                            && matches!(inop.contract.post_condition, Expr::Bool(true))
                        {
                            let diag = crate::errors::Diagnostic::new(
                                "B004",
                                crate::errors::Severity::Error,
                                "%%state access requires contract",
                            ).with_explanation(&format!(
                                "inop `{}`: (%state) marker requires [pre][post] contract",
                                inop.name
                            ));
                            self.diagnostics.borrow_mut().push(diag);
                        }
                    }
                }
                TopLevel::Enum(enum_def) => {
                    for variant in &enum_def.variants {
                        let variant_name = match variant {
                            crate::ast::EnumVariant::Unit(name) => name.clone(),
                            crate::ast::EnumVariant::Tuple(name, _) => name.clone(),
                            crate::ast::EnumVariant::Struct(name, _) => name.clone(),
                        };
                        self.enum_variants.insert(variant_name, enum_def.name.clone());
                    }
                }
                TopLevel::Struct(struct_def) => {
                    let mut fields = HashMap::new();
                    let mut vis = HashMap::new();
                    for field in &struct_def.fields {
                        fields.insert(field.name.clone(), field.ty.clone());
                        vis.insert(field.name.clone(), field.visibility);
                    }
                    self.struct_fields.insert(struct_def.name.clone(), fields);
                    self.struct_field_visibility.insert(struct_def.name.clone(), vis);
                    self.struct_files.insert(struct_def.name.clone(), self.current_file.clone());
                    self.struct_parents.insert(struct_def.name.clone(), struct_def.parent.clone());
                }
                // DEFERRED (D-1): TypeDefs are collected and resolved in Pass 1
                // by type_universe.rs. In Pass 2, we only need to validate
                // usage against the frozen universe — adding that here.
                TopLevel::TypeDef(td) => {
                    // Phase 1.5: TypeDefs are validated in type_universe.rs Pass 1.
                    // Here in Pass 2 we just ensure the type name is registered
                    // for later resolution.
                }
                TopLevel::Test { item: inner, .. } => {
                    // Unwrap Test items — process the inner item's declarations
                    // in Pass 1 so signatures/definitions are registered.
                    match inner.as_ref() {
                        TopLevel::Definition(defn) => {
                            self.definitions.insert(defn.name.clone(), defn.clone());
                        }
                        _ => {}
                    }
                }
                TopLevel::Assertion { .. } => {
                    // Assertions are compile-time only — skip in Pass 1.
                }
                TopLevel::Cell(cell) => {
                    let c: &CellDef = cell;
                    self.cell_defs.insert(c.name.clone(), c.clone());
                }
                _ => {}
            }
        }

        for item in &mut program.items {
            match item {
                TopLevel::StateDecl(decl) => {
                    self.declare_variable(&decl.name, decl.ty.clone());
                    if let Some(expr) = &decl.expr {
                        let expr_ty = self.infer_expression(expr);
                        if !self.types_compatible(&decl.ty, &expr_ty) {
                            let mut diag = Diagnostic::new("B002", Severity::Error, "type mismatch")
                                .with_explanation(&format!(
                                    "expected {} for initial value of state variable '{}', but found {}",
                                    self.type_to_string(&decl.ty),
                                    decl.name,
                                    self.type_to_string(&expr_ty)
                                ));
                            if let Some(span) = decl.span {
                                diag = diag.with_span(span);
                            }
                            self.diagnostics.borrow_mut().push(diag);

                            self.errors.borrow_mut().push(TypeError::TypeMismatch {
                                expected: self.type_to_string(&decl.ty),
                                found: self.type_to_string(&expr_ty),
                                context: format!("initial value of state variable '{}'", decl.name),
                            });
                        }
                    } else {
                        let mut diag =
                            Diagnostic::new("B002", Severity::Warning, "uninitialized signal")
                                .with_explanation(&format!(
                                    "signal '{}' has no initial value specified",
                                    decl.name
                                ))
                                .with_hint(&format!(
                                    "add an initial value: let {}: {} = 0;",
                                    decl.name,
                                    self.type_to_string(&decl.ty)
                                ))
                                .with_note(
                                    "uninitialized signals may contain garbage values at runtime",
                                );
                        if let Some(span) = decl.span {
                            diag = diag.with_span(span);
                        }
                        self.diagnostics.borrow_mut().push(diag);
                    }
                }
                TopLevel::Constant(cons) => {
                    self.declare_variable(&cons.name, cons.ty.clone());
                }
                TopLevel::Signature(sig) => {
                    self.check_signature(sig);
                }
                TopLevel::Definition(defn) => {
                    self.check_definition(defn);
                }
                TopLevel::Transaction(txn) => {
                    self.check_transaction(txn);
                }
                TopLevel::Trigger(trg) => {
                    self.declare_variable(&trg.name, trg.ty.clone());
                    self.trigger_names.insert(trg.name.clone());
                }
                TopLevel::ForeignBinding {
                    name,
                    toml_path,
                    signature,
                    ..
                } => {
                    self.check_frgn_binding(name, toml_path, signature);
                    if let Some(stored_sig) = self.foreign_bindings.get_mut(name) {
                        stored_sig.wasm_impl = signature.wasm_impl.clone();
                        stored_sig.wasm_setup = signature.wasm_setup.clone();
                    }
                }
                TopLevel::Test { item: inner, .. } => {
                    // Unwrap Test items — typecheck the inner item
                    match inner.as_ref() {
                        TopLevel::Definition(defn) => self.check_definition(defn),
                        TopLevel::Transaction(txn) => self.check_transaction(txn),
                        _ => {}
                    }
                }
                TopLevel::Assertion { .. } => {
                    // Assertions are compile-time only — skip in typechecker.
                }
                TopLevel::Meld(meld) => {
                    // Phase 4: Validate meld declarations
                    // First, emit any cycle warnings from TypeUniverse build phase
                    if let Some(ref tu) = self.type_universe {
                        for w in &tu.meld_warnings {
                            let diag = crate::errors::Diagnostic::new(
                                "E002", crate::errors::Severity::Warning, w,
                            );
                            if let Some(span) = meld.span {
                                self.diagnostics.borrow_mut().push(diag.with_span(span));
                            } else {
                                self.diagnostics.borrow_mut().push(diag);
                            }
                        }
                    }

                    let has_type_a = self.type_universe.as_ref()
                        .and_then(|u| u.types.get(&meld.name_a)).is_some();
                    let has_type_b = self.type_universe.as_ref()
                        .and_then(|u| u.types.get(&meld.name_b)).is_some();

                    if meld.routes.is_empty() {
                        // E003: Size mismatch without router
                        if has_type_a && has_type_b {
                            let fields_a = self.struct_fields.get(&meld.name_a);
                            let fields_b = self.struct_fields.get(&meld.name_b);
                            match (fields_a, fields_b) {
                                (Some(fa), Some(fb)) if fa.len() != fb.len() => {
                                    let diag = crate::errors::Diagnostic::new(
                                        "E003", crate::errors::Severity::Warning,
                                        "meld types may need explicit routes",
                                    ).with_explanation(&format!(
                                        "'{}' has {} field(s) but '{}' has {} field(s). \
                                        Without explicit routes, the compiler will use @/ bit-range matching \
                                        which may produce unexpected results for fields in different positions.",
                                        meld.name_a, fa.len(), meld.name_b, fb.len(),
                                    )).with_hint(&format!(
                                        "Add explicit routes: meld {} <:> {} {{ Ptr -> {}.ptr; Size -> {}.len; }};",
                                        meld.name_a, meld.name_b, meld.name_b, meld.name_b,
                                    ));
                                    if let Some(span) = meld.span {
                                        self.diagnostics.borrow_mut().push(diag.with_span(span));
                                    } else {
                                        self.diagnostics.borrow_mut().push(diag);
                                    }
                                }
                                _ => {}
                            }
                        }
                    } else {
                        // E001: Check if explicit routes cover all fields of both types
                        let fields_a = self.struct_fields.get(&meld.name_a);
                        let fields_b = self.struct_fields.get(&meld.name_b);
                        let covered: std::collections::HashSet<&str> =
                            meld.routes.iter().map(|r| r.accessor.as_str()).collect();

                        if let Some(fa) = fields_a {
                            for field_name in fa.keys() {
                                if !covered.contains(field_name.as_str()) {
                                    let diag = crate::errors::Diagnostic::new(
                                        "E001", crate::errors::Severity::Error,
                                        &format!("no route for field — `{}` has no route to `{}`",
                                            meld.name_b, field_name),
                                    ).with_explanation(&format!(
                                        "`{}` has field `{}` but no route in the meld defines how to \
                                         derive it from `{}`.",
                                        meld.name_a, field_name, meld.name_b,
                                    )).with_hint(&format!(
                                        "Add a route: {} -> {}:>Projection; or remove explicit routes to use @/ inference",
                                        field_name, meld.name_b,
                                    ));
                                    if let Some(span) = meld.span {
                                        self.diagnostics.borrow_mut().push(diag.with_span(span));
                                    } else {
                                        self.diagnostics.borrow_mut().push(diag);
                                    }
                                }
                            }
                        }
                        if let Some(fb) = fields_b {
                            for field_name in fb.keys() {
                                if !covered.contains(field_name.as_str()) {
                                    let diag = crate::errors::Diagnostic::new(
                                        "E001", crate::errors::Severity::Error,
                                        &format!("no route for field — `{}` has no route to `{}`",
                                            meld.name_a, field_name),
                                    ).with_explanation(&format!(
                                        "`{}` has field `{}` but no route in the meld defines how to \
                                         derive it from `{}`.",
                                        meld.name_b, field_name, meld.name_a,
                                    )).with_hint(&format!(
                                        "Add a route: {} -> {}:>Projection; or remove explicit routes to use @/ inference",
                                        field_name, meld.name_a,
                                    ));
                                    if let Some(span) = meld.span {
                                        self.diagnostics.borrow_mut().push(diag.with_span(span));
                                    } else {
                                        self.diagnostics.borrow_mut().push(diag);
                                    }
                                }
                            }
                        }

                        // W002: Check if routes are all identity (unnecessary explicit routes)
                        let all_identity = meld.routes.iter().all(|r| {
                            matches!(&r.dest_expr, crate::ast::Expr::Identifier(n) if n == &r.accessor)
                        });
                        if all_identity && meld.routes.len() >= 1 {
                            let diag = crate::errors::Diagnostic::new(
                                "W002", crate::errors::Severity::Note,
                                "unnecessary meld — explicit routes match the default inference",
                            ).with_explanation(&format!(
                                "All routes in `meld {} <:> {}` are identity projections, which is \
                                 what the compiler would infer automatically.",
                                meld.name_a, meld.name_b,
                            )).with_hint("Remove the explicit routes: use `meld A <:> B;` instead.");
                            if let Some(span) = meld.span {
                                self.diagnostics.borrow_mut().push(diag.with_span(span));
                            } else {
                                self.diagnostics.borrow_mut().push(diag);
                            }
                        }
                    }

                    // E005: Check each route's dest_expr for invalid references
                    let known_idents: std::collections::HashSet<&str> = [
                        meld.name_a.as_str(), meld.name_b.as_str(),
                        "Ptr", "Size", "Bytes", "Alignment", "Type", "Range",
                        "Popcount", "LeadingZeros", "TrailingZeros", "Absolute",
                        "BitReverse", "Keys", "Values", "IsEmpty", "Top", "Front",
                    ].iter().cloned().collect();
                    for route in &meld.routes {
                        self.check_route_expr(&route.dest_expr, &known_idents,
                            &meld.name_a, &meld.name_b, &meld.span);
                    }

                    // E004: Field type mismatch — check if fields at matching names have compatible types
                    let fields_a = self.struct_fields.get(&meld.name_a);
                    let fields_b = self.struct_fields.get(&meld.name_b);
                    if let (Some(fa), Some(fb)) = (fields_a, fields_b) {
                        for name in fa.keys() {
                            if let Some(ty_b) = fb.get(name) {
                                let ty_a = &fa[name];
                                if ty_a != ty_b && !self.types_are_width_compatible(ty_a, ty_b) {
                                    let diag = crate::errors::Diagnostic::new(
                                        "E004", crate::errors::Severity::Warning,
                                        &format!("field type mismatch — `{}.{}` is `{}` but `{}.{}` is `{}`",
                                            meld.name_a, name, self.type_to_string(ty_a),
                                            meld.name_b, name, self.type_to_string(ty_b),
                                        ),
                                    ).with_explanation(&format!(
                                        "Both fields occupy the same @/ bit range but have incompatible types. \
                                         The bits are the same width, so the meld is valid, but operations \
                                         on these fields may produce unexpected results.",
                                    )).with_hint("Add an explicit route to override the default identity mapping.");
                                    if let Some(span) = meld.span {
                                        self.diagnostics.borrow_mut().push(diag.with_span(span));
                                    } else {
                                        self.diagnostics.borrow_mut().push(diag);
                                    }
                                }
                            }
                        }
                    }
                }
                TopLevel::Cell(cell) => {
                    self.check_cell_definition(cell);
                }
                _ => {}
            }
        }

        self.errors.borrow().clone()
    }

    pub fn get_diagnostics(&self) -> Vec<Diagnostic> {
        self.diagnostics.borrow().clone()
    }

    fn check_cell_definition(&mut self, cell: &CellDef) {
        // 1. Verify output port names are unique and don't shadow params
        if let Some(ref ot) = cell.output_type {
            let mut seen_names: Vec<String> = Vec::new();
            for (param_name, _) in &cell.parameters {
                seen_names.push(param_name.clone());
            }
            self.check_output_type_names(ot, &mut seen_names);
        }

        // 2. Validate each transaction and definition within the cell
        for txn in &cell.transactions {
            self.check_transaction(txn);
        }
        for defn in &cell.definitions {
            self.check_definition(defn);
        }

        // 3. Check that cell! (persistent) has at least one transaction
        if cell.is_persistent && cell.transactions.is_empty() {
            self.diagnostics.borrow_mut().push(
                Diagnostic::new("C001", Severity::Error, "persistent cell must have at least one transaction")
                    .with_explanation(&format!(
                        "cell! '{}' is declared persistent but has no transactions",
                        cell.name
                    )),
            );
        }
    }

    fn check_output_type_names(&self, ot: &OutputType, seen: &mut Vec<String>) {
        match ot {
            OutputType::Single(_) => {}
            OutputType::Union(types) => {
                for t in types {
                    self.check_output_type_names(t, seen);
                }
            }
            OutputType::Tuple(types) => {
                for t in types {
                    self.check_output_type_names(t, seen);
                }
            }
            OutputType::Array(_) => {}
            OutputType::Named(name, inner) => {
                if seen.contains(name) {
                    self.diagnostics.borrow_mut().push(
                        Diagnostic::new("C002", Severity::Error, "duplicate output port name")
                            .with_explanation(&format!(
                                "output port '{}' shadows an existing parameter or port name",
                                name
                            )),
                    );
                }
                seen.push(name.clone());
                self.check_output_type_names(inner, seen);
            }
        }
    }

    fn output_type_to_type(&self, ot: &OutputType) -> Type {
        match ot {
            OutputType::Single(ty) => ty.clone(),
            OutputType::Union(types) => {
                Type::Union(types.iter().map(|t| self.output_type_to_type(t)).collect())
            }
            OutputType::Tuple(types) => {
                Type::Tuple(types.iter().map(|t| self.output_type_to_type(t)).collect())
            }
            OutputType::Array(ty) => Type::Applied("List".to_string(), vec![ty.as_ref().clone()]),
            OutputType::Named(_, inner) => self.output_type_to_type(inner),
        }
    }

    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn declare_variable(&mut self, name: &str, ty: Type) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.to_string(), ty);
        }
    }

    fn lookup_variable(&self, name: &str) -> Option<Type> {
        for scope in self.scopes.iter().rev() {
            if let Some(ty) = scope.get(name) {
                return Some(ty.clone());
            }
        }
        None
    }

    fn resolve_type(&self, ty: Type) -> Type {
        match ty {
            Type::Custom(name) => {
                if self.signatures.contains_key(&name) {
                    Type::Sig(name)
                } else {
                    Type::Custom(name)
                }
            }
            other => other,
        }
    }

    /// Validate GPU (.abv) restrictions on the program.
    /// Called only when `strict_mode == StrictMode::Gpu`.
    fn validate_gpu_program(&self, program: &Program) {
        for item in &program.items {
            match item {
                TopLevel::ForeignBinding { name, .. } => {
                    // frgn declarations are allowed in .abv for host-side I/O.
                    // The GPU kernel body itself is checked separately by
                    // check_eligibility (no Expr::Call inside kernels).
                    let diag = Diagnostic::new("G001", Severity::Warning, "FFI declared in .abv file")
                        .with_explanation(&format!(
                            "`frgn` '{}' — this FFI call is for host-side I/O. \
                             The GPU kernel body cannot use it; only GPU intrinsics \
                             (sin#, cos#, get_global_id#, etc.) are allowed inside kernels.",
                            name
                        ));
                    self.diagnostics.borrow_mut().push(diag);
                }
                TopLevel::Transaction(txn) => {
                    // Check for missing contracts — warn but don't error
                    let has_pre = txn.contract.pre_condition != Expr::Bool(true);
                    let has_post = txn.contract.post_condition != Expr::Bool(true);
                    if !has_pre && !has_post {
                        let diag = Diagnostic::new("G002", Severity::Warning, "GPU transaction without contracts")
                            .with_explanation(&format!(
                                "`{}` has no contracts — add [pre][post] for better GPU optimization",
                                txn.name
                            ));
                        self.diagnostics.borrow_mut().push(diag);
                    }
                    // Validate types and intrinsics in body statements
                    for stmt in &txn.body {
                        self.validate_gpu_stmt(stmt);
                    }
                }
                _ => {}
            }
        }
    }

    /// Validate types and intrinsics in a GPU statement body.
    fn validate_gpu_stmt(&self, stmt: &Statement) {
        match stmt {
            Statement::Let { ty: Some(t), expr, .. } => {
                self.validate_gpu_type(t);
                if let Some(e) = expr {
                    self.validate_gpu_expr(e);
                }
            }
            Statement::Assignment { expr, .. } => {
                self.validate_gpu_expr(expr);
            }
            Statement::Expression(e) => {
                self.validate_gpu_expr(e);
            }
            Statement::Guarded { condition, statements, .. } => {
                self.validate_gpu_expr(condition);
                for s in statements {
                    self.validate_gpu_stmt(s);
                }
            }
            _ => {}
        }
    }

    /// Validate that a type is allowed in .abv context.
    fn validate_gpu_type(&self, ty: &Type) {
        match ty {
            Type::Int | Type::UInt | Type::Float | Type::Bool | Type::Char => {}
            Type::String => {} // allowed for constants
            Type::Vector(elem, _) => self.validate_gpu_type(elem),
            _ => {
                let diag = Diagnostic::new("G003", Severity::Error, "Type not allowed in GPU kernel")
                    .with_explanation(&format!(
                        "Type `{:?}` is not supported in .abv files. \
                         Allowed types: Int, Float, Bool, Char, String (const), and [T; N]",
                        ty
                    ));
                self.diagnostics.borrow_mut().push(diag);
                self.errors.borrow_mut().push(TypeError::InvalidOperation {
                    operation: "type validation".to_string(),
                    type_name: format!("{:?}", ty),
                });
            }
        }
    }

    /// Check that only allowed GPU intrinsics and no FFI calls are used.
    fn validate_gpu_expr(&self, expr: &Expr) {
        match expr {
            Expr::IntrinsicCall { intrinsic, .. } => {
                let allowed = matches!(intrinsic,
                    Intrinsic::Sin | Intrinsic::Cos | Intrinsic::Pow
                    | Intrinsic::Sqrt | Intrinsic::Fabs
                    | Intrinsic::Ceil | Intrinsic::Floor
                    | Intrinsic::GetGlobalId | Intrinsic::GetLocalId
                    | Intrinsic::GetGroupId | Intrinsic::GetNumGroups
                    | Intrinsic::SubGroupBarrier
                );
                if !allowed {
                    let diag = Diagnostic::new("G004", Severity::Error, "Intrinsic not allowed in GPU kernel")
                        .with_explanation(&format!(
                            "`{:?}#` is not a GPU-safe intrinsic. \
                             Allowed: sin, cos, pow, sqrt, fabs, ceil, floor, \
                             get_global_id, get_local_id, get_group_id, get_num_groups, barrier",
                            intrinsic
                        ));
                    self.diagnostics.borrow_mut().push(diag);
                    self.errors.borrow_mut().push(TypeError::InvalidOperation {
                        operation: format!("{:?}#", intrinsic),
                        type_name: "intrinsic".to_string(),
                    });
                }
            }
            Expr::Call(name, _) => {
                let diag = Diagnostic::new("G001", Severity::Error, "FFI not allowed in GPU kernel")
                    .with_explanation(&format!(
                        "Function call `{}()` is not allowed in .abv files. \
                         Use only built-in GPU intrinsics.",
                        name
                    ));
                self.diagnostics.borrow_mut().push(diag);
                self.errors.borrow_mut().push(TypeError::FFIError {
                    message: format!("call '{}' not allowed in .abv", name),
                });
            }
            // Recurse
            Expr::Add(l, r) | Expr::Sub(l, r) | Expr::Mul(l, r) | Expr::Div(l, r) | Expr::Mod(l, r)
            | Expr::Eq(l, r) | Expr::Ne(l, r) | Expr::Lt(l, r) | Expr::Le(l, r) | Expr::Gt(l, r)
            | Expr::Ge(l, r) | Expr::And(l, r) | Expr::Or(l, r) => {
                self.validate_gpu_expr(l);
                self.validate_gpu_expr(r);
            }
            Expr::Not(e) | Expr::Neg(e) | Expr::BitNot(e) => {
                self.validate_gpu_expr(e);
            }
            _ => {}
        }
    }

    fn check_signature(&mut self, sig: &Signature) {
        for input_ty in &sig.input_types() {
            self.validate_type(input_ty);
        }
        match &sig.result_type {
            ResultType::Projection(types) => {
                for ty in types {
                    self.validate_type(ty);
                }
            }
            ResultType::TrueAssertion => {}
            ResultType::VoidType => {}
        }
        // Verify sig projection against bound defn if specified
        if let Some(ref bound_name) = sig.bound_defn {
            if let Some(defn) = self.definitions.get(bound_name) {
                let sig_types = match &sig.result_type {
                    ResultType::Projection(types) => types.clone(),
                    _ => vec![],
                };
                let defn_output = defn.output_type.as_ref()
                    .map(|ot| ot.all_types())
                    .unwrap_or_else(|| defn.outputs.clone());
                for sig_ty in &sig_types {
                    if !defn_output.contains(sig_ty) {
                        self.errors.borrow_mut().push(TypeError::FFIError {
                            message: format!(
                                "sig '{}' projects type {:?} from defn '{}', which produces {:?}",
                                sig.name, sig_ty, bound_name, defn_output
                            ),
                        });
                    }
                }
            } else {
                self.errors.borrow_mut().push(TypeError::FFIError {
                    message: format!(
                        "sig '{}' references defn '{}' which is not defined",
                        sig.name, bound_name
                    ),
                });
            }
        }
    }

    fn check_definition(&mut self, defn: &Definition) {
        self.push_scope();
        for (param_name, param_ty) in &defn.parameters {
            let resolved_ty = self.resolve_type(param_ty.clone());
            self.declare_variable(param_name, resolved_ty);
        }

        let expected_output_types = self.get_expected_output_types(defn);
        for stmt in &defn.body {
            self.check_statement_with_outputs(stmt, None, &expected_output_types);
        }

        self.pop_scope();
    }

    fn get_expected_output_types(&self, defn: &Definition) -> Vec<Type> {
        if let Some(ref output_type) = defn.output_type {
            output_type.all_types()
        } else if !defn.outputs.is_empty() {
            defn.outputs.clone()
        } else {
            vec![]
        }
    }

    fn check_statement_with_outputs(
        &mut self,
        stmt: &Statement,
        is_async: Option<&bool>,
        expected_outputs: &[Type],
    ) {
        match stmt {
            Statement::Term { values: outputs, .. } | Statement::TermBang { values: outputs, .. } => {
                let actual_count = outputs.len();
                let expected_count = expected_outputs.len();

                if expected_count > 0 && actual_count != expected_count {
                    self.errors.borrow_mut().push(TypeError::TypeMismatch {
                        expected: format!("{} outputs", expected_count),
                        found: format!("{} outputs", actual_count),
                        context: "term statement output count".to_string(),
                    });
                }

                for (i, expr_opt) in outputs.iter().enumerate() {
                    if let Some(expr) = expr_opt {
                        let actual_ty = self.infer_expression(expr);
                        if i < expected_outputs.len() {
                            let expected_ty = &expected_outputs[i];
                            if !self.types_compatible(&actual_ty, expected_ty) {
                                self.errors.borrow_mut().push(TypeError::TypeMismatch {
                                    expected: self.type_to_string(expected_ty),
                                    found: self.type_to_string(&actual_ty),
                                    context: format!("term output {}", i),
                                });
                            }
                        }
                        self.check_expr_for_function_calls(expr);
                    }
                }
            }
            _ => self.check_statement(stmt, is_async),
        }
    }

    fn check_expr_for_function_calls(&mut self, expr: &Expr) {
        match expr {
            Expr::Call(func_name, args) => {
                self.verify_term_function_call(func_name, args);
                self.check_call_argument_types(func_name, args);
                for arg in args {
                    self.check_expr_for_function_calls(arg);
                }
            }
            Expr::IntrinsicCall { intrinsic, args } => {
                if let Intrinsic::UserDefined(name) = intrinsic {
                    if !self.inop_decls.contains_key(name) {
                        let diag = crate::errors::Diagnostic::new("U001", crate::errors::Severity::Error, "Unknown user-defined intrinsic")
                            .with_explanation(&format!(
                                "`{}#` is not a known intrinsic. Did you forget to declare `inop {}`?",
                                name, name
                            ));
                        self.diagnostics.borrow_mut().push(diag);
                        self.errors.borrow_mut().push(crate::errors::TypeError::InvalidOperation {
                            operation: format!("{}#", name),
                            type_name: "intrinsic".to_string(),
                        });
                    }
                }
                for arg in args {
                    self.check_expr_for_function_calls(arg);
                }
            }
            Expr::Add(left, right)
            | Expr::Sub(left, right)
            | Expr::Mul(left, right)
            | Expr::Div(left, right)
            | Expr::Mod(left, right)
            | Expr::Eq(left, right)
            | Expr::Ne(left, right)
            | Expr::Lt(left, right)
            | Expr::Le(left, right)
            | Expr::Gt(left, right)
            | Expr::Ge(left, right)
            | Expr::Or(left, right)
            | Expr::And(left, right)
            | Expr::BitAnd(left, right)
            | Expr::BitOr(left, right)
            | Expr::BitXor(left, right)
            | Expr::Shl(left, right)
            | Expr::Shr(left, right) => {
                self.check_expr_for_function_calls(left);
                self.check_expr_for_function_calls(right);
            }
            Expr::Not(inner) | Expr::Neg(inner) | Expr::BitNot(inner) => {
                self.check_expr_for_function_calls(inner);
            }
            Expr::FieldAccess(obj, _) => {
                self.check_expr_for_function_calls(obj);
            }
            Expr::CellCall(callee, args) => {
                self.check_expr_for_function_calls(callee);
                for arg in args {
                    self.check_expr_for_function_calls(arg);
                }
            }
            Expr::IsType(expr, _) | Expr::FromCheck(expr, _) => {
                self.check_expr_for_function_calls(expr);
            }
            Expr::Like(l, r) => {
                self.check_expr_for_function_calls(l);
                self.check_expr_for_function_calls(r);
            }
            Expr::ListLiteral(elems) => {
                for elem in elems {
                    self.check_expr_for_function_calls(elem);
                }
            }
            _ => {}
        }
    }

    fn verify_term_function_call(&mut self, func_name: &str, args: &[Expr]) {
        let defn = match self.definitions.get(func_name) {
            Some(d) => d,
            None => return,
        };

        let postcond = &defn.contract.post_condition;
        if !self.expr_has_result(postcond) {
            return;
        }

        let precond = &defn.contract.pre_condition;
        let mut state = symbolic::SymbolicState::new(precond);

        for (i, (param_name, _)) in defn.parameters.iter().enumerate() {
            if i < args.len() {
                state.assign(param_name, &args[i]);
            }
        }

        let verified = symbolic::satisfies_postcondition(postcond, &state);
        let postcond_str = format!("{:?}", postcond);
        if verified {
            self.diagnostics.borrow_mut().push(
                Diagnostic::new(
                    "V101",
                    Severity::Info,
                    "Function call postcondition verified",
                )
                .with_explanation(&format!(
                    "term {} uses function '{}' which guarantees {} (symbolically verified)",
                    func_name, func_name, postcond_str
                )),
            );
        } else {
            self.diagnostics.borrow_mut().push(
                Diagnostic::new("V102", Severity::Warning, "Function call postcondition may not be satisfied")
                    .with_explanation(&format!(
                        "term {} uses function '{}' with postcondition {} - could not verify symbolically",
                        func_name, func_name, postcond_str
                    )),
            );
        }
    }

    fn check_call_argument_types(&mut self, func_name: &str, args: &[Expr]) {
        // Collect parameter types from definition, transaction, signature, or foreign binding
        let params: Option<Vec<Type>> = if let Some(defn) = self.definitions.get(func_name) {
            Some(defn.parameters.iter().map(|(_, t)| t.clone()).collect())
        } else if let Some(txn) = self.transactions.get(func_name) {
            Some(txn.parameters.iter().map(|(_, t)| t.clone()).collect())
        } else if let Some(sig) = self.signatures.get(func_name) {
            Some(sig.params.iter().map(|(_, t)| t.clone()).collect())
        } else if let Some(fb) = self.foreign_bindings.get(func_name) {
            Some(fb.inputs.iter().map(|(_, t)| t.clone()).collect())
        } else {
            None
        };

        let params = match params {
            Some(p) => p,
            None => return,
        };

        for (i, param_type) in params.iter().enumerate() {
            if i >= args.len() {
                break;
            }
            let arg_type = self.infer_expression(&args[i]);
            if !self.types_compatible(&arg_type, param_type) {
                self.errors.borrow_mut().push(TypeError::TypeMismatch {
                    expected: self.type_to_string(param_type),
                    found: self.type_to_string(&arg_type),
                    context: format!("argument {} of call to '{}'", i, func_name),
                });
            }
        }
    }

    fn expr_has_result(&self, expr: &Expr) -> bool {
        match expr {
            Expr::Term => true,
            Expr::Identifier(name) => name == "result",
            Expr::Eq(l, r)
            | Expr::Ne(l, r)
            | Expr::Lt(l, r)
            | Expr::Le(l, r)
            | Expr::Gt(l, r)
            | Expr::Ge(l, r)
            | Expr::Add(l, r)
            | Expr::Sub(l, r)
            | Expr::Mul(l, r)
            | Expr::Div(l, r)
            | Expr::Mod(l, r)
            | Expr::And(l, r)
            | Expr::Or(l, r) => self.expr_has_result(l) || self.expr_has_result(r),
            Expr::Not(inner) => self.expr_has_result(inner),
            Expr::Call(_, args) => args.iter().any(|a| self.expr_has_result(a)),
            Expr::IntrinsicCall { intrinsic: _, args } => args.iter().any(|a| self.expr_has_result(a)),
            _ => false,
        }
    }

    fn check_transaction(&mut self, txn: &Transaction) {
        self.push_scope();
        
        for (param_name, param_ty) in &txn.parameters {
            let resolved_ty = self.resolve_type(param_ty.clone());
            self.declare_variable(param_name, resolved_ty);
        }

        for stmt in &txn.body {
            self.check_statement(stmt, Some(&txn.is_async));
        }
        self.pop_scope();
    }

    fn check_frgn_binding(
        &mut self,
        name: &str,
        toml_path: &str,
        signature: &mut ForeignSignature,
    ) {
        // Pipe syntax frgns skip TOML binding validation entirely.
        // The fallback expression provides the error value directly.
        if signature.is_pipe {
            // Validate the fallback expression is compile-time evaluable
            if let Some(ref fallback) = signature.fallback {
                if !Self::is_compile_time_expr(fallback) {
                    self.errors.borrow_mut().push(TypeError::FFIError {
                        message: format!(
                            "Fallback expression in frgn '{}' must be compile-time evaluable \
                             (literals, constructor calls with literal args). Found non-evaluable \
                             expression: {:?}",
                            name, fallback
                        ),
                    });
                }
            }
            return;
        }

        // If toml_path is empty, we're using the new FFI syntax with profile-based resolution
        // Skip binding loading and use profile defaults
        if toml_path.is_empty() {
            if signature.location.is_empty() {
                signature.location = format!("<profile:{}>", name);
            } else {
                // Validate from "..." value is a known FFI target language
                let known_targets = ["c", "rust", "js", "python"];
                if !known_targets.contains(&signature.location.as_str()) {
                    self.errors.borrow_mut().push(TypeError::FFIError {
                        message: format!(
                            "unrecognized FFI target '{}' for '{}' — valid targets: {}",
                            signature.location, name,
                            known_targets.join(", ")
                        ),
                    });
                }
            }
            return;
        }

        let resolved_path = match ffi::resolver::resolve_binding_path(
            toml_path,
            &None,
            &Some(self.current_file.clone()),
            self.no_stdlib,
            &self.custom_stdlib_path,
        ) {
            Ok(path) => path,
            Err(err) => {
                self.diagnostics.borrow_mut().push(
                    Diagnostic::new(
                        "F001",
                        Severity::Error,
                        "FFI binding path resolution failed",
                    )
                    .with_explanation(&format!(
                        "Failed to resolve binding path '{}': {}",
                        toml_path, err
                    )),
                );
                self.errors.borrow_mut().push(TypeError::FFIError {
                    message: format!("Path resolution failed for '{}': {}", name, err),
                });
                return;
            }
        };

        let bindings = match ffi::loader::load_binding(&resolved_path) {
            Ok(b) => b,
            Err(err) => {
                self.diagnostics.borrow_mut().push(
                    Diagnostic::new("F002", Severity::Error, "FFI binding file load failed")
                        .with_explanation(&format!(
                            "Failed to load binding file '{}': {}",
                            toml_path, err
                        )),
                );
                self.errors.borrow_mut().push(TypeError::FFIError {
                    message: format!("Failed to load binding file for '{}': {}", name, err),
                });
                return;
            }
        };

        let primary_binding = bindings.iter().find(|b| b.name == name);
        let binding = match primary_binding {
            Some(b) => b,
            None => {
                self.diagnostics.borrow_mut().push(
                    Diagnostic::new("F003", Severity::Error, "FFI binding not found")
                        .with_explanation(&format!(
                            "No binding found for '{}' in '{}'",
                            name, toml_path
                        )),
                );
                self.errors.borrow_mut().push(TypeError::FFIError {
                    message: format!("Binding '{}' not found in '{}'", name, toml_path),
                });
                return;
            }
        };

        signature.error_fields = binding.error_fields.clone();
        signature.location = binding.location.clone();
        signature.input_layout = binding.input_layout.clone();
        signature.output_layout = binding.output_layout.clone();
        signature.precondition = binding.precondition.clone();
        signature.postcondition = binding.postcondition.clone();
        signature.buffer_mode = binding.buffer_mode.clone();

        if let Err(err) = ffi::validator::validate_frgn_against_binding(signature, binding) {
            self.diagnostics.borrow_mut().push(
                Diagnostic::new("F004", Severity::Error, "FFI binding validation failed")
                    .with_explanation(&format!(
                        "The frgn declaration for '{}' does not match its TOML binding: {}",
                        name, err
                    )),
            );
            self.errors.borrow_mut().push(TypeError::FFIError {
                message: format!("Binding validation failed for '{}': {}", name, err),
            });
        }
    }

    /// Check if an expression is compile-time evaluable (no variable references,
    /// no runtime state, no function calls to user-defined functions).
    /// Used to validate pipe frgn fallback expressions.
    fn is_compile_time_expr(expr: &Expr) -> bool {
        match expr {
            // Literals: always compile-time
            Expr::Integer(_) | Expr::Float(_) | Expr::String(_)
            | Expr::Bool(_) | Expr::Char(_) | Expr::Term
            | Expr::RegexLiteral(_) => true,
            // Constructor calls: allowed if all args are compile-time
            Expr::Call(_, args) => args.iter().all(|a| Self::is_compile_time_expr(a)),
            // Tuples of compile-time items
            Expr::Tuple(items) => items.iter().all(|i| Self::is_compile_time_expr(i)),
            // List literal of compile-time items
            Expr::ListLiteral(items) => items.iter().all(|i| Self::is_compile_time_expr(i)),
            // Disallow variable/function references and runtime state
            Expr::Identifier(_) | Expr::OwnedRef(_) | Expr::PriorState(_) => false,
            // Everything else (operators, blocks, etc.) is rejected for safety
            _ => false,
        }
    }

    fn check_statement(&mut self, stmt: &Statement, is_async: Option<&bool>) {
        match stmt {
            Statement::Assignment { lhs, expr, timeout, .. } => {
                match lhs {
                    Expr::TupleDestructure(names, _) => {
                        self.check_expr_for_ffi_errors(expr);
                        let expr_ty = self.infer_expression(expr);
                        if let Type::Tuple(elem_types) = &expr_ty {
                            for (i, name) in names.iter().enumerate() {
                                if let Some(var_ty) = self.lookup_variable(name) {
                                    if i < elem_types.len() && !self.types_compatible(&var_ty, &elem_types[i]) {
                                        self.errors.borrow_mut().push(TypeError::TypeMismatch {
                                            expected: self.type_to_string(&var_ty),
                                            found: self.type_to_string(&elem_types[i]),
                                            context: format!("tuple destructuring assignment for '{}'", name),
                                        });
                                    }
                                }
                            }
                        } else {
                            self.errors.borrow_mut().push(TypeError::TypeMismatch {
                                expected: "Tuple type".to_string(),
                                found: self.type_to_string(&expr_ty),
                                context: "tuple destructuring assignment".to_string(),
                            });
                        }
                    }
                    _ => {
                        self.check_expr_for_ffi_errors(lhs);
                        self.check_expr_for_ffi_errors(expr);
                        let lhs_ty = self.infer_expression(lhs);
                        let expr_ty = self.infer_expression(expr);

                        if let Some((_t_expr, _unit)) = timeout {
                            if !self.is_error_union(&lhs_ty) {
                                self.errors.borrow_mut().push(TypeError::TypeMismatch {
                                    expected: "Union type containing Error".to_string(),
                                    found: self.type_to_string(&lhs_ty),
                                    context: "assignment with timeout".to_string(),
                                });
                            }
                        }

                        if !self.check_geometry(&lhs_ty, &expr_ty) {
                            if let Expr::OwnedRef(var_name) = lhs {
                                if self.trigger_names.contains(var_name.as_str()) {
                                    self.errors.borrow_mut().push(TypeError::TypeMismatch {
                                        expected: "regular variable".to_string(),
                                        found: "trigger variable".to_string(),
                                        context: format!("cannot assign to trigger '{}' — triggers are read-only", var_name),
                                    });
                                }
                                if self.lookup_variable(var_name).is_none() {
                                    self.declare_variable(var_name, expr_ty.clone());
                                } else {
                                    self.errors.borrow_mut().push(TypeError::TypeMismatch {
                                        expected: self.type_to_string(&lhs_ty),
                                        found: self.type_to_string(&expr_ty),
                                        context: "assignment".to_string(),
                                    });
                                }
                            } else {
                                self.errors.borrow_mut().push(TypeError::TypeMismatch {
                                    expected: self.type_to_string(&lhs_ty),
                                    found: self.type_to_string(&expr_ty),
                                    context: "assignment".to_string(),
                                });
                            }
                        }
                    }
                }
            }
            Statement::Let { name, ty, expr, .. } => {
                // Handle tuple destructuring: let (a, b) = expr;
                // Parser stores names as comma-separated string
                let names: Vec<&str> = name.split(',').collect();
                if names.len() > 1 {
                    // Tuple destructuring
                    let expr_ty = expr.as_ref().map(|e| self.infer_expression(e));
                    if let (Some(expr), Some(Type::Tuple(types))) = (expr, expr_ty) {
                        if types.len() == names.len() {
                            for (var_name, var_ty) in names.iter().zip(types.iter()) {
                                self.declare_variable(var_name.trim(), var_ty.clone());
                            }
                        }
                    }
                } else {
                    let inferred_expr_ty = expr.as_ref().map(|e| {
                        self.check_expr_for_ffi_errors(e);
                        self.infer_expression(e)
                    });
                    let final_ty = ty.clone().or(inferred_expr_ty.clone());
                    if let Some(final_type) = final_ty {
                        if let (Some(_), Some(expr_ty)) = (expr, &inferred_expr_ty) {
                            if !self.types_compatible(expr_ty, &final_type) {
                                self.errors.borrow_mut().push(TypeError::TypeMismatch {
                                    expected: self.type_to_string(&final_type),
                                    found: self.type_to_string(expr_ty),
                                    context: format!("let {}", name),
                                });
                            }
                        }
                        self.declare_variable(name, final_type);
                    }
                }
            }
            Statement::Guarded {
                condition,
                statements,
            } => {
                let cond_ty = self.infer_expression(condition);
                if !self.types_compatible(&cond_ty, &Type::Bool) {
                    self.errors.borrow_mut().push(TypeError::TypeMismatch {
                        expected: "Bool".to_string(),
                        found: self.type_to_string(&cond_ty),
                        context: "guard condition".to_string(),
                    });
                }
                for s in statements {
                    self.check_statement(s, is_async);
                }
            }
            Statement::LocalTrigger { name, ty, expr, .. } => {
                // Local trigger: trg! name: Type = expr;
                // Type-check the expression if present
                if let Some(e) = expr {
                    self.check_expr_for_ffi_errors(e);
                    let expr_ty = self.infer_expression(e);
                    if !self.types_compatible(&expr_ty, ty) {
                        self.errors.borrow_mut().push(TypeError::TypeMismatch {
                            expected: self.type_to_string(ty),
                            found: self.type_to_string(&expr_ty),
                            context: format!("trg! {} (local trigger)", name),
                        });
                    }
                }
                // Declare the trigger variable in the local transaction scope
                self.declare_variable(name, ty.clone());
            }
            Statement::OnExit { body, .. } => {
                for stmt in body {
                    self.check_statement(stmt, is_async);
                }
            }
            Statement::Alka(_) => {} // opaque passthrough, no validation
            Statement::Await { expr, .. } => {
                self.check_async_await_callable(expr);
                self.infer_expression(expr);
            }
            Statement::Async { body, .. } => {
                self.check_statement(body, is_async);
            }
            Statement::AsyncAwait { body, lhs, .. } => {
                if let Statement::Expression(expr) = body.as_ref() {
                    let return_ty = self.infer_expression(expr);
                    if let Some(name) = lhs {
                        if let Some(decl_ty) = self.lookup_variable(name) {
                            if !self.types_compatible(&decl_ty, &return_ty) {
                                self.errors.borrow_mut().push(TypeError::TypeMismatch {
                                    expected: self.type_to_string(&decl_ty),
                                    found: self.type_to_string(&return_ty),
                                    context: format!("async await let {} = ...", name),
                                });
                            }
                        } else {
                            self.declare_variable(name.as_str(), return_ty);
                        }
                    }
                } else {
                    self.check_statement(body, is_async);
                }
            }
            Statement::TrgBinding { name, ty, instance, port, .. } => {
                let instance_ty = self.infer_expression(instance);
                if !self.types_compatible(&instance_ty, &Type::Int) {
                    self.errors.borrow_mut().push(TypeError::TypeMismatch {
                        expected: "Int (cell instance handle)".to_string(),
                        found: self.type_to_string(&instance_ty),
                        context: format!("trg binding '{}'", name),
                    });
                }
                if let Some(decl_ty) = ty {
                    self.declare_variable(name, decl_ty.clone());
                } else {
                    self.declare_variable(name, Type::Int);
                }
            }
            _ => {}
        }
    }

    pub(crate) fn infer_expression(&self, expr: &Expr) -> Type {
        match expr {
            Expr::Integer(_) => Type::Int,
            Expr::Float(_) => {
                match self.target {
                    CompilationTarget::Verilog => {
                        self.errors.borrow_mut().push(TypeError::TypeMismatch {
                            expected: "Fixed-point or Integer".to_string(),
                            found: "Float".to_string(),
                            context: "Verilog synthesis".to_string(),
                        });
                    }
                    CompilationTarget::Embedded => {
                        self.errors.borrow_mut().push(TypeError::TypeMismatch {
                            expected: "Fixed-point or Integer".to_string(),
                            found: "Float".to_string(),
                            context: "Embedded target (no FPU)".to_string(),
                        });
                    }
                    CompilationTarget::Circuit => {
                        self.errors.borrow_mut().push(TypeError::TypeMismatch {
                            expected: "Fixed-point or Integer".to_string(),
                            found: "Float".to_string(),
                            context: "CIRCT hardware synthesis".to_string(),
                        });
                    }
                    _ => {}
                }
                Type::Float
            }
            Expr::String(_) => Type::String,
            Expr::RegexLiteral(_) => Type::String,
            Expr::Char(_) => Type::Char,
            Expr::Bool(_) => Type::Bool,
            Expr::Identifier(name) | Expr::OwnedRef(name) | Expr::PriorState(name) => self
                .lookup_variable(name)
                .unwrap_or(Type::Custom(name.clone())),
            Expr::Add(l, r) | Expr::Sub(l, r) | Expr::Mul(l, r) | Expr::Div(l, r) | Expr::Mod(l, r) => {
                self.binary_op_type(l, r, Type::Int, Type::Float)
            }
            Expr::Eq(_, _)
            | Expr::Ne(_, _)
            | Expr::Lt(_, _)
            | Expr::Le(_, _)
            | Expr::Gt(_, _)
            | Expr::Ge(_, _)
            | Expr::Or(_, _)
            | Expr::And(_, _) => Type::Bool,
            Expr::Not(e) | Expr::Neg(e) | Expr::BitNot(e) => self.infer_expression(e),
            Expr::IntrinsicCall { intrinsic, args } => {
                for arg in args {
                    self.infer_expression(arg);
                }
                match intrinsic {
                    Intrinsic::Sqrt | Intrinsic::Fabs | Intrinsic::Ceil | Intrinsic::Floor => Type::Float,
                    Intrinsic::Ctpop | Intrinsic::Ctlz | Intrinsic::Cttz | Intrinsic::Abs | Intrinsic::Bitreverse => Type::Int,
                    Intrinsic::ByteCount | Intrinsic::Size | Intrinsic::Strlen => Type::Int,
                    Intrinsic::StrBytes => Type::Custom("List".to_string()),
                    Intrinsic::Pop => Type::Int,
                    Intrinsic::Contains => Type::Bool,
                    Intrinsic::Keys | Intrinsic::Values => Type::Custom("List".to_string()),
                    Intrinsic::Println | Intrinsic::WriteFile | Intrinsic::Sleep | Intrinsic::Bind | Intrinsic::Listen => Type::Bool,
                    Intrinsic::Readln | Intrinsic::ReadFile => Type::String,
                    Intrinsic::Exit | Intrinsic::Halt => Type::Bool,
                    Intrinsic::Time | Intrinsic::Socket | Intrinsic::Accept => Type::Int,
                    Intrinsic::Sort | Intrinsic::Reverse => Type::Bool,
                    Intrinsic::Range => Type::Custom("List".to_string()),
                    // String intrinsics (2026-06-18)
                    Intrinsic::Print => Type::Bool,
                    Intrinsic::TrimLeft | Intrinsic::TrimRight | Intrinsic::ToLower => Type::String,
                    Intrinsic::ContainsAt => Type::Bool,
                    Intrinsic::FindFrom => Type::Int,
                    Intrinsic::SplitN => Type::Custom("List".to_string()),
                    Intrinsic::IntToStr => Type::String,
                    Intrinsic::TtyRawMode | Intrinsic::TtySize => Type::Int,
                    Intrinsic::TtyReadKey => Type::Int,
                    Intrinsic::IoCtl => Type::Int,
                    Intrinsic::IsTty => Type::Bool,
                    Intrinsic::SpawnWithOutput => Type::String,
                    Intrinsic::Spawn => Type::Int,
                    // Phase B: Raw File I/O — all return Int (fd, bytes, or -1)
                    Intrinsic::Open | Intrinsic::Close | Intrinsic::Read
                    | Intrinsic::Write | Intrinsic::LSeek | Intrinsic::PRead
                    | Intrinsic::PWrite | Intrinsic::Stat | Intrinsic::FStat
                    | Intrinsic::FTruncate | Intrinsic::FSync
                    | Intrinsic::FDup | Intrinsic::FDup2 | Intrinsic::FCntl => Type::Int,
                    // Phase C: Filesystem
                    Intrinsic::ReadLink | Intrinsic::GetCwd => Type::String,
                    Intrinsic::ReadDir => Type::Custom("List".to_string()),
                    Intrinsic::MkDir | Intrinsic::RmDir | Intrinsic::Unlink
                    | Intrinsic::Rename | Intrinsic::SymLink | Intrinsic::Link
                    | Intrinsic::ChDir | Intrinsic::ChMod | Intrinsic::ChOwn
                    | Intrinsic::UMask | Intrinsic::Access => Type::Int,
                    // Phase D: Memory + Sync — all return Int
                    Intrinsic::Mmap | Intrinsic::MUnmap | Intrinsic::MProtect
                    | Intrinsic::Brk | Intrinsic::MLock | Intrinsic::AtomicLoad
                    | Intrinsic::AtomicStore | Intrinsic::AtomicCas
                    | Intrinsic::AtomicXchg | Intrinsic::AtomicAdd
                    | Intrinsic::Fence | Intrinsic::Futex => Type::Int,
                    // Phase E: IPC — all return Int
                    Intrinsic::Pipe | Intrinsic::ShmOpen | Intrinsic::ShmUnlink
                    | Intrinsic::SemOpen | Intrinsic::SemWait | Intrinsic::SemPost => Type::Int,
                    // Phase F: Signals — all return Int
                    Intrinsic::SigAction | Intrinsic::SigProcMask | Intrinsic::Kill
                    | Intrinsic::SignalFd | Intrinsic::TimerFdCreate => Type::Int,
                    // Phase G: Networking — all return Int
                    Intrinsic::Socket | Intrinsic::Bind | Intrinsic::Listen
                    | Intrinsic::Accept | Intrinsic::Connect | Intrinsic::Send
                    | Intrinsic::Recv | Intrinsic::SendTo | Intrinsic::RecvFrom
                    | Intrinsic::SetSockOpt | Intrinsic::GetSockOpt | Intrinsic::Shutdown
                    | Intrinsic::GetAddrInfo => Type::Int,
                    // Phase H: Everything Else (intrinsics.md D6, D7)
                    Intrinsic::GetEnv => Type::String,
                    Intrinsic::SetEnv | Intrinsic::UnsetEnv => Type::Bool,
                    Intrinsic::GetPid | Intrinsic::GetPPid | Intrinsic::ClockGetTime
                    | Intrinsic::NanoSleep => Type::Int,
                    // Benchmark intrinsics (2026-06-16)
                    Intrinsic::PrintInt | Intrinsic::PutChar | Intrinsic::PrintFloat => Type::Bool,
                    Intrinsic::GetEnvInt => Type::Int,
                    // Math intrinsics
                    Intrinsic::Sin | Intrinsic::Cos | Intrinsic::Pow => Type::Float,
                    // GPU compute intrinsics (2026-06-18)
                    Intrinsic::GetGlobalId | Intrinsic::GetLocalId
                    | Intrinsic::GetGroupId | Intrinsic::GetNumGroups => Type::Int,
                    Intrinsic::SubGroupBarrier => Type::Bool,
                    // String conversion intrinsics
                    Intrinsic::FloatToStr | Intrinsic::ToStr => Type::String,
                    // D12: Random / Entropy
                    Intrinsic::Errno => Type::Int,
                    Intrinsic::GetRandom => Type::Int,
                    // D13: System Info
                    Intrinsic::Hostname | Intrinsic::Uname
                    | Intrinsic::StrError | Intrinsic::StrSignal
                    | Intrinsic::RealPath => Type::String,
                    Intrinsic::PageSize | Intrinsic::CpuCount => Type::Int,
                    // D14: Debugging
                    Intrinsic::Abort => Type::Void,
                    Intrinsic::Backtrace => Type::Custom("List".to_string()),
                    // D15: Scheduling
                    Intrinsic::SchedYield | Intrinsic::GetPriority
                    | Intrinsic::SetPriority => Type::Int,
                    // D16: User / Group
                    Intrinsic::GetUid | Intrinsic::GetEUid
                    | Intrinsic::GetGid | Intrinsic::GetEGid => Type::Int,
                    Intrinsic::GetPwUid | Intrinsic::GetGrGid => Type::String,
                    // D17: Threading
                    Intrinsic::ThreadCreate | Intrinsic::ThreadJoin
                    | Intrinsic::MutexLock | Intrinsic::MutexUnlock
                    | Intrinsic::CondvarWait | Intrinsic::CondvarSignal
                    | Intrinsic::CondvarBroadcast => Type::Int,
                    Intrinsic::ThreadExit => Type::Void,
                    // D18: Resource Limits
                    Intrinsic::GetRlimit | Intrinsic::SetRlimit => Type::Int,
                    // Extra intrinsics
                    Intrinsic::MkStemp | Intrinsic::DlOpen
                    | Intrinsic::DlSym | Intrinsic::DlClose => Type::Int,
                    Intrinsic::MkDtemp | Intrinsic::TtyName => Type::String,
                    // Macro/template intrinsics (compile-time only)
                    Intrinsic::Compile | Intrinsic::MacroError
                    | Intrinsic::MacroWarn | Intrinsic::MacroGenSym => Type::Data,
                    Intrinsic::UserDefined(name) => {
                        self.inop_decls.get(name)
                            .map(|d| {
                                if d.outputs.len() > 1 {
                                    Type::Tuple(d.outputs.clone())
                                } else {
                                    d.outputs.first().cloned().unwrap_or(Type::Void)
                                }
                            })
                            .unwrap_or(Type::Void)
                    }
                }
            }
            Expr::Call(name, args) => {
                if let Some(fb) = self.foreign_bindings.get(name) {
                    fb.success_output
                        .first()
                        .map(|(_, ty)| ty.clone())
                        .unwrap_or(Type::Void)
                } else if let Some(sig) = self.signatures.get(name) {
                    // Handle generic type substitution for signatures
                    let result_types = match &sig.result_type {
                        ResultType::Projection(types) => types.clone(),
                        ResultType::TrueAssertion => vec![Type::Bool],
                        ResultType::VoidType => vec![Type::Void],
                    };
                    // Try to substitute TypeVars based on input types
                    if !args.is_empty() && !sig.input_types().is_empty() {
                        let arg_ty = self.infer_expression(&args[0]);
                        let substitutions = self.extract_type_substitutions(&sig.input_types()[0], &arg_ty);
                        if !substitutions.is_empty() {
                            if let Some(ty) = result_types.first() {
                                return self.substitute_type_vars(ty, &substitutions);
                            }
                        }
                    }
                    result_types.first().cloned().unwrap_or(Type::Void)
                } else if let Some(defn) = self.definitions.get(name) {
                    if let Some(ref output_type) = defn.output_type {
                        output_type.all_types().first().cloned().unwrap_or(Type::Void)
                    } else if !defn.outputs.is_empty() {
                        defn.outputs.first().cloned().unwrap_or(Type::Void)
                    } else {
                        Type::Void
                    }
                } else if let Some(txn) = self.transactions.get(name) {
                    if let Some(ref output_type) = txn.output_type {
                        output_type.all_types().first().cloned().unwrap_or(Type::Void)
                    } else if !txn.outputs.is_empty() {
                        txn.outputs.first().cloned().unwrap_or(Type::Void)
                    } else {
                        Type::Void
                    }
                } else if name == "Ok" {
                    // Ok(value) -> Result<T, E> where T is inferred from value
                    let ok_type = args.first().map(|e| self.infer_expression(e)).unwrap_or(Type::TypeVar("T".to_string()));
                    Type::Applied("Result".to_string(), vec![ok_type, Type::TypeVar("E".to_string())])
                } else if name == "Err" {
                    // Err(value) -> Result<T, E> where E is inferred from value
                    let err_type = args.first().map(|e| self.infer_expression(e)).unwrap_or(Type::TypeVar("E".to_string()));
                    Type::Applied("Result".to_string(), vec![Type::TypeVar("T".to_string()), err_type])
                } else if name == "Some" {
                    // Some(value) -> Option<T> where T is inferred from value
                    let some_type = args.first().map(|e| self.infer_expression(e)).unwrap_or(Type::TypeVar("T".to_string()));
                    Type::Applied("Option".to_string(), vec![some_type])
                } else if name == "None" {
                    // None -> Option<T>
                    Type::Applied("Option".to_string(), vec![Type::TypeVar("T".to_string())])
                } else if let Some(enum_name) = self.enum_variants.get(name) {
                    Type::Custom(enum_name.clone())
                } else {
                    Type::Custom(name.clone())
                }
            }
            Expr::CellCall(callee, args) => {
                for arg in args {
                    self.infer_expression(arg);
                }
                if let Expr::Identifier(name) = callee.as_ref() {
                    if let Some(cell_def) = self.cell_defs.get(name) {
                        if let Some(ref ot) = cell_def.output_type {
                            let ret_ty = self.output_type_to_type(ot);
                            // For union output types, return the union
                            ret_ty
                        } else {
                            Type::Void
                        }
                    } else {
                        Type::Custom("unknown".to_string())
                    }
                } else {
                    Type::Custom("unknown".to_string())
                }
            }
            Expr::ListLiteral(elements) => {
                let elem_type = elements
                    .first()
                    .map(|e| self.infer_expression(e))
                    .unwrap_or(Type::TypeVar("T".to_string()));
                Type::Applied("List".to_string(), vec![elem_type])
            }
            Expr::ListIndex(list_expr, _) => {
                match self.infer_expression(list_expr) {
                Type::Applied(_, args) if !args.is_empty() => args[0].clone(),
                Type::Vector(inner, dims) => {
                    // For multidimensional, indexing returns the inner type with one fewer dimension
                    if dims.len() > 1 {
                        Type::Vector(inner.clone(), dims[1..].to_vec())
                    } else {
                        *inner.clone()
                    }
                }
                Type::Tuple(types) => {
                    // Indexing a tuple: try to infer element type at index
                    // If index is compile-time known, return that type; else unify
                    types.first().cloned().unwrap_or(Type::TypeVar("T".to_string()))
                }
                _ => Type::TypeVar("T".to_string()),
                }
            },
            Expr::Slice { value, mask, .. } => {
                if let Some(mask_expr) = mask {
                    let mask_type = self.infer_expression(mask_expr);
                    if mask_type != Type::Bool {
                        self.errors.borrow_mut().push(TypeError::TypeMismatch {
                            expected: "Bool".to_string(),
                            found: format!("{:?}", mask_type),
                            context: "Slice mask expression".to_string(),
                        });
                    }
                }
                self.infer_expression(value)
            },
            Expr::MultiSlice { value, .. } => {
                self.infer_expression(value)
            },
            Expr::FieldAccess(obj, field) => {
                // Look up the field type from the struct definition
                let obj_ty = self.infer_expression(obj);
                if let Type::Custom(struct_name) = &obj_ty {
                    if let Some(fields) = self.struct_fields.get(struct_name) {
                        if let Some(field_ty) = fields.get(field) {
                            // Check field visibility
                            self.enforce_field_visibility(struct_name, field);
                            return field_ty.clone();
                        }
                    }
                }
                // Fallback: return the object type
                obj_ty
            },
            Expr::StructInstance(name, _fields) => {
                Type::Custom(name.clone())
            },
            Expr::Tuple(elements) => {
                let types: Vec<Type> = elements.iter().map(|e| self.infer_expression(e)).collect();
                Type::Tuple(types)
            },
            Expr::TupleDestructure(_names, expr) => {
                self.infer_expression(expr)
            },
            Expr::SubtypeProjection { source, ops } => {
                // Infer source type; validate ops will be refined later
                let src_ty = self.infer_expression(source);
                // For aggregates, return Int; for collections, return source type
                let is_aggregate = ops.iter().any(|op| matches!(op, SubtypeOp::Count | SubtypeOp::Sum(_) | SubtypeOp::Avg(_) | SubtypeOp::Min(_) | SubtypeOp::Max(_)));
                if is_aggregate { Type::Int } else { src_ty }
            },
            Expr::BitAnd(l, r) | Expr::BitOr(l, r) | Expr::BitXor(l, r) | Expr::Shl(l, r) | Expr::Shr(l, r) => {
                let l_ty = self.infer_expression(l);
                let r_ty = self.infer_expression(r);
                if self.types_compatible(&l_ty, &Type::Int) || self.types_compatible(&l_ty, &Type::UInt) {
                    l_ty
                } else {
                    Type::Int
                }
            },
            Expr::Projection { source, target } => {
                let src_ty = self.infer_expression(source);
                match target {
                    ProjectionTarget::Size => {
                        match &src_ty {
                            Type::Int | Type::Float | Type::Bool | Type::Char => {}
                            Type::Applied(name, _) if name == "List" || name == "Vector" || name == "String" => {}
                            Type::Custom(n) if n == "String" || n == "str" => {}
                            Type::String => {}
                            _ => {
                                self.errors.borrow_mut().push(TypeError::TypeMismatch {
                                    expected: "List, Vector, or String".to_string(),
                                    found: self.type_to_string(&src_ty),
                                    context: "Size projection".to_string(),
                                });
                            }
                        }
                        Type::Int
                    }
                    ProjectionTarget::Bytes => Type::Int,
                    ProjectionTarget::Ptr => {
                        // &x :> Ptr → Ptr<typeof(x)>
                        // list :> Ptr → Ptr<element_type>
                        // ptr :> Ptr on Ptr<T> → Int (escape hatch to raw address)
                        match &src_ty {
                            Type::Applied(name, inner) if name == "Ptr" => {
                                // ptr :> Ptr on a Ptr<T> → raw Int address
                                Type::Int
                            }
                            Type::Applied(name, inner) if name == "List" || name == "Vector" => {
                                // list :> Ptr → Ptr<element_type>
                                let elem_ty = inner.first().cloned().unwrap_or(Type::Int);
                                Type::Applied("Ptr".to_string(), vec![elem_ty])
                            }
                            Type::String => {
                                // string :> Ptr → Ptr<Char>
                                Type::Applied("Ptr".to_string(), vec![Type::Char])
                            }
                            Type::Custom(n) if n == "String" || n == "str" => {
                                // string :> Ptr → Ptr<Char>
                                Type::Applied("Ptr".to_string(), vec![Type::Char])
                            }
                            _ => {
                                // &x :> Ptr → Ptr<typeof(x)>
                                Type::Applied("Ptr".to_string(), vec![src_ty.clone()])
                            }
                        }
                    }
                    ProjectionTarget::Alignment => Type::Int,
                    ProjectionTarget::Range => Type::Int,
                    ProjectionTarget::Popcount |
                    ProjectionTarget::LeadingZeros |
                    ProjectionTarget::TrailingZeros |
                    ProjectionTarget::BitReverse => {
                        match &src_ty {
                            Type::Int | Type::UInt => {}
                            _ => {
                                self.errors.borrow_mut().push(TypeError::TypeMismatch {
                                    expected: "Int or UInt".to_string(),
                                    found: self.type_to_string(&src_ty),
                                    context: format!("{:?} projection", target),
                                });
                            }
                        }
                        Type::Int
                    }
                    ProjectionTarget::Absolute => {
                        match &src_ty {
                            Type::Int | Type::UInt | Type::Float => {}
                            _ => {
                                self.errors.borrow_mut().push(TypeError::TypeMismatch {
                                    expected: "Int, UInt, or Float".to_string(),
                                    found: self.type_to_string(&src_ty),
                                    context: "Absolute projection".to_string(),
                                });
                            }
                        }
                        Type::Int
                    }
                    ProjectionTarget::Type => {
                        // Type projection returns the type itself as a value
                        Type::Int
                    }
                    ProjectionTarget::PtrBang => {
                        // Raw pointer — returns Int address
                        Type::Int
                    }
                    ProjectionTarget::Keys => {
                        match &src_ty {
                            Type::Applied(name, _) if name == "HashMap" => {}
                            _ => {
                                self.errors.borrow_mut().push(TypeError::TypeMismatch {
                                    expected: "HashMap".to_string(),
                                    found: self.type_to_string(&src_ty),
                                    context: "Keys projection".to_string(),
                                });
                            }
                        }
                        Type::Applied("List".to_string(), vec![Type::String])
                    }
                    ProjectionTarget::Values => {
                        match &src_ty {
                            Type::Applied(name, inner) if name == "HashMap" => {
                                let val_ty = inner.get(1).cloned().unwrap_or(Type::String);
                                Type::Applied("List".to_string(), vec![val_ty])
                            }
                            _ => {
                                self.errors.borrow_mut().push(TypeError::TypeMismatch {
                                    expected: "HashMap".to_string(),
                                    found: self.type_to_string(&src_ty),
                                    context: "Values projection".to_string(),
                                });
                                Type::Applied("List".to_string(), vec![Type::String])
                            }
                        }
                    }
                    ProjectionTarget::Contains(_) => {
                        match &src_ty {
                            Type::Applied(name, _) if name == "HashMap" || name == "HashSet" => {}
                            _ => {
                                self.errors.borrow_mut().push(TypeError::TypeMismatch {
                                    expected: "HashMap or HashSet".to_string(),
                                    found: self.type_to_string(&src_ty),
                                    context: "Contains projection".to_string(),
                                });
                            }
                        }
                        Type::Bool
                    }
                    ProjectionTarget::IsEmpty => {
                        match &src_ty {
                            Type::Applied(name, _) if name == "List" || name == "HashMap" || name == "HashSet" => {}
                            Type::Tuple(_) | Type::String => {}
                            _ => {
                                self.errors.borrow_mut().push(TypeError::TypeMismatch {
                                    expected: "List, Tuple, HashMap, HashSet, or String".to_string(),
                                    found: self.type_to_string(&src_ty),
                                    context: "IsEmpty projection".to_string(),
                                });
                            }
                        }
                        Type::Bool
                    }
                    ProjectionTarget::Get(_) => {
                        match &src_ty {
                            Type::Applied(name, inner) if name == "HashMap" => {
                                let val_ty = inner.get(1).cloned().unwrap_or(Type::String);
                                Type::Applied("Option".to_string(), vec![val_ty])
                            }
                            _ => {
                                self.errors.borrow_mut().push(TypeError::TypeMismatch {
                                    expected: "HashMap".to_string(),
                                    found: self.type_to_string(&src_ty),
                                    context: "Get projection".to_string(),
                                });
                                Type::Applied("Option".to_string(), vec![Type::String])
                            }
                        }
                    }
                    ProjectionTarget::Top => {
                        match &src_ty {
                            Type::Applied(name, inner) if name == "Stack" => {
                                let val_ty = inner.first().cloned().unwrap_or(Type::String);
                                Type::Applied("Option".to_string(), vec![val_ty])
                            }
                            _ => {
                                self.errors.borrow_mut().push(TypeError::TypeMismatch {
                                    expected: "Stack".to_string(),
                                    found: self.type_to_string(&src_ty),
                                    context: "Top projection".to_string(),
                                });
                                Type::Applied("Option".to_string(), vec![Type::String])
                            }
                        }
                    }
                    ProjectionTarget::Front => {
                        match &src_ty {
                            Type::Applied(name, inner) if name == "Queue" => {
                                let val_ty = inner.first().cloned().unwrap_or(Type::String);
                                Type::Applied("Option".to_string(), vec![val_ty])
                            }
                            _ => {
                                self.errors.borrow_mut().push(TypeError::TypeMismatch {
                                    expected: "Queue".to_string(),
                                    found: self.type_to_string(&src_ty),
                                    context: "Front projection".to_string(),
                                });
                                Type::Applied("Option".to_string(), vec![Type::String])
                            }
                        }
                    }
                    ProjectionTarget::Elements => {
                        match &src_ty {
                            Type::Applied(name, inner) if name == "HashSet" => {
                                let elem_ty = inner.first().cloned().unwrap_or(Type::String);
                                Type::Applied("List".to_string(), vec![elem_ty])
                            }
                            _ => {
                                self.errors.borrow_mut().push(TypeError::TypeMismatch {
                                    expected: "HashSet".to_string(),
                                    found: self.type_to_string(&src_ty),
                                    context: "Elements projection".to_string(),
                                });
                                Type::Applied("List".to_string(), vec![Type::String])
                            }
                        }
                    }
                    ProjectionTarget::AsStack => {
                        self.diagnostics.borrow_mut().push(Diagnostic::new("D001",
                            Severity::Warning,
                            "AsStack is deprecated, use InsertAt/ExtractFrom type metadata instead"));
                        match &src_ty {
                            Type::Applied(name, inner) if name == "List" => {
                                let elem_ty = inner.first().cloned().unwrap_or(Type::String);
                                Type::Applied("Stack".to_string(), vec![elem_ty])
                            }
                            _ => {
                                self.errors.borrow_mut().push(TypeError::TypeMismatch {
                                    expected: "List".to_string(),
                                    found: self.type_to_string(&src_ty),
                                    context: "AsStack projection".to_string(),
                                });
                                Type::Applied("Stack".to_string(), vec![Type::String])
                            }
                        }
                    }
                    ProjectionTarget::AsQueue => {
                        self.diagnostics.borrow_mut().push(Diagnostic::new("D001",
                            Severity::Warning,
                            "AsQueue is deprecated, use InsertAt/ExtractFrom type metadata instead"));
                        match &src_ty {
                            Type::Applied(n, _) if n == "List" => Type::Applied("Queue".to_string(), vec![Type::String]),
                            _ => {
                                self.errors.borrow_mut().push(TypeError::TypeMismatch {
                                    expected: "List".to_string(),
                                    found: self.type_to_string(&src_ty),
                                    context: "AsQueue projection".to_string(),
                                });
                                Type::Applied("Queue".to_string(), vec![Type::String])
                            }
                        }
                    }
                    ProjectionTarget::BitRange(_) => {
                        match &src_ty {
                            Type::Int | Type::UInt => Type::Int,
                            _ => {
                                self.errors.borrow_mut().push(TypeError::TypeMismatch {
                                    expected: "Int".to_string(),
                                    found: self.type_to_string(&src_ty),
                                    context: "BitRange projection requires integer source".to_string(),
                                });
                                Type::Int
                            }
                        }
                    }
                    ProjectionTarget::UserDefined(name) => {
                        self.resolve_user_projection_type(&src_ty, name)
                    }
                    ProjectionTarget::UserDefinedWithArg(name, _) => {
                        self.resolve_user_projection_type(&src_ty, name)
                    }
                }
            }
            Expr::PatternMatch { .. } => Type::Bool,
            Expr::Block(_stmts, last_expr) => self.infer_expression(last_expr),
            Expr::Match { value: _, arms } => {
                // Try to infer from the last arm first (usually _ = default)
                arms.last().map(|a| self.infer_expression(&a.body))
                    .or_else(|| arms.first().map(|a| self.infer_expression(&a.body)))
                    .unwrap_or(Type::Custom("unknown".to_string()))
            },
Expr::ObjectLiteral(fields) => {
                fields.first().map(|(_, v)| self.infer_expression(v)).unwrap_or(Type::Custom("Object".to_string()))
            },
            Expr::IsType(_, _) | Expr::FromCheck(_, _) | Expr::Like(_, _) => Type::Bool,
            Expr::Cast(inner, target_ty) => {
                let src_ty = self.infer_expression(inner);
                if !self.is_cast_valid(&src_ty, target_ty) {
                    self.errors.borrow_mut().push(TypeError::TypeMismatch {
                        expected: format!("convertible type, found {}", self.type_to_string(&src_ty)),
                        found: self.type_to_string(target_ty),
                        context: format!("cannot cast {} to {}", self.type_to_string(&src_ty), self.type_to_string(target_ty)),
                    });
                }
                target_ty.clone()
            }
            // ── Pattern B routing (direct destructure, not through trait) ──
            Expr::BinaryOp(bop) => {
                use crate::features::binary_op::BinaryOpKind;
                // Comparison and logical operators return Bool regardless of
                // operand types. Arithmetic operators return the operand type.
                match bop.kind {
                    BinaryOpKind::Eq | BinaryOpKind::Ne
                    | BinaryOpKind::Lt | BinaryOpKind::Le
                    | BinaryOpKind::Gt | BinaryOpKind::Ge
                    | BinaryOpKind::And | BinaryOpKind::Or => Type::Bool,
                    _ => {
                        let l_ty = self.infer_expression(&bop.left);
                        let r_ty = self.infer_expression(&bop.right);
                        if l_ty == Type::Float || r_ty == Type::Float { Type::Float }
                        else { Type::Int }
                    }
                }
            }
            Expr::UnaryOp(uop) => {
                let inner = self.infer_expression(&uop.operand);
                match uop.kind {
                    crate::features::unary_op::UnaryOpKind::Not => Type::Bool,
                    _ => inner,
                }
            }
            Expr::Literal(lit) => match lit.as_ref() {
                LiteralExpr::Integer(_) => Type::Int,
                LiteralExpr::Float(_) => Type::Float,
                LiteralExpr::String(_) => Type::String,
                LiteralExpr::Char(_) => Type::Char,
                LiteralExpr::Bool(_) => Type::Bool,
                LiteralExpr::Term => Type::Void,
            },
            Expr::ProjectionExpr(_) | Expr::CallExpr(_)
            | Expr::ListLiteralExpr(_) | Expr::MapLiteralExpr(_) | Expr::SetLiteralExpr(_)
            | Expr::MultiSliceExpr(_) | Expr::FieldAccessExpr(_)
            | Expr::StructInstanceExpr(_) | Expr::ObjectLiteralExpr(_)
            | Expr::TupleExpr(_) | Expr::TupleDestructureExpr(_) | Expr::EllipsisExpr(_)
            | Expr::ArrowMutExpr(_) | Expr::ArrowDiscardExpr(_) | Expr::ArrowTransferExpr(_)
            | Expr::PatternMatchExpr(_) | Expr::MatchExpr(_) | Expr::BlockExpr(_)
            | Expr::SigCallExpr(_) | Expr::SubtypeProjectionExpr(_) | Expr::DbvlTableExpr(_)
            | Expr::TypeRef(_) => Type::Custom("unknown".to_string()),
            Expr::SliceExpr(e) => self.infer_expression(&e.value),
            _ => Type::Custom("unknown".to_string()),
        }
    }

    fn is_error_union(&self, ty: &Type) -> bool {
        match ty {
            Type::Union(types) => types.iter().any(|t| self.is_error_type(t)),
            Type::Applied(name, _) | Type::Generic(name, _) => name == "Result",
            _ => false,
        }
    }

    fn is_error_type(&self, ty: &Type) -> bool {
        if let Type::Custom(name) = ty {
            name == "Error"
        } else {
            false
        }
    }

    fn extract_type_substitutions(&self, expected: &Type, actual: &Type) -> HashMap<String, Type> {
        let mut subs = HashMap::new();
        match (expected, actual) {
            (Type::TypeVar(name), actual_ty) => {
                subs.insert(name.clone(), actual_ty.clone());
            }
            (Type::Applied(en, ea), Type::Applied(an, aa)) if en == an && ea.len() == aa.len() => {
                for (e, a) in ea.iter().zip(aa.iter()) {
                    let s = self.extract_type_substitutions(e, a);
                    subs.extend(s);
                }
            }
            _ => {}
        }
        subs
    }

    fn substitute_type_vars(&self, ty: &Type, subs: &HashMap<String, Type>) -> Type {
        match ty {
            Type::TypeVar(name) => subs.get(name).cloned().unwrap_or(ty.clone()),
            Type::Applied(name, args) => {
                Type::Applied(name.clone(), args.iter().map(|a| self.substitute_type_vars(a, subs)).collect())
            }
            Type::Generic(name, args) => {
                Type::Generic(name.clone(), args.iter().map(|a| self.substitute_type_vars(a, subs)).collect())
            }
            Type::Union(types) => {
                Type::Union(types.iter().map(|t| self.substitute_type_vars(t, subs)).collect())
            }
            Type::Tuple(types) => {
                Type::Tuple(types.iter().map(|t| self.substitute_type_vars(t, subs)).collect())
            }
            Type::Vector(inner, dims) => {
                Type::Vector(
                    Box::new(self.substitute_type_vars(inner, subs)),
                    dims.clone()
                )
            }
            other => other.clone(),
        }
    }

    fn check_geometry(&self, lhs: &Type, rhs: &Type) -> bool {
        match (lhs, rhs) {
            (Type::Vector(inner_lhs, dims_lhs), Type::Vector(inner_rhs, dims_rhs)) => {
                // Check if dimensions match (or one is empty/wildcard)
                let dims_match = dims_lhs.is_empty() || dims_rhs.is_empty() || {
                    if dims_lhs.len() != dims_rhs.len() {
                        false
                    } else {
                        dims_lhs.iter().zip(dims_rhs.iter()).all(|(l, r)| {
                            match (l, r) {
                                (crate::ast::Dimension::Anonymous(sl), crate::ast::Dimension::Anonymous(sr)) => {
                                    *sl == 0 || *sr == 0 || sl == sr
                                }
                                (crate::ast::Dimension::Named(_, sl), crate::ast::Dimension::Named(_, sr)) => {
                                    *sl == 0 || *sr == 0 || sl == sr
                                }
                                (crate::ast::Dimension::Anonymous(sl), crate::ast::Dimension::Named(_, sr)) |
                                (crate::ast::Dimension::Named(_, sl), crate::ast::Dimension::Anonymous(sr)) => {
                                    *sl == 0 || *sr == 0 || sl == sr
                                }
                            }
                        })
                    }
                };
                dims_match && self.check_geometry(inner_lhs, inner_rhs)
            }
            (Type::Vector(inner, _), scalar) | (scalar, Type::Vector(inner, _)) => {
                self.types_compatible(inner, scalar)
            }
            (a, b) => self.types_compatible(a, b),
        }
    }

    fn binary_op_type(&self, l: &Expr, r: &Expr, int_type: Type, float_type: Type) -> Type {
        let l_ty = self.infer_expression(l);
        let r_ty = self.infer_expression(r);
        match (&l_ty, &r_ty) {
            // Vector SIMD operations
            (Type::Vector(inner_l, dims_l), Type::Vector(inner_r, dims_r)) => {
                // Check if dimensions match
                let dims_match = dims_l.len() == dims_r.len() && {
                    dims_l.iter().zip(dims_r.iter()).all(|(ld, rd)| {
                        match (ld, rd) {
                            (crate::ast::Dimension::Anonymous(sl), crate::ast::Dimension::Anonymous(sr)) => sl == sr,
                            (crate::ast::Dimension::Named(_, sl), crate::ast::Dimension::Named(_, sr)) => sl == sr,
                            (crate::ast::Dimension::Anonymous(sl), crate::ast::Dimension::Named(_, sr)) |
                            (crate::ast::Dimension::Named(_, sl), crate::ast::Dimension::Anonymous(sr)) => sl == sr,
                        }
                    })
                };
                if dims_match {
                    Type::Vector(
                        Box::new(self.binary_op_type_scalar(inner_l, inner_r, int_type, float_type)),
                        dims_l.clone(),
                    )
                } else {
                    Type::Custom("vector_dimension_mismatch".to_string())
                }
            }
            (Type::Vector(inner, dims), scalar) | (scalar, Type::Vector(inner, dims)) => {
                Type::Vector(
                    Box::new(self.binary_op_type_scalar(inner, scalar, int_type, float_type)),
                    dims.clone(),
                )
            }
            // List SIMD operations - dynamic length, requires runtime length check
            (Type::Applied(l_name, l_args), Type::Applied(r_name, r_args)) 
                if (l_name == "List" || l_name == "DynamicVector") && 
                   (r_name == "List" || r_name == "DynamicVector") &&
                   l_args.len() == 1 && r_args.len() == 1 => {
                // Both are List<T> or DynamicVector<T>
                // Inject implicit length assertion
                let elem_type_l = &l_args[0];
                let elem_type_r = &r_args[0];
                
                // Element types must be compatible
                if !self.types_compatible(elem_type_l, elem_type_r) {
                    return Type::Custom("list_element_type_mismatch".to_string());
                }
                
                // Return List of result type
                Type::Applied(
                    "List".to_string(),
                    vec![self.binary_op_type_scalar(elem_type_l, elem_type_r, int_type, float_type)]
                )
            }
            // List scalar broadcasting
            (Type::Applied(name, args), scalar) | (scalar, Type::Applied(name, args))
                if (name == "List" || name == "DynamicVector") && args.len() == 1 => {
                let elem_type = &args[0];
                Type::Applied(
                    "List".to_string(),
                    vec![self.binary_op_type_scalar(elem_type, scalar, int_type, float_type)]
                )
            }
            _ => self.binary_op_type_scalar(&l_ty, &r_ty, int_type, float_type),
        }
    }

    fn binary_op_type_scalar(
        &self,
        l_ty: &Type,
        r_ty: &Type,
        int_type: Type,
        float_type: Type,
    ) -> Type {
        match (l_ty, r_ty) {
            (Type::UInt, Type::UInt) | (Type::Int, Type::UInt) | (Type::UInt, Type::Int) => {
                Type::UInt
            }
            (Type::Int, Type::Int) => int_type,
            (Type::Float, _) | (_, Type::Float) => float_type,
            (Type::String, Type::String) => Type::String,
            (Type::String, other) | (other, Type::String) => {
                let type_name = format!("{:?}", other);
                self.errors.borrow_mut().push(TypeError::TypeMismatch {
                    expected: "String".to_string(),
                    found: type_name,
                    context: "cannot perform arithmetic with String and non-String type".to_string(),
                });
                Type::Custom("type_error".to_string())
            }
            _ => Type::Custom("unknown".to_string()),
        }
    }

    /// Enforce field-level visibility at field access sites.
    /// `Sedentary` fields are only accessible from the struct's defining file.
    /// `Private` fields are only accessible from within the struct (TODO).
    fn enforce_field_visibility(&self, struct_name: &str, field: &str) {
        if let Some(vis_map) = self.struct_field_visibility.get(struct_name) {
            if let Some(vis) = vis_map.get(field) {
                match vis {
                    Visibility::Public => {}
                    Visibility::Sedentary => {
                        // Check if the field is accessed from the same file it was defined in.
                        if let Some(struct_file) = self.struct_files.get(struct_name) {
                            if struct_file != &self.current_file {
                                self.errors.borrow_mut().push(TypeError::TypeMismatch {
                                    expected: "field accessible from this file".to_string(),
                                    found: format!("field '{}' of '{}' is sedentary and cannot be accessed from another file", field, struct_name),
                                    context: format!("access to '{}'.{}", struct_name, field),
                                });
                            }
                        }
                    }
                    Visibility::Private => {
                        // TODO: enforce Private when current_struct tracking is available
                    }
                }
            }
        }
    }

    /// Check whether a type conversion is valid.
    /// E005: Check a route's destination expression for invalid references.
    /// Walks the expression tree and warns about identifiers that aren't
    /// the meld type names or known projection targets.
    fn check_route_expr(&self, expr: &Expr, known_idents: &std::collections::HashSet<&str>,
        name_a: &str, name_b: &str, span: &Option<crate::errors::Span>)
    {
        match expr {
            Expr::Identifier(n) => {
                if !known_idents.contains(n.as_str()) {
                    let diag = crate::errors::Diagnostic::new(
                        "E005", crate::errors::Severity::Warning,
                        &format!("invalid route expression — `{}` is not a recognized field or projection", n),
                    ).with_explanation(&format!(
                        "In the route expression `{}`, the identifier `{}` is not a known field or \
                         projection of `{}` or `{}`. Route expressions can reference the meld partner \
                         type name (to access its fields/projections) or standard projection targets \
                         (Ptr, Size, Bytes, Alignment, Type).",
                        format!("{:?}", expr), n, name_a, name_b,
                    )).with_hint(&format!(
                        "Use `{}:>Projection` or `{}.field`, or remove the route to use @/ inference.",
                        name_b, name_b,
                    ));
                    self.diagnostics.borrow_mut().push(diag);
                }
            }
            Expr::FieldAccess(obj, field) => {
                self.check_route_expr(obj, known_idents, name_a, name_b, span);
                // Check if the field exists on the partner type
                if let Expr::Identifier(n) = obj.as_ref() {
                    let partner_fields = if n == name_a {
                        self.struct_fields.get(name_a)
                    } else if n == name_b {
                        self.struct_fields.get(name_b)
                    } else {
                        None
                    };
                    if let Some(fields) = partner_fields {
                        if !fields.contains_key(field) {
                            let diag = crate::errors::Diagnostic::new(
                                "E005", crate::errors::Severity::Error,
                                &format!("invalid route expression — `{}` has no field `{}`", n, field),
                            ).with_explanation(&format!(
                                "Type `{}` doesn't have a field named `{}`. Route expressions can \
                                 only access fields that exist on the meld partner type.",
                                n, field,
                            )).with_hint("Check the field name for typos, or use `:>Projection` instead.");
                            if let Some(s) = span {
                                self.diagnostics.borrow_mut().push(diag.with_span(*s));
                            } else {
                                self.diagnostics.borrow_mut().push(diag);
                            }
                        }
                    }
                }
            }
            Expr::Projection { source, target } => {
                self.check_route_expr(source, known_idents, name_a, name_b, span);
                // Check UserDefined projection targets
                if let crate::ast::ProjectionTarget::UserDefined(ud_name) = target {
                    if let Expr::Identifier(n) = source.as_ref() {
                        if n == name_a || n == name_b {
                            // UserDefined projections are always valid syntactically —
                            // they're resolved at codegen time.
                        }
                    }
                }
            }
            Expr::Add(l, r) | Expr::Sub(l, r) | Expr::Mul(l, r) | Expr::Div(l, r)
            | Expr::Eq(l, r) | Expr::Ne(l, r) | Expr::Lt(l, r) | Expr::Le(l, r)
            | Expr::Gt(l, r) | Expr::Ge(l, r) | Expr::And(l, r) | Expr::Or(l, r) => {
                self.check_route_expr(l, known_idents, name_a, name_b, span);
                self.check_route_expr(r, known_idents, name_a, name_b, span);
            }
            Expr::Not(inner) | Expr::Neg(inner) | Expr::Cast(inner, _) => {
                self.check_route_expr(inner, known_idents, name_a, name_b, span);
            }
            Expr::Call(_, args) => {
                for arg in args {
                    self.check_route_expr(arg, known_idents, name_a, name_b, span);
                }
            }
            Expr::IntrinsicCall { args, .. } => {
                for arg in args {
                    self.check_route_expr(arg, known_idents, name_a, name_b, span);
                }
            }
            _ => {} // literals, other patterns
        }
    }

    fn is_cast_valid(&self, src: &Type, dst: &Type) -> bool {
        if src == dst { return true; }
        // Check primitive cast pairs
        if matches!((src, dst),
            (Type::Int, Type::Float) | (Type::Float, Type::Int) |
            (Type::Int, Type::Char) | (Type::Char, Type::Int) |
            (Type::Int, Type::UInt) | (Type::UInt, Type::Int) |
            (Type::Float, Type::Char) | (Type::Char, Type::Float) |
            (Type::Int, Type::String) | (Type::String, Type::Int) |
            (Type::Char, Type::String) | (Type::String, Type::Char) |
            (Type::Bool, Type::Int) | (Type::Int, Type::Bool) |
            (Type::UInt, Type::Float) | (Type::Float, Type::UInt) |
            (Type::UInt, Type::Char) | (Type::Char, Type::UInt) |
            (Type::UInt, Type::String) | (Type::String, Type::UInt) |
            (Type::Bool, Type::String) | (Type::String, Type::Bool)
        ) { return true; }
        // Check meld-backed cast between custom types
        if let (Type::Custom(src_name), Type::Custom(dst_name)) = (src, dst) {
            if let Some(ref universe) = self.type_universe {
                if universe.find_meld(src_name, dst_name).is_some() {
                    return true;
                }
            }
        }
        false
    }

    /// Check if struct `child` derives from (or transitively derives from) `parent`.
    /// Used for implicit upcast validation: B <: A → B compatible with A.
    fn is_derived_from(&self, child: &str, parent: &str) -> bool {
        let mut current = child.to_string();
        let mut visited = std::collections::HashSet::new();
        loop {
            if !visited.insert(current.clone()) {
                return false; // cycle detected
            }
            match self.struct_parents.get(&current) {
                Some(Some(parent_type)) => {
                    let parent_name = match parent_type {
                        Type::Custom(n) => n.clone(),
                        Type::Applied(n, _) => n.clone(),
                        _ => return false,
                    };
                    if parent_name == parent {
                        return true;
                    }
                    current = parent_name;
                }
                Some(None) | None => return false,
            }
        }
    }

    fn types_compatible(&self, a: &Type, b: &Type) -> bool {
        match (a, b) {
            (Type::Int, Type::Int)
            | (Type::UInt, Type::UInt)
            | (Type::Float, Type::Float)
            | (Type::String, Type::String)
            | (Type::Bool, Type::Bool)
            | (Type::Void, Type::Void)
            | (Type::Char, Type::Char)
            | (Type::Data, Type::Data) => true,
            (Type::Int, Type::UInt) | (Type::UInt, Type::Int) => true,
            (Type::Vector(ia, da), Type::Vector(ib, db)) => {
                da.len() == db.len() && da.iter().zip(db.iter()).all(|(l, r)| {
                    match (l, r) {
                        (crate::ast::Dimension::Anonymous(sl), crate::ast::Dimension::Anonymous(sr)) => sl == sr,
                        (crate::ast::Dimension::Named(nl, sl), crate::ast::Dimension::Named(nr, sr)) => nl == nr && sl == sr,
                        (crate::ast::Dimension::Anonymous(sl), crate::ast::Dimension::Named(_, sr)) |
                        (crate::ast::Dimension::Named(_, sl), crate::ast::Dimension::Anonymous(sr)) => sl == sr,
                    }
                }) && self.types_compatible(ia, ib)
            }
            (Type::Applied(an, aa), Type::Applied(bn, ba)) => {
                an == bn && aa.len() == ba.len() && aa.iter().zip(ba.iter()).all(|(a, b)| self.types_compatible(a, b))
            }
            (Type::Sig(an), Type::Sig(bn)) => an == bn,
            (Type::Union(types), t) | (t, Type::Union(types)) => {
                types.iter().any(|u| self.types_compatible(u, t))
            }
            // Enum variant subtyping: if 'a' is a variant of enum 'b', they're compatible
            (Type::Custom(variant_name), Type::Applied(enum_name, _)) => {
                self.enum_variants.get(variant_name.as_str())
                    .map(|parent| parent == enum_name)
                    .unwrap_or(false)
            }
            // Custom types: exact match, variant-of-enum, or struct derivation upcast
            (Type::Custom(a_name), Type::Custom(b_name)) => {
                if a_name == b_name {
                    true
                } else if let Some(parent) = self.enum_variants.get(a_name.as_str()) {
                    parent == b_name
                } else {
                    // Check struct derivation chain: B <: A → B compatible with A
                    self.is_derived_from(a_name, b_name)
                }
            }
            // TypeVar is compatible with any type (generic placeholder)
            (Type::TypeVar(_), _) | (_, Type::TypeVar(_)) => true,
            // Tuple types: compatible if same length and each element is compatible
            (Type::Tuple(aa), Type::Tuple(ba)) => {
                aa.len() == ba.len() && aa.iter().zip(ba.iter()).all(|(a, b)| self.types_compatible(a, b))
            }
            // Generic types with same name and compatible args
            (Type::Generic(an, aa), Type::Generic(bn, ba)) => {
                an == bn && aa.len() == ba.len() && aa.iter().zip(ba.iter()).all(|(a, b)| self.types_compatible(a, b))
            }
            _ => false,
        }
    }

    /// Check if two types have the same bit width, ignoring exact type identity.
    /// Used by E004 to determine if a field type mismatch is significant.
    fn types_are_width_compatible(&self, a: &Type, b: &Type) -> bool {
        if a == b { return true; }
        // All 64-bit types
        let wide = [Type::Int, Type::UInt, Type::Float, Type::String, Type::Data];
        // All 32-bit types
        let narrow = [Type::Char, Type::Bool];
        // Check if both are in the same width category
        let a_wide = wide.iter().any(|t| self.types_compatible(a, t));
        let b_wide = wide.iter().any(|t| self.types_compatible(b, t));
        if a_wide && b_wide { return true; }
        let a_narrow = narrow.iter().any(|t| self.types_compatible(a, t));
        let b_narrow = narrow.iter().any(|t| self.types_compatible(b, t));
        if a_narrow && b_narrow { return true; }
        // Custom types default to compatible (both i64 width)
        matches!((a, b), (Type::Custom(_), Type::Custom(_)))
            || matches!((a, b), (Type::Custom(_), _) | (_, Type::Custom(_)))
    }

    fn validate_type(&self, ty: &Type) {
        match ty {
            Type::Union(types) | Type::Tuple(types) => {
                for t in types {
                    self.validate_type(t);
                }
            }
            Type::Applied(_, args) | Type::Generic(_, args) => {
                for t in args {
                    self.validate_type(t);
                }
            }
            _ => {}
        }
    }

    fn type_to_string(&self, ty: &Type) -> String {
        format!("{:?}", ty)
    }

    fn check_async_await_callable(&mut self, expr: &Expr) {
        match expr {
            Expr::Call(..) | Expr::IntrinsicCall { .. } | Expr::FieldAccess(..) => {} // callable forms
            _ => {
                self.errors.borrow_mut().push(TypeError::InvalidOperation {
                    operation: "await".to_string(),
                    type_name: format!("non-callable expression: {:?}", expr),
                });
            }
        }
    }

    fn check_expr_for_ffi_errors(&mut self, expr: &Expr) {
        match expr {
            Expr::Call(name, args) => {
                if self.foreign_bindings.contains_key(name) {
                    let binding = self.foreign_bindings.get(name).unwrap();
                    if binding.is_pipe || !binding.success_output.is_empty() || binding.error_type_name != "Error" {
                        let hint = if binding.is_pipe {
                            "Use unification to handle both branches: Ok(val) = func()"
                        } else {
                            "Use unification to handle both branches: Success(val) = func()"
                        };
                        let mut diag = Diagnostic::new(
                            "T001",
                            Severity::Info,
                            "FFI call returns Result type",
                        )
                        .with_explanation(&format!(
                            "FFI function '{}' returns Result. Ensure both Ok and Err branches are handled.",
                            name
                        ))
                        .with_hint(hint);
                        self.diagnostics.borrow_mut().push(diag);
                    }
                    if !args.is_empty() && args.len() != binding.inputs.len() {
                        self.errors.borrow_mut().push(TypeError::TypeMismatch {
                            expected: format!("{} parameters", binding.inputs.len()),
                            found: format!("{} arguments", args.len()),
                            context: format!("FFI call '{}'", name),
                        });
                    }
                }
            }
            Expr::IntrinsicCall { .. } => {}
            Expr::Add(l, r) | Expr::Sub(l, r) | Expr::Mul(l, r) | Expr::Div(l, r) => {
                self.check_expr_for_ffi_errors(l);
                self.check_expr_for_ffi_errors(r);
            }
            Expr::FieldAccess(obj, _) => {
                self.check_expr_for_ffi_errors(obj);
            }
            Expr::IsType(expr, _) | Expr::FromCheck(expr, _) => {
                self.check_expr_for_ffi_errors(expr);
            }
            Expr::Like(l, r) => {
                self.check_expr_for_ffi_errors(l);
                self.check_expr_for_ffi_errors(r);
            }
            _ => {}
        }
    }

    /// Phase 3.5: Resolve the return type of a user-defined projection.
    /// Checks built-in well-known operator names first, then falls back to
    /// TypeUniverse lookup for user-defined types.
    fn resolve_user_projection_type(&self, src_ty: &Type, name: &str) -> Type {
        // Fast-path: known built-in type + well-known operator name
        let known = match (src_ty, name) {
            (Type::Int, "Add" | "Sub" | "Mul" | "Div" | "Mod"
                       | "BitAnd" | "BitOr" | "BitXor" | "Shl" | "Shr") => Some(Type::Int),
            (Type::Int, "Eq" | "Ne" | "Lt" | "Le" | "Gt" | "Ge"
                       | "And" | "Or" | "Not") => Some(Type::Bool),
            (Type::Int, "Neg" | "BitNot") => Some(Type::Int),
            (Type::Float, "Add" | "Sub" | "Mul" | "Div") => Some(Type::Float),
            (Type::Float, "Eq" | "Ne" | "Lt" | "Le" | "Gt" | "Ge") => Some(Type::Bool),
            (Type::Float, "Neg") => Some(Type::Float),
            (Type::Bool, "And" | "Or" | "Eq" | "Ne" | "Not") => Some(Type::Bool),
            (Type::Char, "Eq" | "Ne" | "Lt" | "Le" | "Gt" | "Ge") => Some(Type::Bool),
            _ => None,
        };
        if let Some(ty) = known {
            return ty;
        }

        // Fallback: look up in TypeUniverse for user-defined types
        let type_name = match src_ty {
            Type::Custom(n) => n.clone(),
            Type::Applied(n, _) => n.clone(),
            Type::Enum(n) => n.clone(),
            _ => return Type::Int,
        };

        if let Some(ref universe) = self.type_universe {
            if let Some(resolved) = universe.types.get(&type_name) {
                if let Some(binding) = resolved.projections.get(name) {
                    // Infer the return type from the binding's value expression
                    return self.infer_expression(&binding.value);
                }
            }
        }

        // Default fallback
        Type::Int
    }
}

#[cfg(test)]
mod tests {
    use crate::ast::*;

    fn make_program(items: Vec<TopLevel>) -> Program {
        Program { items, comments: vec![], reactor_speed: None, attrs: vec![], ffi: None, strict_mode: StrictMode::Off, dispatch_mode: Default::default(), exit_condition: None, out_pragmas: vec![], default_sig_modifier: None }
    }

    fn check(prog: &mut Program) -> Vec<super::TypeError> {
        let mut tc = super::TypeChecker::new();
        tc.check_program(prog)
    }

    #[test]
    fn test_check_program_empty() {
        let mut prog = make_program(vec![]);
        let errors = check(&mut prog);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_check_basic_definition() {
        let mut prog = make_program(vec![
            TopLevel::Definition(Definition {
                name: "foo".into(), type_params: vec![], parameters: vec![],
                outputs: vec![Type::Int], output_type: None, output_names: vec![],
                contract: Contract::new(Expr::Bool(true), Expr::Bool(true)),
                body: vec![Statement::Term { values: vec![Some(Expr::Integer(42))], modifiers: vec![], swan_song: None }],
                is_lambda: false, modifiers: vec![], variant_bodies: vec![],
            }),
        ]);
        let errors = check(&mut prog);
        assert!(errors.is_empty(), "Expected no errors, got: {:?}", errors);
    }

    #[test]
    fn test_check_definition_type_mismatch() {
        let mut prog = make_program(vec![
            TopLevel::Definition(Definition {
                name: "foo".into(), type_params: vec![], parameters: vec![],
                outputs: vec![Type::Bool], output_type: None, output_names: vec![],
                contract: Contract::new(Expr::Bool(true), Expr::Bool(true)),
                body: vec![Statement::Term { values: vec![Some(Expr::Integer(42))], modifiers: vec![], swan_song: None }],
                is_lambda: false, modifiers: vec![], variant_bodies: vec![],
            }),
        ]);
        let errors = check(&mut prog);
        assert!(!errors.is_empty(), "Expected type mismatch error for Bool vs Int");
    }

    #[test]
    fn test_check_undefined_variable() {
        let mut prog = make_program(vec![
            TopLevel::Definition(Definition {
                name: "test".into(), type_params: vec![], parameters: vec![],
                outputs: vec![Type::Int], output_type: None, output_names: vec![],
                contract: Contract::new(Expr::Bool(true), Expr::Bool(true)),
                body: vec![
                    Statement::Let { name: "x".into(), ty: Some(Type::Int), expr: Some(Expr::Identifier("y".into())), address: None, address_expr: None, bit_range: None, is_override: false, modifiers: vec![], constraint: None },
                    Statement::Term { values: vec![Some(Expr::Integer(0))], modifiers: vec![], swan_song: None },
                ],
                is_lambda: false, modifiers: vec![], variant_bodies: vec![],
            }),
        ]);
        let errors = check(&mut prog);
        assert!(!errors.is_empty(), "Expected error for undefined 'y'");
    }

    #[test]
    fn test_check_assignment_type_mismatch() {
        let mut prog = make_program(vec![
            TopLevel::StateDecl(StateDecl {
                name: "x".into(), ty: Type::Int, expr: Some(Expr::String("hello".into())),
                address: None, bit_range: None, is_override: false, os_mode: false, span: None, attrs: vec![],
            constraint: None,
            }),
        ]);
        let errors = check(&mut prog);
        assert!(!errors.is_empty(), "Expected type mismatch for Int vs String");
    }

    #[test]
    fn test_check_state_decl_initial_value() {
        let mut prog = make_program(vec![
            TopLevel::StateDecl(StateDecl {
                name: "x".into(), ty: Type::Int, expr: Some(Expr::Integer(5)),
                address: None, bit_range: None, is_override: false, os_mode: false, span: None, attrs: vec![],
            constraint: None,
            }),
        ]);
        let errors = check(&mut prog);
        assert!(errors.is_empty(), "Int initializer should be fine: {:?}", errors);
    }

    #[test]
    fn test_check_state_decl_uninitialized_warning() {
        let mut prog = make_program(vec![
            TopLevel::StateDecl(StateDecl {
                name: "x".into(), ty: Type::Int, expr: None,
                address: None, bit_range: None, is_override: false, os_mode: false, span: None, attrs: vec![],
            constraint: None,
            }),
        ]);
        let errors = check(&mut prog);
        // Uninitialized state decl produces a warning, not an error
        assert!(errors.is_empty(), "Uninitialized is a warning, not error");
    }

    #[test]
    fn test_check_signature_registration() {
        let mut prog = make_program(vec![
            TopLevel::Signature(Signature {
                name: "my_sig".into(), params: vec![("x".into(), Type::Int)],
                result_type: ResultType::Projection(vec![Type::Bool]),
                source: None, alias: None, bound_defn: None, modifier: None, output_type: None,
            }),
        ]);
        let errors = check(&mut prog);
        assert!(errors.is_empty(), "Signature should register: {:?}", errors);
    }

    #[test]
    fn test_check_transaction_basic() {
        let mut prog = make_program(vec![
            TopLevel::Transaction(Transaction {
                name: "tx".into(), is_reactive: false, is_async: false, parameters: vec![],
                contract: Contract { pre_condition: Expr::Gt(Box::new(Expr::Identifier("x".into())), Box::new(Expr::Integer(0))), post_condition: Expr::Eq(Box::new(Expr::Identifier("x".into())), Box::new(Expr::Integer(0))), watchdog: None, span: None },
                body: vec![Statement::Term { values: vec![], modifiers: vec![], swan_song: None }],
                reactor_speed: None, span: None, is_lambda: false,
                dependencies: vec![], attrs: vec![], modifiers: vec![],
                variant_bodies: vec![], outputs: vec![], output_type: None,
            }),
        ]);
        let errors = check(&mut prog);
        assert!(errors.is_empty(), "Transaction should pass: {:?}", errors);
    }

    #[test]
    fn test_check_enum_variant_registration() {
        let mut prog = make_program(vec![
            TopLevel::Enum(EnumDefinition {
                name: "Color".into(), type_params: vec![],
                variants: vec![
                    EnumVariant::Unit("Red".into()),
                    EnumVariant::Tuple("Rgb".into(), vec![Type::Int, Type::Int, Type::Int]),
                ],
                span: None,
            }),
        ]);
        let errors = check(&mut prog);
        assert!(errors.is_empty(), "Enum should register: {:?}", errors);
    }

    #[test]
    fn test_check_constant_declaration() {
        let mut prog = make_program(vec![
            TopLevel::Constant(Constant {
                name: "MAX".into(), ty: Type::Int, expr: Expr::Integer(100),
            }),
        ]);
        let errors = check(&mut prog);
        assert!(errors.is_empty(), "Constant should pass: {:?}", errors);
    }

    #[test]
    fn test_check_frgn_binding_registers_signature() {
        let mut tc = super::TypeChecker::new();
        let sig = ForeignSignature {
            name: "my_fn".into(), location: "std::test::fn".into(),
            wasm_impl: None, wasm_setup: None,
            inputs: vec![("x".into(), Type::Int)],
            success_output: vec![("result".into(), Type::Int)],
            result_type: ResultType::Projection(vec![Type::Int]),
            error_type_name: "Error".into(),
            error_fields: vec![("msg".into(), Type::String)],
            input_layout: None, output_layout: None,
            precondition: None, postcondition: None,
            buffer_mode: None, ffi_kind: None, is_out: false,
            is_pipe: false, fallback: None,
            span: None,
        };
        tc.foreign_bindings.insert("my_fn".into(), sig.clone());
        assert!(tc.foreign_bindings.contains_key("my_fn"));
    }

    #[test]
    fn test_check_geometry_compatible() {
        let tc = super::TypeChecker::new();
        assert!(tc.check_geometry(&Type::Int, &Type::Int));
        assert!(tc.check_geometry(&Type::Bool, &Type::Bool));
    }

    #[test]
    fn test_check_geometry_incompatible() {
        let tc = super::TypeChecker::new();
        assert!(!tc.check_geometry(&Type::Int, &Type::Bool));
        assert!(!tc.check_geometry(&Type::String, &Type::Int));
    }

    #[test]
    fn test_check_diagnostics_collection() {
        let mut tc = super::TypeChecker::new();
        let mut prog = make_program(vec![
            TopLevel::StateDecl(StateDecl {
                name: "unused".into(), ty: Type::Int, expr: None,
                address: None, bit_range: None, is_override: false, os_mode: false, span: None, attrs: vec![],
            constraint: None,
            }),
        ]);
        let _ = tc.check_program(&mut prog);
        let diags = tc.get_diagnostics();
        assert!(!diags.is_empty(), "Should have at least one diagnostic for uninitialized");
    }

    #[test]
    fn test_check_call_arg_type_match() {
        let mut prog = make_program(vec![
            TopLevel::Definition(Definition {
                name: "needs_int".into(),
                type_params: vec![],
                parameters: vec![("x".into(), Type::Int)],
                outputs: vec![Type::Int],
                output_type: None,
                output_names: vec![],
                contract: Contract::new(Expr::Bool(true), Expr::Bool(true)),
                body: vec![
                    Statement::Term { values: vec![Some(Expr::Identifier("x".into()))], modifiers: vec![], swan_song: None },
                ],
                is_lambda: false, modifiers: vec![], variant_bodies: vec![],
            }),
        ]);
        let errors = check(&mut prog);
        assert!(errors.is_empty(), "Expected no errors, got: {:?}", errors);
    }

    #[test]
    fn test_check_call_arg_type_mismatch() {
        let mut prog = make_program(vec![
            TopLevel::Definition(Definition {
                name: "needs_int".into(),
                type_params: vec![],
                parameters: vec![("x".into(), Type::Int)],
                outputs: vec![Type::Int],
                output_type: None,
                output_names: vec![],
                contract: Contract::new(Expr::Bool(true), Expr::Bool(true)),
                body: vec![
                    Statement::Term { values: vec![Some(Expr::Identifier("x".into()))], modifiers: vec![], swan_song: None },
                ],
                is_lambda: false, modifiers: vec![], variant_bodies: vec![],
            }),
            TopLevel::Definition(Definition {
                name: "caller".into(),
                type_params: vec![],
                parameters: vec![],
                outputs: vec![Type::Int],
                output_type: None,
                output_names: vec![],
                contract: Contract::new(Expr::Bool(true), Expr::Bool(true)),
                body: vec![
                    Statement::Term { values: vec![Some(Expr::Call("needs_int".into(), vec![Expr::String("hello".into())]))], modifiers: vec![], swan_song: None },
                ],
                is_lambda: false, modifiers: vec![], variant_bodies: vec![],
            }),
        ]);
        let errors = check(&mut prog);
        assert!(!errors.is_empty(), "Expected type mismatch error");
        let found = errors.iter().any(|e| matches!(e, super::TypeError::TypeMismatch { .. }));
        assert!(found, "Expected TypeMismatch error, got: {:?}", errors);
    }

    #[test]
    fn test_check_call_arg_unknown_fn_skipped() {
        // Calling an undeclared function should not crash — silently skipped
        let mut prog = make_program(vec![
            TopLevel::Definition(Definition {
                name: "caller".into(),
                type_params: vec![],
                parameters: vec![],
                outputs: vec![],
                output_type: None,
                output_names: vec![],
                contract: Contract::new(Expr::Bool(true), Expr::Bool(true)),
                body: vec![
                    Statement::Expression(Expr::Call("undefined_fn".into(), vec![Expr::Integer(42)])),
                ],
                is_lambda: false, modifiers: vec![], variant_bodies: vec![],
            }),
        ]);
        let errors = check(&mut prog);
        // Unknown function is silently skipped — no crash
        assert!(errors.is_empty(), "Expected no errors for unknown fn, got: {:?}", errors);
    }

    #[test]
    fn test_frgn_pipe_registers_signature() {
        let mut tc = super::TypeChecker::new();
        let sig = ForeignSignature {
            name: "pipe_fn".into(), location: "".into(),
            wasm_impl: None, wasm_setup: None,
            inputs: vec![("x".into(), Type::Int)],
            success_output: vec![("result".into(), Type::String)],
            result_type: ResultType::Projection(vec![Type::String]),
            error_type_name: "".into(),
            error_fields: vec![],
            input_layout: None, output_layout: None,
            precondition: None, postcondition: None,
            buffer_mode: None, ffi_kind: None, is_out: false,
            is_pipe: true, fallback: Some(Expr::String("".to_string())),
            span: None,
        };
        tc.foreign_bindings.insert("pipe_fn".into(), sig.clone());
        assert!(tc.foreign_bindings.contains_key("pipe_fn"));
        let stored = tc.foreign_bindings.get("pipe_fn").unwrap();
        assert!(stored.is_pipe);
        assert!(stored.fallback.is_some());
    }

    #[test]
    fn test_frgn_pipe_skips_toml_validation() {
        let mut tc = super::TypeChecker::new();
        let mut sig = ForeignSignature {
            name: "no_toml_fn".into(), location: "".into(),
            wasm_impl: None, wasm_setup: None,
            inputs: vec![],
            success_output: vec![("result".into(), Type::Int)],
            result_type: ResultType::Projection(vec![Type::Int]),
            error_type_name: "".into(),
            error_fields: vec![],
            input_layout: None, output_layout: None,
            precondition: None, postcondition: None,
            buffer_mode: None, ffi_kind: None, is_out: false,
            is_pipe: true, fallback: Some(Expr::Integer(0)),
            span: None,
        };
        tc.check_frgn_binding("no_toml_fn", "", &mut sig);
        assert!(tc.errors.borrow().is_empty(),
            "Pipe frgn should not produce TOML validation errors: {:?}", tc.errors.borrow());
    }

    #[test]
    fn test_is_compile_time_expr_literals() {
        assert!(super::TypeChecker::is_compile_time_expr(&Expr::Integer(42)));
        assert!(super::TypeChecker::is_compile_time_expr(&Expr::Float(3.14)));
        assert!(super::TypeChecker::is_compile_time_expr(&Expr::String("hello".into())));
        assert!(super::TypeChecker::is_compile_time_expr(&Expr::Bool(true)));
        assert!(super::TypeChecker::is_compile_time_expr(&Expr::Char('x')));
        assert!(super::TypeChecker::is_compile_time_expr(&Expr::Term));
        assert!(super::TypeChecker::is_compile_time_expr(&Expr::RegexLiteral(".*".into())));
    }

    #[test]
    fn test_is_compile_time_expr_constructor_call() {
        assert!(super::TypeChecker::is_compile_time_expr(&Expr::Call(
            "Error".into(),
            vec![Expr::String("msg".into())],
        )));
    }

    #[test]
    fn test_is_compile_time_expr_nested_constructor() {
        assert!(super::TypeChecker::is_compile_time_expr(&Expr::Call(
            "CustomError".into(),
            vec![
                Expr::String("code".into()),
                Expr::Integer(42),
            ],
        )));
    }

    #[test]
    fn test_is_compile_time_expr_tuple() {
        assert!(super::TypeChecker::is_compile_time_expr(&Expr::Tuple(vec![
            Expr::Integer(1),
            Expr::String("a".into()),
        ])));
    }

    #[test]
    fn test_is_compile_time_expr_list_literal() {
        assert!(super::TypeChecker::is_compile_time_expr(&Expr::ListLiteral(vec![
            Expr::Integer(1),
            Expr::Integer(2),
            Expr::Integer(3),
        ])));
    }

    #[test]
    fn test_is_compile_time_expr_rejects_identifier() {
        assert!(!super::TypeChecker::is_compile_time_expr(&Expr::Identifier("x".into())));
    }

    #[test]
    fn test_is_compile_time_expr_rejects_owned_ref() {
        assert!(!super::TypeChecker::is_compile_time_expr(&Expr::OwnedRef("x".into())));
    }

    #[test]
    fn test_is_compile_time_expr_rejects_prior_state() {
        assert!(!super::TypeChecker::is_compile_time_expr(&Expr::PriorState("x".into())));
    }

    #[test]
    fn test_is_compile_time_expr_rejects_addition() {
        assert!(!super::TypeChecker::is_compile_time_expr(&Expr::Add(
            Box::new(Expr::Integer(1)),
            Box::new(Expr::Integer(2)),
        )));
    }

    #[test]
    fn test_is_compile_time_expr_rejects_call_with_identifier_arg() {
        assert!(!super::TypeChecker::is_compile_time_expr(&Expr::Call(
            "Error".into(),
            vec![Expr::Identifier("msg".into())],
        )));
    }

    #[test]
    fn test_inop_user_defined_return_type() {
        let mut ctx = super::TypeChecker::new();
        let inop = InopDeclaration {
            name: "test_sadd".into(),
            params: vec![("a".into(), Type::Int), ("b".into(), Type::Int)],
            outputs: vec![Type::Int],
            contract: crate::ast::Contract::new(Expr::Bool(true), Expr::Bool(true)),
            llvm_body: vec![],
            fallback: Some(crate::ast::Expr::Add(
                Box::new(crate::ast::Expr::Identifier("a".into())),
                Box::new(crate::ast::Expr::Identifier("b".into())),
            )),
            has_side_effects: false,
            has_state_access: false,
            llvm_body_spans: vec![],
            span: None,
        };
        ctx.inop_decls.insert("sadd".to_string(), inop);
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::UserDefined("sadd".to_string()),
            args: vec![Expr::Integer(1), Expr::Integer(2)],
        };
        let ty = ctx.infer_expression(&expr);
        assert_eq!(ty, Type::Int, "UserDefined inop should return output type Int");
    }

    #[test]
    fn test_inop_user_defined_unknown_name_emits_diagnostic() {
        let mut ctx = super::TypeChecker::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::UserDefined("bogus".to_string()),
            args: vec![],
        };
        ctx.check_expr_for_function_calls(&expr);
        let has_diag = ctx.diagnostics.borrow().iter().any(|d| d.code == "U001");
        assert!(has_diag, "Unknown inop# should emit U001 diagnostic");
    }

    #[test]
    fn test_meld_e001_missing_route() {
        let meld = TopLevel::Meld(MeldDeclaration {
            name_a: "A".into(),
            name_b: "B".into(),
            routes: vec![
                MeldRouteDef {
                    accessor: "Ptr".into(),
                    dest_expr: Expr::Identifier("B".into()),
                },
            ],
            span: None,
        });
        let prog = make_program(vec![meld]);
        let universe = crate::type_universe::TypeUniverse::build(&prog);
        let ctx = super::TypeChecker::new().with_type_universe(universe);
        assert!(ctx.diagnostics.borrow().is_empty(), "no E001 without struct fields");
    }

    #[test]
    fn test_meld_w002_identity_routes() {
        let meld = TopLevel::Meld(MeldDeclaration {
            name_a: "A".into(),
            name_b: "B".into(),
            routes: vec![
                MeldRouteDef {
                    accessor: "x".into(),
                    dest_expr: Expr::Identifier("x".into()),
                },
            ],
            span: None,
        });
        let mut prog = make_program(vec![meld]);
        let universe = crate::type_universe::TypeUniverse::build(&prog);
        let mut ctx = super::TypeChecker::new().with_type_universe(universe);
        ctx.check_program(&mut prog);
        let diags = ctx.diagnostics.borrow();
        let w002 = diags.iter().find(|d| d.code == "W002");
        assert!(w002.is_some(), "W002 should be emitted for identity routes, got: {:?}", diags);
    }

    #[test]
    fn test_meld_e002_cycle_detection() {
        let mut prog = make_program(vec![
            TopLevel::Meld(MeldDeclaration {
                name_a: "A".into(), name_b: "B".into(),
                routes: vec![], span: None,
            }),
            TopLevel::Meld(MeldDeclaration {
                name_a: "B".into(), name_b: "C".into(),
                routes: vec![], span: None,
            }),
            TopLevel::Meld(MeldDeclaration {
                name_a: "A".into(), name_b: "C".into(),
                routes: vec![], span: None,
            }),
        ]);
        let universe = crate::type_universe::TypeUniverse::build(&prog);
        let mut ctx = super::TypeChecker::new().with_type_universe(universe);
        ctx.check_program(&mut prog);
        let diags = ctx.diagnostics.borrow();
        let e002 = diags.iter().find(|d| d.code == "E002");
        assert!(e002.is_some(), "E002 should be emitted for cycle: {:?}", diags);
    }
}

#[cfg(all(kani, feature = "kani_full"))]
mod kani_full_tests {
    use super::*;


    #[kani::proof]
    fn verify_infer_literal_integer() {
        let ctx = TypeChecker::new();
        let expr = Expr::Literal(Box::new(LiteralExpr::Integer(42)));
        let result = ctx.infer_expression(&expr);
        assert_eq!(result, Type::Int);
    }

    #[kani::proof]
    fn verify_infer_literal_bool() {
        let ctx = TypeChecker::new();
        let expr = Expr::Literal(Box::new(LiteralExpr::Bool(true)));
        let result = ctx.infer_expression(&expr);
        assert_eq!(result, Type::Bool);
    }

    #[kani::proof]
    fn verify_infer_literal_float() {
        let ctx = TypeChecker::new();
        let expr = Expr::Literal(Box::new(LiteralExpr::Float(1.5)));
        let result = ctx.infer_expression(&expr);
        assert_eq!(result, Type::Float);
    }

    #[kani::proof]
    fn verify_infer_literal_string() {
        let ctx = TypeChecker::new();
        let expr = Expr::Literal(Box::new(LiteralExpr::String("x".to_string())));
        let result = ctx.infer_expression(&expr);
        assert_eq!(result, Type::String);
    }

    #[kani::proof]
    fn verify_infer_literal_char() {
        let ctx = TypeChecker::new();
        let expr = Expr::Literal(Box::new(LiteralExpr::Char('a')));
        let result = ctx.infer_expression(&expr);
        assert_eq!(result, Type::Char);
    }

    #[kani::proof]
    fn verify_infer_literal_term() {
        let ctx = TypeChecker::new();
        let expr = Expr::Literal(Box::new(LiteralExpr::Term));
        let result = ctx.infer_expression(&expr);
        assert_eq!(result, Type::Void);
    }

    // ── Intrinsic type inference tests ──────────────────────────

    #[test]
    fn test_check_intrinsic_sqrt_returns_float() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Sqrt,
            args: vec![Expr::Float(9.0)],
        };
        let ctx = TypeChecker::new();
        let ty = ctx.infer_expression(&expr);
        assert_eq!(ty, Type::Float, "sqrt# should infer as Float");
    }

    #[test]
    fn test_check_intrinsic_abs_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Abs,
            args: vec![Expr::Integer(-42)],
        };
        let ctx = TypeChecker::new();
        let ty = ctx.infer_expression(&expr);
        assert_eq!(ty, Type::Int, "abs# should infer as Int");
    }

    #[test]
    fn test_check_intrinsic_ctpop_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Ctpop,
            args: vec![Expr::Integer(255)],
        };
        let ctx = TypeChecker::new();
        let ty = ctx.infer_expression(&expr);
        assert_eq!(ty, Type::Int, "ctpop# should infer as Int");
    }

    #[test]
    fn test_check_intrinsic_contains_returns_bool() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Contains,
            args: vec![
                Expr::ListLiteral(vec![Expr::Integer(1)]),
                Expr::Integer(1),
            ],
        };
        let ctx = TypeChecker::new();
        let ty = ctx.infer_expression(&expr);
        assert_eq!(ty, Type::Bool, "contains# should infer as Bool");
    }

    #[test]
    fn test_check_intrinsic_size_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Size,
            args: vec![Expr::ListLiteral(vec![Expr::Integer(1)])],
        };
        let ctx = TypeChecker::new();
        let ty = ctx.infer_expression(&expr);
        assert_eq!(ty, Type::Int, "size# should infer as Int");
    }

    #[test]
    fn test_check_intrinsic_keys_returns_list() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Keys,
            args: vec![Expr::MapLiteral(vec![])],
        };
        let ctx = TypeChecker::new();
        let ty = ctx.infer_expression(&expr);
        assert_eq!(ty, Type::Custom("List".to_string()), "keys# should infer as List");
    }

    #[test]
    fn test_check_intrinsic_pop_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Pop,
            args: vec![Expr::ListLiteral(vec![Expr::Integer(1)])],
        };
        let ctx = TypeChecker::new();
        let ty = ctx.infer_expression(&expr);
        assert_eq!(ty, Type::Int, "pop# should infer as Int");
    }

    #[test]
    fn test_check_intrinsic_fabs_returns_float() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Fabs,
            args: vec![Expr::Float(-1.0)],
        };
        let ctx = TypeChecker::new();
        let ty = ctx.infer_expression(&expr);
        assert_eq!(ty, Type::Float, "fabs# should infer as Float");
    }

    #[test]
    fn test_check_intrinsic_bytes_returns_int() {
        let mut prog = make_program(vec![]);
        let mut tc = super::TypeChecker::new();
        let ty = tc.infer_expression(&Expr::IntrinsicCall {
            intrinsic: Intrinsic::ByteCount,
            args: vec![Expr::String("hello".into())],
        });
        assert_eq!(ty, Type::Int, "bytes# should infer as Int");
    }

    // ── Phase A: Terminal / TTY + Process type inference tests ─────

    #[test]
    fn test_check_intrinsic_tty_raw_mode_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::TtyRawMode,
            args: vec![Expr::Bool(true)],
        };
        let ctx = TypeChecker::new();
        let ty = ctx.infer_expression(&expr);
        assert_eq!(ty, Type::Int, "tty_raw_mode# should infer as Int");
    }

    #[test]
    fn test_check_intrinsic_tty_size_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::TtySize,
            args: vec![],
        };
        let ctx = TypeChecker::new();
        let ty = ctx.infer_expression(&expr);
        assert_eq!(ty, Type::Int, "tty_size# should infer as Int");
    }

    #[test]
    fn test_check_intrinsic_tty_read_key_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::TtyReadKey,
            args: vec![],
        };
        let ctx = TypeChecker::new();
        let ty = ctx.infer_expression(&expr);
        assert_eq!(ty, Type::Int, "tty_read_key# should infer as Int");
    }

    #[test]
    fn test_check_intrinsic_ioctl_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::IoCtl,
            args: vec![Expr::Integer(0), Expr::Integer(0), Expr::Integer(0)],
        };
        let ctx = TypeChecker::new();
        let ty = ctx.infer_expression(&expr);
        assert_eq!(ty, Type::Int, "ioctl# should infer as Int");
    }

    #[test]
    fn test_check_intrinsic_isatty_returns_bool() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::IsTty,
            args: vec![Expr::Integer(0)],
        };
        let ctx = TypeChecker::new();
        let ty = ctx.infer_expression(&expr);
        assert_eq!(ty, Type::Bool, "isatty# should infer as Bool");
    }

    #[test]
    fn test_check_intrinsic_spawn_with_output_returns_string() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::SpawnWithOutput,
            args: vec![Expr::String("echo hi".into())],
        };
        let ctx = TypeChecker::new();
        let ty = ctx.infer_expression(&expr);
        assert_eq!(ty, Type::String, "spawn_with_output# should infer as String");
    }

    #[test]
    fn test_check_intrinsic_spawn_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Spawn,
            args: vec![Expr::String("true".into())],
        };
        let ctx = TypeChecker::new();
        let ty = ctx.infer_expression(&expr);
        assert_eq!(ty, Type::Int, "spawn# should infer as Int");
    }

    // ── Phase B: Raw File I/O type inference tests ─────────────────

    #[test]
    fn test_check_intrinsic_open_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Open,
            args: vec![Expr::String("/tmp/t".into()), Expr::Integer(0), Expr::Integer(0)],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::Int, "open# should infer as Int");
    }

    #[test]
    fn test_check_intrinsic_close_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Close,
            args: vec![Expr::Integer(0)],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::Int, "close# should infer as Int");
    }

    #[test]
    fn test_check_intrinsic_read_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Read,
            args: vec![Expr::Integer(0), Expr::Integer(0), Expr::Integer(4096)],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::Int, "read# should infer as Int");
    }

    #[test]
    fn test_check_intrinsic_write_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Write,
            args: vec![Expr::Integer(1), Expr::Integer(0), Expr::Integer(8)],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::Int, "write# should infer as Int");
    }

    #[test]
    fn test_check_intrinsic_lseek_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::LSeek,
            args: vec![Expr::Integer(0), Expr::Integer(0), Expr::Integer(0)],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::Int, "lseek# should infer as Int");
    }

    #[test]
    fn test_check_intrinsic_pread_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::PRead,
            args: vec![Expr::Integer(0), Expr::Integer(0), Expr::Integer(16), Expr::Integer(0)],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::Int, "pread# should infer as Int");
    }

    #[test]
    fn test_check_intrinsic_pwrite_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::PWrite,
            args: vec![Expr::Integer(1), Expr::Integer(0), Expr::Integer(8), Expr::Integer(0)],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::Int, "pwrite# should infer as Int");
    }

    #[test]
    fn test_check_intrinsic_stat_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Stat,
            args: vec![Expr::String("/tmp/t".into())],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::Int, "stat# should infer as Int");
    }

    #[test]
    fn test_check_intrinsic_fstat_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::FStat,
            args: vec![Expr::Integer(0)],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::Int, "fstat# should infer as Int");
    }

    #[test]
    fn test_check_intrinsic_truncate_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::FTruncate,
            args: vec![Expr::String("/tmp/t".into()), Expr::Integer(0)],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::Int, "truncate# should infer as Int");
    }

    #[test]
    fn test_check_intrinsic_ftruncate_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::FTruncate,
            args: vec![Expr::Integer(0), Expr::Integer(0)],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::Int, "ftruncate# should infer as Int");
    }

    #[test]
    fn test_check_intrinsic_fsync_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::FSync,
            args: vec![Expr::Integer(0)],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::Int, "fsync# should infer as Int");
    }

    #[test]
    fn test_check_intrinsic_dup_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::FDup,
            args: vec![Expr::Integer(0)],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::Int, "dup# should infer as Int");
    }

    #[test]
    fn test_check_intrinsic_dup2_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::FDup2,
            args: vec![Expr::Integer(0), Expr::Integer(3)],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::Int, "dup2# should infer as Int");
    }

    #[test]
    fn test_check_intrinsic_fcntl_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::FCntl,
            args: vec![Expr::Integer(0), Expr::Integer(0), Expr::Integer(0)],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::Int, "fcntl# should infer as Int");
    }

    // ── Phase C: Filesystem type inference tests ───────────────────

    #[test]
    fn test_check_intrinsic_mkdir_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::MkDir,
            args: vec![Expr::String("/tmp/d".into()), Expr::Integer(0o755)],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::Int, "mkdir# should infer as Int");
    }

    #[test]
    fn test_check_intrinsic_rmdir_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::RmDir,
            args: vec![Expr::String("/tmp/d".into())],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::Int, "rmdir# should infer as Int");
    }

    #[test]
    fn test_check_intrinsic_unlink_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Unlink,
            args: vec![Expr::String("/tmp/f".into())],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::Int, "unlink# should infer as Int");
    }

    #[test]
    fn test_check_intrinsic_rename_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Rename,
            args: vec![Expr::String("a".into()), Expr::String("b".into())],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::Int, "rename# should infer as Int");
    }

    #[test]
    fn test_check_intrinsic_symlink_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::SymLink,
            args: vec![Expr::String("target".into()), Expr::String("link".into())],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::Int, "symlink# should infer as Int");
    }

    #[test]
    fn test_check_intrinsic_readlink_returns_string() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::ReadLink,
            args: vec![Expr::String("/tmp/l".into())],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::String, "readlink# should infer as String");
    }

    #[test]
    fn test_check_intrinsic_link_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Link,
            args: vec![Expr::String("old".into()), Expr::String("new".into())],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::Int, "link# should infer as Int");
    }

    #[test]
    fn test_check_intrinsic_getcwd_returns_string() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::GetCwd,
            args: vec![],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::String, "getcwd# should infer as String");
    }

    #[test]
    fn test_check_intrinsic_chdir_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::ChDir,
            args: vec![Expr::String("/tmp".into())],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::Int, "chdir# should infer as Int");
    }

    #[test]
    fn test_check_intrinsic_readdir_returns_list() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::ReadDir,
            args: vec![Expr::String(".".into())],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::Custom("List".to_string()), "readdir# should infer as List");
    }

    #[test]
    fn test_check_intrinsic_chmod_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::ChMod,
            args: vec![Expr::String("/tmp/f".into()), Expr::Integer(0o644)],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::Int, "chmod# should infer as Int");
    }

    #[test]
    fn test_check_intrinsic_chown_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::ChOwn,
            args: vec![Expr::String("/tmp/f".into()), Expr::Integer(0), Expr::Integer(0)],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::Int, "chown# should infer as Int");
    }

    #[test]
    fn test_check_intrinsic_umask_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::UMask,
            args: vec![Expr::Integer(0o022)],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::Int, "umask# should infer as Int");
    }

    #[test]
    fn test_check_intrinsic_access_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Access,
            args: vec![Expr::String("/tmp".into()), Expr::Integer(0)],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::Int, "access# should infer as Int");
    }

    // ── Phase D: Memory + Synchronization type inference tests ─────

    #[test]
    fn test_check_intrinsic_mmap_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Mmap,
            args: vec![Expr::Integer(0), Expr::Integer(4096), Expr::Integer(3), Expr::Integer(-1), Expr::Integer(-1), Expr::Integer(0)],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::Int, "mmap# should infer as Int");
    }

    #[test]
    fn test_check_intrinsic_munmap_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::MUnmap,
            args: vec![Expr::Integer(0), Expr::Integer(4096)],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::Int);
    }

    #[test]
    fn test_check_intrinsic_mprotect_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::MProtect,
            args: vec![Expr::Integer(0), Expr::Integer(4096), Expr::Integer(3)],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::Int);
    }

    #[test]
    fn test_check_intrinsic_brk_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Brk,
            args: vec![Expr::Integer(0)],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::Int);
    }

    #[test]
    fn test_check_intrinsic_mlock_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::MLock,
            args: vec![Expr::Integer(0), Expr::Integer(4096)],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::Int);
    }

    #[test]
    fn test_check_intrinsic_atomic_load_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::AtomicLoad,
            args: vec![Expr::Integer(0), Expr::Integer(0)],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::Int);
    }

    #[test]
    fn test_check_intrinsic_atomic_store_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::AtomicStore,
            args: vec![Expr::Integer(0), Expr::Integer(42), Expr::Integer(0)],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::Int);
    }

    #[test]
    fn test_check_intrinsic_atomic_cas_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::AtomicCas,
            args: vec![Expr::Integer(0), Expr::Integer(0), Expr::Integer(1), Expr::Integer(0)],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::Int);
    }

    #[test]
    fn test_check_intrinsic_atomic_xchg_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::AtomicXchg,
            args: vec![Expr::Integer(0), Expr::Integer(42), Expr::Integer(0)],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::Int);
    }

    #[test]
    fn test_check_intrinsic_atomic_add_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::AtomicAdd,
            args: vec![Expr::Integer(0), Expr::Integer(1), Expr::Integer(0)],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::Int);
    }

    #[test]
    fn test_check_intrinsic_fence_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Fence,
            args: vec![Expr::Integer(0)],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::Int);
    }

    #[test]
    fn test_check_intrinsic_futex_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Futex,
            args: vec![Expr::Integer(0), Expr::Integer(0), Expr::Integer(0), Expr::Integer(0), Expr::Integer(0), Expr::Integer(0)],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::Int);
    }

    // ── Phase E: IPC type inference tests ──────────────────────────

    #[test]
    fn test_check_intrinsic_pipe_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Pipe,
            args: vec![Expr::Integer(0)],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::Int);
    }

    #[test]
    fn test_check_intrinsic_shm_open_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::ShmOpen,
            args: vec![Expr::String("/s".into()), Expr::Integer(0), Expr::Integer(0)],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::Int);
    }

    #[test]
    fn test_check_intrinsic_shm_unlink_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::ShmUnlink,
            args: vec![Expr::String("/s".into())],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::Int);
    }

    #[test]
    fn test_check_intrinsic_sem_open_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::SemOpen,
            args: vec![Expr::String("/s".into()), Expr::Integer(0), Expr::Integer(0), Expr::Integer(0)],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::Int);
    }

    #[test]
    fn test_check_intrinsic_sem_wait_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::SemWait,
            args: vec![Expr::Integer(0)],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::Int);
    }

    #[test]
    fn test_check_intrinsic_sem_post_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::SemPost,
            args: vec![Expr::Integer(0)],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::Int);
    }

    // ── Phase F: Signals type inference tests ──────────────────────

    #[test]
    fn test_check_intrinsic_sigaction_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::SigAction,
            args: vec![Expr::Integer(0), Expr::Integer(0)],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::Int);
    }

    #[test]
    fn test_check_intrinsic_sigprocmask_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::SigProcMask,
            args: vec![Expr::Integer(0), Expr::Integer(0)],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::Int);
    }

    #[test]
    fn test_check_intrinsic_kill_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Kill,
            args: vec![Expr::Integer(0), Expr::Integer(0)],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::Int);
    }

    #[test]
    fn test_check_intrinsic_signalfd_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::SignalFd,
            args: vec![Expr::Integer(0)],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::Int);
    }

    #[test]
    fn test_check_intrinsic_timerfd_create_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::TimerFdCreate,
            args: vec![Expr::Integer(100)],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::Int);
    }

    // ── Phase G: Networking type inference tests ───────────────────

    #[test]
    fn test_check_intrinsic_socket_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Socket,
            args: vec![Expr::Integer(0), Expr::Integer(0), Expr::Integer(0)],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::Int);
    }

    #[test]
    fn test_check_intrinsic_bind_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Bind,
            args: vec![Expr::Integer(0), Expr::Integer(0), Expr::Integer(0)],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::Int);
    }

    #[test]
    fn test_check_intrinsic_listen_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Listen,
            args: vec![Expr::Integer(0), Expr::Integer(0)],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::Int);
    }

    #[test]
    fn test_check_intrinsic_accept_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Accept,
            args: vec![Expr::Integer(0), Expr::Integer(0), Expr::Integer(0)],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::Int);
    }

    #[test]
    fn test_check_intrinsic_connect_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Connect,
            args: vec![Expr::Integer(0), Expr::Integer(0), Expr::Integer(0)],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::Int);
    }

    #[test]
    fn test_check_intrinsic_send_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Send,
            args: vec![Expr::Integer(0), Expr::Integer(0), Expr::Integer(0), Expr::Integer(0)],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::Int);
    }

    #[test]
    fn test_check_intrinsic_recv_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Recv,
            args: vec![Expr::Integer(0), Expr::Integer(0), Expr::Integer(0), Expr::Integer(0)],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::Int);
    }

    #[test]
    fn test_check_intrinsic_sendto_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::SendTo,
            args: vec![
                Expr::Integer(0), Expr::Integer(0), Expr::Integer(0),
                Expr::Integer(0), Expr::Integer(0), Expr::Integer(0),
            ],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::Int);
    }

    #[test]
    fn test_check_intrinsic_recvfrom_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::RecvFrom,
            args: vec![
                Expr::Integer(0), Expr::Integer(0), Expr::Integer(0),
                Expr::Integer(0), Expr::Integer(0), Expr::Integer(0),
            ],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::Int);
    }

    #[test]
    fn test_check_intrinsic_setsockopt_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::SetSockOpt,
            args: vec![
                Expr::Integer(0), Expr::Integer(0), Expr::Integer(0),
                Expr::Integer(0), Expr::Integer(0),
            ],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::Int);
    }

    #[test]
    fn test_check_intrinsic_getsockopt_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::GetSockOpt,
            args: vec![
                Expr::Integer(0), Expr::Integer(0), Expr::Integer(0),
                Expr::Integer(0), Expr::Integer(0),
            ],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::Int);
    }

    #[test]
    fn test_check_intrinsic_shutdown_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Shutdown,
            args: vec![Expr::Integer(0), Expr::Integer(0)],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::Int);
    }

    #[test]
    fn test_check_intrinsic_getaddrinfo_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::GetAddrInfo,
            args: vec![Expr::String("localhost".into()), Expr::String("80".into())],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::Int);
    }

    // ── Phase H: Everything Else type inference tests ─────────────

    #[test]
    fn test_check_intrinsic_getenv_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::GetEnv,
            args: vec![Expr::String("PATH".into())],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::Int);
    }

    #[test]
    fn test_check_intrinsic_setenv_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::SetEnv,
            args: vec![Expr::String("VAR".into()), Expr::String("val".into())],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::Int);
    }

    #[test]
    fn test_check_intrinsic_unsetenv_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::UnsetEnv,
            args: vec![Expr::String("VAR".into())],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::Int);
    }

    #[test]
    fn test_check_intrinsic_getpid_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::GetPid,
            args: vec![],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::Int);
    }

    #[test]
    fn test_check_intrinsic_getppid_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::GetPPid,
            args: vec![],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::Int);
    }

    #[test]
    fn test_check_intrinsic_clock_gettime_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::ClockGetTime,
            args: vec![Expr::Integer(1)],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::Int);
    }

    #[test]
    fn test_check_intrinsic_nanosleep_returns_int() {
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::NanoSleep,
            args: vec![Expr::Integer(1000)],
        };
        let ctx = TypeChecker::new();
        assert_eq!(ctx.infer_expression(&expr), Type::Int);
    }

    #[test]
    fn test_visibility_sed_same_file_allowed() {
        // Accessing a sedentary field from the same file should NOT produce an error.
        // The struct and field access are both registered with current_file = "main.bv".
        let mut prog = make_program(vec![
            TopLevel::Struct(StructDefinition {
                name: "S".into(), type_params: vec![], parent: None,
                fields: vec![StructField {
                    name: "x".into(), ty: Type::Int, default: None,
                    visibility: Visibility::Sedentary,
                }],
                transactions: vec![], view_html: None, span: None,
                modifiers: vec![], variants: vec![],
            }),
            TopLevel::Definition(Definition {
                name: "f".into(), type_params: vec![], parameters: vec![],
                outputs: vec![Type::Int], output_type: None, output_names: vec![],
                contract: Contract::new(Expr::Bool(true), Expr::Bool(true)),
                body: vec![Statement::Term {
                    values: vec![Some(Expr::FieldAccess(
                        Box::new(Expr::StructInstance("S".into(), vec![])),
                        "x".into(),
                    ))],
                    modifiers: vec![], swan_song: None,
                }],
                is_lambda: false, modifiers: vec![], variant_bodies: vec![],
            }),
        ]);
        let errors = check(&mut prog);
        assert!(errors.is_empty(), "Expected no errors for same-file sed field access, got: {:?}", errors);
    }

    #[test]
    fn test_visibility_public_field_allowed() {
        let mut prog = make_program(vec![
            TopLevel::Struct(StructDefinition {
                name: "S".into(), type_params: vec![], parent: None,
                fields: vec![StructField {
                    name: "x".into(), ty: Type::Int, default: None,
                    visibility: Visibility::Public,
                }],
                transactions: vec![], view_html: None, span: None,
                modifiers: vec![], variants: vec![],
            }),
            TopLevel::Definition(Definition {
                name: "f".into(), type_params: vec![], parameters: vec![],
                outputs: vec![Type::Int], output_type: None, output_names: vec![],
                contract: Contract::new(Expr::Bool(true), Expr::Bool(true)),
                body: vec![Statement::Term {
                    values: vec![Some(Expr::FieldAccess(
                        Box::new(Expr::StructInstance("S".into(), vec![])),
                        "x".into(),
                    ))],
                    modifiers: vec![], swan_song: None,
                }],
                is_lambda: false, modifiers: vec![], variant_bodies: vec![],
            }),
        ]);
        assert!(errors.is_empty(), "Expected no errors for public field access, got: {:?}", errors);
    }

    #[test]
    fn test_infer_is_type_returns_bool() {
        let ctx = TypeChecker::new();
        let expr = Expr::IsType(
            Box::new(Expr::Integer(42)),
            crate::ast::IsTarget::Type(Type::Int),
        );
        let ty = ctx.infer_expression(&expr);
        assert_eq!(ty, Type::Bool, "IsType should infer as Bool");
    }

    #[test]
    fn test_infer_is_variant_returns_bool() {
        let ctx = TypeChecker::new();
        let expr = Expr::IsType(
            Box::new(Expr::Identifier("x".to_string())),
            crate::ast::IsTarget::Variant("Some".to_string()),
        );
        let ty = ctx.infer_expression(&expr);
        assert_eq!(ty, Type::Bool, "IsType(variant) should infer as Bool");
    }

    #[test]
    fn test_infer_from_check_returns_bool() {
        let ctx = TypeChecker::new();
        let expr = Expr::FromCheck(
            Box::new(Expr::Identifier("x".to_string())),
            Type::Custom("Foo".to_string()),
        );
        let ty = ctx.infer_expression(&expr);
        assert_eq!(ty, Type::Bool, "FromCheck should infer as Bool");
    }

    #[test]
    fn test_infer_like_returns_bool() {
        let ctx = TypeChecker::new();
        let expr = Expr::Like(
            Box::new(Expr::Integer(42)),
            Box::new(Expr::Integer(1)),
        );
        let ty = ctx.infer_expression(&expr);
        assert_eq!(ty, Type::Bool, "Like should infer as Bool");
    }

    #[test]
    fn test_infer_cast_int_to_string_valid() {
        let ctx = TypeChecker::new();
        let expr = Expr::Cast(
            Box::new(Expr::Integer(42)),
            Type::String,
        );
        let ty = ctx.infer_expression(&expr);
        assert_eq!(ty, Type::String, "Cast Int -> String should return String");
        assert!(ctx.errors.borrow().is_empty(), "Int -> String should be valid");
    }

    #[test]
    fn test_infer_cast_string_to_int_valid() {
        let ctx = TypeChecker::new();
        let expr = Expr::Cast(
            Box::new(Expr::String("42".to_string())),
            Type::Int,
        );
        let ty = ctx.infer_expression(&expr);
        assert_eq!(ty, Type::Int, "Cast String -> Int should return Int");
        assert!(ctx.errors.borrow().is_empty(), "String -> Int should be valid");
    }

    #[test]
    fn test_infer_cast_char_to_string_valid() {
        let ctx = TypeChecker::new();
        let expr = Expr::Cast(
            Box::new(Expr::Char('A')),
            Type::String,
        );
        let ty = ctx.infer_expression(&expr);
        assert_eq!(ty, Type::String, "Cast Char -> String should return String");
        assert!(ctx.errors.borrow().is_empty(), "Char -> String should be valid");
    }

    #[test]
    fn test_infer_cast_string_to_char_valid() {
        let ctx = TypeChecker::new();
        let expr = Expr::Cast(
            Box::new(Expr::String("hello".to_string())),
            Type::Char,
        );
        let ty = ctx.infer_expression(&expr);
        assert_eq!(ty, Type::Char, "Cast String -> Char should return Char");
        assert!(ctx.errors.borrow().is_empty(), "String -> Char should be valid");
    }

    #[test]
    fn test_infer_cast_int_to_float_valid() {
        let ctx = TypeChecker::new();
        let expr = Expr::Cast(
            Box::new(Expr::Integer(42)),
            Type::Float,
        );
        let ty = ctx.infer_expression(&expr);
        assert_eq!(ty, Type::Float, "Cast Int -> Float should return Float");
        assert!(ctx.errors.borrow().is_empty(), "Int -> Float should be valid");
    }

    #[test]
    fn test_infer_cast_int_to_char_valid() {
        let ctx = TypeChecker::new();
        let expr = Expr::Cast(
            Box::new(Expr::Integer(65)),
            Type::Char,
        );
        let ty = ctx.infer_expression(&expr);
        assert_eq!(ty, Type::Char, "Cast Int -> Char should return Char");
        assert!(ctx.errors.borrow().is_empty(), "Int -> Char should be valid");
    }

    #[test]
    fn test_infer_cast_meld_direct() {
        let meld = TopLevel::Meld(MeldDeclaration {
            name_a: "String".into(),
            name_b: "CString".into(),
            routes: vec![],
            span: None,
        });
        let mut prog = make_program(vec![meld]);
        let universe = crate::type_universe::TypeUniverse::build(&prog);
        let ctx = TypeChecker::new().with_type_universe(universe);
        assert!(ctx.is_cast_valid(&Type::Custom("String".into()), &Type::Custom("CString".into())),
            "meld String <:> CString should allow cast String -> CString");
        assert!(ctx.is_cast_valid(&Type::Custom("CString".into()), &Type::Custom("String".into())),
            "meld String <:> CString should allow cast CString -> String (bidirectional)");
        assert!(!ctx.is_cast_valid(&Type::Custom("String".into()), &Type::Custom("Int".into())),
            "no meld String <:> Int, cast should be invalid");
    }

}
