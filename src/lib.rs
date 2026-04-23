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

pub mod annotator;
pub mod assertion_verify;
pub mod ast;
pub mod analysis;
pub mod backend;
pub mod cache;
pub mod desugarer;
pub mod errors;
pub mod ffi;
pub mod hardware;
pub mod hardware_validator;
pub mod import_resolver;
pub mod interpreter;
pub mod lexer;
pub mod lsp;
pub mod manifest;
pub mod parser;
pub mod proof_engine;
pub mod rbv;
pub mod reactor;
pub mod resolver;
pub mod scheduler;
pub mod sig_casting;
pub mod signal_graph;
pub mod symbolic;
pub mod typechecker;
pub mod view_compiler;
pub mod watch;
pub mod wrapper;