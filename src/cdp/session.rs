//! Chrome DevTools Protocol Session & Domain Dispatcher.
//!
//! Dispatches JSON-RPC 2.0 messages across Target, Page, DOM, CSS,
//! Runtime, Network, Input, and Browser domains.

use std::collections::{HashMap, HashSet};

use mango_html::dom::{Document, NodeData, NodeId};
use serde_json::{Value, json};

use super::target::CdpTarget;
use super::types::{CdpCookie, JsonRpcRequest, JsonRpcResponse, TargetInfo};

/// Active DevTools protocol session bound to a page target.
#[derive(Debug, Clone)]
pub struct CdpSession {
    pub session_id: String,
    pub target_id: String,
    pub enabled_domains: HashSet<String>,
}

impl CdpSession {
    pub fn new(session_id: String, target_id: String) -> Self {
        Self {
            session_id,
            target_id,
            enabled_domains: HashSet::new(),
        }
    }

    /// Dispatches an incoming JSON-RPC command against the target collection.
    pub fn handle_request(
        &mut self,
        req: &JsonRpcRequest,
        all_targets: &mut HashMap<String, CdpTarget>,
    ) -> JsonRpcResponse {
        let id = req.id.clone();
        let sid = self.session_id.clone();

        match req.method.as_str() {
            // ── Target Domain ────────────────────────────────────────────────
            "Target.getTargets" => {
                let mut target_infos = Vec::new();
                for (tid, t) in all_targets.iter() {
                    target_infos.push(TargetInfo {
                        target_id: tid.clone(),
                        target_type: "page".to_string(),
                        title: t.title.clone(),
                        url: t.url.clone(),
                        attached: tid == &self.target_id,
                        browser_context_id: None,
                    });
                }
                JsonRpcResponse::success(id, json!({ "targetInfos": target_infos }), Some(sid))
            }
            "Target.attachToTarget" => {
                let target_id = req
                    .params
                    .get("targetId")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&self.target_id);
                let session_id = format!("mango-session-{target_id}");
                JsonRpcResponse::success(id, json!({ "sessionId": session_id }), Some(sid))
            }
            "Target.detachFromTarget" => JsonRpcResponse::success(id, json!({}), Some(sid)),
            "Target.createTarget" => {
                let url = req
                    .params
                    .get("url")
                    .and_then(|v| v.as_str())
                    .unwrap_or("about:blank");
                let new_id = format!("page-{}", all_targets.len() + 1);
                let mut new_target = CdpTarget::new(new_id.clone(), url);
                new_target.load_url(url);
                all_targets.insert(new_id.clone(), new_target);
                JsonRpcResponse::success(id, json!({ "targetId": new_id }), Some(sid))
            }
            "Target.closeTarget" => {
                let target_id = req
                    .params
                    .get("targetId")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&self.target_id);
                let success = all_targets.remove(target_id).is_some();
                JsonRpcResponse::success(id, json!({ "success": success }), Some(sid))
            }
            "Target.setDiscoverTargets" | "Target.setAutoAttach" => {
                JsonRpcResponse::success(id, json!({}), Some(sid))
            }
            "Browser.getVersion" => {
                let ua = all_targets
                    .get(&self.target_id)
                    .map(|t| t.user_agent.clone())
                    .unwrap_or_else(|| "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/133.0.0.0 Safari/537.36 Mango/0.2.4".to_string());
                JsonRpcResponse::success(
                    id,
                    json!({
                        "protocolVersion": "1.3",
                        "product": "Mango/0.2.4 Chrome/133.0.0.0",
                        "revision": "mango-core-0.2.4",
                        "userAgent": ua,
                        "jsVersion": "deno_core"
                    }),
                    Some(sid),
                )
            }
            "Browser.close" => JsonRpcResponse::success(id, json!({}), Some(sid)),
            _ => {
                let target = match all_targets.get_mut(&self.target_id) {
                    Some(t) => t,
                    None => {
                        return JsonRpcResponse::error(id, -32000, "Target not found", Some(sid));
                    }
                };
                self.handle_target_request(req, target, id, sid)
            }
        }
    }

    fn handle_target_request(
        &mut self,
        req: &JsonRpcRequest,
        target: &mut CdpTarget,
        id: Option<Value>,
        sid: String,
    ) -> JsonRpcResponse {
        match req.method.as_str() {
            // ── Page Domain ──────────────────────────────────────────────────
            "Page.enable" => {
                self.enabled_domains.insert("Page".to_string());
                JsonRpcResponse::success(id, json!({}), Some(sid))
            }
            "Page.disable" => {
                self.enabled_domains.remove("Page");
                JsonRpcResponse::success(id, json!({}), Some(sid))
            }
            "Page.navigate" => {
                let url = match req.params.get("url").and_then(|v| v.as_str()) {
                    Some(u) => u,
                    None => {
                        return JsonRpcResponse::invalid_params(
                            id,
                            "Missing 'url' parameter",
                            Some(sid),
                        );
                    }
                };
                target.load_url(url);
                JsonRpcResponse::success(
                    id,
                    json!({
                        "frameId": "main-frame",
                        "loaderId": format!("mango-loader-{}", target.id),
                    }),
                    Some(sid),
                )
            }
            "Page.reload" => {
                let current_url = target.url.clone();
                target.load_url(&current_url);
                JsonRpcResponse::success(id, json!({}), Some(sid))
            }
            "Page.captureScreenshot" => match target.capture_screenshot_png() {
                Ok(png_bytes) => {
                    let base64_data = mango_net::websocket::base64_encode(&png_bytes);
                    JsonRpcResponse::success(id, json!({ "data": base64_data }), Some(sid))
                }
                Err(e) => JsonRpcResponse::error(id, -32000, e, Some(sid)),
            },
            "Page.printToPDF" => {
                // Generate a PDF representation using display list commands
                let dl = if let Some(ref root) = target.root_box {
                    mango_layout::display_list::build_display_list(root)
                } else {
                    mango_render::DisplayList::new()
                };
                let temp_path = std::env::temp_dir().join(format!("mango_print_{}.pdf", target.id));
                let opts = mango_render::PdfOptions::default();
                match mango_render::print_to_pdf(&dl, &temp_path, &opts) {
                    Ok(_) => {
                        let bytes = std::fs::read(&temp_path).unwrap_or_default();
                        let _ = std::fs::remove_file(&temp_path);
                        let b64 = mango_net::websocket::base64_encode(&bytes);
                        JsonRpcResponse::success(id, json!({ "data": b64 }), Some(sid))
                    }
                    Err(e) => JsonRpcResponse::error(
                        id,
                        -32000,
                        format!("PDF export error: {e}"),
                        Some(sid),
                    ),
                }
            }
            "Page.getNavigationHistory" => JsonRpcResponse::success(
                id,
                json!({
                    "currentIndex": 0,
                    "entries": [{
                        "id": 1,
                        "url": target.url,
                        "userTypedURL": target.url,
                        "title": target.title,
                        "transitionType": "typed"
                    }]
                }),
                Some(sid),
            ),
            "Page.getFrameTree" => JsonRpcResponse::success(
                id,
                json!({
                    "frameTree": {
                        "frame": {
                            "id": "main-frame",
                            "loaderId": format!("mango-loader-{}", target.id),
                            "url": target.url,
                            "mimeType": "text/html",
                            "securityOrigin": target.url
                        },
                        "childFrames": []
                    }
                }),
                Some(sid),
            ),

            // ── DOM Domain ───────────────────────────────────────────────────
            "DOM.enable" => {
                self.enabled_domains.insert("DOM".to_string());
                JsonRpcResponse::success(id, json!({}), Some(sid))
            }
            "DOM.disable" => {
                self.enabled_domains.remove("DOM");
                JsonRpcResponse::success(id, json!({}), Some(sid))
            }
            "DOM.getDocument" => {
                let depth = req
                    .params
                    .get("depth")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(-1);
                let root_json = serialize_dom_node(&target.doc, target.doc.root(), depth, 0);
                JsonRpcResponse::success(id, json!({ "root": root_json }), Some(sid))
            }
            "DOM.querySelector" => {
                let node_id = req
                    .params
                    .get("nodeId")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as usize)
                    .unwrap_or(0);
                let selector = match req.params.get("selector").and_then(|v| v.as_str()) {
                    Some(s) => s,
                    None => {
                        return JsonRpcResponse::invalid_params(
                            id,
                            "Missing 'selector' parameter",
                            Some(sid),
                        );
                    }
                };
                let result_id = target.query_selector(node_id, selector).unwrap_or(0);
                JsonRpcResponse::success(id, json!({ "nodeId": result_id }), Some(sid))
            }
            "DOM.querySelectorAll" => {
                let node_id = req
                    .params
                    .get("nodeId")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as usize)
                    .unwrap_or(0);
                let selector = match req.params.get("selector").and_then(|v| v.as_str()) {
                    Some(s) => s,
                    None => {
                        return JsonRpcResponse::invalid_params(
                            id,
                            "Missing 'selector' parameter",
                            Some(sid),
                        );
                    }
                };
                let result_ids = target.query_selector_all(node_id, selector);
                JsonRpcResponse::success(id, json!({ "nodeIds": result_ids }), Some(sid))
            }
            "DOM.getOuterHTML" => {
                let node_id = req
                    .params
                    .get("nodeId")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as usize)
                    .unwrap_or_else(|| target.doc.root().raw() as usize);
                let html = target.get_outer_html(node_id).unwrap_or_default();
                JsonRpcResponse::success(id, json!({ "outerHTML": html }), Some(sid))
            }
            "DOM.setOuterHTML" => {
                let node_id = req
                    .params
                    .get("nodeId")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as usize)
                    .unwrap_or(0);
                let outer_html = req
                    .params
                    .get("outerHTML")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let nid = NodeId::from_raw(node_id as u32);
                if let Some(node) = target.doc.get_mut(nid) {
                    if let NodeData::Element(ref mut el) = node.data {
                        // Replace tag or inner attributes
                        let fragment = mango_html::parse_html(outer_html);
                        for child in fragment.children(fragment.root()) {
                            if let NodeData::Element(ref f_el) = child.data {
                                el.tag_name = f_el.tag_name.clone();
                                el.attributes = f_el.attributes.clone();
                                break;
                            }
                        }
                    }
                }
                target.relayout();
                JsonRpcResponse::success(id, json!({}), Some(sid))
            }
            "DOM.getBoxModel" => {
                let node_id = req
                    .params
                    .get("nodeId")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as usize)
                    .or_else(|| {
                        req.params
                            .get("objectId")
                            .and_then(|v| v.as_str())
                            .and_then(|s| {
                                s.strip_prefix("node:")
                                    .or_else(|| s.strip_prefix("nodeId:"))
                                    .unwrap_or(s)
                                    .parse::<usize>()
                                    .ok()
                            })
                    });

                let nid = match node_id {
                    Some(nid) => nid,
                    None => {
                        return JsonRpcResponse::invalid_params(
                            id,
                            "Missing 'nodeId' or 'objectId' parameter",
                            Some(sid),
                        );
                    }
                };

                match target.get_box_model(nid) {
                    Some(model) => {
                        JsonRpcResponse::success(id, json!({ "model": model }), Some(sid))
                    }
                    None => {
                        JsonRpcResponse::error(id, -32000, "Could not compute box model", Some(sid))
                    }
                }
            }
            "DOM.requestChildNodes" => JsonRpcResponse::success(id, json!({}), Some(sid)),
            "DOM.setAttributeValue" => {
                let node_id = req
                    .params
                    .get("nodeId")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as usize)
                    .unwrap_or(0);
                let name = req
                    .params
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let value = req
                    .params
                    .get("value")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let nid = NodeId::from_raw(node_id as u32);
                if let Some(node) = target.doc.get_mut(nid) {
                    if let NodeData::Element(ref mut el) = node.data {
                        el.set_attribute(name, value);
                    }
                }
                target.relayout();
                JsonRpcResponse::success(id, json!({}), Some(sid))
            }
            "DOM.removeAttribute" => {
                let node_id = req
                    .params
                    .get("nodeId")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as usize)
                    .unwrap_or(0);
                let name = req
                    .params
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let nid = NodeId::from_raw(node_id as u32);
                if let Some(node) = target.doc.get_mut(nid) {
                    if let NodeData::Element(ref mut el) = node.data {
                        el.remove_attribute(name);
                    }
                }
                target.relayout();
                JsonRpcResponse::success(id, json!({}), Some(sid))
            }
            "DOM.removeNode" => {
                let node_id = req
                    .params
                    .get("nodeId")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as usize)
                    .unwrap_or(0);
                let nid = NodeId::from_raw(node_id as u32);
                target.doc.remove_child_and_recycle(nid);
                target.relayout();
                JsonRpcResponse::success(id, json!({}), Some(sid))
            }
            "DOM.focus" => JsonRpcResponse::success(id, json!({}), Some(sid)),

            // ── CSS Domain ───────────────────────────────────────────────────
            "CSS.enable" => {
                self.enabled_domains.insert("CSS".to_string());
                JsonRpcResponse::success(id, json!({}), Some(sid))
            }
            "CSS.disable" => {
                self.enabled_domains.remove("CSS");
                JsonRpcResponse::success(id, json!({}), Some(sid))
            }
            "CSS.getComputedStyleForNode" => {
                let node_id = req
                    .params
                    .get("nodeId")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as usize)
                    .unwrap_or(0);
                let styles = target.get_computed_styles(node_id);
                JsonRpcResponse::success(id, json!({ "computedStyle": styles }), Some(sid))
            }
            "CSS.getMatchedStylesForNode" => JsonRpcResponse::success(
                id,
                json!({
                    "matchedCSSRules": [],
                    "pseudoElements": [],
                    "inherited": [],
                    "inlineStyle": null
                }),
                Some(sid),
            ),
            "CSS.setStyleTexts" => JsonRpcResponse::success(id, json!({ "styles": [] }), Some(sid)),

            // ── Runtime Domain ───────────────────────────────────────────────
            "Runtime.enable" => {
                self.enabled_domains.insert("Runtime".to_string());
                JsonRpcResponse::success(id, json!({}), Some(sid))
            }
            "Runtime.disable" => {
                self.enabled_domains.remove("Runtime");
                JsonRpcResponse::success(id, json!({}), Some(sid))
            }
            "Runtime.evaluate" => {
                let expression = match req.params.get("expression").and_then(|v| v.as_str()) {
                    Some(expr) => expr,
                    None => {
                        return JsonRpcResponse::invalid_params(
                            id,
                            "Missing 'expression' parameter",
                            Some(sid),
                        );
                    }
                };
                let remote_obj = target.evaluate_js(expression);
                JsonRpcResponse::success(id, json!({ "result": remote_obj }), Some(sid))
            }
            "Runtime.callFunctionOn" => {
                let func_declaration = req
                    .params
                    .get("functionDeclaration")
                    .and_then(|v| v.as_str())
                    .unwrap_or("() => {}");

                let object_id = req.params.get("objectId").and_then(|v| v.as_str());

                let args_json: Vec<String> = req
                    .params
                    .get("arguments")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .map(|arg| {
                                if let Some(val) = arg.get("value") {
                                    serde_json::to_string(val)
                                        .unwrap_or_else(|_| "undefined".to_string())
                                } else if let Some(obj_id) =
                                    arg.get("objectId").and_then(|v| v.as_str())
                                {
                                    if let Some(nid) = obj_id
                                        .strip_prefix("node:")
                                        .or_else(|| obj_id.strip_prefix("nodeId:"))
                                    {
                                        format!("_wrapElement({nid})")
                                    } else if let Ok(nid) = obj_id.parse::<u32>() {
                                        format!("_wrapElement({nid})")
                                    } else {
                                        "null".to_string()
                                    }
                                } else {
                                    "undefined".to_string()
                                }
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                let this_expr = if let Some(oid) = object_id {
                    if let Some(nid) = oid
                        .strip_prefix("node:")
                        .or_else(|| oid.strip_prefix("nodeId:"))
                    {
                        format!("_wrapElement({nid})")
                    } else if let Ok(nid) = oid.parse::<u32>() {
                        format!("_wrapElement({nid})")
                    } else {
                        "globalThis".to_string()
                    }
                } else {
                    "globalThis".to_string()
                };

                let joined_args = args_json.join(", ");
                let await_promise = req
                    .params
                    .get("awaitPromise")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                let expr = if await_promise {
                    format!(
                        "(async () => await ({func_declaration}).call({this_expr}, {joined_args}))()"
                    )
                } else {
                    format!("(({func_declaration}).call({this_expr}, {joined_args}))")
                };

                let remote_obj = target.evaluate_js(&expr);
                JsonRpcResponse::success(id, json!({ "result": remote_obj }), Some(sid))
            }
            "Runtime.getProperties" => {
                JsonRpcResponse::success(id, json!({ "result": [] }), Some(sid))
            }

            // ── Network Domain ───────────────────────────────────────────────
            "Network.enable" => {
                self.enabled_domains.insert("Network".to_string());
                JsonRpcResponse::success(id, json!({}), Some(sid))
            }
            "Network.disable" => {
                self.enabled_domains.remove("Network");
                JsonRpcResponse::success(id, json!({}), Some(sid))
            }
            "Network.getCookies" => {
                let mut cookies = Vec::new();
                if let Ok(jar) = target.cookie_jar.lock() {
                    for c in jar.all() {
                        cookies.push(CdpCookie {
                            name: c.name.clone(),
                            value: c.value.clone(),
                            domain: c.domain.clone(),
                            path: c.path.clone(),
                            expires: c.expires_at.unwrap_or(0) as f64,
                            size: c.name.len() + c.value.len(),
                            http_only: c.http_only,
                            secure: c.secure,
                            session: c.expires_at.is_none(),
                            same_site: match c.same_site {
                                mango_net::SameSite::Strict => Some("Strict".to_string()),
                                mango_net::SameSite::Lax => Some("Lax".to_string()),
                                mango_net::SameSite::None => Some("None".to_string()),
                                mango_net::SameSite::Unspecified => None,
                            },
                        });
                    }
                }
                JsonRpcResponse::success(id, json!({ "cookies": cookies }), Some(sid))
            }
            "Network.setCookie" => {
                let name = req
                    .params
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let value = req
                    .params
                    .get("value")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let domain = req
                    .params
                    .get("domain")
                    .and_then(|v| v.as_str())
                    .unwrap_or("localhost");
                let path = req
                    .params
                    .get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or("/");

                if let Ok(mut jar) = target.cookie_jar.lock() {
                    let cookie = mango_net::Cookie::new(name, value, domain, path);
                    jar.store_cookie(cookie);
                }
                JsonRpcResponse::success(id, json!({ "success": true }), Some(sid))
            }
            "Network.clearBrowserCookies" => {
                if let Ok(mut jar) = target.cookie_jar.lock() {
                    jar.clear();
                }
                JsonRpcResponse::success(id, json!({}), Some(sid))
            }
            "Network.getResponseBody" => {
                let html = target
                    .get_outer_html(target.doc.root().raw() as usize)
                    .unwrap_or_default();
                JsonRpcResponse::success(
                    id,
                    json!({
                        "body": html,
                        "base64Encoded": false,
                    }),
                    Some(sid),
                )
            }
            "Network.setUserAgentOverride" => {
                if let Some(ua) = req.params.get("userAgent").and_then(|v| v.as_str()) {
                    target.user_agent = ua.to_string();
                }
                JsonRpcResponse::success(id, json!({}), Some(sid))
            }

            // ── Input Domain ─────────────────────────────────────────────────
            "Input.dispatchMouseEvent" => JsonRpcResponse::success(id, json!({}), Some(sid)),
            "Input.dispatchKeyEvent" => JsonRpcResponse::success(id, json!({}), Some(sid)),

            other => JsonRpcResponse::method_not_found(id, other, Some(sid)),
        }
    }
}

/// Recursively serializes a DOM node hierarchy into CDP `Node` objects.
fn serialize_dom_node(doc: &Document, nid: NodeId, max_depth: i64, current_depth: i64) -> Value {
    let node = match doc.get(nid) {
        Some(n) => n,
        None => return Value::Null,
    };

    let idx = nid.raw() as usize;
    match &node.data {
        NodeData::Document => {
            let mut children_json = Vec::new();
            if max_depth < 0 || current_depth < max_depth {
                for child in doc.children(nid) {
                    children_json.push(serialize_dom_node(
                        doc,
                        child.id,
                        max_depth,
                        current_depth + 1,
                    ));
                }
            }
            json!({
                "nodeId": idx,
                "backendNodeId": idx,
                "nodeType": 9,
                "nodeName": "#document",
                "localName": "",
                "nodeValue": "",
                "childNodeCount": doc.children(nid).count(),
                "children": children_json,
                "documentURL": "about:blank",
                "baseURL": "about:blank"
            })
        }
        NodeData::Element(elem) => {
            let mut attrs = Vec::new();
            for (k, v) in &elem.attributes {
                attrs.push(k.clone());
                attrs.push(v.clone());
            }

            let mut children_json = Vec::new();
            let child_count = doc.children(nid).count();
            if max_depth < 0 || current_depth < max_depth {
                for child in doc.children(nid) {
                    children_json.push(serialize_dom_node(
                        doc,
                        child.id,
                        max_depth,
                        current_depth + 1,
                    ));
                }
            }

            json!({
                "nodeId": idx,
                "backendNodeId": idx,
                "nodeType": 1,
                "nodeName": elem.tag_name.to_ascii_uppercase(),
                "localName": elem.tag_name.to_ascii_lowercase(),
                "nodeValue": "",
                "attributes": attrs,
                "childNodeCount": child_count,
                "children": children_json
            })
        }
        NodeData::Text(txt) => {
            json!({
                "nodeId": idx,
                "backendNodeId": idx,
                "nodeType": 3,
                "nodeName": "#text",
                "localName": "",
                "nodeValue": txt
            })
        }
        NodeData::Comment(cmt) => {
            json!({
                "nodeId": idx,
                "backendNodeId": idx,
                "nodeType": 8,
                "nodeName": "#comment",
                "localName": "",
                "nodeValue": cmt
            })
        }
        _ => json!({
            "nodeId": idx,
            "backendNodeId": idx,
            "nodeType": 1,
            "nodeName": "UNKNOWN",
            "localName": ""
        }),
    }
}
