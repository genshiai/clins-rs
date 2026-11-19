// SPDX-License-Identifier: Apache-2.0
//! Syntax validation of the brief ECL focus/set-expression grammar.
//! Refinements, filters and alternate identifiers are explicitly unsupported;
//! parsing a supported expression does not validate its terminology membership.
//! Grammar: https://github.com/IHTSDO/snomed-expression-constraint-language
#[derive(Debug, PartialEq)]
pub enum Syntax {
    Valid,
    Invalid,
    Unsupported,
}
pub fn check(text: &str) -> Syntax {
    if text.len() > 4096 {
        return Syntax::Unsupported;
    }
    let mut input = text;
    let mut tokens = Vec::new();
    while !input.is_empty() {
        input = input.trim_start();
        if input.is_empty() {
            break;
        }
        if input.starts_with('|') {
            let Some(end) = input[1..].find('|') else {
                return Syntax::Invalid;
            };
            input = &input[end + 2..];
            continue;
        }
        if let Some(op) = ["<<", ">>", "<!", ">!", "<", ">", "^", "(", ")", "*", ","]
            .into_iter()
            .find(|op| input.starts_with(op))
        {
            tokens.push(op);
            input = &input[op.len()..];
            continue;
        }
        if input.starts_with(|c: char| c.is_ascii_digit()) {
            let end = input
                .find(|c: char| !c.is_ascii_digit())
                .unwrap_or(input.len());
            let id = &input[..end];
            if !(6..=18).contains(&id.len()) || id.starts_with('0') {
                return Syntax::Invalid;
            }
            tokens.push(id);
            input = &input[end..];
            continue;
        }
        if let Some(op) = ["AND", "OR", "MINUS"].into_iter().find(|op| {
            input.to_ascii_uppercase().starts_with(op)
                && input[op.len()..]
                    .chars()
                    .next()
                    .is_none_or(|c| !c.is_ascii_alphanumeric())
        }) {
            tokens.push(op);
            input = &input[op.len()..];
            continue;
        }
        // The remaining ECL grammar must not be confused with invalid input.
        if input.starts_with([':', '{', '}', '.', '!', '$', '#', '"', '['])
            || input.starts_with(|c: char| c.is_ascii_alphabetic())
        {
            return Syntax::Unsupported;
        }
        return Syntax::Invalid;
    }
    fn expression(tokens: &[&str], index: &mut usize, depth: usize) -> Result<(), Syntax> {
        if depth > 32 {
            return Err(Syntax::Unsupported);
        }
        fn focus(tokens: &[&str], index: &mut usize, depth: usize) -> Result<(), Syntax> {
            if tokens
                .get(*index)
                .is_some_and(|t| matches!(*t, "<<" | ">>" | "<!" | ">!" | "<" | ">"))
            {
                *index += 1;
            }
            if tokens.get(*index) == Some(&"^") {
                *index += 1;
            }
            match tokens.get(*index) {
                Some(&"(") => {
                    *index += 1;
                    expression(tokens, index, depth + 1)?;
                    if tokens.get(*index) != Some(&")") {
                        return Err(Syntax::Invalid);
                    }
                    *index += 1;
                    Ok(())
                }
                Some(t) if *t == "*" || t.bytes().all(|b| b.is_ascii_digit()) => {
                    *index += 1;
                    Ok(())
                }
                _ => Err(Syntax::Invalid),
            }
        }
        focus(tokens, index, depth)?;
        while tokens
            .get(*index)
            .is_some_and(|t| matches!(*t, "AND" | "OR" | "MINUS" | ","))
        {
            *index += 1;
            focus(tokens, index, depth)?;
        }
        Ok(())
    }
    let mut index = 0;
    match expression(&tokens, &mut index, 0) {
        Ok(()) if index == tokens.len() => Syntax::Valid,
        Ok(()) => Syntax::Invalid,
        Err(e) => e,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn set_expressions_reject_trailing_focus_operators_without_claiming_refinement_support() {
        for value in [
            "<< 404684003 |Clinical finding|",
            "(< 404684003 OR ^ 900000000000497000) MINUS 64572001",
            "*",
            "234234 and 64572001",
        ] {
            assert_eq!(check(value), Syntax::Valid, "{value}");
        }
        for value in [
            "234234 << 234234",
            "<<",
            "(404684003",
            "404684003 OR",
            "",
            "<< 012345",
        ] {
            assert_eq!(check(value), Syntax::Invalid, "{value}");
        }
        for value in [
            "<< 404684003 : 363698007 = 39057004",
            "^ * {{ M active = true }}",
        ] {
            assert_eq!(check(value), Syntax::Unsupported, "{value}");
        }
    }
}
