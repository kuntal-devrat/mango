use std::collections::HashMap;

use mango_html::dom::{Document, NodeId};
use mango_layout::box_tree::LayoutBox;

/// Parses a simple JSON-RPC 2.0 request: id, method, and string key-value params.
pub(crate) fn parse_json_rpc(input: &str) -> Option<(u64, String, HashMap<String, String>)> {
    let trimmed = input.trim();
    if !trimmed.starts_with('{') || !trimmed.ends_with('}') {
        return None;
    }

    // Extract id: "id": 123 or "id":"123"
    let id = if let Some(pos) = trimmed.find("\"id\"") {
        let after = &trimmed[pos + 4..];
        if let Some(colon) = after.find(':') {
            let val_str = after[colon + 1..].trim();
            val_str
                .chars()
                .take_while(|c| c.is_ascii_digit())
                .collect::<String>()
                .parse::<u64>()
                .unwrap_or(0)
        } else {
            0
        }
    } else {
        0
    };

    // Extract method: "method": "DOM.getDocument"
    let pos = trimmed.find("\"method\"")?;
    let after = &trimmed[pos + 8..];
    let colon = after.find(':')?;
    let val_str = after[colon + 1..].trim();
    let first_quote = val_str.find('"')?;
    let rest = &val_str[first_quote + 1..];
    let second_quote = rest.find('"')?;
    let method = rest[..second_quote].to_string();

    let mut params = HashMap::new();
    if let Some(pos) = trimmed.find("\"params\"") {
        let after = &trimmed[pos + 8..];
        if let Some(start_brace) = after.find('{') {
            let inner = &after[start_brace + 1..];
            if let Some(end_brace) = inner.rfind('}') {
                let params_body = &inner[..end_brace];
                for part in params_body.split(',') {
                    if let Some((k, v)) = part.split_once(':') {
                        let key = k.trim().trim_matches('"');
                        let val = v.trim().trim_matches('"');
                        if !key.is_empty() {
                            params.insert(key.to_string(), val.to_string());
                        }
                    }
                }
            }
        }
    }

    Some((id, method, params))
}

/// Chrome DevTools Protocol (CDP) session dispatcher.
#[derive(Debug, Clone, Default)]
pub struct CdpSession {
    pub session_id: String,
    pub enabled_domains: Vec<String>,
}

impl CdpSession {
    pub fn new() -> Self {
        Self {
            session_id: "mango-cdp-session-1".to_string(),
            enabled_domains: Vec::new(),
        }
    }

    /// Dispatches an incoming JSON-RPC 2.0 message and returns the response string.
    pub fn handle_message(
        &mut self,
        json_str: &str,
        doc: Option<&Document>,
        root_box: Option<&LayoutBox>,
    ) -> String {
        let (id, method, params) = match parse_json_rpc(json_str) {
            Some(parsed) => parsed,
            None => {
                return r#"{"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"Parse error"}}"#.to_string();
            }
        };

        match method.as_str() {
            // Page Domain
            "Page.enable" => {
                self.enable_domain("Page");
                format!(r#"{{"jsonrpc":"2.0","id":{id},"result":{{}}}}"#)
            }
            "Page.navigate" => {
                format!(
                    r#"{{"jsonrpc":"2.0","id":{id},"result":{{"frameId":"main-frame","loaderId":"mango-loader-1"}}}}"#
                )
            }
            "Page.reload" => {
                format!(r#"{{"jsonrpc":"2.0","id":{id},"result":{{}}}}"#)
            }
            "Page.captureScreenshot" => {
                format!(r#"{{"jsonrpc":"2.0","id":{id},"result":{{"data":""}}}}"#)
            }

            // DOM Domain
            "DOM.enable" => {
                self.enable_domain("DOM");
                format!(r#"{{"jsonrpc":"2.0","id":{id},"result":{{}}}}"#)
            }
            "DOM.getDocument" => {
                if let Some(d) = doc {
                    let root_id = d.root();
                    let child_count = d.children(root_id).count();
                    format!(
                        r##"{{"jsonrpc":"2.0","id":{id},"result":{{"root":{{"nodeId":{},"backendNodeId":{},"nodeType":9,"nodeName":"#document","childNodeCount":{}}}}}}}"##,
                        root_id.index(),
                        root_id.index(),
                        child_count
                    )
                } else {
                    format!(
                        r##"{{"jsonrpc":"2.0","id":{id},"result":{{"root":{{"nodeId":1,"nodeType":9,"nodeName":"#document"}}}}}}"##
                    )
                }
            }
            "DOM.querySelector" => {
                let target_node_id = params
                    .get("nodeId")
                    .and_then(|s| s.parse::<usize>().ok())
                    .unwrap_or(0);
                format!(r#"{{"jsonrpc":"2.0","id":{id},"result":{{"nodeId":{target_node_id}}}}}"#)
            }
            "DOM.querySelectorAll" => {
                format!(r#"{{"jsonrpc":"2.0","id":{id},"result":{{"nodeIds":[]}}}}"#)
            }
            "DOM.getBoxModel" => {
                if let Some(target_id_str) = params.get("nodeId")
                    && let Ok(nid_idx) = target_id_str.parse::<usize>()
                    && let Some(root) = root_box
                    && let Some(target_box) =
                        root.find_box_for_node(NodeId::from_raw(nid_idx as u32))
                {
                    let bb = target_box.dimensions.border_box();
                    let x = bb.x();
                    let y = bb.y();
                    let w = bb.width();
                    let h = bb.height();
                    format!(
                        r#"{{"jsonrpc":"2.0","id":{id},"result":{{"model":{{"content":[{x},{y},{x_plus_w},{y},{x_plus_w},{y_plus_h},{x},{y_plus_h}],"width":{w},"height":{h}}}}}}}"#,
                        x_plus_w = x + w,
                        y_plus_h = y + h,
                    )
                } else {
                    format!(
                        r#"{{"jsonrpc":"2.0","id":{id},"result":{{"model":{{"content":[0,0,100,0,100,100,0,100],"width":100,"height":100}}}}}}"#
                    )
                }
            }

            // CSS Domain
            "CSS.enable" => {
                self.enable_domain("CSS");
                format!(r#"{{"jsonrpc":"2.0","id":{id},"result":{{}}}}"#)
            }
            "CSS.getComputedStyleForNode" => {
                format!(
                    r#"{{"jsonrpc":"2.0","id":{id},"result":{{"computedStyle":[{{"name":"display","value":"block"}},{{"name":"box-sizing","value":"content-box"}},{{"name":"position","value":"static"}}]}}}}"#
                )
            }
            "CSS.getMatchedStylesForNode" => {
                format!(
                    r#"{{"jsonrpc":"2.0","id":{id},"result":{{"matchedCSSRules":[],"inlineStyle":null}}}}"#
                )
            }

            // Runtime Domain
            "Runtime.enable" => {
                self.enable_domain("Runtime");
                format!(r#"{{"jsonrpc":"2.0","id":{id},"result":{{}}}}"#)
            }
            "Runtime.evaluate" => {
                let expr = params.get("expression").cloned().unwrap_or_default();
                format!(
                    r#"{{"jsonrpc":"2.0","id":{id},"result":{{"result":{{"type":"string","value":{:?}}}}}}}"#,
                    expr
                )
            }
            "Runtime.getProperties" => {
                format!(r#"{{"jsonrpc":"2.0","id":{id},"result":{{"result":[]}}}}"#)
            }

            // Network Domain
            "Network.enable" => {
                self.enable_domain("Network");
                format!(r#"{{"jsonrpc":"2.0","id":{id},"result":{{}}}}"#)
            }
            "Network.getResponseBody" => {
                format!(
                    r#"{{"jsonrpc":"2.0","id":{id},"result":{{"body":"","base64Encoded":false}}}}"#
                )
            }

            other => {
                format!(
                    r#"{{"jsonrpc":"2.0","id":{id},"error":{{"code":-32601,"message":"Method not found: {other}"}}}}"#
                )
            }
        }
    }

    fn enable_domain(&mut self, domain: &str) {
        if !self.enabled_domains.iter().any(|d| d == domain) {
            self.enabled_domains.push(domain.to_string());
        }
    }
}
