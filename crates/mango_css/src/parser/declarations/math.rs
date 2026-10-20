use super::super::stream::CssParser;
use crate::tokenizer::Token;
use crate::values::{Length, MathExpr, Value};

impl CssParser {
    pub fn parse_math_call(&self, tokens: &[Token]) -> Option<(Value, usize)> {
        let mut idx = 0;
        while idx < tokens.len() && matches!(tokens[idx], Token::Whitespace | Token::Comment(_)) {
            idx += 1;
        }
        if idx >= tokens.len() {
            return None;
        }
        let fn_name = match &tokens[idx] {
            Token::Ident(name) => {
                let lower = name.to_ascii_lowercase();
                if matches!(
                    lower.as_str(),
                    "calc"
                        | "min"
                        | "max"
                        | "clamp"
                        | "round"
                        | "mod"
                        | "rem"
                        | "abs"
                        | "sign"
                        | "sin"
                        | "cos"
                        | "tan"
                        | "asin"
                        | "acos"
                        | "atan"
                        | "atan2"
                        | "sqrt"
                        | "pow"
                        | "hypot"
                ) {
                    lower
                } else {
                    return None;
                }
            }
            _ => return None,
        };
        idx += 1;
        while idx < tokens.len() && matches!(tokens[idx], Token::Whitespace | Token::Comment(_)) {
            idx += 1;
        }
        if idx >= tokens.len() || tokens[idx] != Token::OpenParen {
            return None;
        }
        idx += 1; // consume OpenParen

        let mut depth = 1;
        let mut close_idx = None;
        for (j, tok) in tokens.iter().enumerate().skip(idx) {
            match tok {
                Token::OpenParen => depth += 1,
                Token::CloseParen => {
                    depth -= 1;
                    if depth == 0 {
                        close_idx = Some(j);
                        break;
                    }
                }
                _ => {}
            }
        }
        let close_idx = close_idx?;
        let inner_tokens = &tokens[idx..close_idx];

        let filtered: Vec<&Token> = inner_tokens
            .iter()
            .filter(|t| !matches!(t, Token::Whitespace | Token::Comment(_)))
            .collect();

        let math_expr = Self::parse_math_inner(&fn_name, &filtered, 0)?;
        let length = crate::values::make_math_length(math_expr);
        Some((Value::Length(length), close_idx + 1))
    }

    #[allow(dead_code)]
    pub(crate) fn parse_calc_call(&self, tokens: &[Token]) -> Option<(Value, usize)> {
        self.parse_math_call(tokens)
    }

    pub(crate) fn parse_math_inner(
        fn_name: &str,
        tokens: &[&Token],
        depth: usize,
    ) -> Option<MathExpr> {
        if depth >= 32 || tokens.is_empty() {
            return None;
        }
        match fn_name {
            "calc" => Self::parse_calc_sum(tokens, depth + 1),
            "min" => {
                let args = Self::split_comma_args(tokens);
                if args.is_empty() {
                    return None;
                }
                let mut exprs = Vec::with_capacity(args.len());
                for arg in args {
                    exprs.push(Self::parse_calc_sum(&arg, depth + 1)?);
                }
                Some(MathExpr::Min(exprs))
            }
            "max" => {
                let args = Self::split_comma_args(tokens);
                if args.is_empty() {
                    return None;
                }
                let mut exprs = Vec::with_capacity(args.len());
                for arg in args {
                    exprs.push(Self::parse_calc_sum(&arg, depth + 1)?);
                }
                Some(MathExpr::Max(exprs))
            }
            "clamp" => {
                let args = Self::split_comma_args(tokens);
                if args.len() != 3 {
                    return None;
                }
                let min = Self::parse_calc_sum(&args[0], depth + 1)?;
                let val = Self::parse_calc_sum(&args[1], depth + 1)?;
                let max = Self::parse_calc_sum(&args[2], depth + 1)?;
                Some(MathExpr::Clamp {
                    min: Box::new(min),
                    val: Box::new(val),
                    max: Box::new(max),
                })
            }
            "round" => {
                let args = Self::split_comma_args(tokens);
                let (strategy, val_idx, step_idx) = if args.len() == 3 {
                    let strat = if let Some(Token::Ident(s)) = args[0].first() {
                        match s.to_ascii_lowercase().as_str() {
                            "up" => crate::values::RoundStrategy::Up,
                            "down" => crate::values::RoundStrategy::Down,
                            "to-zero" => crate::values::RoundStrategy::ToZero,
                            _ => crate::values::RoundStrategy::Nearest,
                        }
                    } else {
                        crate::values::RoundStrategy::Nearest
                    };
                    (strat, 1, 2)
                } else if args.len() == 2 {
                    (crate::values::RoundStrategy::Nearest, 0, 1)
                } else {
                    return None;
                };
                let val = Self::parse_calc_sum(&args[val_idx], depth + 1)?;
                let step = Self::parse_calc_sum(&args[step_idx], depth + 1)?;
                Some(MathExpr::Round {
                    strategy,
                    val: Box::new(val),
                    step: Box::new(step),
                })
            }
            "mod" => {
                let args = Self::split_comma_args(tokens);
                if args.len() != 2 {
                    return None;
                }
                let a = Self::parse_calc_sum(&args[0], depth + 1)?;
                let b = Self::parse_calc_sum(&args[1], depth + 1)?;
                Some(MathExpr::Mod(Box::new(a), Box::new(b)))
            }
            "rem" => {
                let args = Self::split_comma_args(tokens);
                if args.len() != 2 {
                    return None;
                }
                let a = Self::parse_calc_sum(&args[0], depth + 1)?;
                let b = Self::parse_calc_sum(&args[1], depth + 1)?;
                Some(MathExpr::Rem(Box::new(a), Box::new(b)))
            }
            "abs" => {
                let args = Self::split_comma_args(tokens);
                if args.len() != 1 {
                    return None;
                }
                let a = Self::parse_calc_sum(&args[0], depth + 1)?;
                Some(MathExpr::Abs(Box::new(a)))
            }
            "sign" => {
                let args = Self::split_comma_args(tokens);
                if args.len() != 1 {
                    return None;
                }
                let a = Self::parse_calc_sum(&args[0], depth + 1)?;
                Some(MathExpr::Sign(Box::new(a)))
            }
            "sin" => {
                let args = Self::split_comma_args(tokens);
                if args.len() != 1 {
                    return None;
                }
                let a = Self::parse_calc_sum(&args[0], depth + 1)?;
                Some(MathExpr::Sin(Box::new(a)))
            }
            "cos" => {
                let args = Self::split_comma_args(tokens);
                if args.len() != 1 {
                    return None;
                }
                let a = Self::parse_calc_sum(&args[0], depth + 1)?;
                Some(MathExpr::Cos(Box::new(a)))
            }
            "tan" => {
                let args = Self::split_comma_args(tokens);
                if args.len() != 1 {
                    return None;
                }
                let a = Self::parse_calc_sum(&args[0], depth + 1)?;
                Some(MathExpr::Tan(Box::new(a)))
            }
            "asin" => {
                let args = Self::split_comma_args(tokens);
                if args.len() != 1 {
                    return None;
                }
                let a = Self::parse_calc_sum(&args[0], depth + 1)?;
                Some(MathExpr::Asin(Box::new(a)))
            }
            "acos" => {
                let args = Self::split_comma_args(tokens);
                if args.len() != 1 {
                    return None;
                }
                let a = Self::parse_calc_sum(&args[0], depth + 1)?;
                Some(MathExpr::Acos(Box::new(a)))
            }
            "atan" => {
                let args = Self::split_comma_args(tokens);
                if args.len() != 1 {
                    return None;
                }
                let a = Self::parse_calc_sum(&args[0], depth + 1)?;
                Some(MathExpr::Atan(Box::new(a)))
            }
            "atan2" => {
                let args = Self::split_comma_args(tokens);
                if args.len() != 2 {
                    return None;
                }
                let y = Self::parse_calc_sum(&args[0], depth + 1)?;
                let x = Self::parse_calc_sum(&args[1], depth + 1)?;
                Some(MathExpr::Atan2(Box::new(y), Box::new(x)))
            }
            "sqrt" => {
                let args = Self::split_comma_args(tokens);
                if args.len() != 1 {
                    return None;
                }
                let a = Self::parse_calc_sum(&args[0], depth + 1)?;
                Some(MathExpr::Sqrt(Box::new(a)))
            }
            "pow" => {
                let args = Self::split_comma_args(tokens);
                if args.len() != 2 {
                    return None;
                }
                let base = Self::parse_calc_sum(&args[0], depth + 1)?;
                let exp = Self::parse_calc_sum(&args[1], depth + 1)?;
                Some(MathExpr::Pow(Box::new(base), Box::new(exp)))
            }
            "hypot" => {
                let args = Self::split_comma_args(tokens);
                if args.is_empty() {
                    return None;
                }
                let mut exprs = Vec::with_capacity(args.len());
                for arg in args {
                    exprs.push(Self::parse_calc_sum(&arg, depth + 1)?);
                }
                Some(MathExpr::Hypot(exprs))
            }
            _ => None,
        }
    }

    pub(crate) fn split_comma_args<'a>(tokens: &[&'a Token]) -> Vec<Vec<&'a Token>> {
        let mut args = Vec::new();
        let mut current = Vec::new();
        let mut depth = 0;
        for tok in tokens {
            match tok {
                Token::OpenParen => {
                    depth += 1;
                    current.push(*tok);
                }
                Token::CloseParen => {
                    if depth > 0 {
                        depth -= 1;
                    }
                    current.push(*tok);
                }
                Token::Comma if depth == 0 => {
                    args.push(std::mem::take(&mut current));
                }
                _ => {
                    current.push(*tok);
                }
            }
        }
        if !current.is_empty() {
            args.push(current);
        }
        args
    }

    pub(crate) fn is_operator_token(tok: &Token) -> bool {
        match tok {
            Token::Delim('+' | '-' | '*' | '/') => true,
            Token::Ident(s) if s == "+" || s == "-" || s == "*" || s == "/" => true,
            _ => false,
        }
    }

    pub(crate) fn parse_calc_sum(tokens: &[&Token], depth: usize) -> Option<MathExpr> {
        if depth >= 32 || tokens.is_empty() {
            return None;
        }
        let mut paren_depth = 0;
        let mut split_idx = None;
        let mut is_sub = false;

        // Scan right-to-left for left-associative + and -
        for (i, tok) in tokens.iter().enumerate().rev() {
            match tok {
                Token::CloseParen => paren_depth += 1,
                Token::OpenParen => {
                    if paren_depth > 0 {
                        paren_depth -= 1;
                    }
                }
                _ if paren_depth == 0 && i > 0 => {
                    let is_plus = matches!(tok, Token::Delim('+'))
                        || match tok {
                            Token::Ident(s) => s == "+",
                            _ => false,
                        };
                    if is_plus && !Self::is_operator_token(tokens[i - 1]) {
                        split_idx = Some(i);
                        is_sub = false;
                        break;
                    }
                    let is_minus = matches!(tok, Token::Delim('-'))
                        || match tok {
                            Token::Ident(s) => s == "-",
                            _ => false,
                        };
                    if is_minus && !Self::is_operator_token(tokens[i - 1]) {
                        split_idx = Some(i);
                        is_sub = true;
                        break;
                    }
                }
                _ => {}
            }
        }

        if let Some(i) = split_idx {
            let left = Self::parse_calc_sum(&tokens[..i], depth + 1)?;
            let right = Self::parse_calc_product(&tokens[i + 1..], depth + 1)?;
            if is_sub {
                Some(MathExpr::Sub(Box::new(left), Box::new(right)))
            } else {
                Some(MathExpr::Add(Box::new(left), Box::new(right)))
            }
        } else {
            Self::parse_calc_product(tokens, depth + 1)
        }
    }

    pub(crate) fn parse_calc_product(tokens: &[&Token], depth: usize) -> Option<MathExpr> {
        if depth >= 32 || tokens.is_empty() {
            return None;
        }
        let mut paren_depth = 0;
        let mut split_idx = None;
        let mut is_div = false;

        // Scan right-to-left for left-associative * and /
        for (i, tok) in tokens.iter().enumerate().rev() {
            match tok {
                Token::CloseParen => paren_depth += 1,
                Token::OpenParen => {
                    if paren_depth > 0 {
                        paren_depth -= 1;
                    }
                }
                _ if paren_depth == 0 && i > 0 => {
                    let is_mul = matches!(tok, Token::Delim('*'))
                        || match tok {
                            Token::Ident(s) => s == "*",
                            _ => false,
                        };
                    if is_mul && !Self::is_operator_token(tokens[i - 1]) {
                        split_idx = Some(i);
                        is_div = false;
                        break;
                    }
                    let is_div_op = matches!(tok, Token::Delim('/'))
                        || match tok {
                            Token::Ident(s) => s == "/",
                            _ => false,
                        };
                    if is_div_op && !Self::is_operator_token(tokens[i - 1]) {
                        split_idx = Some(i);
                        is_div = true;
                        break;
                    }
                }
                _ => {}
            }
        }

        if let Some(i) = split_idx {
            let left = Self::parse_calc_product(&tokens[..i], depth + 1)?;
            let right = Self::parse_calc_atom(&tokens[i + 1..], depth + 1)?;
            if is_div {
                Some(MathExpr::Div(Box::new(left), Box::new(right)))
            } else {
                Some(MathExpr::Mul(Box::new(left), Box::new(right)))
            }
        } else {
            Self::parse_calc_atom(tokens, depth + 1)
        }
    }

    pub(crate) fn parse_calc_atom(tokens: &[&Token], depth: usize) -> Option<MathExpr> {
        if depth >= 32 || tokens.is_empty() {
            return None;
        }

        // 1. Parenthesized expression: `( ... )`
        if tokens.len() >= 2
            && *tokens[0] == Token::OpenParen
            && *tokens[tokens.len() - 1] == Token::CloseParen
        {
            let mut paren_depth = 0;
            let mut matches_outer = true;
            for (i, tok) in tokens.iter().enumerate() {
                if **tok == Token::OpenParen {
                    paren_depth += 1;
                } else if **tok == Token::CloseParen {
                    paren_depth -= 1;
                    if paren_depth == 0 && i < tokens.len() - 1 {
                        matches_outer = false;
                        break;
                    }
                }
            }
            if matches_outer {
                return Self::parse_calc_sum(&tokens[1..tokens.len() - 1], depth + 1);
            }
        }

        // 2. Nested function call: `min(...)`, `max(...)`, `clamp(...)`, `calc(...)`
        if tokens.len() >= 3
            && *tokens[1] == Token::OpenParen
            && *tokens[tokens.len() - 1] == Token::CloseParen
            && let Token::Ident(fn_name) = tokens[0]
        {
            let lower = fn_name.to_ascii_lowercase();
            if lower == "var" {
                let mut paren_depth = 0;
                let mut matches_outer = true;
                for (i, tok) in tokens[1..].iter().enumerate() {
                    if **tok == Token::OpenParen {
                        paren_depth += 1;
                    } else if **tok == Token::CloseParen {
                        paren_depth -= 1;
                        if paren_depth == 0 && i < tokens.len() - 2 {
                            matches_outer = false;
                            break;
                        }
                    }
                }
                if matches_outer {
                    return Self::parse_calc_var(&tokens[2..tokens.len() - 1], depth + 1);
                }
            } else if matches!(
                lower.as_str(),
                "calc"
                    | "min"
                    | "max"
                    | "clamp"
                    | "round"
                    | "mod"
                    | "rem"
                    | "abs"
                    | "sign"
                    | "sin"
                    | "cos"
                    | "tan"
                    | "asin"
                    | "acos"
                    | "atan"
                    | "atan2"
                    | "sqrt"
                    | "pow"
                    | "hypot"
            ) {
                let mut paren_depth = 0;
                let mut matches_outer = true;
                for (i, tok) in tokens[1..].iter().enumerate() {
                    if **tok == Token::OpenParen {
                        paren_depth += 1;
                    } else if **tok == Token::CloseParen {
                        paren_depth -= 1;
                        if paren_depth == 0 && i < tokens.len() - 2 {
                            matches_outer = false;
                            break;
                        }
                    }
                }
                if matches_outer {
                    return Self::parse_math_inner(&lower, &tokens[2..tokens.len() - 1], depth + 1);
                }
            }
        }

        // 3. Unary minus: `-` followed by atom
        let is_minus = matches!(tokens[0], Token::Delim('-'))
            || match tokens[0] {
                Token::Ident(s) => s == "-",
                _ => false,
            };
        if tokens.len() >= 2 && is_minus {
            let inner = Self::parse_calc_atom(&tokens[1..], depth + 1)?;
            return Some(MathExpr::Sub(
                Box::new(MathExpr::Number(0.0)),
                Box::new(inner),
            ));
        }

        // 4. Unary plus: `+` followed by atom
        let is_plus = matches!(tokens[0], Token::Delim('+'))
            || match tokens[0] {
                Token::Ident(s) => s == "+",
                _ => false,
            };
        if tokens.len() >= 2 && is_plus {
            return Self::parse_calc_atom(&tokens[1..], depth + 1);
        }

        // 5. Single leaf token:
        if tokens.len() == 1 {
            match tokens[0] {
                Token::Dimension { value, unit } => {
                    let u = unit.to_ascii_lowercase();
                    match u.as_str() {
                        "deg" => return Some(MathExpr::Number(value.to_radians())),
                        "grad" => {
                            return Some(MathExpr::Number(*value * std::f32::consts::PI / 200.0));
                        }
                        "rad" => return Some(MathExpr::Number(*value)),
                        "turn" => {
                            return Some(MathExpr::Number(*value * 2.0 * std::f32::consts::PI));
                        }
                        _ => {}
                    }
                    let len = match u.as_str() {
                        "px" => Length::Px(*value),
                        "em" => Length::Em(*value),
                        "rem" => Length::Rem(*value),
                        "vw" => Length::Vw(*value),
                        "vh" => Length::Vh(*value),
                        "vmin" => Length::Vmin(*value),
                        "vmax" => Length::Vmax(*value),
                        "cqw" => Length::Cqw(*value),
                        "cqh" => Length::Cqh(*value),
                        "cqi" => Length::Cqi(*value),
                        "cqb" => Length::Cqb(*value),
                        "cqmin" => Length::Cqmin(*value),
                        "cqmax" => Length::Cqmax(*value),
                        "pt" => Length::Px(*value * 4.0 / 3.0),
                        "pc" => Length::Px(*value * 16.0),
                        "in" => Length::Px(*value * 96.0),
                        "cm" => Length::Px(*value * 96.0 / 2.54),
                        "mm" => Length::Px(*value * 9.6 / 2.54),
                        "q" => Length::Px(*value * 96.0 / 101.6),
                        "ch" => Length::Em(*value * 0.5),
                        "ex" => Length::Em(*value * 0.5),
                        _ => Length::Px(*value),
                    };
                    return Some(MathExpr::Length(len));
                }
                Token::Percentage(pct) => {
                    return Some(MathExpr::Length(Length::Percent(*pct)));
                }
                Token::Number(n) => {
                    if *n == 0.0 {
                        return Some(MathExpr::Length(Length::Px(0.0)));
                    } else {
                        return Some(MathExpr::Number(*n));
                    }
                }
                _ => return None,
            }
        }

        None
    }

    pub(crate) fn parse_calc_var(tokens: &[&Token], depth: usize) -> Option<MathExpr> {
        let args = Self::split_comma_args(tokens);
        if args.is_empty() {
            return None;
        }
        let var_name = match args[0].first() {
            Some(Token::Ident(name)) if name.starts_with("--") => (*name).clone(),
            _ => return None,
        };
        let fallback = if args.len() > 1 {
            Self::parse_calc_sum(&args[1], depth + 1).map(Box::new)
        } else {
            None
        };
        Some(MathExpr::Var {
            name: var_name,
            fallback,
        })
    }
}

pub fn parse_math_call(tokens: &[Token]) -> Option<(Value, usize)> {
    let parser = CssParser::new(Vec::new());
    parser.parse_math_call(tokens)
}
