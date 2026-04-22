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

//! Interactive mode for wrapper generation
//!
//! Prompts user when there are ambiguous choices

use super::AnalyzedFunction;
use std::io::{self, Write};

/// Prompt user to select from multiple options
pub fn prompt_choice<T: std::fmt::Display>(
    question: &str,
    options: &[T],
    allow_default: bool,
) -> usize {
    if options.is_empty() {
        return 0;
    }

    if options.len() == 1 && allow_default {
        return 0;
    }

    println!("\n{}", question);

    for (i, opt) in options.iter().enumerate() {
        println!("  [{}] {}", i + 1, opt);
    }

    if allow_default {
        println!("  [0] Use default (first option)");
    }

    print!("\nEnter choice: ");
    io::stdout().flush().ok();

    let mut input = String::new();
    if io::stdin().read_line(&mut input).is_ok() {
        if let Ok(choice) = input.trim().parse::<usize>() {
            if choice == 0 && allow_default {
                return 0;
            }
            if choice > 0 && choice <= options.len() {
                return choice - 1;
            }
        }
    }

    println!("Invalid choice, using default.");
    0
}

/// Prompt user to select contract conditions
pub fn choose_contracts(func: &AnalyzedFunction, suggestions: &[String]) -> Vec<String> {
    if suggestions.is_empty() {
        return vec!["true".to_string()];
    }

    if suggestions.len() == 1 {
        return suggestions.to_vec();
    }

    println!("\nMultiple contract options for '{}':", func.name);

    for (i, contract) in suggestions.iter().enumerate() {
        println!("  [{}] {}", i + 1, contract);
    }

    println!("  [A] All of the above");
    println!("  [0] Use default (first option)");

    print!("\nEnter choice: ");
    io::stdout().flush().ok();

    let mut input = String::new();
    if io::stdin().read_line(&mut input).is_ok() {
        let choice = input.trim();

        if choice.to_uppercase() == "A" {
            return suggestions.to_vec();
        }

        if let Ok(num) = choice.parse::<usize>() {
            if num == 0 {
                return vec![suggestions[0].clone()];
            }
            if num > 0 && num <= suggestions.len() {
                return vec![suggestions[num - 1].clone()];
            }
        }
    }

    vec![suggestions[0].clone()]
}

/// Check if a function has ambiguous signatures and needs user input
pub fn has_ambiguity(func: &AnalyzedFunction) -> bool {
    // Check for function overloading (multiple signatures with same name)
    // For now, just check for variadic functions
    func.is_variadic
}

/// Display function signature options
pub fn display_signature_options(func: &AnalyzedFunction, signatures: &[AnalyzedFunction]) {
    println!("\nFunction '{}' has multiple signatures:", func.name);

    for (i, sig) in signatures.iter().enumerate() {
        let params: Vec<String> = sig
            .parameters
            .iter()
            .map(|(n, t)| format!("{}: {}", n, t))
            .collect();

        println!(
            "  [{}] ({}) -> {}",
            i + 1,
            params.join(", "),
            sig.return_type
        );
    }
}

/// Interactive function selection
pub fn select_function(functions: &[AnalyzedFunction]) -> Vec<AnalyzedFunction> {
    if functions.len() <= 1 {
        return functions.to_vec();
    }

    println!("\n=== Select functions to wrap ===");
    println!(
        "Found {} functions. Select which to include:",
        functions.len()
    );

    for (i, func) in functions.iter().enumerate() {
        let params: Vec<String> = func
            .parameters
            .iter()
            .map(|(n, t)| format!("{}: {}", n, t))
            .collect();

        println!(
            "  [{}] {} ({}) -> {}",
            i + 1,
            func.name,
            params.join(", "),
            func.return_type
        );
    }

    println!("\n  [A] Select all");
    println!("  [0] Done (no more)");

    let mut selected = Vec::new();

    loop {
        print!("\nEnter function number (or A/0): ");
        io::stdout().flush().ok();

        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_ok() {
            let choice = input.trim();

            if choice.to_uppercase() == "A" {
                selected = functions.to_vec();
                break;
            }

            if choice == "0" {
                break;
            }

            if let Ok(num) = choice.parse::<usize>() {
                if num > 0 && num <= functions.len() {
                    selected.push(functions[num - 1].clone());
                    println!("  Added: {}", functions[num - 1].name);
                }
            }
        }
    }

    if selected.is_empty() {
        println!("No functions selected, using all.");
        functions.to_vec()
    } else {
        selected
    }
}
