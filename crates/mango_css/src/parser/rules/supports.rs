use super::super::stream::CssParser;
use crate::tokenizer::Token;

impl CssParser {
    pub(crate) fn eval_supports_condition(&self, tokens: &[Token]) -> bool {
        let filtered: Vec<&Token> = tokens
            .iter()
            .filter(|t| !matches!(t, Token::Whitespace | Token::Comment(_) | Token::Eof))
            .collect();
        self.eval_supports_tokens(&filtered, 0)
    }

    pub(crate) fn eval_supports_tokens(&self, tokens: &[&Token], depth: usize) -> bool {
        if depth > 32 || tokens.is_empty() {
            return false;
        }

        // Check for 'or' at paren depth 0
        let mut p_depth = 0;
        for (i, &tok) in tokens.iter().enumerate() {
            match tok {
                Token::OpenParen => p_depth += 1,
                Token::CloseParen => {
                    if p_depth > 0 {
                        p_depth -= 1;
                    }
                }
                Token::Ident(w) if p_depth == 0 && w.eq_ignore_ascii_case("or") => {
                    return self.eval_supports_tokens(&tokens[..i], depth + 1)
                        || self.eval_supports_tokens(&tokens[i + 1..], depth + 1);
                }
                _ => {}
            }
        }

        // Check for 'and' at paren depth 0
        p_depth = 0;
        for (i, &tok) in tokens.iter().enumerate() {
            match tok {
                Token::OpenParen => p_depth += 1,
                Token::CloseParen => {
                    if p_depth > 0 {
                        p_depth -= 1;
                    }
                }
                Token::Ident(w) if p_depth == 0 && w.eq_ignore_ascii_case("and") => {
                    return self.eval_supports_tokens(&tokens[..i], depth + 1)
                        && self.eval_supports_tokens(&tokens[i + 1..], depth + 1);
                }
                _ => {}
            }
        }

        // Check for 'not <condition>'
        if tokens.len() >= 2
            && matches!(tokens[0], Token::Ident(w) if w.eq_ignore_ascii_case("not"))
        {
            return !self.eval_supports_tokens(&tokens[1..], depth + 1);
        }

        // Check if enclosed in parentheses: ( ... )
        if tokens.len() >= 2
            && *tokens[0] == Token::OpenParen
            && *tokens[tokens.len() - 1] == Token::CloseParen
        {
            let mut inner_depth = 0;
            let mut matches_outer = true;
            for &t in &tokens[..tokens.len() - 1] {
                match t {
                    Token::OpenParen => inner_depth += 1,
                    Token::CloseParen => {
                        inner_depth -= 1;
                        if inner_depth == 0 {
                            matches_outer = false;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            if matches_outer {
                let inner = &tokens[1..tokens.len() - 1];
                let mut colon_idx = None;
                let mut inner_p_depth = 0;
                for (j, &tok) in inner.iter().enumerate() {
                    match tok {
                        Token::OpenParen => inner_p_depth += 1,
                        Token::CloseParen => {
                            if inner_p_depth > 0 {
                                inner_p_depth -= 1;
                            }
                        }
                        Token::Colon if inner_p_depth == 0 => {
                            colon_idx = Some(j);
                            break;
                        }
                        _ => {}
                    }
                }

                if let Some(c_idx) = colon_idx {
                    let prop_tokens = &inner[..c_idx];
                    let val_tokens = &inner[c_idx + 1..];

                    let prop_name: String = prop_tokens
                        .iter()
                        .map(|t| self.token_to_string(t))
                        .collect::<String>()
                        .trim()
                        .to_ascii_lowercase();

                    if prop_name.starts_with("--") {
                        return !val_tokens.is_empty();
                    }

                    // CSS Conditional Rules Level 3 § 3.1: a declaration condition
                    // is true only if the UA supports that property *and* that
                    // value. An unsupported property makes the condition false
                    // (so `not (...)` is true).
                    if !crate::properties::is_supported_property(&prop_name) {
                        return false;
                    }

                    let mut val_str = String::new();
                    for t in val_tokens {
                        val_str.push_str(&self.token_to_string(t));
                    }
                    let decls =
                        crate::parser::parse_declaration_list(&format!("{prop_name}: {val_str}"));
                    return !decls.is_empty();
                }

                return self.eval_supports_tokens(inner, depth + 1);
            }
        }

        // Check for selector(...) functional syntax
        if tokens.len() >= 3
            && matches!(tokens[0], Token::Ident(name) if name.eq_ignore_ascii_case("selector"))
            && *tokens[1] == Token::OpenParen
            && *tokens[tokens.len() - 1] == Token::CloseParen
        {
            let sel_tokens = &tokens[2..tokens.len() - 1];
            let sel_str: String = sel_tokens.iter().map(|t| self.token_to_string(t)).collect();
            return super::super::parse_selectors(sel_str.trim()).is_some();
        }

        false
    }
}
