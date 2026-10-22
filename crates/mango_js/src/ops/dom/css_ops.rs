//! CSS Deno ops: declaration validation, shorthand expansion, and supports checking.

use deno_core::op2;

/// Validates and parses a CSS declaration (property + value).
///
/// Returns `None` if the value is invalid for the property under W3C CSS grammar.
/// Returns `Some(Vec<(name, value)>)` with the expanded longhands and canonical string representations.
#[op2]
#[serde]
pub fn op_mango_css_parse_declaration(
    #[string] prop: String,
    #[string] value: String,
) -> Option<Vec<(String, String)>> {
    let p = prop.trim().to_ascii_lowercase();
    let v = value.trim();

    // Custom properties: accept any non-empty value
    if p.starts_with("--") {
        if v.is_empty() {
            return None;
        }
        return Some(vec![(prop, value)]);
    }

    let decls = mango_css::parse_declaration_list(&format!("{p}: {v}"));
    if decls.is_empty() {
        return None;
    }

    // CSSOM § 6.7.2 ("serialize a CSS value") requires the *specified* value to
    // round-trip through `setProperty()` / `getPropertyValue()`. Recomputing the
    // value from the parsed AST would rewrite `color: red` into
    // `rgb(255, 0, 0)`, so for a non-shorthand property we echo the value text
    // back. Shorthands are still expanded into their longhands.
    if decls.len() == 1 && decls[0].name == p {
        if p == "text-decoration-line" {
            return Some(vec![(p.clone(), decls[0].value.to_css_string())]);
        }
        let specified = strip_important(v);
        return Some(vec![(p.clone(), specified)]);
    }

    let mut result = Vec::with_capacity(decls.len());
    for d in decls {
        result.push((d.name, d.value.to_css_string()));
    }
    Some(result)
}

/// Removes a trailing `!important` annotation from a declaration value.
fn strip_important(value: &str) -> String {
    let lower = value.to_ascii_lowercase();
    match lower.rfind("!important") {
        Some(idx) if idx + "!important".len() >= lower.len() => {
            let trimmed = value[..idx].trim_end();
            trimmed.to_string()
        }
        _ => value.to_string(),
    }
}

/// Evaluates `CSS.supports(property, value)` or `CSS.supports(condition)`.
#[op2]
pub fn op_mango_css_supports(
    #[string] prop_or_condition: String,
    #[string] value: Option<String>,
) -> bool {
    mango_css::supports(&prop_or_condition, value.as_deref())
}
