//! CSS3 lexical scanner and tokenizer.
//!
//! Converts raw CSS text into a sequence of [`Token`]s according to the
//! W3C CSS Syntax Module Level 3 specification.

/// Tokens produced by the CSS tokenizer.
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    /// An identifier name (e.g. `color`, `div`, `sans-serif`).
    Ident(String),
    /// An at-keyword (e.g. `@media`, `@import`).
    AtKeyword(String),
    /// A hash token (e.g. `#header`, `#ff0033`).
    Hash(String),
    /// A quoted string literal (`"hello"` or `'world'`).
    String(String),
    /// A bad string literal (e.g. unescaped newline).
    BadString,
    /// A numeric value without a unit (e.g. `1.5`, `400`).
    Number(f32),
    /// A percentage value (e.g. `50%`).
    Percentage(f32),
    /// A numeric dimension with a unit (e.g. `16px`, `1.5em`, `2rem`, `100vh`).
    Dimension { value: f32, unit: String },
    /// Whitespace (spaces, tabs, newlines).
    Whitespace,
    /// Colon `:`
    Colon,
    /// Semicolon `;`
    Semicolon,
    /// Comma `,`
    Comma,
    /// Open bracket `[`
    OpenBracket,
    /// Close bracket `]`
    CloseBracket,
    /// Open parenthesis `(`
    OpenParen,
    /// Close parenthesis `)`
    CloseParen,
    /// Open curly brace `{`
    OpenCurly,
    /// Close curly brace `}`
    CloseCurly,
    /// Single delimiter character (e.g. `.`, `>`, `+`, `~`, `*`, `!`, `=`).
    Delim(char),
    /// Comment block `/* ... */`
    Comment(String),
    /// End of input stream.
    Eof,
}

/// Tokenizer for CSS text.
pub struct CssTokenizer<'a> {
    chars: std::str::Chars<'a>,
    current: Option<char>,
}

impl<'a> CssTokenizer<'a> {
    pub fn new(input: &'a str) -> Self {
        let mut chars = input.chars();
        let current = chars.next();
        Self { chars, current }
    }

    /// Tokenizes an entire string into a vector of [`Token`]s.
    pub fn tokenize(input: &'a str) -> Vec<Token> {
        let mut tokenizer = Self::new(input);
        let mut tokens = Vec::new();
        loop {
            let tok = tokenizer.next_token();
            if tok == Token::Eof {
                tokens.push(tok);
                break;
            }
            tokens.push(tok);
        }
        tokens
    }

    fn advance(&mut self) -> Option<char> {
        let prev = self.current;
        self.current = self.chars.next();
        prev
    }

    fn peek(&self) -> Option<char> {
        self.current
    }

    fn peek2(&self) -> Option<char> {
        self.chars.clone().next()
    }

    /// Pulls the next token from the stream.
    pub fn next_token(&mut self) -> Token {
        let ch = match self.advance() {
            Some(c) => c,
            None => return Token::Eof,
        };

            // Comments /* ... */ or Delim('/')
            if ch == '/' {
                if self.peek() == Some('*') {
                    self.advance(); // consume '*'
                    let mut comment = String::new();
                    while let Some(c) = self.advance() {
                        if c == '*' && self.peek() == Some('/') {
                            self.advance(); // consume '/'
                            break;
                        }
                        comment.push(c);
                    }
                    return Token::Comment(comment);
                }
                return Token::Delim('/');
            }

            // Whitespace
            if ch.is_ascii_whitespace() {
                while let Some(next) = self.peek() {
                    if next.is_ascii_whitespace() {
                        self.advance();
                    } else {
                        break;
                    }
                }
                return Token::Whitespace;
            }

            // Strings "..." or '...'
            if ch == '"' || ch == '\'' {
                let quote = ch;
                let mut string_val = String::new();
                let mut is_bad = false;
                while let Some(c) = self.advance() {
                    if c == quote {
                        break;
                    }
                    if c == '\n' || c == '\r' {
                        is_bad = true;
                        break;
                    }
                    if c == '\\' {
                        // String continuation: backslash followed by newline
                        if self.peek() == Some('\r') {
                            self.advance();
                            if self.peek() == Some('\n') {
                                self.advance();
                            }
                        } else if self.peek() == Some('\n') {
                            self.advance();
                        } else if let Some(escaped) = self.consume_escape() {
                            string_val.push(escaped);
                        }
                    } else {
                        string_val.push(c);
                    }
                }
                if is_bad {
                    return Token::BadString;
                }
                return Token::String(string_val);
            }

            // Punctuation
            match ch {
                ':' => return Token::Colon,
                ';' => return Token::Semicolon,
                ',' => return Token::Comma,
                '[' => return Token::OpenBracket,
                ']' => return Token::CloseBracket,
                '(' => return Token::OpenParen,
                ')' => return Token::CloseParen,
                '{' => return Token::OpenCurly,
                '}' => return Token::CloseCurly,
                _ => {}
            }

            // At-keyword (@media, @import, etc.)
            if ch == '@' {
                let name = self.consume_ident();
                return Token::AtKeyword(name);
            }

            // Hash (#header, #ff9900)
            if ch == '#' {
                let mut hash = String::new();
                while let Some(next) = self.peek() {
                    if next.is_ascii_alphanumeric() || next == '-' || next == '_' {
                        hash.push(self.advance().unwrap());
                    } else {
                        break;
                    }
                }
                return Token::Hash(hash);
            }

            // Numbers, Percentages, Dimensions (e.g. 10px, -5em, 50%, -.5px)
            let starts_number = ch.is_ascii_digit()
                || ((ch == '+' || ch == '-')
                    && (self.peek().map(|p| p.is_ascii_digit()).unwrap_or(false)
                        || (self.peek() == Some('.')
                            && self.peek2().map(|p| p.is_ascii_digit()).unwrap_or(false))))
                || (ch == '.' && self.peek().map(|p| p.is_ascii_digit()).unwrap_or(false));

            if starts_number {
                let mut num_str = String::from(ch);
                let mut has_dot = ch == '.';

                while let Some(next) = self.peek() {
                    if next.is_ascii_digit() {
                        num_str.push(self.advance().unwrap());
                    } else if next == '.' && !has_dot {
                        has_dot = true;
                        num_str.push(self.advance().unwrap());
                    } else {
                        break;
                    }
                }

                // Exponent notation: 1e3, 2.5e-2, etc.
                if let Some(e) = self.peek() {
                    if e == 'e' || e == 'E' {
                        let mut it = self.chars.clone();
                        let next1 = it.next();
                        let (is_exp, has_sign) = match next1 {
                            Some('+' | '-') => (it.next().map(|c| c.is_ascii_digit()).unwrap_or(false), true),
                            Some(c) if c.is_ascii_digit() => (true, false),
                            _ => (false, false),
                        };
                        if is_exp {
                            num_str.push(self.advance().unwrap());
                            if has_sign {
                                num_str.push(self.advance().unwrap());
                            }
                            while let Some(next) = self.peek() {
                                if next.is_ascii_digit() {
                                    num_str.push(self.advance().unwrap());
                                } else {
                                    break;
                                }
                            }
                        }
                    }
                }

                let value = num_str.parse::<f32>().unwrap_or(0.0);

                if self.peek() == Some('%') {
                    self.advance();
                    return Token::Percentage(value);
                }

                if self.peek().map(is_ident_start).unwrap_or(false) {
                    let unit = self.consume_ident();
                    return Token::Dimension { value, unit };
                }

                return Token::Number(value);
            }

            // Identifiers
            if is_ident_start(ch) || ch == '\\' {
                let mut ident = String::new();
                if ch == '\\' {
                    if let Some(escaped) = self.consume_escape() {
                        ident.push(escaped);
                    }
                } else {
                    ident.push(ch);
                }
                while let Some(next) = self.peek() {
                    if is_ident_char(next) {
                        ident.push(self.advance().unwrap());
                    } else if next == '\\' {
                        self.advance();
                        if let Some(escaped) = self.consume_escape() {
                            ident.push(escaped);
                        }
                    } else {
                        break;
                    }
                }
                if ident == "-" {
                    return Token::Delim('-');
                }
                return Token::Ident(ident);
            }

            // Any other delimiter
            Token::Delim(ch)
    }

    fn consume_ident(&mut self) -> String {
        let mut ident = String::new();
        while let Some(next) = self.peek() {
            if is_ident_char(next) {
                ident.push(self.advance().unwrap());
            } else if next == '\\' {
                self.advance();
                if let Some(escaped) = self.consume_escape() {
                    ident.push(escaped);
                }
            } else {
                break;
            }
        }
        ident
    }

    fn consume_escape(&mut self) -> Option<char> {
        let first = self.advance()?;
        if first.is_ascii_hexdigit() {
            let mut hex_val = first.to_digit(16).unwrap();
            let mut count = 1;
            while count < 6 {
                if let Some(p) = self.peek() {
                    if p.is_ascii_hexdigit() {
                        let digit = self.advance().unwrap().to_digit(16).unwrap();
                        hex_val = hex_val * 16 + digit;
                        count += 1;
                        continue;
                    }
                }
                break;
            }
            // If the next input code point is whitespace, consume it as well.
            if let Some(p) = self.peek() {
                if p == ' ' || p == '\t' || p == '\n' || p == '\r' || p == '\x0C' {
                    self.advance();
                    // If it was \r\n, consume the \n as well
                    if p == '\r' && self.peek() == Some('\n') {
                        self.advance();
                    }
                }
            }
            // If zero, surrogate, or > 0x10FFFF -> U+FFFD
            if hex_val == 0 || (0xD800..=0xDFFF).contains(&hex_val) || hex_val > 0x10FFFF {
                Some('\u{FFFD}')
            } else {
                std::char::from_u32(hex_val).or(Some('\u{FFFD}'))
            }
        } else {
            Some(first)
        }
    }
}

fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_' || c == '-' || !c.is_ascii()
}

fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-' || !c.is_ascii()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenize_ruleset() {
        let css = r#"h1.title { color: #ffa136; margin: 10px; font-size: 2rem; }"#;
        let tokens = CssTokenizer::tokenize(css);

        assert!(tokens.contains(&Token::Ident("h1".to_string())));
        assert!(tokens.contains(&Token::Delim('.')));
        assert!(tokens.contains(&Token::Ident("title".to_string())));
        assert!(tokens.contains(&Token::OpenCurly));
        assert!(tokens.contains(&Token::Ident("color".to_string())));
        assert!(tokens.contains(&Token::Colon));
        assert!(tokens.contains(&Token::Hash("ffa136".to_string())));
        assert!(tokens.contains(&Token::Dimension {
            value: 10.0,
            unit: "px".to_string()
        }));
        assert!(tokens.contains(&Token::Dimension {
            value: 2.0,
            unit: "rem".to_string()
        }));
        assert!(tokens.contains(&Token::CloseCurly));
    }

    #[test]
    fn test_tokenize_strings_and_comments() {
        let css = r#"/* Comment */ font-family: "Helvetica Neue", 'Arial';"#;
        let tokens = CssTokenizer::tokenize(css);

        assert_eq!(tokens[0], Token::Comment(" Comment ".to_string()));
        assert!(tokens.contains(&Token::String("Helvetica Neue".to_string())));
        assert!(tokens.contains(&Token::String("Arial".to_string())));
    }

    #[test]
    fn test_tokenize_hex_escapes() {
        let css = r#"content: "\a0· "; font-family: \48 elvetica;"#;
        let tokens = CssTokenizer::tokenize(css);
        assert!(tokens.contains(&Token::String("\u{00A0}· ".to_string())));
        assert!(tokens.contains(&Token::Ident("Helvetica".to_string())));
    }

    #[test]
    fn test_tokenize_signed_numbers_and_non_ascii() {
        let css = r#"margin: -.5px; padding: +.5em; opacity: .8; top: -10px;"#;
        let tokens = CssTokenizer::tokenize(css);
        assert!(tokens.contains(&Token::Dimension {
            value: -0.5,
            unit: "px".to_string()
        }));
        assert!(tokens.contains(&Token::Dimension {
            value: 0.5,
            unit: "em".to_string()
        }));
        assert!(tokens.contains(&Token::Number(0.8)));
        assert!(tokens.contains(&Token::Dimension {
            value: -10.0,
            unit: "px".to_string()
        }));

        // Non-ASCII idents (B5)
        let non_ascii_css = r#".café { color: red; }"#;
        let tokens2 = CssTokenizer::tokenize(non_ascii_css);
        assert!(tokens2.contains(&Token::Ident("café".to_string())));
    }

    #[test]
    fn test_tokenize_bad_string() {
        let css = "content: \"hello\nworld\";";
        let tokens = CssTokenizer::tokenize(css);
        assert!(tokens.contains(&Token::BadString));
    }

    #[test]
    fn test_tokenize_scientific_notation() {
        let css = "opacity: 1e3; margin: 2.5e-2px;";
        let tokens = CssTokenizer::tokenize(css);
        assert!(tokens.contains(&Token::Number(1000.0)));
        assert!(tokens.contains(&Token::Dimension {
            value: 0.025,
            unit: "px".to_string(),
        }));
    }
}
