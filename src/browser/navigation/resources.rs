//! Web font loading, image resource harvesting, data URI decoding, and title extraction.

use mango_css::Stylesheet;
use mango_html::dom::{Document, NodeData, NodeId};
use mango_net::{ResourceLoader, Url};
use mango_render::{FontWeight, font_manager};

use super::super::types::{
    MAX_CONCURRENT_FETCHES, SUBRESOURCE_FETCH_TIMEOUT, assert_not_on_ui_thread, fetch_with_timeout,
};
use std::sync::OnceLock;

static FONT_LOAD_POOL: OnceLock<rayon::ThreadPool> = OnceLock::new();
static FONT_FETCH_POOL: OnceLock<rayon::ThreadPool> = OnceLock::new();

fn font_load_pool() -> &'static rayon::ThreadPool {
    FONT_LOAD_POOL.get_or_init(|| {
        rayon::ThreadPoolBuilder::new()
            .num_threads(2)
            .thread_name(|i| format!("mango-font-load-{i}"))
            .build()
            .expect("failed to initialize font load pool")
    })
}

fn font_fetch_pool() -> &'static rayon::ThreadPool {
    FONT_FETCH_POOL.get_or_init(|| {
        rayon::ThreadPoolBuilder::new()
            .num_threads(MAX_CONCURRENT_FETCHES)
            .thread_name(|i| format!("mango-font-fetch-{i}"))
            .build()
            .expect("failed to initialize font fetch pool")
    })
}
static PENDING_WEB_FONT_LOADS: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

/// Returns true if background web font fetches or decodes are in flight.
pub fn has_pending_font_fetches() -> bool {
    PENDING_WEB_FONT_LOADS.load(std::sync::atomic::Ordering::Relaxed) > 0
}

/// Returns the number of active background web font fetch tasks.
pub fn pending_font_fetch_count() -> usize {
    PENDING_WEB_FONT_LOADS.load(std::sync::atomic::Ordering::Relaxed)
}

/// Non-blocking font registration for the UI thread: returns immediately and
/// fetches + registers web fonts on a detached background thread.
///
/// The background pipeline calls the blocking [`load_web_fonts`] directly;
/// UI code paths must call this async variant so font downloads never freeze
/// the event loop.
pub(crate) fn load_web_fonts_async(
    loader: ResourceLoader,
    base_url: Option<Url>,
    stylesheets: Vec<Stylesheet>,
) {
    PENDING_WEB_FONT_LOADS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    font_load_pool().spawn(move || {
        struct FontLoadDropGuard;
        impl Drop for FontLoadDropGuard {
            fn drop(&mut self) {
                PENDING_WEB_FONT_LOADS.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
            }
        }
        let _guard = FontLoadDropGuard;
        load_web_fonts(&loader, base_url.as_ref(), &stylesheets);
    });
}

fn collect_font_faces(
    rules: &[mango_css::parser::Rule],
    out: &mut Vec<mango_css::parser::FontFaceRule>,
) {
    for rule in rules {
        match rule {
            mango_css::parser::Rule::FontFace(font_face) => {
                out.push(font_face.clone());
            }
            mango_css::parser::Rule::Layer(l) => {
                collect_font_faces(&l.rules, out);
            }
            _ => {}
        }
    }
}

pub fn load_web_fonts(loader: &ResourceLoader, base_url: Option<&Url>, stylesheets: &[Stylesheet]) {
    assert_not_on_ui_thread("load_web_fonts");
    let mut font_faces: Vec<mango_css::parser::FontFaceRule> = Vec::new();
    for sheet in stylesheets {
        collect_font_faces(&sheet.rules, &mut font_faces);
    }
    if font_faces.is_empty() {
        return;
    }

    // Phase 1: decide which (face, weight) pairs still need registration,
    // selecting at most one face per (family, weight) to avoid downloading
    // dozens of multi-slice subfonts (e.g. Cyrillic/Greek/Vietnamese in Google Fonts).
    struct FontJob {
        face_idx: usize,
        weight: FontWeight,
    }
    let mut jobs: Vec<FontJob> = Vec::new();
    let mut queued_family_weights: std::collections::HashSet<(String, FontWeight)> =
        std::collections::HashSet::new();

    let score_face = |face: &mango_css::parser::FontFaceRule| -> i32 {
        let mut score = 0;
        let url_lower = face.src_url.to_ascii_lowercase();
        if url_lower.contains("latin") {
            score += 10;
        }
        if url_lower.contains("regular") || url_lower.contains("bold") {
            score += 5;
        }
        score
    };

    let mut candidates: std::collections::HashMap<(String, FontWeight), Vec<usize>> =
        std::collections::HashMap::new();

    for (i, face) in font_faces.iter().enumerate() {
        if face.font_style != mango_css::values::FontStyle::Normal {
            continue;
        }
        let family_key = face
            .font_family
            .trim()
            .trim_matches('\'')
            .trim_matches('"')
            .to_ascii_lowercase();
        let is_range = face.font_weight_range.is_some();
        let (min_w, max_w) = face.font_weight_range.unwrap_or((400, 400));
        let is_true_bold = match face.font_weight {
            mango_css::values::FontWeight::Bold | mango_css::values::FontWeight::Bolder => true,
            mango_css::values::FontWeight::Numeric(w) => w >= 600,
            _ => is_range && max_w >= 600 && min_w >= 600,
        };
        let is_regular = match face.font_weight {
            mango_css::values::FontWeight::Normal => true,
            mango_css::values::FontWeight::Numeric(w) => w <= 500,
            _ => is_range && min_w <= 500,
        };
        if is_true_bold {
            candidates
                .entry((family_key.clone(), FontWeight::Bold))
                .or_default()
                .push(i);
        }
        if is_regular {
            candidates
                .entry((family_key, FontWeight::Regular))
                .or_default()
                .push(i);
        }
    }

    for ((family_key, weight), mut indices) in candidates {
        if font_manager().has_web_font_weight(&family_key, weight) {
            continue;
        }
        if queued_family_weights.contains(&(family_key.clone(), weight)) {
            continue;
        }
        indices.sort_by_key(|&idx| std::cmp::Reverse(score_face(&font_faces[idx])));
        if let Some(&best_idx) = indices.first() {
            jobs.push(FontJob {
                face_idx: best_idx,
                weight,
            });
            queued_family_weights.insert((family_key, weight));
        }
        if jobs.len() >= 8 {
            break;
        }
    }
    if jobs.is_empty() {
        return;
    }

    // Parallel byte fetch (bounded chunks + timeout), serial registration.
    let mut fetched: std::collections::HashMap<usize, Vec<u8>> = std::collections::HashMap::new();
    // Data: URIs decode inline without threads.
    let mut remote_jobs: Vec<(usize, FontJob)> = Vec::new();
    for (job_seq, job) in jobs.into_iter().enumerate() {
        let face = &font_faces[job.face_idx];
        let trimmed_url = face.src_url.trim();
        if trimmed_url.len() >= 5 && trimmed_url[..5].eq_ignore_ascii_case("data:") {
            match decode_data_uri_font(trimmed_url) {
                Ok(bytes) => {
                    fetched.insert(job_seq, bytes);
                }
                Err(e) => {
                    log::warn!("Failed to decode font data URI: {e}");
                }
            }
            // Keep job_seq stable by storing weight alongside below.
            remote_jobs.push((
                job_seq,
                FontJob {
                    face_idx: job.face_idx,
                    weight: job.weight,
                },
            ));
            // Mark data jobs as already-fetched so the parallel phase skips them.
            continue;
        } else {
            remote_jobs.push((job_seq, job));
        }
    }
    // Rebuild ordered job list: data URIs already in `fetched`, remotes need fetching.
    let mut ordered: Vec<(usize, usize, FontWeight)> = Vec::new();
    for (job_seq, job) in remote_jobs {
        if fetched.contains_key(&job_seq) {
            ordered.push((job_seq, job.face_idx, job.weight));
            continue;
        }
        ordered.push((job_seq, job.face_idx, job.weight));
    }
    // Fetch remote bytes in bounded parallel chunks.
    let remotes: Vec<(usize, usize, FontWeight, String)> = ordered
        .iter()
        .filter(|(seq, _, _)| !fetched.contains_key(seq))
        .map(|(seq, face_idx, weight)| {
            (
                *seq,
                *face_idx,
                *weight,
                font_faces[*face_idx].src_url.clone(),
            )
        })
        .collect();
    if let Some(base) = base_url {
        let mut unique_urls = Vec::new();
        for (_, _, _, url) in &remotes {
            if !unique_urls.contains(url) {
                unique_urls.push(url.clone());
            }
        }
        let mut url_bytes = std::collections::HashMap::new();
        for chunk in unique_urls.chunks(MAX_CONCURRENT_FETCHES) {
            let (tx, rx) = std::sync::mpsc::channel();
            let n = chunk.len();
            for url in chunk.iter() {
                let tx = tx.clone();
                let loader = loader.clone();
                let base = base.clone();
                let url = url.clone();
                font_fetch_pool().spawn(move || {
                    match fetch_with_timeout(
                        {
                            let loader = loader.clone();
                            let base = base.clone();
                            let url = url.clone();
                            move || loader.fetch_font_bytes(&base, &url)
                        },
                        SUBRESOURCE_FETCH_TIMEOUT,
                    ) {
                        Ok(Ok(bytes)) => {
                            let _ = tx.send((url, bytes));
                        }
                        Ok(Err(e)) => {
                            log::warn!("Font fetch failed for {url}: {e}");
                        }
                        Err(msg) => {
                            log::warn!("Font fetch timed out for {url}: {msg}");
                        }
                    }
                });
            }
            drop(tx);
            for _ in 0..n {
                match rx.recv_timeout(SUBRESOURCE_FETCH_TIMEOUT + std::time::Duration::from_secs(2))
                {
                    Ok((url, bytes)) => {
                        url_bytes.insert(url, bytes);
                    }
                    Err(_) => break,
                }
            }
        }
        for (seq, _, _, url) in &remotes {
            if let Some(bytes) = url_bytes.get(url) {
                fetched.insert(*seq, bytes.clone());
            }
        }
    }
    // Serial registration in job order.
    ordered.sort_by_key(|(seq, _, _)| *seq);
    for (seq, face_idx, weight) in ordered {
        let Some(bytes) = fetched.remove(&seq) else {
            continue;
        };
        let face = &font_faces[face_idx];
        match font_manager().register_web_font(&face.font_family, weight, &bytes) {
            Ok(id) => {
                log::info!(
                    "Loaded and registered web font '{}' (ID: {}, weight: {:?})",
                    face.font_family,
                    id,
                    weight
                );
            }
            Err(e) => {
                log::warn!(
                    "Failed to register web font '{}' from {}: {}",
                    face.font_family,
                    face.src_url,
                    e
                );
            }
        }
    }
}

/// Helper: finds the comma separating metadata and payload in a data: URI,
/// ignoring any commas inside quoted parameter values (e.g., `charset="a,b"`).
fn find_data_uri_comma(input: &str) -> Option<usize> {
    let mut in_quotes = false;
    let mut escaped = false;
    for (i, b) in input.bytes().enumerate() {
        if escaped {
            escaped = false;
            continue;
        }
        if b == b'\\' && in_quotes {
            escaped = true;
            continue;
        }
        if b == b'"' {
            in_quotes = !in_quotes;
            continue;
        }
        if b == b',' && !in_quotes {
            return Some(i);
        }
    }
    None
}

/// Helper: checks whether the metadata portion of a data: URI specifies base64 encoding.
fn is_base64_metadata(metadata: &str) -> bool {
    let trimmed = metadata.trim();
    if trimmed.eq_ignore_ascii_case("base64") {
        return true;
    }
    let mut in_quotes = false;
    let mut escaped = false;
    let mut start = 0;
    for (i, b) in metadata.bytes().enumerate() {
        if escaped {
            escaped = false;
            continue;
        }
        if b == b'\\' && in_quotes {
            escaped = true;
            continue;
        }
        if b == b'"' {
            in_quotes = !in_quotes;
            continue;
        }
        if b == b';' && !in_quotes {
            let param = metadata[start..i].trim();
            if param.eq_ignore_ascii_case("base64") {
                return true;
            }
            start = i + 1;
        }
    }
    let param = metadata[start..].trim();
    param.eq_ignore_ascii_case("base64")
}

/// BUG-005 fix: Decode a `data:` URI into raw bytes according to RFC 2397.
/// Supports complex MIME types with quoted parameters, case-insensitive schemes,
/// whitespace tolerance, unpadded & URL-safe base64, and percent-encoding.
/// Format: `data:[<mediatype>][;base64],<data>`
pub fn decode_data_uri_font(data_uri: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let trimmed = data_uri.trim();
    let rest = if trimmed.len() >= 5 && trimmed[..5].eq_ignore_ascii_case("data:") {
        &trimmed[5..]
    } else {
        return Err("Not a data: URI".into());
    };

    let Some(comma_pos) = find_data_uri_comma(rest) else {
        return Err("Malformed data: URI — missing comma separator".into());
    };

    let metadata = &rest[..comma_pos];
    let payload = &rest[comma_pos + 1..];

    if is_base64_metadata(metadata) {
        base64_decode(payload)
    } else {
        Ok(percent_encoding::percent_decode_str(payload).collect::<Vec<u8>>())
    }
}

/// Robust base64 decoder (RFC 4648) supporting standard and URL-safe alphabets,
/// whitespace filtering, percent-encoded padding/chars, and unpadded payloads.
fn base64_decode(input: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    fn val(c: u8) -> Result<u8, Box<dyn std::error::Error>> {
        match c {
            b'A'..=b'Z' => Ok(c - b'A'),
            b'a'..=b'z' => Ok(c - b'a' + 26),
            b'0'..=b'9' => Ok(c - b'0' + 52),
            b'+' | b'-' => Ok(62),
            b'/' | b'_' => Ok(63),
            _ => Err(format!("Invalid base64 character: {}", c as char).into()),
        }
    }

    let decoded_input = if input.contains('%') {
        percent_encoding::percent_decode_str(input)
            .decode_utf8_lossy()
            .into_owned()
    } else {
        input.to_string()
    };

    let filtered: Vec<u8> = decoded_input
        .bytes()
        .filter(|b| !b.is_ascii_whitespace() && *b != b'=')
        .collect();
    let mut out = Vec::with_capacity(filtered.len() * 3 / 4);

    let chunks = filtered.chunks(4);
    for chunk in chunks {
        if chunk.is_empty() {
            continue;
        }
        let b0 = val(chunk[0])?;
        let b1 = if chunk.len() > 1 { val(chunk[1])? } else { 0 };
        let b2 = if chunk.len() > 2 { val(chunk[2])? } else { 0 };
        let b3 = if chunk.len() > 3 { val(chunk[3])? } else { 0 };

        out.push((b0 << 2) | (b1 >> 4));
        if chunk.len() > 2 {
            out.push(((b1 & 0x0F) << 4) | (b2 >> 2));
        }
        if chunk.len() > 3 {
            out.push(((b2 & 0x03) << 6) | b3);
        }
    }

    Ok(out)
}

pub fn collect_image_sources(doc: &Document, node_id: NodeId, sources: &mut Vec<String>) {
    if let Some(node) = doc.get(node_id)
        && let NodeData::Element(elem) = &node.data
    {
        // 1. If <img>, fetch only `src` and lazy-loading `data-src` resources.
        // Responsive candidates are currently not wired into image painting,
        // so prefetching them here would consume bandwidth without rendering
        // them and can duplicate the fallback `src` request.
        if elem.tag_name.eq_ignore_ascii_case("img") {
            if let Some(src) = elem.get_attribute("src") {
                let trimmed = src.trim();
                if !trimmed.is_empty() && !trimmed.starts_with("data:") {
                    sources.push(trimmed.to_string());
                }
            }
            if let Some(data_src) = elem.get_attribute("data-src") {
                let trimmed = data_src.trim();
                if !trimmed.is_empty() && !trimmed.starts_with("data:") {
                    sources.push(trimmed.to_string());
                }
            }
        }

        // 2. Check inline style attribute for url(...)
        if let Some(style_attr) = elem.get_attribute("style") {
            let lower = style_attr.to_ascii_lowercase();
            let mut search_idx = 0;
            while let Some(pos) = lower[search_idx..].find("url(") {
                let start_idx = search_idx + pos + 4;
                if let Some(end_rel) = style_attr[start_idx..].find(')') {
                    let raw_url = &style_attr[start_idx..start_idx + end_rel];
                    let trimmed = raw_url.trim().trim_matches('\'').trim_matches('"').trim();
                    if !trimmed.is_empty() && !trimmed.starts_with("data:") {
                        sources.push(trimmed.to_string());
                    }
                    search_idx = start_idx + end_rel + 1;
                } else {
                    break;
                }
            }
        }
    }
    for child in doc.children(node_id) {
        collect_image_sources(doc, child.id, sources);
    }
}

/// Collects image URLs together with their owning DOM node. This is used after
/// layout to prioritize resources whose boxes intersect the viewport.
pub fn collect_image_node_sources(
    doc: &Document,
    node_id: NodeId,
    sources: &mut Vec<(NodeId, String)>,
) {
    if let Some(node) = doc.get(node_id)
        && let NodeData::Element(elem) = &node.data
        && elem.tag_name.eq_ignore_ascii_case("img")
        && let Some(src) = elem.get_attribute("src")
    {
        let src = src.trim();
        if !src.is_empty() && !src.starts_with("data:") {
            sources.push((node_id, src.to_string()));
        }
    }
    for child in doc.children(node_id) {
        collect_image_node_sources(doc, child.id, sources);
    }
}

pub fn collect_stylesheet_images(
    _doc: &Document,
    stylesheets: &[mango_css::parser::Stylesheet],
    sources: &mut Vec<String>,
) {
    for sheet in stylesheets {
        for rule in &sheet.rules {
            match rule {
                mango_css::parser::Rule::Style(style_rule) => {
                    for decl in &style_rule.declarations {
                        extract_value_images(&decl.value, sources);
                    }
                }
                mango_css::parser::Rule::Media(media_rule) => {
                    for style_rule in &media_rule.rules {
                        for decl in &style_rule.declarations {
                            extract_value_images(&decl.value, sources);
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

fn extract_value_images(val: &mango_css::values::Value, sources: &mut Vec<String>) {
    match val {
        mango_css::values::Value::Url(url) => {
            let trimmed = url.trim();
            if !trimmed.is_empty() && !trimmed.starts_with("data:") {
                sources.push(trimmed.to_string());
            }
        }
        mango_css::values::Value::List(items) => {
            for item in items {
                extract_value_images(item, sources);
            }
        }
        _ => {}
    }
}

#[allow(dead_code)]
pub fn collect_box_background_images(b: &mango_layout::box_tree::LayoutBox, out: &mut Vec<String>) {
    if let Some(ref s) = b.style {
        if let Some(ref bg) = s.background_image {
            out.push(bg.clone());
        }
        for layer in &s.background_layers {
            if let Some(ref bg) = layer.image {
                out.push(bg.clone());
            }
        }
        if let Some(ref mask) = s.mask_image {
            out.push(mask.clone());
        }
    }
    for c in &b.children {
        collect_box_background_images(c, out);
    }
}

pub fn extract_title(doc: &Document) -> Option<String> {
    let title_id = doc.find_element_by_tag(doc.root(), "title")?;
    let text = doc.text_content(title_id).trim().to_string();
    if text.is_empty() { None } else { Some(text) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_data_uri() {
        // Base64
        let b64_uri = "data:text/plain;base64,SGVsbG8sIFdvcmxkIQ==";
        let decoded = decode_data_uri_font(b64_uri).unwrap();
        assert_eq!(decoded, b"Hello, World!");

        // Percent-encoded
        let pct_uri = "data:text/plain,Hello%2C%20World%21";
        let decoded2 = decode_data_uri_font(pct_uri).unwrap();
        assert_eq!(decoded2, b"Hello, World!");

        // Plain text
        let plain_uri = "data:text/plain,Hello";
        let decoded3 = decode_data_uri_font(plain_uri).unwrap();
        assert_eq!(decoded3, b"Hello");
    }

    #[test]
    fn test_decode_data_uri_complex_mime_and_rfc_compliance() {
        // Complex MIME type with parameters
        let uri = "data:application/font-woff;charset=utf-8;base64,SGVsbG8=";
        assert_eq!(decode_data_uri_font(uri).unwrap(), b"Hello");

        // Quoted parameter with embedded comma
        let uri_quoted_comma = "data:text/plain;charset=\"utf-8,latin\";base64,SGVsbG8=";
        assert_eq!(decode_data_uri_font(uri_quoted_comma).unwrap(), b"Hello");

        // Case-insensitive scheme and parameter with whitespace around separator
        let uri_case = "  DATA:font/woff2;  BASE64  ,SGVsbG8=  ";
        assert_eq!(decode_data_uri_font(uri_case).unwrap(), b"Hello");

        // Empty MIME type with base64
        let uri_empty_mime = "data:;base64,SGVsbG8=";
        assert_eq!(decode_data_uri_font(uri_empty_mime).unwrap(), b"Hello");

        // No leading semicolon before base64
        let uri_bare_b64 = "data:base64,SGVsbG8=";
        assert_eq!(decode_data_uri_font(uri_bare_b64).unwrap(), b"Hello");

        // Multiline / embedded whitespace in base64 payload (common in CSS @font-face)
        let uri_multiline = "data:font/woff;base64,\r\n SG\nVs\r\nbG8= \t\n";
        assert_eq!(decode_data_uri_font(uri_multiline).unwrap(), b"Hello");

        // Unpadded base64 (missing trailing '=')
        let uri_unpadded = "data:text/plain;base64,SGVsbG8";
        assert_eq!(decode_data_uri_font(uri_unpadded).unwrap(), b"Hello");

        // URL-safe base64 (- and _)
        // Standard "+/==" is byte [251], URL-safe "-_==" is byte [251]
        let uri_urlsafe = "data:application/octet-stream;base64,-_==";
        assert_eq!(decode_data_uri_font(uri_urlsafe).unwrap(), vec![0xFB]);

        // Percent-encoded characters in base64 payload
        let uri_pct_b64 = "data:application/octet-stream;base64,%2B%2F%3D%3D";
        assert_eq!(decode_data_uri_font(uri_pct_b64).unwrap(), vec![0xFB]);

        // Empty data payload
        let uri_empty = "data:,";
        assert_eq!(decode_data_uri_font(uri_empty).unwrap(), Vec::<u8>::new());

        // Missing comma separator should fail
        assert!(decode_data_uri_font("data:text/plain;base64").is_err());

        // Non-data URI prefix should fail
        assert!(decode_data_uri_font("http://example.com/font.woff").is_err());
    }
}
