use super::super::stream::CssParser;
use crate::tokenizer::Token;
use crate::values::{GridAutoFlow, GridPlacement, GridTrackSize, Length, Value};
use mango_core::Color;

impl CssParser {
    pub(crate) fn parse_grid_track_list(&self, tokens: &[Token]) -> Option<Value> {
        let filtered: Vec<&Token> = tokens
            .iter()
            .filter(|t| !matches!(t, Token::Whitespace | Token::Comment(_)))
            .collect();

        if filtered.is_empty() {
            return None;
        }

        if filtered.len() == 1
            && let Token::Ident(w) = filtered[0]
            && w.eq_ignore_ascii_case("none")
        {
            return Some(Value::Keyword("none".to_string()));
        }

        let (tracks, lines) = self.parse_raw_tracks_with_lines(&filtered);
        if tracks.is_empty() {
            None
        } else if !lines.is_empty() {
            Some(Value::GridTrackListWithLines { tracks, lines })
        } else {
            Some(Value::GridTrackList(tracks))
        }
    }

    /// Parses a single shadow chunk into a `BoxShadow`.
    pub(crate) fn parse_single_box_shadow(
        &self,
        tokens: &[Token],
    ) -> Option<crate::values::BoxShadow> {
        let val = self.parse_value_from_tokens(tokens)?;
        match val {
            Value::BoxShadow(bs) => Some(bs),
            Value::List(ref items) => {
                if items.iter().any(|it| matches!(it, Value::Var { .. })) {
                    return None;
                }
                let mut offset_x = None;
                let mut offset_y = None;
                let mut blur_radius = 0.0f32;
                let mut spread_radius = 0.0f32;
                let mut color = Color::rgba(0, 0, 0, 255);
                let mut inset = false;
                let mut len_count = 0;

                for item in items {
                    match item {
                        Value::Keyword(k) if k.eq_ignore_ascii_case("inset") => inset = true,
                        Value::Length(len) => {
                            let px = match len {
                                Length::Px(p) => *p,
                                Length::Em(e) => *e * 16.0,
                                Length::Rem(r) => *r * 16.0,
                                Length::Calc(c) => c.px,
                                _ => 0.0,
                            };
                            match len_count {
                                0 => offset_x = Some(px),
                                1 => offset_y = Some(px),
                                2 => blur_radius = px,
                                3 => spread_radius = px,
                                _ => {}
                            }
                            len_count += 1;
                        }
                        Value::Number(n) => {
                            let px = *n;
                            match len_count {
                                0 => offset_x = Some(px),
                                1 => offset_y = Some(px),
                                2 => blur_radius = px,
                                3 => spread_radius = px,
                                _ => {}
                            }
                            len_count += 1;
                        }
                        Value::Color(c) => color = *c,
                        Value::Var { fallback, .. } => {
                            if let Some(fb) = fallback
                                && let Value::Color(c) = &**fb
                            {
                                color = *c;
                            }
                        }
                        _ => {}
                    }
                }

                if let (Some(ox), Some(oy)) = (offset_x, offset_y) {
                    Some(crate::values::BoxShadow {
                        offset_x: ox,
                        offset_y: oy,
                        blur_radius,
                        spread_radius,
                        color,
                        inset,
                    })
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Parses a CSS `box-shadow` declaration value, supporting comma-separated multiple shadows.
    pub(crate) fn parse_box_shadow(&self, tokens: &[Token]) -> Option<Value> {
        let filtered: Vec<&Token> = tokens
            .iter()
            .filter(|t| !matches!(t, Token::Whitespace | Token::Comment(_)))
            .collect();
        if filtered.is_empty() {
            return None;
        }
        if filtered.len() == 1
            && let Token::Ident(k) = filtered[0]
            && k.eq_ignore_ascii_case("none")
        {
            return Some(Value::Keyword("none".to_string()));
        }

        let chunks = super::transforms::split_tokens_by_top_level_comma(tokens);
        let mut shadows = Vec::new();
        for chunk in chunks {
            let chunk_filtered: Vec<&Token> = chunk
                .iter()
                .filter(|t| !matches!(t, Token::Whitespace | Token::Comment(_)))
                .collect();
            if chunk_filtered.is_empty() {
                continue;
            }
            if let Some(bs) = self.parse_single_box_shadow(chunk) {
                shadows.push(Value::BoxShadow(bs));
            } else if let Some(val) = self.parse_value_from_tokens(chunk) {
                shadows.push(val);
            }
        }

        if shadows.is_empty() {
            None
        } else if shadows.len() == 1 {
            Some(shadows.remove(0))
        } else {
            Some(Value::List(shadows))
        }
    }

    pub(crate) fn parse_raw_tracks_with_lines(
        &self,
        filtered: &[&Token],
    ) -> (Vec<GridTrackSize>, Vec<(String, usize)>) {
        let mut tracks = Vec::new();
        let mut lines = Vec::new();
        let mut i = 0;

        while i < filtered.len() {
            match filtered[i] {
                Token::OpenBracket => {
                    let close_idx = filtered
                        .iter()
                        .enumerate()
                        .skip(i + 1)
                        .find(|(_, tok)| ***tok == Token::CloseBracket)
                        .map(|(j, _)| j);
                    if let Some(end) = close_idx {
                        let line_tokens = &filtered[i + 1..end];
                        for tok in line_tokens {
                            if let Token::Ident(name) = tok {
                                lines.push((name.to_ascii_lowercase(), tracks.len()));
                            }
                        }
                        i = end + 1;
                        continue;
                    }
                }
                Token::Ident(name) if name.eq_ignore_ascii_case("repeat") => {
                    if i + 1 < filtered.len() && *filtered[i + 1] == Token::OpenParen {
                        let mut depth = 1usize;
                        let mut close_idx = None;
                        for (idx, tok) in filtered.iter().enumerate().skip(i + 2) {
                            if matches!(tok, Token::OpenParen) {
                                depth += 1;
                            } else if matches!(tok, Token::CloseParen) {
                                depth -= 1;
                                if depth == 0 {
                                    close_idx = Some(idx);
                                    break;
                                }
                            }
                        }
                        if let Some(end) = close_idx {
                            let inner = &filtered[i + 2..end];
                            let mut paren_depth = 0usize;
                            let mut comma_idx = None;
                            for (c_idx, tok) in inner.iter().enumerate() {
                                if matches!(tok, Token::OpenParen) {
                                    paren_depth += 1;
                                } else if matches!(tok, Token::CloseParen) {
                                    paren_depth = paren_depth.saturating_sub(1);
                                } else if paren_depth == 0 && matches!(tok, Token::Comma) {
                                    comma_idx = Some(c_idx);
                                    break;
                                }
                            }
                            if let Some(comma_idx) = comma_idx {
                                let count_tokens = &inner[..comma_idx];
                                let track_tokens = &inner[comma_idx + 1..];
                                let is_auto_fill = count_tokens.iter().any(|tok| match tok {
                                    Token::Ident(name) => name.eq_ignore_ascii_case("auto-fill"),
                                    _ => false,
                                });
                                let is_auto_fit = count_tokens.iter().any(|tok| match tok {
                                    Token::Ident(name) => name.eq_ignore_ascii_case("auto-fit"),
                                    _ => false,
                                });
                                let (repeated_tracks, repeated_lines) =
                                    self.parse_raw_tracks_with_lines(track_tokens);
                                if is_auto_fill {
                                    for t in repeated_tracks {
                                        tracks.push(GridTrackSize::RepeatAutoFill(Box::new(t)));
                                    }
                                } else if is_auto_fit {
                                    for t in repeated_tracks {
                                        tracks.push(GridTrackSize::RepeatAutoFit(Box::new(t)));
                                    }
                                } else {
                                    let count = match count_tokens.first() {
                                        Some(Token::Number(n)) => (*n as usize).max(1),
                                        _ => 1,
                                    };
                                    for _ in 0..count {
                                        let base_idx = tracks.len();
                                        tracks.extend(repeated_tracks.clone());
                                        for (lname, lidx) in &repeated_lines {
                                            lines.push((lname.clone(), base_idx + *lidx));
                                        }
                                    }
                                }
                            }
                            i = end + 1;
                            continue;
                        }
                    }
                }
                Token::Ident(name) if name.eq_ignore_ascii_case("minmax") => {
                    if i + 1 < filtered.len() && *filtered[i + 1] == Token::OpenParen {
                        let mut depth = 1usize;
                        let mut close_idx = None;
                        for (idx, tok) in filtered.iter().enumerate().skip(i + 2) {
                            if matches!(tok, Token::OpenParen) {
                                depth += 1;
                            } else if matches!(tok, Token::CloseParen) {
                                depth -= 1;
                                if depth == 0 {
                                    close_idx = Some(idx);
                                    break;
                                }
                            }
                        }
                        if let Some(end) = close_idx {
                            let inner = &filtered[i + 2..end];
                            let mut paren_depth = 0usize;
                            let mut comma_idx = None;
                            for (c_idx, tok) in inner.iter().enumerate() {
                                if matches!(tok, Token::OpenParen) {
                                    paren_depth += 1;
                                } else if matches!(tok, Token::CloseParen) {
                                    paren_depth = paren_depth.saturating_sub(1);
                                } else if paren_depth == 0 && matches!(tok, Token::Comma) {
                                    comma_idx = Some(c_idx);
                                    break;
                                }
                            }
                            if let Some(comma_idx) = comma_idx {
                                let min_tokens = &inner[..comma_idx];
                                let max_tokens = &inner[comma_idx + 1..];
                                if let (Some(min_tok), Some(max_tok)) =
                                    (min_tokens.first(), max_tokens.first())
                                    && let (Some(min_t), Some(max_t)) = (
                                        self.parse_single_track(min_tok),
                                        self.parse_single_track(max_tok),
                                    )
                                {
                                    tracks.push(GridTrackSize::MinMax(
                                        Box::new(min_t),
                                        Box::new(max_t),
                                    ));
                                }
                            }
                            i = end + 1;
                            continue;
                        }
                    }
                }
                token => {
                    if let Some(track) = self.parse_single_track(token) {
                        tracks.push(track);
                    }
                }
            }
            i += 1;
        }

        (tracks, lines)
    }

    #[allow(dead_code)]
    pub(crate) fn parse_raw_tracks(&self, filtered: &[&Token]) -> Vec<GridTrackSize> {
        self.parse_raw_tracks_with_lines(filtered).0
    }

    pub(crate) fn parse_grid_auto_flow(&self, tokens: &[Token]) -> Option<Value> {
        let filtered: Vec<&Token> = tokens
            .iter()
            .filter(|t| !matches!(t, Token::Whitespace | Token::Comment(_)))
            .collect();
        if filtered.is_empty() {
            return None;
        }

        let mut is_column = false;
        let mut is_dense = false;

        for tok in filtered {
            if let Token::Ident(ident) = tok {
                match ident.to_ascii_lowercase().as_str() {
                    "column" => is_column = true,
                    "row" => is_column = false,
                    "dense" => is_dense = true,
                    _ => return None,
                }
            } else {
                return None;
            }
        }

        let flow = match (is_column, is_dense) {
            (true, true) => GridAutoFlow::ColumnDense,
            (true, false) => GridAutoFlow::Column,
            (false, true) => GridAutoFlow::RowDense,
            (false, false) => GridAutoFlow::Row,
        };

        Some(Value::GridAutoFlow(flow))
    }

    pub(crate) fn parse_single_track(&self, token: &Token) -> Option<GridTrackSize> {
        match token {
            Token::Dimension { value, unit } => {
                if unit.eq_ignore_ascii_case("fr") {
                    Some(GridTrackSize::Fr(*value))
                } else if let Some(Value::Length(len)) = self.parse_single_value(token) {
                    Some(GridTrackSize::Length(len))
                } else {
                    None
                }
            }
            Token::Percentage(pct) => Some(GridTrackSize::Length(Length::Percent(*pct))),
            Token::Number(n) if *n == 0.0 => Some(GridTrackSize::Length(Length::Px(0.0))),
            Token::Ident(word) => match word.to_ascii_lowercase().as_str() {
                "auto" => Some(GridTrackSize::Auto),
                "min-content" => Some(GridTrackSize::MinContent),
                "max-content" => Some(GridTrackSize::MaxContent),
                "subgrid" => Some(GridTrackSize::Subgrid),
                _ => None,
            },
            _ => None,
        }
    }

    /// Parses `grid-template: <rows> / <columns>` shorthand or with template areas.
    pub(crate) fn parse_grid_template_shorthand(&self, tokens: &[Token]) -> Option<Value> {
        let slash_pos = tokens.iter().position(|t| *t == Token::Delim('/'))?;
        let rows_tokens = &tokens[..slash_pos];
        let cols_tokens = &tokens[slash_pos + 1..];

        let rows = self
            .parse_grid_track_list(rows_tokens)
            .unwrap_or(Value::GridTrackList(Vec::new()));
        let cols = self
            .parse_grid_track_list(cols_tokens)
            .unwrap_or(Value::GridTrackList(Vec::new()));

        Some(Value::List(vec![rows, cols]))
    }

    /// Parses `grid-template-areas: "..." "..."`.
    pub(crate) fn parse_grid_template_areas(&self, tokens: &[Token]) -> Option<Value> {
        let strings: Vec<Value> = tokens
            .iter()
            .filter_map(|t| match t {
                Token::String(s) => Some(Value::String(s.clone())),
                Token::Ident(w) if w.eq_ignore_ascii_case("none") => {
                    Some(Value::Keyword("none".to_string()))
                }
                _ => None,
            })
            .collect();
        if strings.is_empty() {
            None
        } else if strings.len() == 1 {
            Some(strings[0].clone())
        } else {
            Some(Value::List(strings))
        }
    }

    /// Parses grid placement shorthand (`1 / 3`, `1 / span 2`, `titlebar / sidebar`) or individual placement (`span 2`, `1`, `auto`, `content`).
    pub(crate) fn parse_grid_placement_or_shorthand(&self, tokens: &[Token]) -> Option<Value> {
        let filtered: Vec<&Token> = tokens
            .iter()
            .filter(|t| !matches!(t, Token::Whitespace | Token::Comment(_)))
            .collect();

        if filtered.is_empty() {
            return None;
        }

        let parts: Vec<&[&Token]> = filtered.split(|t| **t == Token::Delim('/')).collect();
        if parts.len() > 1 {
            let placements: Vec<Value> = parts
                .iter()
                .map(|p| Value::GridPlacement(self.parse_placement_component(p)))
                .collect();
            Some(Value::List(placements))
        } else {
            let placement = self.parse_placement_component(&filtered);
            Some(Value::GridPlacement(placement))
        }
    }

    pub(crate) fn parse_placement_component(&self, tokens: &[&Token]) -> GridPlacement {
        if tokens.is_empty() {
            return GridPlacement::Auto;
        }

        if tokens.len() == 1 {
            match tokens[0] {
                Token::Number(n) => return GridPlacement::Line(*n as i32),
                Token::Ident(s) if s.eq_ignore_ascii_case("auto") => return GridPlacement::Auto,
                Token::Ident(s) => return GridPlacement::Area(s.to_ascii_lowercase()),
                Token::String(s) => return GridPlacement::Area(s.to_ascii_lowercase()),
                _ => {}
            }
        }

        if tokens.len() == 2 {
            if let (Token::Ident(span_word), Token::Number(n)) = (tokens[0], tokens[1])
                && span_word.eq_ignore_ascii_case("span")
            {
                return GridPlacement::Span((*n as u16).max(1));
            }
            if let (Token::Ident(span_word), Token::Ident(name)) = (tokens[0], tokens[1])
                && span_word.eq_ignore_ascii_case("span")
            {
                return GridPlacement::Area(name.to_ascii_lowercase());
            }
            if let (Token::Delim('-'), Token::Number(n)) = (tokens[0], tokens[1]) {
                return GridPlacement::Line(-(*n as i32));
            }
        }

        if tokens.len() == 3 {
            if let (Token::Ident(span_word), Token::Number(n), Token::Ident(_)) =
                (tokens[0], tokens[1], tokens[2])
                && span_word.eq_ignore_ascii_case("span")
            {
                return GridPlacement::Span((*n as u16).max(1));
            }
            if let (Token::OpenBracket, Token::Ident(name), Token::CloseBracket) =
                (tokens[0], tokens[1], tokens[2])
            {
                return GridPlacement::Area(name.to_ascii_lowercase());
            }
        }

        GridPlacement::Auto
    }
}
