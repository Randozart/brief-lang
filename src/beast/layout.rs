// ── Layout DSL Parser ───────────────────────────────────────────────────
// 2026-07-14: Recursive descent parser for the Layout DSL.
// Takes raw pattern string from layout <~ <...> and produces LayoutPattern.
// Max 2 nesting depth. Extract helpers.

use crate::ast::layout::*;

/// Parse a layout pattern string into a LayoutPattern AST.
/// Expected input: "le: [sign: 1, exp: 8, mant: 23]" or
/// "(@codepoint: (0x00..0x7F | ...))*"
pub fn parse_layout_pattern(input: &str) -> Result<LayoutPattern, String> {
    let tokens = tokenize(input)?;
    let mut pos = 0;
    parse_layout(&tokens, &mut pos)
}

fn expect_token<'a>(tokens: &'a [String], pos: &mut usize) -> Result<&'a str, String> {
    let t = tokens.get(*pos).ok_or_else(|| "unexpected end of input".to_string())?;
    *pos += 1;
    Ok(t.as_str())
}

fn peek(tokens: &[String], pos: usize) -> Option<&str> {
    tokens.get(pos).map(|s| s.as_str())
}

// ── Tokenizer ───────────────────────────────────────────────────────────

fn tokenize(input: &str) -> Result<Vec<String>, String> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '[' | ']' | '(' | ')' | '{' | '}' | ',' | '|' | '*' | '?' | ':' => {
                tokens.push(ch.to_string());
            }
            '!' | '$' | '@' | '.' => {
                tokens.push(ch.to_string());
            }
            ' ' | '\t' | '\n' | '\r' => {}
            '0' if chars.peek() == Some(&'x') => {
                chars.next();
                let mut hex = String::from("0x");
                while let Some(&c) = chars.peek() {
                    if c.is_ASCII_hexdigit() { hex.push(chars.next().unwrap()); } else { break; }
                }
                tokens.push(hex);
            }
            _ if ch.is_alphanumeric() || ch == '_' || ch == '#' || ch == '-' => {
                let mut ident = String::new();
                ident.push(ch);
                while let Some(&c) = chars.peek() {
                    if c.is_alphanumeric() || c == '_' || c == '#' || c == '-' { ident.push(chars.next().unwrap()); } else { break; }
                }
                tokens.push(ident);
            }
            _ if ch.is_digit(10) => {
                let mut num = String::new();
                num.push(ch);
                while let Some(&c) = chars.peek() {
                    if c.is_digit(10) { num.push(chars.next().unwrap()); } else { break; }
                }
                tokens.push(num);
            }
            _ => return Err(format!("unexpected character '{}' in layout pattern", ch)),
        }
    }
    Ok(tokens)
}

// ── Top-level ──────────────────────────────────────────────────────────

fn parse_layout(tokens: &[String], pos: &mut usize) -> Result<LayoutPattern, String> {
    // 2026-07-16: Tokenizer splits "le:" into ["le", ":"], so check for
    // "le" or "be" followed by ":". Also accept bare "le:" for compat.
    let t = peek(tokens, *pos);
    match t {
        Some("le:") => { *pos += 1; Ok(LayoutPattern::Slice(parse_slice(tokens, pos)?)) }
        Some("be:") => { *pos += 1; Ok(LayoutPattern::Slice(parse_slice(tokens, pos)?)) }
        Some("le") if peek(tokens, *pos + 1) == Some(":") => {
            *pos += 2; // consume "le" and ":"
            Ok(LayoutPattern::Slice(parse_slice(tokens, pos)?))
        }
        Some("be") if peek(tokens, *pos + 1) == Some(":") => {
            *pos += 2; // consume "be" and ":"
            Ok(LayoutPattern::Slice(parse_slice(tokens, pos)?))
        }
        _ => parse_pattern(tokens, pos),
    }
}

// ── Fixed-width slice ──────────────────────────────────────────────────

fn parse_slice(tokens: &[String], pos: &mut usize) -> Result<Vec<LayoutField>, String> {
    expect_token(tokens, pos)?; // consume '['
    let mut fields = Vec::new();
    loop {
        let next = peek(tokens, *pos);
        if next == Some("]") { *pos += 1; return Ok(fields); }
        fields.push(parse_field(tokens, pos)?);
        let next = peek(tokens, *pos);
        if next == Some(",") { *pos += 1; }
        else if next != Some("]") { return Err(format!("expected ',' or ']' in slice, got {:?}", next)); }
    }
}

fn parse_field(tokens: &[String], pos: &mut usize) -> Result<LayoutField, String> {
    let mutable = peek(tokens, *pos) == Some("!");
    if mutable { *pos += 1; }
    let structural = peek(tokens, *pos) == Some("$");
    if structural && !mutable { *pos += 1; }
    else if structural { return Err("!$ is not allowed — structural fields cannot be mutable".to_string()); }
    let name = expect_token(tokens, pos)?.to_string();
    expect_token(tokens, pos)?; // consume ':'
    let bits_str = expect_token(tokens, pos)?;
    let bits = bits_str.parse::<u64>().map_err(|_| format!("expected bit count, got '{}'", bits_str))?;
    Ok(LayoutField { name, bits, mutable, structural })
}

// ── Variable-width pattern ─────────────────────────────────────────────

fn parse_pattern(tokens: &[String], pos: &mut usize) -> Result<LayoutPattern, String> {
    parse_alternation(tokens, pos)
}

fn parse_alternation(tokens: &[String], pos: &mut usize) -> Result<LayoutPattern, String> {
    let mut items = Vec::new();
    items.push(parse_sequence(tokens, pos)?);
    while peek(tokens, *pos) == Some("|") {
        *pos += 1;
        items.push(parse_sequence(tokens, pos)?);
    }
    if items.len() == 1 { Ok(items.remove(0)) } else { Ok(LayoutPattern::Alternation(items)) }
}

fn parse_sequence(tokens: &[String], pos: &mut usize) -> Result<LayoutPattern, String> {
    let mut items = Vec::new();
    while let Some(next) = peek(tokens, *pos) {
        if matches!(next, "|" | ")" | "]" | "}" | "*" | "?") { break; }
        items.push(parse_primary(tokens, pos)?);
    }
    if items.len() == 1 { Ok(items.remove(0)) } else { Ok(LayoutPattern::Sequence(items)) }
}

fn parse_primary(tokens: &[String], pos: &mut usize) -> Result<LayoutPattern, String> {
    let next = peek(tokens, *pos).ok_or_else(|| "unexpected end of input".to_string())?;

    // Label: @name: pattern
    if next.starts_with('@') {
        let label = next[1..].to_string();
        *pos += 1;
        expect_token(tokens, pos)?; // consume ':'
        let inner = parse_pattern(tokens, pos)?;
        return apply_repetition(tokens, pos, LayoutPattern::SemanticLabel(label, Box::new(inner)));
    }

    // Generic param: $T, $K, $V
    if next == "$" {
        *pos += 1;
        let name = expect_token(tokens, pos)?;
        if name.chars().all(|c: char| c.is_uppercase() || c == '_') {
            return apply_repetition(tokens, pos, LayoutPattern::GenericParam(name.to_string()));
        }
        return apply_repetition(tokens, pos, LayoutPattern::VariableRef(name.to_string()));
    }

    // Pointer ref: *elements
    if next == "*" {
        *pos += 1;
        let name = expect_token(tokens, pos)?;
        return apply_repetition(tokens, pos, LayoutPattern::PointerRef(name.to_string()));
    }

    match next {
        "(" => {
            *pos += 1;
            let inner = parse_pattern(tokens, pos)?;
            expect_token(tokens, pos)?; // consume ')'
            apply_repetition(tokens, pos, inner)
        }
        "[" => {
            // 2026-07-16: Advance past '[' to prevent infinite recursion.
            // parse_layout() expects '[' already consumed when called from
            // a non-le:/be: path (parse_primary is the only such caller).
            *pos += 1;
            parse_layout(tokens, pos)
        }
        "{" => parse_any_bytes(tokens, pos),
        _ => {
            if next.starts_with("0x") {
                return parse_byte_literal_or_range(tokens, pos);
            }
            // Try as integer (for {N} count at start)
            if next.chars().all(|c| c.is_digit(10)) {
                let n = next.parse::<u64>().unwrap();
                *pos += 1;
                return Ok(LayoutPattern::AnyBytes(n));
            }
            Err(format!("unexpected token '{}' in layout pattern", next))
        }
    }
}

// ── Repetition ─────────────────────────────────────────────────────────

fn apply_repetition(tokens: &[String], pos: &mut usize, pattern: LayoutPattern) -> Result<LayoutPattern, String> {
    match peek(tokens, *pos) {
        Some("*") => { *pos += 1; Ok(LayoutPattern::Repetition(Box::new(pattern))) }
        Some("?") => { *pos += 1; Ok(LayoutPattern::Optional(Box::new(pattern))) }
        _ => Ok(pattern),
    }
}

// ── Byte literals and ranges ──────────────────────────────────────────

fn parse_byte_literal_or_range(tokens: &[String], pos: &mut usize) -> Result<LayoutPattern, String> {
    let first = expect_token(tokens, pos)?;
    let a = parse_hex_byte(first)?;
    if peek(tokens, *pos) == Some("..") {
        *pos += 1;
        let second = expect_token(tokens, pos)?;
        let b = parse_hex_byte(second)?;
        Ok(LayoutPattern::ByteRange(a, b))
    } else {
        Ok(LayoutPattern::ByteLiteral(a))
    }
}

fn parse_hex_byte(s: &str) -> Result<u8, String> {
    let hex = s.strip_prefix("0x").unwrap_or(s);
    u8::from_str_radix(hex, 16).map_err(|_| format!("invalid hex byte '{}'", s))
}

// ── Any bytes / typed ref ──────────────────────────────────────────────

fn parse_any_bytes(tokens: &[String], pos: &mut usize) -> Result<LayoutPattern, String> {
    *pos += 1; // consume '{'
    let first = expect_token(tokens, pos)?;

    // Typed ref: {$count, T}
    if peek(tokens, *pos) == Some(",") {
        *pos += 1;
        let mut type_parts = Vec::new();
        while peek(tokens, *pos) != Some("}") {
            type_parts.push(expect_token(tokens, pos)?);
        }
        expect_token(tokens, pos)?; // consume '}'
        let type_expr = type_parts.join(" ");
        let count_ref = if first.starts_with('$') {
            first[1..].to_string()
        } else {
            first.to_string()
        };
        let elem = if type_expr.starts_with('(') {
            parse_pair_type(&type_expr)?
        } else if type_expr.starts_with('$') {
            LayoutPattern::GenericParam(type_expr[1..].to_string())
        } else {
            LayoutPattern::GenericParam(type_expr)
        };
        return apply_repetition(tokens, pos, LayoutPattern::TypedRef(count_ref, Box::new(elem)));
    }

    // Simple {N}
    expect_token(tokens, pos)?; // consume '}'
    let n = first.parse::<u64>().map_err(|_| format!("expected number, got '{}'", first))?;
    apply_repetition(tokens, pos, LayoutPattern::AnyBytes(n))
}

/// Parse a pair type like "(K, V)" into a Sequence pattern.
fn parse_pair_type(s: &str) -> Result<LayoutPattern, String> {
    let inner = s.trim().strip_prefix('(').and_then(|s| s.strip_suffix(')'))
        .ok_or_else(|| format!("expected (Type, Type), got '{}'", s))?;
    let parts: Vec<&str> = inner.splitn(2, ',').map(|s| s.trim()).collect();
    if parts.len() != 2 {
        return Err(format!("pair type should have exactly 2 elements, got '{}'", inner));
    }
    let a = if parts[0].starts_with('$') {
        LayoutPattern::GenericParam(parts[0][1..].to_string())
    } else {
        LayoutPattern::GenericParam(parts[0].to_string())
    };
    let b = if parts[1].starts_with('$') {
        LayoutPattern::GenericParam(parts[1][1..].to_string())
    } else {
        LayoutPattern::GenericParam(parts[1].to_string())
    };
    Ok(LayoutPattern::Sequence(vec![a, b]))
}
