//! Web font loading, image resource harvesting, data URI decoding, and title extraction.

use mango_css::Stylesheet;
use mango_html::dom::{Document, NodeData, NodeId};
use mango_net::{ResourceLoader, Url};
use mango_render::{FontWeight, font_manager};

use super::super::types::{
    MAX_CONCURRENT_FETCHES, SUBRESOURCE_FETCH_TIMEOUT, assert_not_on_ui_thread, fetch_with_timeout,
};
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
    if let Err(e) = std::thread::Builder::new()
        .name("mango-font-async".to_string())
        .spawn(move || {
            load_web_fonts(&loader, base_url.as_ref(), &stylesheets);
        })
    {
        log::warn!("Failed to spawn async font loader: {e}");
    }
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
        if face.src_url.starts_with("data:") {
            match decode_data_uri_font(&face.src_url) {
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
                if let Err(e) = std::thread::Builder::new()
                    .name("mango-font-fetch".to_string())
                    .spawn(move || {
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
                    })
                {
                    log::warn!("Failed to spawn font fetch thread: {e}");
                }
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

/// BUG-005 fix: Decode a `data:` URI into raw bytes.
/// Supports both base64-encoded and plain text data URIs.
/// Format: `data:[<mediatype>][;base64],<data>`
fn decode_data_uri_font(data_uri: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let rest = data_uri.strip_prefix("data:").ok_or("Not a data: URI")?;
    if let Some(comma_pos) = rest.find(',') {
        let metadata = &rest[..comma_pos];
        let payload = &rest[comma_pos + 1..];
        if metadata.contains(";base64") {
            let decoded = base64_decode(payload.trim())?;
            Ok(decoded)
        } else {
            // Plain text / percent-encoded
            Ok(payload.as_bytes().to_vec())
        }
    } else {
        Err("Malformed data: URI — no comma separator".into())
    }
}

/// Minimal base64 decoder (RFC 4648). Avoids adding a crate dependency.
fn base64_decode(input: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    fn val(c: u8) -> Result<u8, Box<dyn std::error::Error>> {
        match c {
            b'A'..=b'Z' => Ok(c - b'A'),
            b'a'..=b'z' => Ok(c - b'a' + 26),
            b'0'..=b'9' => Ok(c - b'0' + 52),
            b'+' => Ok(62),
            b'/' => Ok(63),
            _ => Err(format!("Invalid base64 character: {}", c as char).into()),
        }
    }

    let filtered: Vec<u8> = input
        .bytes()
        .filter(|b| !b.is_ascii_whitespace() && *b != b'=')
        .collect();
    let mut out = Vec::with_capacity(filtered.len() * 3 / 4);

    let chunks = filtered.chunks(4);
    for chunk in chunks {
        let b0 = val(chunk[0])?;
        let b1 = if chunk.len() > 1 { val(chunk[1])? } else { 0 };
        let b2 = if chunk.len() > 2 { val(chunk[2])? } else { 0 };
        let b3 = if chunk.len() > 3 { val(chunk[3])? } else { 0 };

        out.push((b0 << 2) | (b1 >> 4));
        if chunk.len() > 2 {
            out.push((b1 << 4) | (b2 >> 2));
        }
        if chunk.len() > 3 {
            out.push((b2 << 6) | b3);
        }
    }

    Ok(out)
}

pub fn collect_image_sources(doc: &Document, node_id: NodeId, sources: &mut Vec<String>) {
    if let Some(node) = doc.get(node_id)
        && let NodeData::Element(elem) = &node.data
    {
        // 1. If <img>, check src, srcset, data-src
        if elem.tag_name.eq_ignore_ascii_case("img") {
            if let Some(src) = elem.get_attribute("src") {
                let trimmed = src.trim();
                if !trimmed.is_empty() && !trimmed.starts_with("data:") {
                    sources.push(trimmed.to_string());
                }
            }
            if let Some(srcset) = elem.get_attribute("srcset") {
                for candidate in srcset.split(',') {
                    let url = candidate.split_whitespace().next().unwrap_or("");
                    if !url.is_empty() && !url.starts_with("data:") {
                        sources.push(url.to_string());
                    }
                }
            }
            if let Some(data_src) = elem.get_attribute("data-src") {
                let trimmed = data_src.trim();
                if !trimmed.is_empty() && !trimmed.starts_with("data:") {
                    sources.push(trimmed.to_string());
                }
            }
        }

        // 1b. If <source> (e.g. inside <picture>), collect srcset URLs for prefetching
        if elem.tag_name.eq_ignore_ascii_case("source") {
            if let Some(srcset) = elem.get_attribute("srcset") {
                for candidate in srcset.split(',') {
                    let url = candidate.split_whitespace().next().unwrap_or("");
                    if !url.is_empty() && !url.starts_with("data:") {
                        sources.push(url.to_string());
                    }
                }
            }
            if let Some(src) = elem.get_attribute("src") {
                let trimmed = src.trim();
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
