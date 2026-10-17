//! URL navigation, document loading, subresource pipelines, and page lifecycle.

use mango_css::{Stylesheet, parse_stylesheet};
use mango_html::dom::{Document, NodeData, NodeId};
use mango_html::parse_html;
use mango_js::JsRuntime;
use mango_net::{FetchedDocument, ResourceLoader, Url};
use mango_render::display_list::DisplayList;
use mango_render::{FontWeight, cache_image, decode_data_uri, font_manager, get_cached_image};
use std::collections::HashMap;

use super::BrowserChrome;
use super::forms::find_element_by_id;
use super::types::{
    MAX_CONCURRENT_FETCHES, PreparedDocument, SUBRESOURCE_FETCH_TIMEOUT, SYNC_NAVIGATION_TIMEOUT,
    ScriptToRun, assert_not_on_ui_thread, fetch_with_timeout,
};
use super::welcome::{preprocess_wikipedia_appearance_html, resolve_navigation_input};
use crate::chrome_ui::*;
use crate::navigation::*;

/// Prepares a fetched document on a background worker thread: parsing HTML, prefetching
/// external stylesheets, web fonts, and scripts, so the main UI thread never blocks on I/O.
pub fn prepare_document_pipeline(
    loader: &ResourceLoader,
    doc_result: FetchedDocument,
) -> PreparedDocument {
    prepare_document_pipeline_cancellable(loader, doc_result, &|| false)
}

/// Cancellable variant of [`prepare_document_pipeline`].
///
/// `should_cancel` is polled before every fetch batch and after every batch;
/// when it returns true the pipeline abandons remaining subresource fetches
/// and returns a partial document immediately, so superseded navigations free
/// their worker without wasting network/time on a dead page.
/// Prepares an incremental DOM document snapshot for progressive rendering (FCP).
pub fn prepare_document_pipeline_from_doc(
    loader: &ResourceLoader,
    doc: mango_html::Document,
    final_url: Url,
    should_cancel: &dyn Fn() -> bool,
) -> PreparedDocument {
    prepare_document_pipeline_internal(
        loader,
        doc,
        final_url,
        String::new(),
        200,
        "text/html".to_string(),
        mango_net::encoding::Encoding::Utf8,
        Vec::new(),
        should_cancel,
        true,
    )
}

/// Cancellable variant of [`prepare_document_pipeline`].
///
/// `should_cancel` is polled before every fetch batch and after every batch;
/// when it returns true the pipeline abandons remaining subresource fetches
/// and returns a partial document immediately, so superseded navigations free
/// their worker without wasting network/time on a dead page.
pub fn prepare_document_pipeline_cancellable(
    loader: &ResourceLoader,
    mut doc_result: FetchedDocument,
    should_cancel: &dyn Fn() -> bool,
) -> PreparedDocument {
    let final_url = doc_result.url.clone();
    preprocess_wikipedia_appearance_html(&mut doc_result.html, &final_url.as_str());

    // Speculative Preload Scanner & Incremental Parser
    let mut inc_parser = mango_html::IncrementalParser::new();
    let early_hints = inc_parser.feed_str(&doc_result.html);

    let mut doc = inc_parser.finish();
    doc.character_set = doc_result.encoding.name().to_string();
    doc.content_type = doc_result.content_type.clone();

    prepare_document_pipeline_internal(
        loader,
        doc,
        final_url,
        doc_result.html,
        doc_result.status,
        doc_result.content_type,
        doc_result.encoding,
        early_hints,
        should_cancel,
        false,
    )
}

#[allow(clippy::too_many_arguments)]
fn prepare_document_pipeline_internal(
    loader: &ResourceLoader,
    doc: mango_html::Document,
    final_url: Url,
    html: String,
    status: u16,
    content_type: String,
    encoding: mango_net::encoding::Encoding,
    early_hints: Vec<mango_html::PreloadHint>,
    should_cancel: &dyn Fn() -> bool,
    is_progressive: bool,
) -> PreparedDocument {
    let preload_scan = loader.scan_and_preload(&html, &final_url, None);

    let mut stylesheet_links = Vec::new();
    collect_stylesheet_links(&doc, doc.root(), &mut stylesheet_links);
    for hint in early_hints {
        if hint.kind == mango_html::PreloadHintKind::Style && !stylesheet_links.contains(&hint.url)
        {
            stylesheet_links.push(hint.url);
        }
    }
    for item in &preload_scan.preloads {
        if item.kind == mango_net::PreloadKind::Style && !stylesheet_links.contains(&item.url) {
            stylesheet_links.push(item.url.clone());
        }
    }
    let mut visited_css = std::collections::HashSet::new();
    let mut cached_stylesheets = Vec::new();
    if !should_cancel() {
        load_stylesheets_parallel_cancellable(
            loader,
            &final_url,
            &stylesheet_links,
            &mut visited_css,
            &mut cached_stylesheets,
            should_cancel,
        );
    }

    let inline_sheets = mango_layout::extract_style_elements(&doc);
    let mut import_links = Vec::new();
    for sheet in &inline_sheets {
        for rule in &sheet.rules {
            if let mango_css::parser::Rule::Import(import_path) = rule {
                import_links.push(import_path.clone());
            }
        }
    }
    if !import_links.is_empty() && !should_cancel() {
        load_stylesheets_parallel_cancellable(
            loader,
            &final_url,
            &import_links,
            &mut visited_css,
            &mut cached_stylesheets,
            should_cancel,
        );
    }

    if !is_progressive && !should_cancel() {
        load_web_fonts_async(
            loader.clone(),
            Some(final_url.clone()),
            cached_stylesheets.clone(),
        );
        load_web_fonts_async(
            loader.clone(),
            Some(final_url.clone()),
            inline_sheets.clone(),
        );
    }

    let mut raw_scripts = Vec::new();
    collect_scripts(&doc, doc.root(), &mut raw_scripts);
    for item in &preload_scan.preloads {
        if item.kind == mango_net::PreloadKind::Script {
            let already_present = raw_scripts.iter().any(|s| match s {
                ScriptToRun::External { src, .. } | ScriptToRun::Preloaded { src, .. } => {
                    src == &item.url
                }
                _ => false,
            });
            if !already_present {
                raw_scripts.push(ScriptToRun::External {
                    src: item.url.clone(),
                    is_defer: false,
                    is_async: true,
                });
            }
        }
    }

    const MAX_PRELOAD_SCRIPTS: usize = 24;
    const SCRIPT_FETCH_TIMEOUT: std::time::Duration = SUBRESOURCE_FETCH_TIMEOUT;
    let mut to_fetch: Vec<(usize, String)> = Vec::new();
    if !is_progressive {
        for (idx, s) in raw_scripts.iter().enumerate() {
            if let ScriptToRun::External { src, .. } = s {
                if to_fetch.len() < MAX_PRELOAD_SCRIPTS {
                    to_fetch.push((idx, src.clone()));
                }
            }
        }
    }

    // Bounded parallel script prefetch: at most MAX_CONCURRENT_FETCHES in
    // flight, per-fetch timeout, epoch abandonment between batches. Sequence
    // numbers (idx) preserve document order regardless of completion order.
    let mut fetched_scripts: std::collections::HashMap<
        usize,
        Result<String, mango_net::NetworkError>,
    > = std::collections::HashMap::new();
    if !to_fetch.is_empty() && !should_cancel() {
        for chunk in to_fetch.chunks(MAX_CONCURRENT_FETCHES) {
            if should_cancel() {
                break;
            }
            let (tx, rx) = std::sync::mpsc::channel();
            let n = chunk.len();
            for (idx, src) in chunk.iter() {
                let tx = tx.clone();
                let loader = loader.clone();
                let base = final_url.clone();
                let src = src.clone();
                let idx = *idx;
                if let Err(e) = std::thread::Builder::new()
                    .name("mango-script-fetch".to_string())
                    .spawn(move || {
                        let res = loader.fetch_script(&base, &src);
                        if let Err(ref err) = res {
                            log::warn!("Background fetch_script error for {src}: {err}");
                        }
                        let _ = tx.send((idx, res));
                    })
                {
                    log::warn!("Failed to spawn script fetch thread: {e}");
                }
            }
            drop(tx);
            for _ in 0..n {
                if should_cancel() {
                    break;
                }
                match rx.recv_timeout(SCRIPT_FETCH_TIMEOUT + std::time::Duration::from_millis(500))
                {
                    Ok((idx, res)) => {
                        log::info!("Fetched script {idx} (ok: {})", res.is_ok());
                        fetched_scripts.insert(idx, res);
                    }
                    Err(err) => {
                        log::warn!("rx.recv_timeout failed: {err}");
                        break;
                    }
                }
            }
        }
    }

    let mut preloaded_scripts = Vec::with_capacity(raw_scripts.len());
    for (idx, s) in raw_scripts.into_iter().enumerate() {
        match s {
            ScriptToRun::Inline(c) => preloaded_scripts.push(ScriptToRun::Inline(c)),
            ScriptToRun::External {
                src,
                is_defer,
                is_async,
            } => {
                if let Some(Ok(code)) = fetched_scripts.remove(&idx) {
                    preloaded_scripts.push(ScriptToRun::Preloaded {
                        src,
                        code,
                        is_defer,
                        is_async,
                    });
                } else {
                    preloaded_scripts.push(ScriptToRun::External {
                        src,
                        is_defer,
                        is_async,
                    });
                }
            }
            ScriptToRun::Preloaded {
                src,
                code,
                is_defer,
                is_async,
            } => {
                preloaded_scripts.push(ScriptToRun::Preloaded {
                    src,
                    code,
                    is_defer,
                    is_async,
                });
            }
        }
    }

    let mut doc_images = Vec::new();
    collect_image_sources(&doc, doc.root(), &mut doc_images);
    for item in &preload_scan.preloads {
        if item.kind == mango_net::PreloadKind::Image && !doc_images.contains(&item.url) {
            doc_images.push(item.url.clone());
        }
    }

    let mut sheet_images = Vec::new();
    collect_stylesheet_images(&doc, &cached_stylesheets, &mut sheet_images);
    collect_stylesheet_images(&doc, &inline_sheets, &mut sheet_images);

    let mut image_sources = Vec::new();
    let sheet_take = 6.min(sheet_images.len());
    let doc_take = 6.min(doc_images.len());
    image_sources.extend(sheet_images[..sheet_take].iter().cloned());
    image_sources.extend(doc_images[..doc_take].iter().cloned());
    if sheet_images.len() > sheet_take {
        image_sources.extend(sheet_images[sheet_take..].iter().cloned());
    }
    if doc_images.len() > doc_take {
        image_sources.extend(doc_images[doc_take..].iter().cloned());
    }

    let mut seen = std::collections::HashSet::new();
    image_sources.retain(|src| seen.insert(src.clone()));

    PreparedDocument {
        url: final_url,
        doc,
        html,
        status,
        content_type,
        encoding,
        cached_stylesheets,
        preloaded_scripts,
        image_sources,
    }
}

fn is_js_script_type(type_attr: Option<&str>) -> bool {
    match type_attr {
        None => true,
        Some(t) => {
            let t = t.trim().to_ascii_lowercase();
            t.is_empty()
                || t == "text/javascript"
                || t == "application/javascript"
                || t == "text/ecmascript"
                || t == "application/ecmascript"
                || t == "javascript"
        }
    }
}

fn collect_scripts(doc: &Document, node_id: NodeId, scripts: &mut Vec<ScriptToRun>) {
    if let Some(node) = doc.get(node_id)
        && let NodeData::Element(ref elem) = node.data
        && elem.tag_name.eq_ignore_ascii_case("script")
        && is_js_script_type(elem.get_attribute("type"))
    {
        let is_defer = elem.get_attribute("defer").is_some();
        let is_async = elem.get_attribute("async").is_some();
        if let Some(src) = elem.get_attribute("src") {
            let trimmed = src.trim();
            if !trimmed.is_empty() {
                scripts.push(ScriptToRun::External {
                    src: trimmed.to_string(),
                    is_defer,
                    is_async,
                });
            }
        } else {
            let code = doc.text_content(node_id);
            if !code.trim().is_empty() {
                scripts.push(ScriptToRun::Inline(code));
            }
        }
    }
    for child in doc.children(node_id) {
        collect_scripts(doc, child.id, scripts);
    }
}

fn collect_stylesheet_links(doc: &Document, node_id: NodeId, links: &mut Vec<String>) {
    if let Some(node) = doc.get(node_id)
        && let NodeData::Element(elem) = &node.data
        && elem.tag_name.eq_ignore_ascii_case("link")
    {
        let is_stylesheet = elem
            .get_attribute("rel")
            .map(|r| {
                r.split_whitespace()
                    .any(|t| t.eq_ignore_ascii_case("stylesheet"))
            })
            .unwrap_or(false);
        if is_stylesheet && let Some(href) = elem.get_attribute("href") {
            let trimmed = href.trim();
            if !trimmed.is_empty() {
                links.push(trimmed.to_string());
            }
        }
    }
    for child in doc.children(node_id) {
        collect_stylesheet_links(doc, child.id, links);
    }
}

#[allow(dead_code)]
fn load_stylesheets_parallel(
    loader: &ResourceLoader,
    base_url: &Url,
    links: &[String],
    visited: &mut std::collections::HashSet<String>,
    out: &mut Vec<Stylesheet>,
) {
    load_stylesheets_parallel_cancellable(loader, base_url, links, visited, out, &|| false);
}

/// Bounded stylesheet fan-out: at most [`MAX_CONCURRENT_FETCHES`] fetches in
/// flight, per-fetch timeout, sequence-numbered results preserving document
/// order, and early epoch abandonment via `should_cancel`.
fn load_stylesheets_parallel_cancellable(
    loader: &ResourceLoader,
    base_url: &Url,
    links: &[String],
    visited: &mut std::collections::HashSet<String>,
    out: &mut Vec<Stylesheet>,
    should_cancel: &dyn Fn() -> bool,
) {
    if links.is_empty() || should_cancel() {
        return;
    }
    let mut to_fetch = Vec::new();
    for href in links {
        if let Ok(resolved) = base_url.resolve(href) {
            let key = resolved.to_string();
            if visited.insert(key) {
                to_fetch.push((href.clone(), resolved));
            }
        }
    }
    if to_fetch.is_empty() {
        return;
    }

    // Scoped fan-out in chunks of MAX_CONCURRENT_FETCHES so concurrency is
    // capped at 6 regardless of link count. Each chunk joins with a timeout;
    // sequence numbers (idx) restore document order.
    let mut results_map: std::collections::HashMap<usize, Result<String, mango_net::NetworkError>> =
        std::collections::HashMap::new();
    for chunk in to_fetch
        .iter()
        .enumerate()
        .collect::<Vec<_>>()
        .chunks(MAX_CONCURRENT_FETCHES)
    {
        if should_cancel() {
            break;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let n = chunk.len();
        for (idx, (href, _)) in chunk.iter() {
            let tx = tx.clone();
            let loader = loader.clone();
            let base = base_url.clone();
            let href = (*href).clone();
            let idx = *idx;
            if let Err(e) = std::thread::Builder::new()
                .name("mango-css-fetch".to_string())
                .spawn(move || {
                    if href.trim_start().starts_with("data:") {
                        let res = loader.fetch_stylesheet(&base, &href);
                        let _ = tx.send((idx, res));
                        return;
                    }
                    match fetch_with_timeout(
                        {
                            let loader = loader.clone();
                            let base = base.clone();
                            let href = href.clone();
                            move || loader.fetch_stylesheet(&base, &href)
                        },
                        SUBRESOURCE_FETCH_TIMEOUT,
                    ) {
                        Ok(res) => {
                            let _ = tx.send((idx, res));
                        }
                        Err(msg) => {
                            log::warn!("Stylesheet fetch timed out for {href}: {msg}");
                        }
                    }
                })
            {
                log::warn!("Failed to spawn stylesheet fetch thread: {e}");
            }
        }
        drop(tx);
        for _ in 0..n {
            if should_cancel() {
                break;
            }
            match rx.recv_timeout(SUBRESOURCE_FETCH_TIMEOUT + std::time::Duration::from_secs(2)) {
                Ok((idx, res)) => {
                    results_map.insert(idx, res);
                }
                Err(_) => break,
            }
        }
    }

    let mut import_rules = Vec::new();
    for (idx, (_href, resolved)) in to_fetch.into_iter().enumerate() {
        if should_cancel() {
            break;
        }
        if let Some(res) = results_map.remove(&idx) {
            match res {
                Ok(css_text) => {
                    log::info!("Loaded external stylesheet: {}", resolved);
                    let sheet = parse_stylesheet(&css_text);
                    for rule in &sheet.rules {
                        if let mango_css::parser::Rule::Import(import_path) = rule {
                            import_rules.push((resolved.clone(), import_path.clone()));
                        }
                    }
                    out.push(sheet);
                }
                Err(e) => {
                    log::warn!("Failed to fetch external stylesheet {}: {}", resolved, e);
                }
            }
        }
    }

    if !should_cancel() {
        load_imported_stylesheets_parallel_cancellable(
            loader,
            import_rules,
            visited,
            out,
            should_cancel,
        );
    }
}

fn load_imported_stylesheets_parallel(
    loader: &ResourceLoader,
    pending_imports: Vec<(Url, String)>,
    visited: &mut std::collections::HashSet<String>,
    out: &mut Vec<Stylesheet>,
) {
    load_imported_stylesheets_parallel_cancellable(loader, pending_imports, visited, out, &|| {
        false
    });
}

fn load_imported_stylesheets_parallel_cancellable(
    loader: &ResourceLoader,
    mut pending_imports: Vec<(Url, String)>,
    visited: &mut std::collections::HashSet<String>,
    out: &mut Vec<Stylesheet>,
    should_cancel: &dyn Fn() -> bool,
) {
    const MAX_IMPORT_DEPTH: usize = 8;
    let mut depth = 0;
    while !pending_imports.is_empty() && !should_cancel() {
        if depth >= MAX_IMPORT_DEPTH {
            log::warn!("Stylesheet @import depth cap reached; abandoning remaining imports");
            break;
        }
        depth += 1;
        let current_batch = std::mem::take(&mut pending_imports);
        let mut to_fetch = Vec::new();

        for (base, href) in current_batch {
            if let Ok(resolved) = base.resolve(&href) {
                let key = resolved.to_string();
                if visited.insert(key) {
                    to_fetch.push((base, href, resolved));
                }
            }
        }

        if to_fetch.is_empty() {
            break;
        }

        // Bounded fan-out preserving order via sequence numbers.
        let mut results: Vec<(usize, Url, Result<String, mango_net::NetworkError>)> = Vec::new();
        for chunk in to_fetch
            .into_iter()
            .enumerate()
            .collect::<Vec<_>>()
            .chunks(MAX_CONCURRENT_FETCHES)
        {
            if should_cancel() {
                break;
            }
            let (tx, rx) = std::sync::mpsc::channel();
            let n = chunk.len();
            for (idx, (base, href, resolved)) in chunk.iter() {
                let tx = tx.clone();
                let loader = loader.clone();
                let base = (*base).clone();
                let href = (*href).clone();
                let resolved = (*resolved).clone();
                let idx = *idx;
                if let Err(e) = std::thread::Builder::new()
                    .name("mango-css-import".to_string())
                    .spawn(move || {
                        match fetch_with_timeout(
                            {
                                let loader = loader.clone();
                                let base = base.clone();
                                let href = href.clone();
                                move || loader.fetch_stylesheet(&base, &href)
                            },
                            SUBRESOURCE_FETCH_TIMEOUT,
                        ) {
                            Ok(res) => {
                                let _ = tx.send((idx, resolved, res));
                            }
                            Err(msg) => {
                                log::warn!("Stylesheet @import fetch timed out for {href}: {msg}");
                            }
                        }
                    })
                {
                    log::warn!("Failed to spawn @import fetch thread: {e}");
                }
            }
            drop(tx);
            for _ in 0..n {
                if should_cancel() {
                    break;
                }
                match rx.recv_timeout(SUBRESOURCE_FETCH_TIMEOUT + std::time::Duration::from_secs(2))
                {
                    Ok(item) => results.push(item),
                    Err(_) => break,
                }
            }
        }

        results.sort_by_key(|(idx, _, _)| *idx);

        for (_idx, resolved, res) in results {
            if should_cancel() {
                break;
            }
            match res {
                Ok(css_text) => {
                    log::info!("Loaded external stylesheet: {}", resolved);
                    let sheet = parse_stylesheet(&css_text);
                    for rule in &sheet.rules {
                        if let mango_css::parser::Rule::Import(import_path) = rule {
                            pending_imports.push((resolved.clone(), import_path.clone()));
                        }
                    }
                    out.push(sheet);
                }
                Err(e) => {
                    log::warn!("Failed to fetch external stylesheet {}: {}", resolved, e);
                }
            }
        }
    }
}

/// Non-blocking font registration for the UI thread: returns immediately and
/// fetches + registers web fonts on a detached background thread.
///
/// The background pipeline calls the blocking [`load_web_fonts`] directly;
/// UI code paths must call this async variant so font downloads never freeze
/// the event loop.
fn load_web_fonts_async(
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

fn load_web_fonts(loader: &ResourceLoader, base_url: Option<&Url>, stylesheets: &[Stylesheet]) {
    assert_not_on_ui_thread("load_web_fonts");
    let mut font_faces: Vec<mango_css::parser::FontFaceRule> = Vec::new();
    for sheet in stylesheets {
        for rule in &sheet.rules {
            if let mango_css::parser::Rule::FontFace(font_face) = rule {
                font_faces.push(font_face.clone());
            }
        }
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
        let is_true_bold = match face.font_weight {
            mango_css::values::FontWeight::Bold | mango_css::values::FontWeight::Bolder => true,
            mango_css::values::FontWeight::Numeric(w) => w >= 700,
            _ => false,
        };
        let is_regular = match face.font_weight {
            mango_css::values::FontWeight::Normal => true,
            mango_css::values::FontWeight::Numeric(w) => w <= 500,
            _ => false,
        };
        if is_true_bold {
            candidates
                .entry((family_key, FontWeight::Bold))
                .or_default()
                .push(i);
        } else if is_regular {
            candidates
                .entry((family_key, FontWeight::Regular))
                .or_default()
                .push(i);
        } else if let mango_css::values::FontWeight::Numeric(w) = face.font_weight {
            if w >= 600 {
                candidates
                    .entry((family_key, FontWeight::Bold))
                    .or_default()
                    .push(i);
            }
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
        for chunk in remotes.chunks(MAX_CONCURRENT_FETCHES) {
            let (tx, rx) = std::sync::mpsc::channel();
            let n = chunk.len();
            for (seq, _face_idx, _weight, url) in chunk.iter() {
                let tx = tx.clone();
                let loader = loader.clone();
                let base = base.clone();
                let url = url.clone();
                let seq = *seq;
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
                                let _ = tx.send((seq, bytes));
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
                    Ok((seq, bytes)) => {
                        fetched.insert(seq, bytes);
                    }
                    Err(_) => break,
                }
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

fn collect_image_sources(doc: &Document, node_id: NodeId, sources: &mut Vec<String>) {
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

pub(crate) fn collect_stylesheet_images(
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
fn collect_box_background_images(b: &mango_layout::box_tree::LayoutBox, out: &mut Vec<String>) {
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

fn extract_title(doc: &Document) -> Option<String> {
    let title_id = doc.find_element_by_tag(doc.root(), "title")?;
    let text = doc.text_content(title_id).trim().to_string();
    if text.is_empty() { None } else { Some(text) }
}

impl BrowserChrome {
    /// Navigates the browser to the specified URL or input string.
    pub fn navigate(&mut self, url: &str) {
        self.address_text = url.to_string();
        self.cursor_pos = self.address_text.len();
        self.navigate_to_address(true);
    }

    /// Navigates back in history if possible, restoring from BFCache when available.
    pub fn go_back(&mut self) {
        let nav_info = if let Some(tab) = self.tabs.get_mut(self.active_tab_idx) {
            if let Some(ref root) = self.root_box {
                let (w, h) = (
                    self.width as f32,
                    (self.height.saturating_sub(HEADER_HEIGHT as u32)) as f32,
                );
                let snapshot = mango_layout::LayoutSnapshot::from_layout_box(
                    root,
                    mango_core::Size::new(w, h),
                );
                tab.store_bfcache_snapshot(Box::new(snapshot));
            }

            if let Some((url, scroll_pos)) = tab.go_back_f64() {
                let title = tab.title.clone();
                let snapshot = tab.current_bfcache_snapshot().cloned();
                Some((url, title, scroll_pos, snapshot))
            } else {
                None
            }
        } else {
            None
        };

        if let Some((url, title, scroll_pos, snapshot)) = nav_info {
            self.address_text = url;
            self.page_title = title;
            self.cursor_pos = self.address_text.len();
            self.set_scroll_y_f64(scroll_pos);

            if let Some(snapshot) = snapshot {
                let dl = if self.scroll_y == 0.0 {
                    snapshot.display_list().clone()
                } else {
                    snapshot.display_list_with_scroll(self.scroll_y)
                };
                self.page_display_list = dl;
                self.root_box = Some(snapshot.root_box().clone());
                self.cached_scrollable_height =
                    crate::scroll::compute_scrollable_height(self.root_box.as_ref());
                self.page_dirty = true;
                self.status_text = format!("Restored from BFCache: {}", self.page_title);
                return;
            }

            self.navigate_to_address(false);
        }
    }

    /// Navigates forward in history if possible, restoring from BFCache when available.
    pub fn go_forward(&mut self) {
        let nav_info = if let Some(tab) = self.tabs.get_mut(self.active_tab_idx) {
            if let Some(ref root) = self.root_box {
                let (w, h) = (
                    self.width as f32,
                    (self.height.saturating_sub(HEADER_HEIGHT as u32)) as f32,
                );
                let snapshot = mango_layout::LayoutSnapshot::from_layout_box(
                    root,
                    mango_core::Size::new(w, h),
                );
                tab.store_bfcache_snapshot(Box::new(snapshot));
            }

            if let Some((url, scroll_pos)) = tab.go_forward_f64() {
                let title = tab.title.clone();
                let snapshot = tab.current_bfcache_snapshot().cloned();
                Some((url, title, scroll_pos, snapshot))
            } else {
                None
            }
        } else {
            None
        };

        if let Some((url, title, scroll_pos, snapshot)) = nav_info {
            self.address_text = url;
            self.page_title = title;
            self.cursor_pos = self.address_text.len();
            self.set_scroll_y_f64(scroll_pos);

            if let Some(snapshot) = snapshot {
                let dl = if self.scroll_y == 0.0 {
                    snapshot.display_list().clone()
                } else {
                    snapshot.display_list_with_scroll(self.scroll_y)
                };
                self.page_display_list = dl;
                self.root_box = Some(snapshot.root_box().clone());
                self.cached_scrollable_height =
                    crate::scroll::compute_scrollable_height(self.root_box.as_ref());
                self.page_dirty = true;
                self.status_text = format!("Restored from BFCache: {}", self.page_title);
                return;
            }

            self.navigate_to_address(false);
        }
    }

    /// Public method to load HTML into the active tab.
    #[allow(dead_code)]
    pub fn load_html(&mut self, html: String, url: String) {
        self.load_html_internal(html, url, true);
    }

    /// Internal method to load HTML, execute scripts, initialize JS runtime, and re-layout.
    pub(crate) fn load_html_internal(&mut self, html: String, url: String, push_history: bool) {
        self.load_html_with_metadata(html, url, push_history, None, None);
    }

    /// Internal method to load HTML with known encoding and content-type metadata.
    fn load_html_with_metadata(
        &mut self,
        mut html: String,
        url: String,
        push_history: bool,
        encoding: Option<&str>,
        content_type: Option<&str>,
    ) {
        preprocess_wikipedia_appearance_html(&mut html, &url);
        let mut doc = parse_html(&html);
        if let Some(enc) = encoding {
            doc.character_set = enc.to_string();
        }
        if let Some(ct) = content_type {
            doc.content_type = ct.to_string();
        }
        self.load_document_internal(doc, html, url, push_history, false);
    }

    /// Loads an already parsed Document into the browser without re-parsing HTML.
    fn load_document_internal(
        &mut self,
        doc: Document,
        html: String,
        url: String,
        push_history: bool,
        skip_embedded_sheets_and_fonts: bool,
    ) {
        self.load_document_internal_with_scripts(
            doc,
            html,
            url,
            push_history,
            skip_embedded_sheets_and_fonts,
            None,
        );
    }

    fn load_document_internal_with_scripts(
        &mut self,
        doc: Document,
        html: String,
        url: String,
        push_history: bool,
        skip_embedded_sheets_and_fonts: bool,
        preloaded_scripts: Option<Vec<ScriptToRun>>,
    ) {
        self.current_html = html;
        self.address_text = url.clone();
        self.cursor_pos = self.address_text.len();
        self.context_menu = None;
        self.select_dropdown = None;
        self.picker = None;
        self.validation_bubble = None;
        self.range_dragging = None;
        self.range_drag_start_value = None;
        self.focused_control = None;
        self.focused_initial_value = None;
        self.control_cursor_pos = 0;

        if let Some(title) = extract_title(&doc) {
            self.page_title = title;
        } else {
            self.page_title = url.clone();
        }
        if let Some(tab) = self.tabs.get_mut(self.active_tab_idx) {
            if push_history {
                if let Some(ref root) = self.root_box {
                    let (w, h) = (
                        self.width as f32,
                        (self.height.saturating_sub(HEADER_HEIGHT as u32)) as f32,
                    );
                    let snapshot = mango_layout::LayoutSnapshot::from_layout_box(
                        root,
                        mango_core::Size::new(w, h),
                    );
                    tab.store_bfcache_snapshot(Box::new(snapshot));
                }
                tab.push_history(url.clone(), self.page_title.clone());
                self.scroll_y = 0.0;
                self.scroll_y_f64 = 0.0;
            } else {
                tab.url = url.clone();
                tab.set_title(self.page_title.clone());
            }
        }

        self.cached_document = None;
        self.root_box = None;
        self.page_display_list = DisplayList::new();
        self.display_list_cache.lock().unwrap().clear();
        self.layer_tree.clear_page_tiles();
        mango_render::prune_image_caches();
        mango_render::prune_web_fonts();
        mango_js::JsRuntime::force_gc();

        // ── Initialize JavaScript Runtime ──
        self.js_runtime = None;
        let w = self.width as f32;
        let content_h = (self.height as f32 - HEADER_HEIGHT - STATUS_BAR_HEIGHT - 1.0).max(10.0);
        let session_storage = self
            .tabs
            .get(self.active_tab_idx)
            .map(|t| t.session_storage.clone())
            .unwrap_or_else(|| std::sync::Arc::new(std::sync::Mutex::new(HashMap::new())));
        let mut js_rt = JsRuntime::new_with_url_storage_session_and_cookies(
            doc,
            w,
            content_h,
            &url,
            self.local_storage.clone(),
            session_storage,
            self.cookie_jar.clone(),
        );

        // Load web fonts and imported stylesheets from embedded and cached stylesheets.
        // Stylesheet @imports are bounded (6 concurrent + timeout) and font
        // registration is fully asynchronous: the UI thread never blocks here.
        let embedded_sheets = mango_layout::extract_style_elements(&js_rt.document_ref());
        self.base_url = Url::parse(&url).ok();
        if !skip_embedded_sheets_and_fonts {
            self.cached_stylesheets.clear();
            if self.cached_stylesheets.is_empty()
                && let Some(ref base) = self.base_url.clone()
            {
                let mut visited = std::collections::HashSet::new();
                let mut pending_imports = Vec::new();
                for sheet in &embedded_sheets {
                    for rule in &sheet.rules {
                        if let mango_css::parser::Rule::Import(import_path) = rule {
                            pending_imports.push((base.clone(), import_path.clone()));
                        }
                    }
                }
                load_imported_stylesheets_parallel(
                    &self.loader,
                    pending_imports,
                    &mut visited,
                    &mut self.cached_stylesheets,
                );
            }
            load_web_fonts_async(
                self.loader.clone(),
                self.base_url.clone(),
                self.cached_stylesheets.clone(),
            );
            load_web_fonts_async(
                self.loader.clone(),
                self.base_url.clone(),
                embedded_sheets.clone(),
            );
        }

        // Inject extension stylesheets
        let (ext_css, ext_js) = self
            .extension_manager
            .get_content_scripts_for_url(&url, crate::extensions::RunAt::DocumentEnd);
        for css in ext_css {
            let sheet = mango_css::parser::parse_stylesheet(&css);
            self.cached_stylesheets.push(sheet);
        }

        // Reset the animation runtime and load the page's @keyframes definitions.
        // Author sheets win over embedded ones, matching the cascade order used
        // for style resolution.
        self.anim_state = mango_css::AnimationState::new();
        self.style_snapshot.clear();
        self.hovered_dom_nodes.clear();
        self.active_dom_nodes.clear();
        self.anim_state
            .animations
            .set_keyframes(mango_css::collect_keyframes(
                embedded_sheets.iter().chain(self.cached_stylesheets.iter()),
            ));

        // Pre-layout so the page renders immediately even if scripts take time.
        // CSS background images are queued from the style scan BEFORE the first
        // layout (no box tree needed) and fetched on a background worker, so
        // there is exactly ONE layout here and no middle blocking relayout.
        if let Some(ref base) = self.base_url.clone() {
            let mut bg_images = Vec::new();
            let dummy_doc = js_rt.document_ref();
            collect_stylesheet_images(&dummy_doc, &self.cached_stylesheets, &mut bg_images);
            collect_stylesheet_images(&dummy_doc, &embedded_sheets, &mut bg_images);
            drop(dummy_doc);
            let mut seen_bg = std::collections::HashSet::new();
            bg_images.retain(|u| seen_bg.insert(u.clone()));
            // Inline fast path only (data: URIs + memory cache). Remote URLs go
            // to the background backlog; never block the UI thread here.
            let mut remote_bg = Vec::new();
            for bg in bg_images {
                if bg.trim_start().starts_with("data:") {
                    if let Some(decoded) = decode_data_uri(&bg) {
                        cache_image(&bg, decoded);
                    }
                } else if get_cached_image(&bg).is_none() {
                    remote_bg.push(bg);
                }
            }
            if !remote_bg.is_empty() {
                self.queue_image_backlog(remote_bg, base.clone());
            }
        }
        self.relayout();

        // Extract and execute scripts in document order
        let scripts = if let Some(pre) = preloaded_scripts {
            pre
        } else {
            let mut s = Vec::new();
            let doc_ref = self.document_view();
            collect_scripts(&doc_ref, doc_ref.root(), &mut s);
            s
        };

        // Separate scripts into sync, async, and defer per HTML spec (§4.12.1)
        let mut sync_scripts = Vec::new();
        let mut async_scripts = Vec::new();
        let mut defer_scripts = Vec::new();

        for s in scripts {
            if s.is_defer() {
                defer_scripts.push(s);
            } else if s.is_async() {
                async_scripts.push(s);
            } else {
                sync_scripts.push(s);
            }
        }

        let mut external_count = 0;
        // Non-blocking external scripts on the UI thread: serve from the
        // memory cache only. Uncached remote scripts were already preloaded on
        // the background pipeline (`prepare_document_pipeline`); blocking the
        // UI on a network fetch here would freeze the page.
        let run_script = |s: ScriptToRun, rt: &mut mango_js::JsRuntime, ext_cnt: &mut usize| match s
        {
            ScriptToRun::Inline(code) => {
                if let Err(e) = rt.execute_script(&code) {
                    log::warn!("Inline script error: {e}");
                }
            }
            ScriptToRun::Preloaded { code, src, .. } => {
                if let Err(e) = rt.execute_script(&code) {
                    log::warn!("Preloaded script error ({src}): {e}");
                }
            }
            ScriptToRun::External { src, .. } => {
                const MAX_EXTERNAL_SCRIPTS: usize = 128;
                if *ext_cnt >= MAX_EXTERNAL_SCRIPTS {
                    return;
                }
                let Some(ref base) = self.base_url else {
                    return;
                };
                let Ok(resolved) = base.resolve(&src) else {
                    return;
                };
                let key = resolved.as_str();
                if let Some(cached) = self.loader.cache().get(&key) {
                    let script_code = String::from_utf8_lossy(&cached.body).into_owned();
                    *ext_cnt += 1;
                    if let Err(e) = rt.execute_script(&script_code) {
                        log::warn!("External script error ({src}): {e}");
                    }
                } else {
                    log::warn!(
                        "Skipping uncached external script on UI thread (preloaded on background): {}",
                        src
                    );
                }
            }
        };

        // 1. Run synchronous inline and external scripts in document order
        for s in sync_scripts {
            run_script(s, &mut js_rt, &mut external_count);
        }

        // 2. Run async external scripts as available
        for s in async_scripts {
            run_script(s, &mut js_rt, &mut external_count);
        }

        // 3. Run extension content scripts
        for js_code in ext_js {
            let _ = js_rt.execute_script(&js_code);
        }

        // 4. Run defer scripts in document order
        for s in defer_scripts {
            run_script(s, &mut js_rt, &mut external_count);
        }

        // 5. Dispatch DOMContentLoaded and window load events
        let _ = js_rt.dispatch_dom_content_loaded();
        let _ = js_rt.dispatch_load();

        if let Some(alert_msg) = js_rt.take_status_text() {
            self.status_text = alert_msg;
        }

        let redirect_url = js_rt.take_pending_navigation();

        self.js_runtime = Some(js_rt);

        // ── Autofocus on interactive form control if specified ──
        if !self.address_focused {
            fn find_autofocus(doc: &Document, nid: NodeId) -> Option<NodeId> {
                if let Some(node) = doc.get(nid) {
                    if let NodeData::Element(elem) = &node.data {
                        let tag = elem.tag_name.to_ascii_lowercase();
                        if (tag == "input"
                            || tag == "textarea"
                            || tag == "button"
                            || tag == "select")
                            && elem.get_attribute("autofocus").is_some()
                        {
                            return Some(nid);
                        }
                    }
                    for child in doc.children(nid) {
                        if let Some(found) = find_autofocus(doc, child.id) {
                            return Some(found);
                        }
                    }
                }
                None
            }
            let autofocus_nid = {
                let doc_snap = self.document_view();
                find_autofocus(&doc_snap, doc_snap.root())
            };
            if let Some(autofocus_nid) = autofocus_nid {
                self.focused_control = Some(autofocus_nid);
                let current_val = if let Some(ref rt) = self.js_runtime {
                    rt.get_node_attribute(autofocus_nid, "value")
                        .unwrap_or_default()
                } else {
                    String::new()
                };
                self.control_cursor_pos = current_val.chars().count();
                if let Some(ref mut rt) = self.js_runtime {
                    rt.set_node_attribute(autofocus_nid, "data-mango-focused", "true");
                    rt.set_node_attribute(
                        autofocus_nid,
                        "_mango_cursor_pos",
                        &self.control_cursor_pos.to_string(),
                    );
                }
            }
        }

        // Post-script relayout: only run if the JS runtime's DOM dirty bit was
        // set during script execution.  The pre-layout already ran above at line
        // ~1080; a second unconditional full layout wastes ~50ms on every page
        // load for pages with no script-driven DOM mutations.
        let needs_post_layout = self.js_runtime.as_ref().is_some_and(|rt| rt.is_dom_dirty());
        if needs_post_layout {
            self.relayout();
        }

        // If a script requested navigation (e.g. window.location.href = ...), follow it
        if let Some(redirect) = redirect_url
            && redirect != url
        {
            self.navigate(&redirect);
        }

        // If a script requested opening a window/tab (window.open)
        if let Some(open_req) = self
            .js_runtime
            .as_mut()
            .and_then(|rt| rt.take_pending_open_window())
        {
            let nav_url = if let Some(base) = &self.base_url {
                base.resolve(&open_req.url)
                    .map(|u| u.to_string())
                    .unwrap_or(open_req.url)
            } else {
                open_req.url
            };
            self.new_tab_and_navigate(&nav_url);
        }
    }

    /// Ticks the JavaScript event loop (executing setTimeout/setInterval).
    /// Navigates the current browser tab to the address in `self.address_text`.
    pub(crate) fn navigate_to_address(&mut self, push_history: bool) {
        self.is_all_selected = false;
        let mut input = self.address_text.trim().to_string();
        log::info!("Navigate to: {}", input);

        if input.starts_with('#')
            && let Some(base) = &self.base_url
            && let Ok(resolved) = base.resolve(&input)
        {
            input = resolved.to_string();
            self.address_text = input.clone();
        }

        if is_special_or_local_page(&input) {
            self.base_url = None;
            self.cached_stylesheets.clear();
            let html = resolve_navigation_input(&input);
            self.status_text = format!("Rendered: {}", input);
            self.load_html_internal(html, input, push_history);
            self.address_focused = false;
            return;
        }

        if is_search_query(&input) {
            let search_url = self.config.format_search_url(&input);
            log::info!(
                "Search query: routing to configured search engine: {}",
                search_url
            );
            input = search_url;
            self.address_text = input.clone();
        }

        let parsed_url = match Url::parse(&input) {
            Ok(url) => url,
            Err(_) if !input.contains("://") && !input.starts_with('/') => {
                let with_https = format!("https://{}", input);
                match Url::parse(&with_https) {
                    Ok(url) => {
                        input = with_https;
                        self.address_text = input.clone();
                        url
                    }
                    Err(e) => {
                        let err_msg = format!("Invalid URL: {}", e);
                        let err_html = error_page_html(&input, &err_msg);
                        self.base_url = None;
                        self.cached_stylesheets.clear();
                        self.status_text = err_msg;
                        self.load_html_internal(err_html, input, push_history);
                        self.address_focused = false;
                        return;
                    }
                }
            }
            Err(e) => {
                let err_msg = format!("Invalid URL: {}", e);
                let err_html = error_page_html(&input, &err_msg);
                self.base_url = None;
                self.cached_stylesheets.clear();
                self.status_text = err_msg;
                self.load_html_internal(err_html, input, push_history);
                self.address_focused = false;
                return;
            }
        };

        // Same-document fragment navigation: avoid full network re-fetch when only the #hash changes
        if let Some(base) = &self.base_url
            && parsed_url.scheme == base.scheme
            && parsed_url.host == base.host
            && parsed_url.port == base.port
            && parsed_url.path == base.path
            && parsed_url.query == base.query
        {
            if push_history && let Some(tab) = self.tabs.get_mut(self.active_tab_idx) {
                tab.push_history(base.to_string(), self.page_title.clone());
            }
            self.base_url = Some(parsed_url.clone());
            self.address_text = parsed_url.to_string();
            self.address_focused = false;
            self.is_loading = false;
            if let Some(ref frag) = parsed_url.fragment {
                let frag_str = frag.clone();
                if let Some(ref mut doc) = self.cached_document {
                    doc.set_target_id(Some(frag_str.clone()));
                }
                if let Some(ref mut rt) = self.js_runtime {
                    rt.set_target_id(Some(frag_str.clone()));
                }
                let doc = self.active_document();
                if let Some(node_id) = find_element_by_id(&doc, &frag_str)
                    && let Some(root) = &self.root_box
                    && let Some(lbox) = root.find_box_for_node(node_id)
                {
                    let target_y = lbox.dimensions.content.y();
                    let max_s = self.max_scroll();
                    self.scroll_y = target_y.clamp(0.0, max_s);
                    if let Some(tab) = self.tabs.get_mut(self.active_tab_idx) {
                        tab.scroll_y = self.scroll_y;
                    }
                }
            } else {
                if let Some(ref mut doc) = self.cached_document {
                    doc.set_target_id(None);
                }
                if let Some(ref mut rt) = self.js_runtime {
                    rt.set_target_id(None);
                }
            }
            self.relayout();
            return;
        }

        self.status_text = format!("Loading {}...", parsed_url);
        self.is_loading = true;
        self.address_focused = false;

        // P0 UI-freeze fix: ALL network navigations dispatch via the background
        // NavigationActor pool, even when `async_navigation == false`. The UI
        // thread returns immediately with a loading spinner; completion is
        // delivered through `tick_navigation()` (desktop event loop) or the
        // bounded `wait_for_navigation()` bridge (tests/headless). No
        // synchronous `fetch_document` ever runs on the UI thread here.
        self.nav_actor.navigate(self.active_tab_idx, parsed_url, push_history);
        if !self.async_navigation {
            self.wait_for_navigation();
        }
    }

    /// Polls background navigation. Returns true if a page navigation completed and UI needs redrawing.
    ///
    /// Drains the entire result queue and applies only the newest result whose
    /// epoch matches the active epoch; stale superseded results are discarded.
    pub fn tick_navigation(&mut self) -> bool {
        let mut batch: Vec<crate::navigation_actor::NavResult> = Vec::new();
        loop {
            match self.nav_actor.try_recv() {
                Ok(res) => {
                    batch.push(res);
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.is_loading = false;
                    break;
                }
            }
        }
        if batch.is_empty() {
            return false;
        }
        // Keep newest epoch only: select the drained result with the highest
        // epoch; anything older is a superseded navigation and is dropped.
        let active = self.nav_actor.active_epoch();
        let mut best_idx: Option<usize> = None;
        let mut best_epoch: u64 = 0;
        let mut best_is_final = false;
        for (i, res) in batch.iter().enumerate() {
            let (epoch, is_final) = match res {
                crate::navigation_actor::NavResult::DocumentReady { epoch, .. }
                | crate::navigation_actor::NavResult::Error { epoch, .. } => (*epoch, true),
                crate::navigation_actor::NavResult::ProgressiveReady { epoch, .. } => {
                    (*epoch, false)
                }
            };
            if epoch > best_epoch || (epoch == best_epoch && is_final && !best_is_final) {
                best_epoch = epoch;
                best_is_final = is_final;
                best_idx = Some(i);
            }
        }
        let Some(idx) = best_idx else {
            return false;
        };
        if best_epoch != active {
            // Newest drained result is still stale; a newer navigation is in
            // flight. Keep the spinner and wait for it.
            return false;
        }
        match batch.into_iter().nth(idx).expect("best index valid") {
            crate::navigation_actor::NavResult::DocumentReady {
                tab_id,
                prep,
                push_history,
                ..
            } => {
                self.is_loading = false;
                if tab_id == self.active_tab_idx {
                    self.apply_prepared_document(prep, push_history);
                    self.persist_cookies();
                    true
                } else if let Some(tab) = self.tabs.get_mut(tab_id) {
                    let final_url = prep.url.clone();
                    let page_title = extract_title(&prep.doc).unwrap_or_else(|| final_url.to_string());
                    tab.url = final_url.to_string();
                    tab.title = page_title.clone();
                    if push_history {
                        tab.push_history(final_url.to_string(), page_title);
                    }
                    tab.contents = Some(crate::tab::TabWebContents {
                        current_html: prep.html,
                        cached_document: Some(prep.doc),
                        base_url: Some(final_url),
                        cached_stylesheets: prep.cached_stylesheets,
                        root_box: None,
                        js_runtime: None,
                        style_snapshot: HashMap::new(),
                        focused_control: None,
                        page_display_list: DisplayList::new(),
                        display_list_cache: mango_layout::display_list::DisplayListCache::new(),
                        image_rx: None,
                        has_active_animations: false,
                        canvas_bg: mango_core::Color::WHITE,
                        engine: None,
                    });
                    self.persist_cookies();
                    false
                } else {
                    false
                }
            }
            crate::navigation_actor::NavResult::ProgressiveReady {
                tab_id,
                prep,
                ..
            } => {
                if tab_id == self.active_tab_idx {
                    self.apply_prepared_document(prep, false);
                    self.page_dirty = true;
                    true
                } else {
                    false
                }
            }
            crate::navigation_actor::NavResult::Error {
                tab_id,
                url,
                message,
                push_history,
                ..
            } => {
                self.is_loading = false;
                let target = url.to_string();
                let err_html = error_page_html(&target, &message);
                if tab_id == self.active_tab_idx {
                    self.base_url = None;
                    self.cached_stylesheets.clear();
                    self.status_text = format!("Error: {message}");
                    self.load_html_internal(err_html, target, push_history);
                    self.address_focused = false;
                    self.persist_cookies();
                    true
                } else if let Some(tab) = self.tabs.get_mut(tab_id) {
                    tab.url = target.clone();
                    tab.title = format!("Error: {message}");
                    if push_history {
                        tab.push_history(target, tab.title.clone());
                    }
                    tab.contents = Some(crate::tab::TabWebContents {
                        current_html: err_html,
                        cached_document: None,
                        base_url: None,
                        cached_stylesheets: Vec::new(),
                        root_box: None,
                        js_runtime: None,
                        style_snapshot: HashMap::new(),
                        focused_control: None,
                        page_display_list: DisplayList::new(),
                        display_list_cache: mango_layout::display_list::DisplayListCache::new(),
                        image_rx: None,
                        has_active_animations: false,
                        canvas_bg: mango_core::Color::WHITE,
                        engine: None,
                    });
                    self.persist_cookies();
                    false
                } else {
                    false
                }
            }
        }
    }

    /// Synchronously waits for any pending navigation to complete.
    ///
    /// Used by headless rendering and tests. Bounded by
    /// [`SYNC_NAVIGATION_TIMEOUT`]: on timeout an error page is shown instead
    /// of hanging forever. Stale superseded results are discarded while
    /// waiting for the active epoch.
    pub fn wait_for_navigation(&mut self) {
        if !self.is_loading {
            return;
        }
        let deadline = std::time::Instant::now() + SYNC_NAVIGATION_TIMEOUT;
        loop {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                self.is_loading = false;
                let target = self.address_text.clone();
                let err_html = error_page_html(&target, "Navigation timed out after 30s");
                self.base_url = None;
                self.cached_stylesheets.clear();
                self.status_text = "Error: Navigation timed out after 30s".to_string();
                self.load_html_internal(err_html, target, true);
                self.address_focused = false;
                self.persist_cookies();
                return;
            }
            // First drain anything already delivered without blocking.
            if self.tick_navigation() && !self.is_loading {
                return;
            }
            match self
                .nav_actor
                .recv_timeout(remaining.min(std::time::Duration::from_millis(200)))
            {
                Ok(crate::navigation_actor::NavResult::ProgressiveReady { tab_id, prep, epoch }) => {
                    if epoch == self.nav_actor.active_epoch() && tab_id == self.active_tab_idx {
                        self.apply_prepared_document(prep, false);
                        self.page_dirty = true;
                    }
                    continue;
                }
                Ok(crate::navigation_actor::NavResult::DocumentReady {
                    tab_id,
                    prep,
                    push_history,
                    epoch,
                }) => {
                    if epoch != self.nav_actor.active_epoch() {
                        continue;
                    }
                    self.is_loading = false;
                    if tab_id == self.active_tab_idx {
                        self.apply_prepared_document(prep, push_history);
                    }
                    self.persist_cookies();
                    return;
                }
                Ok(crate::navigation_actor::NavResult::Error {
                    tab_id,
                    url,
                    message,
                    push_history,
                    epoch,
                }) => {
                    if epoch != self.nav_actor.active_epoch() {
                        continue;
                    }
                    self.is_loading = false;
                    let err_html = error_page_html(&url.as_str(), &message);
                    if tab_id == self.active_tab_idx {
                        self.base_url = None;
                        self.cached_stylesheets.clear();
                        self.status_text = format!("Error: {}", message);
                        self.load_html_internal(err_html, url.to_string(), push_history);
                        self.address_focused = false;
                    }
                    self.persist_cookies();
                    return;
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    // Poll again until the deadline; tick_navigation above
                    // already applied anything that arrived.
                    continue;
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    self.is_loading = false;
                    return;
                }
            }
        }
    }

    /// Applies an asynchronously prepared document to the browser UI without freezing the event loop.
    pub fn apply_prepared_document(&mut self, prep: PreparedDocument, push_history: bool) {
        let final_url = prep.url.clone();
        self.base_url = Some(final_url.clone());
        self.cached_stylesheets = prep.cached_stylesheets;
        self.layer_tree.clear_page_tiles();
        self.display_list_cache.lock().unwrap().clear();
        self.page_display_list = DisplayList::new();
        self.root_box = None;
        mango_render::prune_image_caches();
        mango_render::prune_web_fonts();
        mango_render::clear_global_svg_symbols();

        // Web fonts are already fetched and registered on the background worker
        // thread in prepare_document_pipeline. Calling load_web_fonts here on the
        // main UI thread would block the UI with redundant network / lock work.

        // Queue images with epoch guarding, dedup, and memory-bounded cap.
        self.queue_image_backlog(prep.image_sources, final_url.clone());

        self.status_text = format!(
            "Loaded {} ({} bytes, HTTP {})",
            final_url,
            prep.html.len(),
            prep.status
        );
        self.devtools.record_network(
            final_url.to_string(),
            "GET",
            prep.status,
            prep.content_type.clone(),
            prep.html.len(),
        );
        self.devtools.log(
            crate::devtools::ConsoleLevel::Info,
            format!("Navigated to {}", final_url),
        );
        self.load_document_internal_with_scripts(
            prep.doc,
            prep.html,
            final_url.to_string(),
            push_history,
            true,
            Some(prep.preloaded_scripts),
        );
        self.address_focused = false;
    }

    /// Loads a fetched document into the browser, recursively fetching external stylesheets,
    /// web fonts, and images, and updating the page metadata and history.
    pub fn process_and_load_document(&mut self, doc_result: FetchedDocument, push_history: bool) {
        let prep = prepare_document_pipeline(&self.loader, doc_result);
        self.apply_prepared_document(prep, push_history);
    }

    /// Returns true if a navigation or page load is currently in progress.
    pub fn is_loading(&self) -> bool {
        self.is_loading
    }
}
