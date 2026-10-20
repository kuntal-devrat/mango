use super::super::stream::CssParser;
use crate::parser::types::FontFaceRule;
use crate::tokenizer::Token;
use crate::values::{FontDisplay, FontStyle, FontWeight};

impl CssParser {
    pub(crate) fn parse_font_face_rule(&mut self) -> Option<FontFaceRule> {
        self.skip_whitespace_and_comments();
        if self.advance() != Token::OpenCurly {
            return None;
        }

        let mut font_family = String::new();
        let mut src_url = String::new();
        let mut font_weight = FontWeight::Normal;
        let mut font_weight_range = None;
        let mut font_style = FontStyle::Normal;
        let mut font_display = FontDisplay::Auto;

        loop {
            self.skip_whitespace_and_comments();
            if *self.peek() == Token::CloseCurly || *self.peek() == Token::Eof {
                if *self.peek() == Token::CloseCurly {
                    self.advance();
                }
                break;
            }

            let prop_name = match self.advance() {
                Token::Ident(name) => name,
                _ => {
                    self.skip_until(Token::Semicolon);
                    if *self.peek() == Token::Semicolon {
                        self.advance();
                    }
                    continue;
                }
            };

            self.skip_whitespace_and_comments();
            if self.advance() != Token::Colon {
                self.skip_until(Token::Semicolon);
                if *self.peek() == Token::Semicolon {
                    self.advance();
                }
                continue;
            }

            // Gather value tokens until semicolon, close curly, or EOF
            let mut val_tokens = Vec::new();
            while *self.peek() != Token::Semicolon
                && *self.peek() != Token::CloseCurly
                && *self.peek() != Token::Eof
            {
                val_tokens.push(self.advance());
            }

            if *self.peek() == Token::Semicolon {
                self.advance();
            }

            let prop_lower = prop_name.to_ascii_lowercase();
            match prop_lower.as_str() {
                "font-family" => {
                    let mut fam = String::new();
                    for tok in &val_tokens {
                        match tok {
                            Token::String(s) => {
                                fam = s.clone();
                                break;
                            }
                            Token::Ident(s) => {
                                if !fam.is_empty() {
                                    fam.push(' ');
                                }
                                fam.push_str(s);
                            }
                            _ => {}
                        }
                    }
                    font_family = fam.trim().to_string();
                }
                "src" => {
                    // Extract URL(s) and their optional format(...) from tokens
                    let mut candidates: Vec<(String, Option<String>)> = Vec::new();
                    let mut i = 0;
                    while i < val_tokens.len() {
                        if let Token::Ident(name) = &val_tokens[i]
                            && name.eq_ignore_ascii_case("url")
                            && i + 1 < val_tokens.len()
                            && val_tokens[i + 1] == Token::OpenParen
                        {
                            let mut u = String::new();
                            let mut j = i + 2;
                            while j < val_tokens.len() && val_tokens[j] != Token::CloseParen {
                                match &val_tokens[j] {
                                    Token::String(s) => u = s.clone(),
                                    other => u.push_str(&self.token_to_string(other)),
                                }
                                j += 1;
                            }
                            let trimmed_u = u.trim().to_string();
                            let mut format_hint = None;

                            // Check if followed by format(...)
                            let mut k =
                                if j < val_tokens.len() && val_tokens[j] == Token::CloseParen {
                                    j + 1
                                } else {
                                    j
                                };
                            while k < val_tokens.len() && matches!(val_tokens[k], Token::Whitespace)
                            {
                                k += 1;
                            }
                            if k < val_tokens.len()
                                && matches!(&val_tokens[k], Token::Ident(fmt) if fmt.eq_ignore_ascii_case("format"))
                                && k + 1 < val_tokens.len()
                                && val_tokens[k + 1] == Token::OpenParen
                            {
                                let mut fmt_str = String::new();
                                let mut m = k + 2;
                                while m < val_tokens.len() && val_tokens[m] != Token::CloseParen {
                                    match &val_tokens[m] {
                                        Token::String(s) => fmt_str = s.clone(),
                                        Token::Ident(s) => fmt_str = s.clone(),
                                        other => fmt_str.push_str(&self.token_to_string(other)),
                                    }
                                    m += 1;
                                }
                                let cleaned = fmt_str
                                    .trim()
                                    .trim_matches('\'')
                                    .trim_matches('"')
                                    .to_ascii_lowercase();
                                if !cleaned.is_empty() {
                                    format_hint = Some(cleaned);
                                }
                                j = m;
                            }

                            if !trimmed_u.is_empty() {
                                candidates.push((trimmed_u, format_hint));
                            }
                            i = j;
                        }
                        i += 1;
                    }

                    // Format priority order per Section 6.4: woff2 > woff > ttf > otf
                    fn format_rank(url: &str, hint: Option<&str>) -> u32 {
                        let hint_l = hint.unwrap_or("");
                        let url_l = url.to_ascii_lowercase();

                        if hint_l == "woff2"
                            || url_l.contains(".woff2")
                            || url_l.contains("font/woff2")
                        {
                            4
                        } else if hint_l == "woff"
                            || url_l.contains(".woff")
                            || url_l.contains("font/woff")
                        {
                            3
                        } else if hint_l == "truetype"
                            || hint_l == "ttf"
                            || url_l.contains(".ttf")
                            || url_l.contains("font/ttf")
                        {
                            2
                        } else if hint_l == "opentype"
                            || hint_l == "otf"
                            || url_l.contains(".otf")
                            || url_l.contains("font/otf")
                        {
                            1
                        } else {
                            0
                        }
                    }

                    if !candidates.is_empty() {
                        let mut best_url = &candidates[0].0;
                        let mut best_rank =
                            format_rank(&candidates[0].0, candidates[0].1.as_deref());

                        for (cand_url, cand_hint) in &candidates[1..] {
                            let rank = format_rank(cand_url, cand_hint.as_deref());
                            if rank > best_rank {
                                best_rank = rank;
                                best_url = cand_url;
                            }
                        }
                        src_url = best_url.clone();
                    }
                }
                "font-weight" => {
                    let mut numbers = Vec::new();
                    for tok in &val_tokens {
                        match tok {
                            Token::Ident(s) if s.eq_ignore_ascii_case("bold") => {
                                font_weight = FontWeight::Bold;
                                break;
                            }
                            Token::Ident(s) if s.eq_ignore_ascii_case("normal") => {
                                font_weight = FontWeight::Normal;
                                break;
                            }
                            Token::Number(n) => {
                                numbers.push(*n as u16);
                            }
                            _ => {}
                        }
                    }
                    if numbers.len() >= 2 {
                        let min_w = numbers[0];
                        let max_w = numbers[1];
                        font_weight_range = Some((min_w, max_w));
                        font_weight = FontWeight::Normal;
                    } else if let Some(&w) = numbers.first() {
                        font_weight = match w {
                            400 => FontWeight::Normal,
                            700 => FontWeight::Bold,
                            100..=900 => FontWeight::Numeric(w),
                            _ if w >= 600 => FontWeight::Bold,
                            _ => FontWeight::Normal,
                        };
                    }
                }
                "font-style" => {
                    for tok in &val_tokens {
                        if let Token::Ident(s) = tok {
                            if s.eq_ignore_ascii_case("italic") || s.eq_ignore_ascii_case("oblique")
                            {
                                font_style = FontStyle::Italic;
                                break;
                            } else if s.eq_ignore_ascii_case("normal") {
                                font_style = FontStyle::Normal;
                                break;
                            }
                        }
                    }
                }
                "font-display" => {
                    for tok in &val_tokens {
                        if let Token::Ident(s) = tok
                            && let Some(fd) = FontDisplay::parse(s)
                        {
                            font_display = fd;
                            break;
                        }
                    }
                }
                _ => {}
            }
        }

        if !font_family.is_empty() && !src_url.is_empty() {
            Some(FontFaceRule {
                font_family,
                src_url,
                font_weight,
                font_weight_range,
                font_style,
                font_display,
            })
        } else {
            None
        }
    }
}
