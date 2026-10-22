//! Document preparation pipeline: background parsing, parallel stylesheet downloading, and script harvesting.

use mango_css::{Stylesheet, parse_stylesheet};
use mango_html::dom::{Document, NodeData, NodeId};
use mango_net::{FetchedDocument, ResourceLoader, Url};

use super::super::types::{
    MAX_CONCURRENT_FETCHES, PreparedDocument, SUBRESOURCE_FETCH_TIMEOUT, ScriptToRun,
};
use super::super::welcome::preprocess_wikipedia_appearance_html;
use super::resources::{collect_image_sources, collect_stylesheet_images, load_web_fonts};
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
    collect_stylesheet_links(&doc, doc.root(), &final_url, &mut stylesheet_links);
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
        load_web_fonts(loader, Some(&final_url), &cached_stylesheets);
        load_web_fonts(loader, Some(&final_url), &inline_sheets);
    }

    let mut raw_scripts = Vec::new();
    collect_scripts(&doc, doc.root(), &mut raw_scripts);
    for item in &preload_scan.preloads {
        if item.kind == mango_net::PreloadKind::Script {
            let loader_clone = loader.clone();
            let base_clone = final_url.clone();
            let url_clone = item.url.clone();
            let _ = std::thread::Builder::new()
                .name("mango-preload-cache".to_string())
                .spawn(move || {
                    let _ = loader_clone.fetch_script(&base_clone, &url_clone);
                });
        }
    }

    const MAX_PRELOAD_SCRIPTS: usize = 64;
    const SCRIPT_FETCH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);
    let mut to_fetch: Vec<(usize, String, Option<String>)> = Vec::new();
    if !is_progressive {
        for (idx, s) in raw_scripts.iter().enumerate() {
            if let ScriptToRun::External { src, nonce, .. } = s {
                if to_fetch.len() < MAX_PRELOAD_SCRIPTS {
                    to_fetch.push((idx, src.clone(), nonce.clone()));
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
            for (idx, src, nonce) in chunk.iter() {
                let tx = tx.clone();
                let loader = loader.clone();
                let base = final_url.clone();
                let src = src.clone();
                let nonce = nonce.clone();
                let idx = *idx;
                if let Err(e) = std::thread::Builder::new()
                    .name("mango-script-fetch".to_string())
                    .spawn(move || {
                        let res = loader.fetch_script_with_nonce(&base, &src, nonce.as_deref());
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
                    }
                }
            }
        }
    }

    let mut preloaded_scripts = Vec::with_capacity(raw_scripts.len());
    for (idx, s) in raw_scripts.into_iter().enumerate() {
        match s {
            ScriptToRun::Inline { code, is_module } => {
                preloaded_scripts.push(ScriptToRun::Inline { code, is_module })
            }
            ScriptToRun::External {
                src,
                is_defer,
                is_async,
                is_module,
                nonce,
            } => {
                if let Some(Ok(code)) = fetched_scripts.remove(&idx) {
                    if is_module {
                        scan_and_precache_module_imports(loader, &final_url, &src, &code);
                    }
                    preloaded_scripts.push(ScriptToRun::Preloaded {
                        src,
                        code,
                        is_defer,
                        is_async,
                        is_module,
                        nonce,
                    });
                } else {
                    preloaded_scripts.push(ScriptToRun::External {
                        src,
                        is_defer,
                        is_async,
                        is_module,
                        nonce,
                    });
                }
            }
            ScriptToRun::Preloaded {
                src,
                code,
                is_defer,
                is_async,
                is_module,
                nonce,
            } => {
                preloaded_scripts.push(ScriptToRun::Preloaded {
                    src,
                    code,
                    is_defer,
                    is_async,
                    is_module,
                    nonce,
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
    // Prioritize visible content images (<img>) over background stylesheet textures
    image_sources.extend(doc_images);
    image_sources.extend(sheet_images);

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
                || t == "module"
        }
    }
}

pub(crate) fn collect_scripts(doc: &Document, root_id: NodeId, scripts: &mut Vec<ScriptToRun>) {
    let mut stack = vec![root_id];
    while let Some(node_id) = stack.pop() {
        if let Some(node) = doc.get(node_id)
            && let NodeData::Element(ref elem) = node.data
            && elem.tag_name.eq_ignore_ascii_case("script")
            && is_js_script_type(elem.get_attribute("type"))
        {
            let is_module = elem
                .get_attribute("type")
                .is_some_and(|t| t.trim().eq_ignore_ascii_case("module"));
            let is_defer = elem.get_attribute("defer").is_some() || is_module;
            let is_async = elem.get_attribute("async").is_some();
            let nonce = elem.get_attribute("nonce").map(|s| s.to_string());
            if let Some(src) = elem.get_attribute("src") {
                let trimmed = src.trim();
                if !trimmed.is_empty() {
                    scripts.push(ScriptToRun::External {
                        src: trimmed.to_string(),
                        is_defer,
                        is_async,
                        is_module,
                        nonce,
                    });
                }
            } else {
                let code = doc.text_content(node_id);
                if !code.trim().is_empty() {
                    scripts.push(ScriptToRun::Inline { code, is_module });
                }
            }
        }
        let mut kids: Vec<NodeId> = doc.children(node_id).map(|c| c.id).collect();
        kids.reverse();
        stack.extend(kids);
    }
}

fn collect_stylesheet_links(
    doc: &Document,
    root_id: NodeId,
    base_url: &Url,
    links: &mut Vec<String>,
) {
    let mut stack = vec![root_id];
    while let Some(node_id) = stack.pop() {
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
                    let resolved = base_url
                        .resolve(trimmed)
                        .map(|u| u.to_string())
                        .unwrap_or_else(|_| trimmed.to_string());
                    if !links.contains(&resolved) {
                        links.push(resolved);
                    }
                }
            }
        }
        let mut kids: Vec<NodeId> = doc.children(node_id).map(|c| c.id).collect();
        kids.reverse();
        stack.extend(kids);
    }
}

fn resolve_font_faces_in_rules(rules: &mut [mango_css::parser::Rule], base_url: &Url) {
    for rule in rules {
        match rule {
            mango_css::parser::Rule::FontFace(ff) => {
                if let Ok(abs) = base_url.resolve(&ff.src_url) {
                    ff.src_url = abs.to_string();
                }
            }
            mango_css::parser::Rule::Style(sr) => {
                for decl in &mut sr.declarations {
                    resolve_val_urls(&mut decl.value, base_url);
                }
            }
            mango_css::parser::Rule::Media(mr) => {
                for sr in &mut mr.rules {
                    for decl in &mut sr.declarations {
                        resolve_val_urls(&mut decl.value, base_url);
                    }
                }
            }
            mango_css::parser::Rule::Layer(l) => {
                resolve_font_faces_in_rules(&mut l.rules, base_url);
            }
            _ => {}
        }
    }
}

fn resolve_val_urls(val: &mut mango_css::values::Value, base_url: &Url) {
    match val {
        mango_css::values::Value::Url(u) => {
            if let Ok(abs) = base_url.resolve(u) {
                *u = abs.to_string();
            }
        }
        mango_css::values::Value::List(items) => {
            for item in items {
                resolve_val_urls(item, base_url);
            }
        }
        _ => {}
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
                    let res = loader.fetch_stylesheet(&base, &href);
                    let _ = tx.send((idx, res));
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
                    let mut sheet = parse_stylesheet(&css_text);
                    resolve_font_faces_in_rules(&mut sheet.rules, &resolved);
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

pub(crate) fn load_imported_stylesheets_parallel(
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
                        let res = loader.fetch_stylesheet(&base, &href);
                        let _ = tx.send((idx, resolved, res));
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
                    let mut sheet = parse_stylesheet(&css_text);
                    resolve_font_faces_in_rules(&mut sheet.rules, &resolved);
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

/// Scans the top lines of an ES module on a background worker thread for static imports
/// and asynchronously prefetches them into the shared resource cache.
fn scan_and_precache_module_imports(
    loader: &ResourceLoader,
    base_url: &Url,
    script_src: &str,
    code: &str,
) {
    let script_base = base_url
        .resolve(script_src)
        .unwrap_or_else(|_| base_url.clone());
    let mut import_specs = Vec::new();

    for line in code.lines().take(150) {
        let trimmed = line.trim();
        if !trimmed.starts_with("import") && !trimmed.starts_with("export") {
            continue;
        }
        if let Some(pos) = trimmed.find('\'') {
            if let Some(end) = trimmed[pos + 1..].find('\'') {
                let spec = &trimmed[pos + 1..pos + 1 + end];
                if spec.ends_with(".js")
                    || spec.starts_with("./")
                    || spec.starts_with("../")
                    || spec.starts_with("http")
                {
                    import_specs.push(spec.to_string());
                }
                continue;
            }
        }
        if let Some(pos) = trimmed.find('"') {
            if let Some(end) = trimmed[pos + 1..].find('"') {
                let spec = &trimmed[pos + 1..pos + 1 + end];
                if spec.ends_with(".js")
                    || spec.starts_with("./")
                    || spec.starts_with("../")
                    || spec.starts_with("http")
                {
                    import_specs.push(spec.to_string());
                }
            }
        }
    }

    for spec in import_specs {
        if let Ok(target) = script_base.resolve(&spec) {
            let target_str = target.to_string();
            if loader.cache().get(&target_str).is_none() {
                let loader_clone = loader.clone();
                let base_clone = script_base.clone();
                let _ = std::thread::Builder::new()
                    .name("mango-module-precache".to_string())
                    .spawn(move || {
                        let _ = loader_clone.fetch_script(&base_clone, &spec);
                    });
            }
        }
    }
}
