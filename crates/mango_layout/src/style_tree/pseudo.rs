use mango_css::computed::ComputedStyle;
use mango_css::values::{ContentItem, Display};
use mango_html::dom::{Document, NodeData, NodeId};

use super::counters::{CounterContext, format_counter_value};
use super::node::StyledNode;

pub enum PseudoPart {
    Text(String),
    Image(String),
}

pub fn create_pseudo_styled_node(
    host_node_id: NodeId,
    doc: &Document,
    mut style: ComputedStyle,
    counter_ctx: &mut CounterContext,
) -> Option<StyledNode> {
    let has_content = style.content.is_some()
        || style
            .content_items
            .as_ref()
            .is_some_and(|items| !items.is_empty());
    let mut parts = Vec::new();
    let mut text_acc = String::new();
    let quotes = style.effective_quotes().to_vec();

    if let Some(items) = style.content_items.take() {
        for item in items {
            match item {
                ContentItem::String(s) => text_acc.push_str(&s),
                ContentItem::Attr(attr_name) => {
                    if let Some(host_node) = doc.get(host_node_id)
                        && let NodeData::Element(host_elem) = &host_node.data
                        && let Some(val) = host_elem.get_attribute(&attr_name)
                    {
                        text_acc.push_str(val);
                    }
                }
                ContentItem::Counter {
                    name,
                    style: cstyle,
                } => {
                    let val = counter_ctx.get(&name);
                    let style_name = cstyle.as_deref().unwrap_or("decimal");
                    let formatted = format_counter_value(val, style_name);
                    text_acc.push_str(&formatted);
                }
                ContentItem::Counters {
                    name,
                    separator,
                    style: cstyle,
                } => {
                    let vals = counter_ctx.get_all(&name);
                    let style_name = cstyle.as_deref().unwrap_or("decimal");
                    let formatted: Vec<String> = vals
                        .iter()
                        .map(|&v| format_counter_value(v, style_name))
                        .collect();
                    text_acc.push_str(&formatted.join(&separator));
                }
                ContentItem::OpenQuote => {
                    if !quotes.is_empty() {
                        let idx = counter_ctx.quote_depth.min(quotes.len() - 1);
                        text_acc.push_str(&quotes[idx].0);
                    }
                    counter_ctx.quote_depth += 1;
                }
                ContentItem::CloseQuote => {
                    if counter_ctx.quote_depth > 0 {
                        counter_ctx.quote_depth -= 1;
                    }
                    if !quotes.is_empty() {
                        let idx = counter_ctx.quote_depth.min(quotes.len() - 1);
                        text_acc.push_str(&quotes[idx].1);
                    }
                }
                ContentItem::NoOpenQuote => {
                    counter_ctx.quote_depth += 1;
                }
                ContentItem::NoCloseQuote => {
                    if counter_ctx.quote_depth > 0 {
                        counter_ctx.quote_depth -= 1;
                    }
                }
                ContentItem::Url(url) => {
                    if !text_acc.is_empty() {
                        parts.push(PseudoPart::Text(std::mem::take(&mut text_acc)));
                    }
                    parts.push(PseudoPart::Image(url));
                }
            }
        }
    } else if let Some(legacy_text) = style.content.take() {
        text_acc.push_str(&legacy_text);
    }

    if !text_acc.is_empty() {
        parts.push(PseudoPart::Text(text_acc));
    } else if parts.is_empty() && has_content {
        // Synthesize empty text for pseudo-elements with empty content (e.g. content: ""; clearfix)
        parts.push(PseudoPart::Text(String::new()));
    }

    if parts.is_empty() {
        return None;
    }

    if parts.len() == 1 {
        match parts.into_iter().next().unwrap() {
            PseudoPart::Text(text) => {
                if style.display == Display::Inline {
                    Some(StyledNode::with_text(None, style, text))
                } else {
                    let mut pseudo_node = StyledNode::new(None, style.clone());
                    let mut inline_style = style;
                    inline_style.display = Display::Inline;
                    pseudo_node
                        .children
                        .push(StyledNode::with_text(None, inline_style, text));
                    Some(pseudo_node)
                }
            }
            PseudoPart::Image(url) => {
                let mut img_style = style;
                if img_style.display == Display::Inline {
                    img_style.display = Display::InlineBlock;
                }
                let mut img_node = StyledNode::new(None, img_style);
                img_node.tag_name = Some("img".to_string());
                img_node.attributes = vec![("src".to_string(), url)];
                Some(img_node)
            }
        }
    } else {
        let mut pseudo_node = StyledNode::new(None, style.clone());
        for part in parts {
            match part {
                PseudoPart::Text(t) => {
                    let mut inline_style = style.clone();
                    inline_style.display = Display::Inline;
                    pseudo_node
                        .children
                        .push(StyledNode::with_text(None, inline_style, t));
                }
                PseudoPart::Image(url) => {
                    let mut img_style = style.clone();
                    if img_style.display == Display::Inline {
                        img_style.display = Display::InlineBlock;
                    }
                    let mut img_node = StyledNode::new(None, img_style);
                    img_node.tag_name = Some("img".to_string());
                    img_node.attributes = vec![("src".to_string(), url)];
                    pseudo_node.children.push(img_node);
                }
            }
        }
        Some(pseudo_node)
    }
}

/// Collapses runs of whitespace characters into a single space, per CSS white-space: normal.
pub fn collapse_whitespace(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut in_whitespace = false;
    for ch in s.chars() {
        if ch.is_ascii_whitespace() {
            if !in_whitespace {
                result.push(' ');
                in_whitespace = true;
            }
        } else {
            result.push(ch);
            in_whitespace = false;
        }
    }
    result
}
