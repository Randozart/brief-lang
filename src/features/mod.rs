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

// ── Pattern B Feature Modules ──────────────────────────────────────────
//
// Each module contains one coherent feature group — struct definition,
// parsing, typechecking, evaluation, and per-backend codegen — all
// co-located in a single file.
//
// New features: add the module declaration here, create the file, add
// the enum variant in ast.rs, and add a 1-line router arm in each pass.

pub mod traits;

// Expression features (to be populated during migration)
// pub mod literal;
// pub mod identifier;
// pub mod binary_op;
// pub mod unary_op;
// pub mod call;
// pub mod projection;
// pub mod collection;
// pub mod map;
// pub mod set;
// pub mod tuple;
// pub mod field;
// pub mod pattern_match;
// pub mod match_expr;
// pub mod block;
// pub mod arrow;
// pub mod subtype;
// pub mod cast;
// pub mod concat;
// pub mod sig_call;
// pub mod dbvl;
// pub mod ellipsis;

// Statement features
// pub mod stmt;

// TopLevel features
// pub mod toplevel;
