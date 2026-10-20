//! Chrome DevTools Protocol Target Implementation.
//!
//! Owns an independent page target: DOM tree, layout box tree,
//! active JavaScript runtime, and rendering canvas.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use mango_core::{Color, Rect, Size};
use mango_css::Stylesheet;
use mango_html::dom::{Document, NodeData, NodeId};
use mango_layout::box_tree::LayoutBox;
use mango_net::CookieJar;

use super::types::{BoxModel, ComputedStyleProperty, RemoteObject};

/// Managed target instance representing an inspectable page.
pub struct CdpTarget {
    pub id: String,
    pub url: String,
    pub title: String,
    pub doc: Document,
    pub root_box: Option<LayoutBox>,
    pub js_runtime: Option<mango_js::JsRuntime>,
    pub cookie_jar: Arc<Mutex<CookieJar>>,
    pub stylesheets: Vec<Stylesheet>,
    pub viewport_width: u32,
    pub viewport_height: u32,
    pub user_agent: String,
}

impl CdpTarget {
    pub fn new(id: String, initial_url: &str) -> Self {
        let cookie_jar = Arc::new(Mutex::new(CookieJar::new()));
        let mut target = Self {
            id,
            url: initial_url.to_string(),
            title: "About Blank".to_string(),
            doc: mango_html::parse_html("<!DOCTYPE html><html><head></head><body></body></html>"),
            root_box: None,
            js_runtime: None,
            cookie_jar,
            stylesheets: Vec::new(),
            viewport_width: 1280,
            viewport_height: 800,
            user_agent: "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/133.0.0.0 Safari/537.36 Mango/0.2.4".to_string(),
        };
        target.relayout();
        target
    }

    /// Loads HTML markup directly into this target, refreshing DOM, styles, layout, and JS runtime.
    pub fn load_html(&mut self, html: &str, url: &str) {
        self.url = url.to_string();
        self.doc = mango_html::parse_html(html);

        // Extract title
        self.title = self.extract_title().unwrap_or_else(|| url.to_string());

        // Extract stylesheets
        let style_elements = mango_layout::extract_style_elements(&self.doc);
        self.stylesheets = style_elements;

        // Initialize JavaScript runtime
        let w = self.viewport_width as f32;
        let h = self.viewport_height as f32;
        let local_storage = Arc::new(Mutex::new(HashMap::new()));
        let session_storage = Arc::new(Mutex::new(HashMap::new()));

        let mut rt = mango_js::JsRuntime::new_with_url_storage_session_and_cookies(
            self.doc.clone(),
            w,
            h,
            url,
            local_storage,
            session_storage,
            self.cookie_jar.clone(),
        );

        // Execute inline script tags
        let mut scripts = Vec::new();
        extract_scripts(&self.doc, self.doc.root(), &mut scripts);
        for script_code in &scripts {
            let _ = rt.execute_script(script_code);
        }
        let _ = rt.dispatch_dom_content_loaded();
        let _ = rt.dispatch_load();

        // Sync back mutated document
        self.doc = rt.document_ref().clone();
        self.js_runtime = Some(rt);

        self.relayout();
    }

    /// Loads content from a URL (supports data:, file:, and http/https).
    pub fn load_url(&mut self, url: &str) {
        if url.starts_with("data:") {
            let html = if url.contains(";base64,") {
                if let Some((_, data)) = url.split_once(',') {
                    if let Some(bytes) = mango_render::image_decode::base64_decode(data.trim()) {
                        String::from_utf8(bytes).unwrap_or_else(|_| data.to_string())
                    } else {
                        data.to_string()
                    }
                } else {
                    url.to_string()
                }
            } else if let Some((_, data)) = url.split_once(',') {
                data.to_string()
            } else {
                url.to_string()
            };
            self.load_html(&html, url);
        } else if let Some(path) = url.strip_prefix("file://") {
            if let Ok(content) = std::fs::read_to_string(path) {
                self.load_html(&content, url);
            } else {
                self.load_html(
                    &format!("<html><body><h1>File Not Found</h1><p>{url}</p></body></html>"),
                    url,
                );
            }
        } else if (url.starts_with("http://") || url.starts_with("https://"))
            && let Ok(url_obj) = mango_net::Url::parse(url)
        {
            let client = mango_net::HttpClient::with_cookie_jar(self.cookie_jar.clone());
            match client.get_single(&url_obj) {
                Ok(resp) => {
                    let html = resp.body_as_string();
                    self.load_html(&html, url);
                }
                Err(e) => {
                    self.load_html(
                        &format!("<html><body><h1>Network Error</h1><p>{e}</p></body></html>"),
                        url,
                    );
                }
            }
        } else {
            self.load_html(
                &format!("<html><body><h1>Unable to resolve URL</h1><p>{url}</p></body></html>"),
                url,
            );
        }
    }

    /// Recomputes style tree and builds layout box tree for the current viewport.
    pub fn relayout(&mut self) {
        let size = Size::new(self.viewport_width as f32, self.viewport_height as f32);
        let sheet_refs: Vec<&Stylesheet> = self.stylesheets.iter().collect();
        let (root_box, _) = mango_layout::layout_document(&self.doc, &sheet_refs, size);
        self.root_box = Some(root_box);
    }

    /// Evaluates a JavaScript expression in the target page runtime.
    pub fn evaluate_js(&mut self, expr: &str) -> RemoteObject {
        if let Some(ref mut rt) = self.js_runtime {
            match rt.eval(expr) {
                Ok(val_str) => {
                    if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(&val_str) {
                        match json_val {
                            serde_json::Value::Null => RemoteObject::null(),
                            serde_json::Value::Bool(b) => RemoteObject::boolean(b),
                            serde_json::Value::Number(n) => {
                                if let Some(f) = n.as_f64() {
                                    RemoteObject::number(f)
                                } else {
                                    RemoteObject::string(n.to_string())
                                }
                            }
                            serde_json::Value::String(s) => RemoteObject::string(s),
                            other => RemoteObject {
                                object_type: "object".to_string(),
                                subtype: None,
                                class_name: Some("Object".to_string()),
                                description: Some(val_str),
                                value: Some(other),
                                object_id: None,
                            },
                        }
                    } else if val_str == "undefined" {
                        RemoteObject::undefined()
                    } else {
                        RemoteObject::string(val_str)
                    }
                }
                Err(err) => RemoteObject {
                    object_type: "object".to_string(),
                    subtype: Some("error".to_string()),
                    class_name: Some("Error".to_string()),
                    description: Some(err),
                    value: None,
                    object_id: None,
                },
            }
        } else {
            RemoteObject::undefined()
        }
    }

    /// Rasterizes the target viewport to a PNG byte vector.
    pub fn capture_screenshot_png(&mut self) -> Result<Vec<u8>, String> {
        let w = self.viewport_width;
        let h = self.viewport_height;
        if w == 0 || h == 0 {
            return Err("Zero viewport dimensions".to_string());
        }

        self.relayout();

        let dl = if let Some(ref root) = self.root_box {
            mango_layout::display_list::build_display_list(root)
        } else {
            mango_render::DisplayList::new()
        };

        let mut buffer = vec![0u32; (w * h) as usize];
        buffer.fill(Color::rgb(255, 255, 255).to_argb_u32());
        mango_render::painter::paint(&dl, &mut buffer, w, h);

        let mut img = image::RgbImage::new(w, h);
        for y in 0..h {
            for x in 0..w {
                let px = buffer[(y * w + x) as usize];
                img.put_pixel(
                    x,
                    y,
                    image::Rgb([
                        ((px >> 16) & 0xFF) as u8,
                        ((px >> 8) & 0xFF) as u8,
                        (px & 0xFF) as u8,
                    ]),
                );
            }
        }

        let mut png_bytes = Vec::new();
        let encoder = image::codecs::png::PngEncoder::new(&mut png_bytes);
        image::ImageEncoder::write_image(
            encoder,
            img.as_raw(),
            w,
            h,
            image::ExtendedColorType::Rgb8,
        )
        .map_err(|e| format!("PNG encoding error: {e}"))?;

        Ok(png_bytes)
    }

    fn all_nodes(&self) -> Vec<NodeId> {
        let mut nodes = Vec::new();
        fn collect(doc: &Document, nid: NodeId, out: &mut Vec<NodeId>) {
            out.push(nid);
            for child in doc.children(nid) {
                collect(doc, child.id, out);
            }
        }
        collect(&self.doc, self.doc.root(), &mut nodes);
        nodes
    }

    /// Queries the first element matching a CSS selector starting from root or specified node.
    pub fn query_selector(&self, _root_id: usize, selector: &str) -> Option<usize> {
        let sel = selector.trim();
        for nid in self.all_nodes() {
            if let Some(node) = self.doc.get(nid) {
                if let NodeData::Element(el) = &node.data {
                    if sel.starts_with('#') && el.get_attribute("id") == Some(&sel[1..]) {
                        return Some(nid.raw() as usize);
                    }
                    if let Some(class_name) = sel.strip_prefix('.') {
                        if let Some(classes) = el.get_attribute("class") {
                            if classes.split_whitespace().any(|c| c == class_name) {
                                return Some(nid.raw() as usize);
                            }
                        }
                    }
                    if el.tag_name.eq_ignore_ascii_case(sel) {
                        return Some(nid.raw() as usize);
                    }
                }
            }
        }
        None
    }

    /// Queries all elements matching a CSS selector.
    pub fn query_selector_all(&self, _root_id: usize, selector: &str) -> Vec<usize> {
        let sel = selector.trim();
        let mut results = Vec::new();
        for nid in self.all_nodes() {
            if let Some(node) = self.doc.get(nid) {
                if let NodeData::Element(el) = &node.data {
                    let matched = if let Some(id_val) = sel.strip_prefix('#') {
                        el.get_attribute("id") == Some(id_val)
                    } else if let Some(class_name) = sel.strip_prefix('.') {
                        el.get_attribute("class")
                            .map(|c| c.split_whitespace().any(|part| part == class_name))
                            .unwrap_or(false)
                    } else {
                        el.tag_name.eq_ignore_ascii_case(sel)
                    };

                    if matched {
                        results.push(nid.raw() as usize);
                    }
                }
            }
        }
        results
    }

    /// Computes box model quad coordinates for `DOM.getBoxModel`.
    pub fn get_box_model(&self, node_id: usize) -> Option<BoxModel> {
        let nid = NodeId::from_raw(node_id as u32);
        let root = self.root_box.as_ref()?;
        let target_box = root.find_box_for_node(nid)?;

        let bb = target_box.dimensions.border_box();
        let cb = target_box.dimensions.content_box();
        let pb = target_box.dimensions.padding_box();
        let mb = target_box.dimensions.margin_box();

        fn to_quad(r: &Rect) -> [f32; 8] {
            let x = r.x();
            let y = r.y();
            let w = r.width();
            let h = r.height();
            [x, y, x + w, y, x + w, y + h, x, y + h]
        }

        Some(BoxModel {
            content: to_quad(&cb),
            padding: to_quad(&pb),
            border: to_quad(&bb),
            margin: to_quad(&mb),
            width: bb.width(),
            height: bb.height(),
        })
    }

    /// Retrieves computed style properties for a node.
    pub fn get_computed_styles(&self, node_id: usize) -> Vec<ComputedStyleProperty> {
        let nid = NodeId::from_raw(node_id as u32);
        let mut styles = Vec::new();

        if let Some(ref root) = self.root_box
            && let Some(target_box) = root.find_box_for_node(nid)
            && let Some(ref style) = target_box.style
        {
            styles.push(ComputedStyleProperty {
                name: "display".to_string(),
                value: format!("{:?}", style.display).to_ascii_lowercase(),
            });
            styles.push(ComputedStyleProperty {
                name: "position".to_string(),
                value: format!("{:?}", style.position).to_ascii_lowercase(),
            });
            styles.push(ComputedStyleProperty {
                name: "box-sizing".to_string(),
                value: format!("{:?}", style.box_sizing).to_ascii_lowercase(),
            });
            styles.push(ComputedStyleProperty {
                name: "width".to_string(),
                value: format!("{}px", target_box.dimensions.border_box().width()),
            });
            styles.push(ComputedStyleProperty {
                name: "height".to_string(),
                value: format!("{}px", target_box.dimensions.border_box().height()),
            });
            styles.push(ComputedStyleProperty {
                name: "color".to_string(),
                value: format!(
                    "rgb({}, {}, {})",
                    style.color.r, style.color.g, style.color.b
                ),
            });
            styles.push(ComputedStyleProperty {
                name: "background-color".to_string(),
                value: format!(
                    "rgba({}, {}, {}, {})",
                    style.background_color.r,
                    style.background_color.g,
                    style.background_color.b,
                    style.background_color.a as f32 / 255.0
                ),
            });
            styles.push(ComputedStyleProperty {
                name: "font-size".to_string(),
                value: format!("{}px", style.font_size),
            });
            styles.push(ComputedStyleProperty {
                name: "font-family".to_string(),
                value: style.font_family.clone(),
            });
        } else {
            styles.push(ComputedStyleProperty {
                name: "display".to_string(),
                value: "block".to_string(),
            });
            styles.push(ComputedStyleProperty {
                name: "box-sizing".to_string(),
                value: "content-box".to_string(),
            });
            styles.push(ComputedStyleProperty {
                name: "position".to_string(),
                value: "static".to_string(),
            });
        }

        styles
    }

    /// Serializes outer HTML of a node and its descendants.
    pub fn get_outer_html(&self, node_id: usize) -> Option<String> {
        let nid = NodeId::from_raw(node_id as u32);
        if self.doc.get(nid).is_some() {
            Some(self.doc.serialize_html(nid))
        } else {
            None
        }
    }

    fn extract_title(&self) -> Option<String> {
        for nid in self.all_nodes() {
            if let Some(node) = self.doc.get(nid) {
                if let NodeData::Element(el) = &node.data {
                    if el.tag_name.eq_ignore_ascii_case("title") {
                        let text = self.doc.text_content(nid);
                        return Some(text.trim().to_string());
                    }
                }
            }
        }
        None
    }
}

/// Recursively traverses the DOM and extracts contents of `<script>` elements.
fn extract_scripts(doc: &Document, node_id: NodeId, out: &mut Vec<String>) {
    if let Some(node) = doc.get(node_id) {
        if let NodeData::Element(ref elem) = node.data {
            if elem.tag_name.eq_ignore_ascii_case("script") {
                let code = doc.text_content(node_id);
                if !code.trim().is_empty() {
                    out.push(code);
                }
            }
        }
        for child in doc.children(node_id) {
            extract_scripts(doc, child.id, out);
        }
    }
}
