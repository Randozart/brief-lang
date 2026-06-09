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
    ffi_results: RefCell<HashMap<String, ResultCheckStatus>>,
    foreign_bindings: HashMap<String, ForeignSignature>,
    pub target: CompilationTarget,
    enum_variants: HashMap<String, String>,  // variant_name -> enum_name
    struct_fields: HashMap<String, HashMap<String, Type>>,  // struct_name -> {field_name -> type}
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
            ffi_results: RefCell::new(HashMap::new()),
            foreign_bindings: HashMap::new(),
            target: CompilationTarget::Interpreter,
            enum_variants: HashMap::new(),
            struct_fields: HashMap::new(),
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

        // Option<T> methods
        self.signatures.insert(
            "is_some".to_string(),
            Signature {
                name: "is_some".to_string(),
                params: vec![("".to_string(), Type::Applied("Option".to_string(), vec![Type::TypeVar("T".to_string())]))], modifier: None, output_type: None,
                result_type: ResultType::Projection(vec![Type::Bool]),
                source: None,
                alias: None,
                bound_defn: None,
            },
        );

        self.signatures.insert(
            "is_none".to_string(),
            Signature {
                name: "is_none".to_string(),
                params: vec![("".to_string(), Type::Applied("Option".to_string(), vec![Type::TypeVar("T".to_string())]))], modifier: None, output_type: None,
                result_type: ResultType::Projection(vec![Type::Bool]),
                source: None,
                alias: None,
                bound_defn: None,
            },
        );

        self.signatures.insert(
            "unwrap".to_string(),
            Signature {
                name: "unwrap".to_string(),
                params: vec![("".to_string(), Type::Applied("Option".to_string(), vec![Type::TypeVar("T".to_string())]))], modifier: None, output_type: None,
                result_type: ResultType::Projection(vec![Type::TypeVar("T".to_string())]),
                source: None,
                alias: None,
                bound_defn: None,
            },
        );

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

        // Result<T, E> methods
        self.signatures.insert(
            "is_ok".to_string(),
            Signature {
                name: "is_ok".to_string(),
                params: vec![("".to_string(), Type::Applied("Result".to_string(), vec![Type::TypeVar("T".to_string()), Type::TypeVar("E".to_string())]))], modifier: None, output_type: None,
                result_type: ResultType::Projection(vec![Type::Bool]),
                source: None,
                alias: None,
                bound_defn: None,
            },
        );

        self.signatures.insert(
            "is_err".to_string(),
            Signature {
                name: "is_err".to_string(),
                params: vec![("".to_string(), Type::Applied("Result".to_string(), vec![Type::TypeVar("T".to_string()), Type::TypeVar("E".to_string())]))], modifier: None, output_type: None,
                result_type: ResultType::Projection(vec![Type::Bool]),
                source: None,
                alias: None,
                bound_defn: None,
            },
        );

        self.signatures.insert(
            "unwrap".to_string(),
            Signature {
                name: "unwrap".to_string(),
                params: vec![("".to_string(), Type::Applied("Result".to_string(), vec![Type::TypeVar("T".to_string()), Type::TypeVar("E".to_string())]))], modifier: None, output_type: None,
                result_type: ResultType::Projection(vec![Type::TypeVar("T".to_string())]),
                source: None,
                alias: None,
                bound_defn: None,
            },
        );

        self.signatures.insert(
            "unwrap_err".to_string(),
            Signature {
                name: "unwrap_err".to_string(),
                params: vec![("".to_string(), Type::Applied("Result".to_string(), vec![Type::TypeVar("T".to_string()), Type::TypeVar("E".to_string())]))], modifier: None, output_type: None,
                result_type: ResultType::Projection(vec![Type::TypeVar("E".to_string())]),
                source: None,
                alias: None,
                bound_defn: None,
            },
        );

        self.signatures.insert(
            "unwrap_or".to_string(),
            Signature {
                name: "unwrap_or".to_string(),
                params: vec![("".to_string(), Type::Applied("Option".to_string(), vec![Type::TypeVar("T".to_string())])), ("".to_string(), Type::TypeVar("T".to_string()))], modifier: None, output_type: None,
                result_type: ResultType::Projection(vec![Type::TypeVar("T".to_string())]),
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
                TopLevel::ForeignBinding {
                    name, signature, ..
                } => {
                    // Collect foreign binding signature for type inference
                    self.foreign_bindings
                        .insert(name.clone(), signature.clone());
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
                    for field in &struct_def.fields {
                        fields.insert(field.name.clone(), field.ty.clone());
                    }
                    self.struct_fields.insert(struct_def.name.clone(), fields);
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
                _ => {}
            }
        }

        self.errors.borrow().clone()
    }

    pub fn get_diagnostics(&self) -> Vec<Diagnostic> {
        self.diagnostics.borrow().clone()
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

    fn check_statement(&mut self, stmt: &Statement, is_async: Option<&bool>) {
        match stmt {
            Statement::Assignment { lhs, expr, timeout, .. } => {
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
                    self.errors.borrow_mut().push(TypeError::TypeMismatch {
                        expected: self.type_to_string(&lhs_ty),
                        found: self.type_to_string(&expr_ty),
                        context: "assignment".to_string(),
                    });
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
            _ => {}
        }
    }

    fn infer_expression(&self, expr: &Expr) -> Type {
        match expr {
            Expr::Integer(_) => Type::Int,
            Expr::Float(_) => {
                if self.target == CompilationTarget::Verilog {
                    self.errors.borrow_mut().push(TypeError::TypeMismatch {
                        expected: "Fixed-point or Integer".to_string(),
                        found: "Float".to_string(),
                        context: "Verilog synthesis".to_string(),
                    });
                }
                Type::Float
            }
            Expr::String(_) => Type::String,
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
            Expr::Call(name, args) => {
                if let Some(fb) = self.foreign_bindings.get(name) {
                    fb.success_output
                        .first()
                        .map(|(_, ty)| ty.clone())
                        .unwrap_or(Type::Void)
                } else if name == "unwrap" && !args.is_empty() {
                    // Special handling for unwrap: extract inner type from Option/Result
                    let arg_ty = self.infer_expression(&args[0]);
                    match &arg_ty {
                        Type::Applied(inner_name, type_args) if inner_name == "Option" && !type_args.is_empty() => {
                            type_args[0].clone()
                        }
                        Type::Applied(inner_name, type_args) if inner_name == "Result" && !type_args.is_empty() => {
                            type_args[0].clone()
                        }
                        _ => Type::TypeVar("T".to_string()),
                    }
                } else if name == "is_some" || name == "is_none" {
                    Type::Bool
                } else if name == "is_ok" || name == "is_err" {
                    Type::Bool
                } else if name == "unwrap_err" && !args.is_empty() {
                    let arg_ty = self.infer_expression(&args[0]);
                    match &arg_ty {
                        Type::Applied(inner_name, type_args) if inner_name == "Result" && type_args.len() >= 2 => {
                            type_args[1].clone()
                        }
                        _ => Type::TypeVar("E".to_string()),
                    }
                } else if let Some(sig) = self.signatures.get(name) {
                    // Handle generic type substitution for unwrap/is_some/etc
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
            Expr::ListLiteral(elements) => {
                let elem_type = elements
                    .first()
                    .map(|e| self.infer_expression(e))
                    .unwrap_or(Type::TypeVar("T".to_string()));
                Type::Applied("List".to_string(), vec![elem_type])
            }
            Expr::ListIndex(list_expr, _) => match self.infer_expression(list_expr) {
                Type::Applied(_, args) if !args.is_empty() => args[0].clone(),
                Type::Vector(inner, dims) => {
                    // For multidimensional, indexing returns the inner type with one fewer dimension
                    if dims.len() > 1 {
                        Type::Vector(inner.clone(), dims[1..].to_vec())
                    } else {
                        *inner.clone()
                    }
                }
                _ => Type::TypeVar("T".to_string()),
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
            Expr::FieldAccess(obj, field) => {
                // Look up the field type from the struct definition
                let obj_ty = self.infer_expression(obj);
                if let Type::Custom(struct_name) = &obj_ty {
                    if let Some(fields) = self.struct_fields.get(struct_name) {
                        if let Some(field_ty) = fields.get(field) {
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
                    ProjectionTarget::Pop => {
                        match &src_ty {
                            Type::Applied(name, inner) if name == "HashSet" => {
                                inner.first().cloned().unwrap_or(Type::String)
                            }
                            _ => {
                                self.errors.borrow_mut().push(TypeError::TypeMismatch {
                                    expected: "HashSet".to_string(),
                                    found: self.type_to_string(&src_ty),
                                    context: "Pop projection".to_string(),
                                });
                                Type::String
                            }
                        }
                    }
                    ProjectionTarget::Index(n) => {
                        match &src_ty {
                            Type::Tuple(types) => {
                                if *n < types.len() {
                                    types[*n].clone()
                                } else {
                                    self.errors.borrow_mut().push(TypeError::TypeMismatch {
                                        expected: format!("tuple with at least {} elements", n + 1),
                                        found: format!("tuple of length {}", types.len()),
                                        context: "Index projection".to_string(),
                                    });
                                    Type::Int
                                }
                            }
                            _ => {
                                self.errors.borrow_mut().push(TypeError::TypeMismatch {
                                    expected: "Tuple".to_string(),
                                    found: self.type_to_string(&src_ty),
                                    context: "Index projection".to_string(),
                                });
                                Type::Int
                            }
                        }
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
                        match &src_ty {
                            Type::Applied(name, inner) if name == "List" => {
                                let elem_ty = inner.first().cloned().unwrap_or(Type::String);
                                Type::Applied("Queue".to_string(), vec![elem_ty])
                            }
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
            Expr::Cast(..) => Type::Custom("unknown".to_string()),
            // ── Pattern B routing (direct destructure, not through trait) ──
            Expr::BinaryOp(bop) => {
                let l_ty = self.infer_expression(&bop.left);
                let r_ty = self.infer_expression(&bop.right);
                if l_ty == Type::Int && r_ty == Type::Int { Type::Int }
                else if l_ty == Type::Float && r_ty == Type::Float { Type::Float }
                else { Type::Int }
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
            | Expr::SliceExpr(_) | Expr::MultiSliceExpr(_) | Expr::FieldAccessExpr(_)
            | Expr::StructInstanceExpr(_) | Expr::ObjectLiteralExpr(_)
            | Expr::TupleExpr(_) | Expr::TupleDestructureExpr(_) | Expr::EllipsisExpr(_)
            | Expr::ArrowMutExpr(_) | Expr::ArrowDiscardExpr(_) | Expr::ArrowTransferExpr(_)
            | Expr::PatternMatchExpr(_) | Expr::MatchExpr(_) | Expr::BlockExpr(_)
            | Expr::SigCallExpr(_) | Expr::SubtypeProjectionExpr(_) | Expr::DbvlTableExpr(_)
            | Expr::TypeRef(_) => Type::Custom("unknown".to_string()),
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
            _ => Type::Custom("unknown".to_string()),
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
            // Custom types: exact match or variant-of-enum relationship
            (Type::Custom(a_name), Type::Custom(b_name)) => {
                if a_name == b_name {
                    true
                } else if let Some(parent) = self.enum_variants.get(a_name.as_str()) {
                    parent == b_name
                } else {
                    false
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

    fn check_expr_for_ffi_errors(&mut self, expr: &Expr) {
        match expr {
            Expr::Call(name, args) => {
                if self.foreign_bindings.contains_key(name) {
                    let binding = self.foreign_bindings.get(name).unwrap();
                    if !binding.success_output.is_empty() || binding.error_type_name != "Error" {
                        let mut diag = Diagnostic::new(
                            "T001",
                            Severity::Info,
                            "FFI call returns Result type",
                        )
                        .with_explanation(&format!(
                            "FFI function '{}' returns Result. Ensure both Success and Error branches are handled.",
                            name
                        ))
                        .with_hint("Use unification to handle both branches: Success(val) = func()");
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
            Expr::Add(l, r) | Expr::Sub(l, r) | Expr::Mul(l, r) | Expr::Div(l, r) => {
                self.check_expr_for_ffi_errors(l);
                self.check_expr_for_ffi_errors(r);
            }
            Expr::FieldAccess(obj, _) => {
                self.check_expr_for_ffi_errors(obj);
            }
            _ => {}
        }
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
                    Statement::Let { name: "x".into(), ty: Some(Type::Int), expr: Some(Expr::Identifier("y".into())), address: None, address_expr: None, bit_range: None, is_override: false, modifiers: vec![] },
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
            }),
        ]);
        let _ = tc.check_program(&mut prog);
        let diags = tc.get_diagnostics();
        assert!(!diags.is_empty(), "Should have at least one diagnostic for uninitialized");
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
}
