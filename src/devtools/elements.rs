//! Live Elements inspector and Console REPL evaluator for DevTools.
//!
//! Provides DOM tree inspection models, HTML tag formatting, CSS computed style
//! inspection, and interactive JavaScript console evaluation.

use mango_html::dom::{Document, NodeData, NodeId};
use serde::{Deserialize, Serialize};

use super::types::{ConsoleEntry, ConsoleLevel};

/// Serialized DOM node representing an element or text node in the Elements inspector.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InspectorNode {
    pub node_id: u32,
    pub node_type: u16,
    pub tag_name: String,
    pub attributes: Vec<(String, String)>,
    pub text_content: Option<String>,
    pub children: Vec<InspectorNode>,
}

impl InspectorNode {
    /// Formats the opening HTML tag representation (e.g. `<div id="app" class="main">`).
    pub fn format_tag(&self) -> String {
        if self.node_type == 3 {
            // Text node
            let txt = self.text_content.as_deref().unwrap_or("").trim();
            if txt.len() > 60 {
                format!("\"{}...\"", &txt[..57])
            } else {
                format!("\"{txt}\"")
            }
        } else if self.node_type == 8 {
            // Comment node
            format!("<!-- {} -->", self.text_content.as_deref().unwrap_or(""))
        } else {
            // Element
            let mut tag = format!("<{}", self.tag_name);
            for (k, v) in &self.attributes {
                tag.push_str(&format!(" {k}=\"{v}\""));
            }
            if self.children.is_empty() && self.text_content.is_none() {
                tag.push_str(" />");
            } else {
                tag.push('>');
            }
            tag
        }
    }

    /// Recursively finds a node by its unique ID.
    pub fn find_by_id(&self, target_id: u32) -> Option<&InspectorNode> {
        if self.node_id == target_id {
            return Some(self);
        }
        for child in &self.children {
            if let Some(found) = child.find_by_id(target_id) {
                return Some(found);
            }
        }
        None
    }
}

/// Builds an [`InspectorNode`] tree from a live parsed [`Document`].
pub fn build_inspector_tree(doc: &Document, root_id: NodeId) -> Option<InspectorNode> {
    let node = doc.get(root_id)?;
    let raw_id = root_id.raw();

    match &node.data {
        NodeData::Element(el) => {
            let mut children = Vec::new();
            for child in doc.children(root_id) {
                if let Some(child_node) = build_inspector_tree(doc, child.id) {
                    children.push(child_node);
                }
            }

            let attrs: Vec<(String, String)> = el
                .attributes
                .iter()
                .map(|(k, v)| (k.to_string(), v.clone()))
                .collect();

            Some(InspectorNode {
                node_id: raw_id,
                node_type: 1,
                tag_name: el.tag_name.to_lowercase(),
                attributes: attrs,
                text_content: None,
                children,
            })
        }
        NodeData::Text(txt) => Some(InspectorNode {
            node_id: raw_id,
            node_type: 3,
            tag_name: "#text".to_string(),
            attributes: Vec::new(),
            text_content: Some(txt.clone()),
            children: Vec::new(),
        }),
        NodeData::Comment(c) => Some(InspectorNode {
            node_id: raw_id,
            node_type: 8,
            tag_name: "#comment".to_string(),
            attributes: Vec::new(),
            text_content: Some(c.clone()),
            children: Vec::new(),
        }),
        _ => None,
    }
}

/// In-browser interactive console expression evaluator.
pub struct ConsoleEvaluator;

impl ConsoleEvaluator {
    /// Evaluates a JavaScript expression in the active `JsRuntime` and returns a formatted result.
    pub fn eval(
        js_rt: &mut Option<mango_js::JsRuntime>,
        expression: &str,
    ) -> (ConsoleEntry, Result<String, String>) {
        let trimmed = expression.trim();
        let time_str = "just now".to_string();

        let (entry, result) = match js_rt {
            Some(rt) => match rt.eval(trimmed) {
                Ok(val) => {
                    let msg = format!("> {trimmed}\n{val}");
                    (
                        ConsoleEntry {
                            level: ConsoleLevel::Log,
                            message: msg,
                            time_str,
                        },
                        Ok(val),
                    )
                }
                Err(e) => {
                    let err_msg = format!("> {trimmed}\nUncaught {e}");
                    (
                        ConsoleEntry {
                            level: ConsoleLevel::Error,
                            message: err_msg,
                            time_str,
                        },
                        Err(e),
                    )
                }
            },
            None => {
                let err_msg = format!("> {trimmed}\nRuntimeError: No active JavaScript realm");
                (
                    ConsoleEntry {
                        level: ConsoleLevel::Error,
                        message: err_msg.clone(),
                        time_str,
                    },
                    Err(err_msg),
                )
            }
        };

        (entry, result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_tag_element_and_text() {
        let el = InspectorNode {
            node_id: 1,
            node_type: 1,
            tag_name: "button".to_string(),
            attributes: vec![("class".to_string(), "btn-primary".to_string())],
            text_content: None,
            children: vec![InspectorNode {
                node_id: 2,
                node_type: 3,
                tag_name: "#text".to_string(),
                attributes: Vec::new(),
                text_content: Some("Click Me".to_string()),
                children: Vec::new(),
            }],
        };

        assert_eq!(el.format_tag(), "<button class=\"btn-primary\">");
        assert_eq!(el.children[0].format_tag(), "\"Click Me\"");

        let found = el.find_by_id(2);
        assert!(found.is_some());
        assert_eq!(found.unwrap().node_type, 3);
    }

    #[test]
    fn test_build_inspector_tree_from_document() {
        let mut doc = Document::new();
        let html_id = doc.create_element("html", Vec::new());
        let body_id = doc.create_element("body", Vec::new());
        let h1_id = doc.create_element("h1", vec![("id".into(), "heading".to_string())]);
        let txt_id = doc.create_text("Hello Mango");

        doc.append_child(h1_id, txt_id);
        doc.append_child(body_id, h1_id);
        doc.append_child(html_id, body_id);

        let tree = build_inspector_tree(&doc, html_id).expect("tree built");
        assert_eq!(tree.tag_name, "html");
        assert_eq!(tree.children.len(), 1); // body
        assert_eq!(tree.children[0].children.len(), 1); // h1
        assert_eq!(tree.children[0].children[0].attributes[0].0, "id");
    }
}
