//! Container size evaluation and query matching for CSS Container Queries.

use mango_html::dom::{Document, NodeData, NodeId};

use crate::parser::parse_declaration_list;
use crate::values::{Length, Value};

use super::index::RuleIndex;

pub(crate) fn find_container_size_for_node(
    index: &RuleIndex,
    node_id: NodeId,
    doc: &Document,
    name_filter: Option<&str>,
) -> Option<(f32, f32)> {
    let mut current = doc.get(node_id).and_then(|n| n.parent);
    while let Some(ancestor_id) = current {
        let Some(ancestor_node) = doc.get(ancestor_id) else {
            break;
        };
        if let NodeData::Element(elem) = &ancestor_node.data {
            let mut container_type = None;
            let mut container_name = None;
            let mut explicit_width = None;
            let mut explicit_height = None;

            // 1. Inspect element's inline style
            if let Some(style_attr) = elem.get_attribute("style") {
                let decls = parse_declaration_list(style_attr);
                for d in decls {
                    let name = d.name.to_ascii_lowercase();
                    if name == "container-type" {
                        container_type = Some(d.value);
                    } else if name == "contain" {
                        let val_str = format!("{:?}", d.value).to_ascii_lowercase();
                        if val_str.contains("size")
                            || val_str.contains("inline-size")
                            || val_str.contains("strict")
                        {
                            if container_type.is_none() {
                                container_type = Some(Value::Keyword("inline-size".into()));
                            }
                        }
                    } else if name == "container-name" {
                        if let Value::String(s) = d.value {
                            container_name = Some(s);
                        } else if let Value::Keyword(s) = d.value {
                            container_name = Some(s);
                        }
                    } else if name == "container" {
                        for exp in d.expand_shorthand() {
                            if exp.name == "container-name" {
                                if let Value::String(s) = exp.value {
                                    container_name = Some(s);
                                } else if let Value::Keyword(s) = exp.value {
                                    container_name = Some(s);
                                }
                            } else if exp.name == "container-type" {
                                container_type = Some(exp.value);
                            }
                        }
                    } else if name == "width" {
                        if let Value::Length(Length::Px(px)) = d.value {
                            explicit_width = Some(px);
                        }
                    } else if name == "height"
                        && let Value::Length(Length::Px(px)) = d.value
                    {
                        explicit_height = Some(px);
                    }
                }
            }

            // 2. Inspect author stylesheet rules defining container properties
            for cand in &index.container_def_rules {
                for complex_sel in &cand.style_rule.selectors.selectors {
                    if complex_sel.matches(ancestor_id, doc) {
                        for decl in &cand.style_rule.declarations {
                            let mut check_decl = |dname: &str, dval: &Value| {
                                if dname == "container-type" && container_type.is_none() {
                                    container_type = Some(dval.clone());
                                } else if dname == "contain" && container_type.is_none() {
                                    let val_str = format!("{:?}", dval).to_ascii_lowercase();
                                    if val_str.contains("size")
                                        || val_str.contains("inline-size")
                                        || val_str.contains("strict")
                                    {
                                        container_type = Some(Value::Keyword("inline-size".into()));
                                    }
                                } else if dname == "container-name" && container_name.is_none() {
                                    if let Value::String(s) = dval {
                                        container_name = Some(s.clone());
                                    } else if let Value::Keyword(s) = dval {
                                        container_name = Some(s.clone());
                                    }
                                } else if dname == "width" && explicit_width.is_none() {
                                    if let Value::Length(Length::Px(px)) = dval {
                                        explicit_width = Some(*px);
                                    }
                                } else if dname == "height"
                                    && explicit_height.is_none()
                                    && let Value::Length(Length::Px(px)) = dval
                                {
                                    explicit_height = Some(*px);
                                }
                            };
                            if decl.is_shorthand() {
                                for exp in decl.expand_shorthand_ref() {
                                    check_decl(&exp.name.to_ascii_lowercase(), &exp.value);
                                }
                            } else {
                                check_decl(&decl.name.to_ascii_lowercase(), &decl.value);
                            }
                        }
                        break;
                    }
                }
            }

            // 3. HTML attributes
            if explicit_width.is_none()
                && let Some(w_str) = elem.get_attribute("width")
                && let Ok(w) = w_str.trim_end_matches("px").parse::<f32>()
            {
                explicit_width = Some(w);
            }
            if explicit_height.is_none()
                && let Some(h_str) = elem.get_attribute("height")
                && let Ok(h) = h_str.trim_end_matches("px").parse::<f32>()
            {
                explicit_height = Some(h);
            }
            if container_name.is_none()
                && let Some(cn) = elem.get_attribute("data-container-name")
            {
                container_name = Some(cn.to_string());
            }
            if container_type.is_none()
                && let Some(ct) = elem.get_attribute("data-container-type")
                && let Some(parsed_ct) = crate::values::ContainerType::parse(ct)
            {
                container_type = Some(Value::ContainerType(parsed_ct));
            }
            if container_type.is_none()
                && let Some(c) = elem.get_attribute("contain")
                && (c.contains("size") || c.contains("inline-size") || c.contains("strict"))
            {
                container_type = Some(Value::Keyword("inline-size".into()));
            }

            // Check if this element establishes a container
            let is_container = match &container_type {
                Some(Value::ContainerType(ct)) => *ct != crate::values::ContainerType::Normal,
                Some(Value::Keyword(k)) => k != "normal",
                _ => container_name.is_some(),
            };

            if is_container {
                let matches_name = match name_filter {
                    Some(filter) => {
                        if let Some(ref cn) = container_name {
                            cn.split_whitespace()
                                .any(|part| part.eq_ignore_ascii_case(filter))
                        } else {
                            false
                        }
                    }
                    None => true,
                };

                if matches_name {
                    let w = explicit_width.unwrap_or(index.viewport_width);
                    let h = explicit_height.unwrap_or(index.viewport_height);
                    return Some((w, h));
                }
            }
        }
        current = ancestor_node.parent;
    }
    None
}
