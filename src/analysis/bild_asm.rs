/// BILD `asm target { }` desugaring.
///
/// Converts architecture-specific inline assembly blocks in BILD bodies into
/// a single LLVM `call asm` instruction, selected at BILD compile time based
/// on the host target architecture.

/// Represents a single architecture arm in an `asm target { }` block.
#[derive(Debug, Clone)]
struct AsmTargetArm {
    /// Architecture names to match (e.g. "x86_64", "aarch64").
    /// Empty for `default` arm.
    arch_names: Vec<String>,
    /// Operating system to match (e.g. "linux"). Empty = any OS.
    os_name: Option<String>,
    /// Assembly instruction string (e.g. `"mov %2, %%r10; syscall"`).
    instruction: String,
    /// LLVM constraint string (e.g. `"={rax},{rax},{rdi},{rsi},{rdx},{r10}"`).
    constraints: String,
    /// Parameter types and registers (e.g. "i64 %nr, i64 %a1").
    params: String,
    /// Whether this is the `default` arm (matches everything).
    is_default: bool,
}

/// Parse an `asm target { }` block from BILD body lines.
/// `lines` is a slice starting with the `%res = asm target {` line
/// and ending with the `}` line.
fn parse_asm_target_block(lines: &[String]) -> Result<Vec<AsmTargetArm>, String> {
    if lines.len() < 3 {
        return Err("asm target block too short".to_string());
    }

    let mut arms = Vec::new();

    // Lines 1..len-1 are the arm lines (skip opening `asm target {` and closing `}`)
    for line in &lines[1..lines.len() - 1] {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Parse: [arch("x86_64")]: "instr" : "constraints" : (params)
        // or: default: "instr" : "constraints" : (params)
        let is_default = trimmed.starts_with("default");

        let arch_names = if is_default {
            Vec::new()
        } else if trimmed.starts_with('[') {
            // Extract arch names from [arch("x86_64")] or [arch("x86_64", "amd64")]
            let bracket_end = trimmed.find(']').ok_or("unclosed arch bracket")?;
            let inner = &trimmed[1..bracket_end];
            let parts: Vec<&str> = inner.split(',').collect();
            let names: Vec<String> = parts
                .iter()
                .filter_map(|p| {
                    let p = p.trim();
                    if let Some(start) = p.find('"') {
                        let rest = &p[start + 1..];
                        if let Some(end) = rest.find('"') {
                            return Some(rest[..end].to_string());
                        }
                    }
                    None
                })
                .collect();
            if names.is_empty() {
                return Err("no arch names found in bracket".to_string());
            }
            names
        } else {
            return Err(format!("expected [arch(...)] or default, got: {}", trimmed));
        };

        // Find the instruction string (first quoted string after `]:` or `default:`)
        let after_bracket = if is_default {
            trimmed.strip_prefix("default:").unwrap_or(trimmed)
        } else {
            let bracket_end = trimmed.find(']').ok_or("unclosed bracket")?;
            &trimmed[bracket_end + 1..]
        };

        // Content after the bracket/default label: `: "instr" : "constraints" : (params)`
        // Split on the content between three colon-separated parts
        // First strip leading `: ` after the bracket
        let content = if is_default {
            after_bracket.trim()
        } else {
            // After `[arch("x86_64")]` there's `: "instr" : ...`
            // Strip leading `:`
            let trimmed = after_bracket.trim();
            trimmed.strip_prefix(':').unwrap_or(trimmed).trim()
        };

        let parts: Vec<&str> = content.splitn(3, ':').map(|s| s.trim()).collect();
        if parts.len() < 3 {
            return Err(format!("expected instruction : constraints : params, got: {}", after_bracket));
        }

        // Extract instruction from first quoted string
        let instr_raw = parts[0].trim();
        let instruction = if let Some(start) = instr_raw.find('"') {
            let rest = &instr_raw[start + 1..];
            if let Some(end) = rest.rfind('"') {
                rest[..end].to_string()
            } else {
                return Err("unclosed instruction string".to_string());
            }
        } else {
            return Err("expected instruction string".to_string());
        };

        // Extract constraints from second quoted string
        let constraints_raw = parts[1].trim();
        let constraints = if let Some(start) = constraints_raw.find('"') {
            let rest = &constraints_raw[start + 1..];
            if let Some(end) = rest.rfind('"') {
                rest[..end].to_string()
            } else {
                return Err("unclosed constraints string".to_string());
            }
        } else {
            return Err("expected constraints string".to_string());
        };

        // Extract params from parenthesized expression
        let params_raw = parts[2].trim();
        let params = if let Some(start) = params_raw.find('(') {
            let rest = &params_raw[start + 1..];
            if let Some(end) = rest.rfind(')') {
                rest[..end].to_string()
            } else {
                return Err("unclosed params".to_string());
            }
        } else {
            // No params is OK
            String::new()
        };

        arms.push(AsmTargetArm {
            arch_names,
            os_name: None,
            instruction,
            constraints,
            params,
            is_default,
        });
    }

    if arms.is_empty() {
        return Err("no arms in asm target block".to_string());
    }

    Ok(arms)
}

/// Select the matching arm for the current target.
/// Uses the host's architecture and OS from `std::env::consts`.
fn select_arm<'a>(arms: &'a [AsmTargetArm]) -> Option<&'a AsmTargetArm> {
    let host_arch = std::env::consts::ARCH;
    let host_os = std::env::consts::OS;

    // First, try to find an explicit arch match
    for arm in arms {
        if arm.is_default {
            continue;
        }
        if arm.arch_names.iter().any(|a| a == host_arch) {
            // Check OS if specified
            if let Some(ref os) = arm.os_name {
                if os == host_os {
                    return Some(arm);
                }
            } else {
                return Some(arm);
            }
        }
    }

    // Fall back to default arm
    arms.iter().find(|a| a.is_default)
}

/// Build the LLVM `call asm` instruction string from a selected arm.
fn build_call_asm(arm: &AsmTargetArm, result_reg: &str) -> String {
    let params_trimmed = arm.params.trim();
    if params_trimmed.is_empty() {
        format!(
            "{} = call i64 asm \"{}\", \"{}\"()",
            result_reg, arm.instruction, arm.constraints
        )
    } else {
        // The params already contain types (e.g. "i64 %nr, i64 %a1")
        // Use them as-is in the call asm parameter list.
        format!(
            "{} = call i64 asm \"{}\", \"{}\"({})",
            result_reg,
            arm.instruction,
            arm.constraints,
            params_trimmed,
        )
    }
}

/// Desugar `asm target { }` blocks in a BILD body, returning new lines.
///
/// Scans for lines containing "asm target {", collects the block until
/// the matching "}", parses the arch arms, selects the matching arm for
/// the host target, and replaces the entire block with a single
/// `%res = call i64 asm ...` instruction.
pub fn desugar_asm_target(body: &[String]) -> Vec<String> {
    let host_arch = std::env::consts::ARCH;
    let mut result: Vec<String> = Vec::new();
    let mut i = 0;

    while i < body.len() {
        let line = body[i].trim().to_string();

        if line.contains("asm target {") {
            // Collect the block: from this line to the closing }
            let start_idx = i;
            let mut block_lines = Vec::new();

            // Extract the result register name from the opening line
            let result_reg = if let Some(eq_pos) = line.find(" = ") {
                line[..eq_pos].trim().to_string()
            } else {
                "%asm_result".to_string()
            };

            block_lines.push(body[i].clone());
            i += 1;

            // Look for the closing `}` line (line that is just "}" or starts with "}")
            let mut found_close = false;
            while i < body.len() {
                let cline = body[i].trim();
                if cline == "}" || cline.starts_with('}') {
                    block_lines.push(body[i].clone());
                    found_close = true;
                    i += 1;
                    break;
                }
                block_lines.push(body[i].clone());
                i += 1;
            }

            if !found_close {
                // Unclosed asm target block — keep original lines
                result.extend(block_lines);
                continue;
            }

            // Parse the block and desugar
            match parse_asm_target_block(&block_lines) {
                Ok(arms) => {
                    if let Some(arm) = select_arm(&arms) {
                        let desugared = build_call_asm(arm, &result_reg);
                        result.push(desugared);
                    } else {
                        // No matching arm found — emit diagnostic and fall through
                        eprintln!(
                            "warning: no matching asm target arm for {}-{}, using first arm",
                            host_arch,
                            std::env::consts::OS
                        );
                        if let Some(arm) = arms.first() {
                            let desugared = build_call_asm(arm, &result_reg);
                            result.push(desugared);
                        } else {
                            result.extend(block_lines);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("warning: asm target parse error: {}", e);
                    result.extend(block_lines);
                }
            }
        } else {
            result.push(body[i].clone());
            i += 1;
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_asm_body(lines: Vec<&str>) -> Vec<String> {
        lines.into_iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn test_parse_single_arm() {
        let body = make_asm_body(vec![
            "%res = asm target {",
            r#"  [arch("x86_64")]: "nop" : "={rax},{rax}" : (i64 %x);"#,
            "}",
            "term %res;",
        ]);
        let arms = parse_asm_target_block(&body[..3]).unwrap();
        assert_eq!(arms.len(), 1);
        assert_eq!(arms[0].arch_names, vec!["x86_64"]);
        assert_eq!(arms[0].instruction, "nop");
        assert_eq!(arms[0].constraints, "={rax},{rax}");
        assert_eq!(arms[0].params, "i64 %x");
        assert!(!arms[0].is_default);
    }

    #[test]
    fn test_parse_default_arm() {
        let body = make_asm_body(vec![
            "%res = asm target {",
            r#"  default: "ud2" : "={rax}" : ();"#,
            "}",
        ]);
        let arms = parse_asm_target_block(&body[..3]).unwrap();
        assert_eq!(arms.len(), 1);
        assert!(arms[0].is_default);
        assert_eq!(arms[0].instruction, "ud2");
    }

    #[test]
    fn test_build_call_asm_with_params() {
        let arm = AsmTargetArm {
            arch_names: vec!["x86_64".to_string()],
            os_name: None,
            instruction: "add %1, %0".to_string(),
            constraints: "={rax},{rsi},{rdi}".to_string(),
            params: "i64 %a, i64 %b".to_string(),
            is_default: false,
        };
        let result = build_call_asm(&arm, "%res");
        assert_eq!(
            result,
            "%res = call i64 asm \"add %1, %0\", \"={rax},{rsi},{rdi}\"(i64 %a, i64 %b)"
        );
    }

    #[test]
    fn test_build_call_asm_no_params() {
        let arm = AsmTargetArm {
            arch_names: vec!["x86_64".to_string()],
            os_name: None,
            instruction: "nop".to_string(),
            constraints: "".to_string(),
            params: "".to_string(),
            is_default: false,
        };
        let result = build_call_asm(&arm, "%res");
        assert_eq!(
            result,
            "%res = call i64 asm \"nop\", \"\"()"
        );
    }

    #[test]
    fn test_desugar_body() {
        let body = make_asm_body(vec![
            "%res = asm target {",
            r#"  [arch("x86_64")]: "mov %2, %%r10; syscall" : "={rax},{rax},{rdi},{rsi},{rdx},{r10}" : (i64 %nr, i64 %a1, i64 %a2, i64 %a3);"#,
            r#"  default: "ud2" : "={rax},{rax},{rdi},{rsi},{rdx}" : (i64 %nr, i64 %a1, i64 %a2, i64 %a3);"#,
            "}",
            "term %res;",
        ]);
        let desugared = desugar_asm_target(&body);
        // The desugared body should have one line (the call asm) + the term line
        assert_eq!(desugared.len(), 2);
        // First line should be a call asm
        assert!(desugared[0].contains("call i64 asm"));
        // Second line should be the term unchanged
        assert_eq!(desugared[1].trim(), "term %res;");
    }

    #[test]
    fn test_no_asm_target_passthrough() {
        let body = make_asm_body(vec![
            "%res = add i64 %a, %b;",
            "term %res;",
        ]);
        let desugared = desugar_asm_target(&body);
        assert_eq!(desugared.len(), 2);
        assert_eq!(desugared[0].trim(), "%res = add i64 %a, %b;");
        assert_eq!(desugared[1].trim(), "term %res;");
    }

    #[test]
    fn test_multiple_arch_arms() {
        let arms_data = vec![
            AsmTargetArm {
                arch_names: vec!["x86_64".to_string()],
                os_name: None,
                instruction: "syscall".to_string(),
                constraints: "={rax}".to_string(),
                params: "i64 %x".to_string(),
                is_default: false,
            },
            AsmTargetArm {
                arch_names: vec!["aarch64".to_string()],
                os_name: None,
                instruction: "svc #0".to_string(),
                constraints: "={x0}".to_string(),
                params: "i64 %x".to_string(),
                is_default: false,
            },
            AsmTargetArm {
                arch_names: vec![],
                os_name: None,
                instruction: "ud2".to_string(),
                constraints: "".to_string(),
                params: "".to_string(),
                is_default: true,
            },
        ];
        let selected = select_arm(&arms_data);
        // On x86_64, should select the x86_64 arm
        assert!(selected.is_some());
        let host_arch = std::env::consts::ARCH;
        if host_arch == "x86_64" {
            assert_eq!(selected.unwrap().instruction, "syscall");
        } else if host_arch == "aarch64" {
            assert_eq!(selected.unwrap().instruction, "svc #0");
        } else {
            assert!(selected.unwrap().is_default);
        }
    }
}
