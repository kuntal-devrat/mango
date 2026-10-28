//! Document preparation pipeline: background parsing, parallel stylesheet downloading, and script harvesting.

use mango_css::{Stylesheet, parse_stylesheet};
use mango_html::dom::{Document, NodeData, NodeId};
use mango_net::{FetchedDocument, ResourceLoader, Url};

use super::super::types::{
    MAX_CONCURRENT_FETCHES, PreparedDocument, SUBRESOURCE_FETCH_TIMEOUT, ScriptToRun,
};
use super::super::welcome::preprocess_wikipedia_appearance_html;
use super::resources::{collect_image_sources, load_web_fonts};
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
    // Progressive navigation owns only the already-parsed document snapshot.
    // Do not parse the complete external stylesheet graph twice: the final
    // preparation below will fetch and parse every stylesheet exactly once.
    // This is deferral, not stylesheet filtering; the final document still
    // receives the complete author stylesheet set.
    if !is_progressive && !should_cancel() {
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
            if let mango_css::parser::Rule::Import(import) = rule {
                import_links.push(import.url.clone());
            }
        }
    }
    if !is_progressive && !import_links.is_empty() && !should_cancel() {
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
    const SCRIPT_FETCH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
    let mut to_fetch: Vec<(usize, String, Option<String>)> = Vec::new();
    if !is_progressive {
        for (idx, s) in raw_scripts.iter().enumerate() {
            if let ScriptToRun::External { src, nonce, .. } = s {
                // Discovery is complete and independent of execution policy.
                // The bounded fetch pool limits concurrency; this queue must
                // not silently drop scripts from large applications.
                to_fetch.push((idx, src.clone(), nonce.clone()));
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
                crate::browser::types::spawn_fetch_task(move || {
                    let res = loader.fetch_script_with_nonce(&base, &src, nonce.as_deref());
                    if let Err(ref err) = res {
                        log::warn!("Background fetch_script error for {src}: {err}");
                    }
                    let _ = tx.send((idx, res));
                });
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

    // Prefetch transitive ES module imports for any fetched modules
    if !should_cancel() {
        for (idx, res) in fetched_scripts.iter() {
            if let Ok(code) = res {
                if let Some(s) = raw_scripts.get(*idx) {
                    if s.is_module() {
                        let module_base = if let Some(src) = s.src() {
                            final_url.resolve(src).unwrap_or_else(|_| final_url.clone())
                        } else {
                            final_url.clone()
                        };
                        for import_path in scan_es_module_imports(code) {
                            if let Ok(resolved) = module_base.resolve(&import_path) {
                                let key = resolved.as_str();
                                if loader.cache().get(&key).is_none() {
                                    let _ = loader.fetch_script_with_nonce(
                                        &module_base,
                                        &import_path,
                                        None,
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let mut preloaded_scripts = Vec::with_capacity(raw_scripts.len());
    for (idx, s) in raw_scripts.into_iter().enumerate() {
        match s {
            ScriptToRun::Inline {
                node_id,
                code,
                is_module,
            } => preloaded_scripts.push(ScriptToRun::Inline {
                node_id,
                code,
                is_module,
            }),
            ScriptToRun::External {
                node_id,
                src,
                is_defer,
                is_async,
                is_module,
                nonce,
            } => {
                if let Some(Ok(code)) = fetched_scripts.remove(&idx) {
                    preloaded_scripts.push(ScriptToRun::Preloaded {
                        node_id,
                        src,
                        code,
                        is_defer,
                        is_async,
                        is_module,
                        nonce,
                    });
                } else {
                    preloaded_scripts.push(ScriptToRun::External {
                        node_id,
                        src,
                        is_defer,
                        is_async,
                        is_module,
                        nonce,
                    });
                }
            }
            ScriptToRun::Preloaded {
                node_id,
                src,
                code,
                is_defer,
                is_async,
                is_module,
                nonce,
            } => {
                preloaded_scripts.push(ScriptToRun::Preloaded {
                    node_id,
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

    // CSS background URLs are resolved lazily after layout/visibility is known.
    // Harvesting every URL in every stylesheet eagerly decodes off-screen
    // theme textures and can dominate memory on modern sites.
    let mut image_sources = doc_images;

    let mut seen = std::collections::HashSet::new();
    let mut keep = Vec::with_capacity(image_sources.len());
    for src in &image_sources {
        keep.push(seen.insert(src.as_str()));
    }
    drop(seen);
    let mut iter = keep.into_iter();
    image_sources.retain(|_| iter.next().unwrap_or(false));

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
                        node_id: Some(node_id),
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
                    scripts.push(ScriptToRun::Inline {
                        node_id: Some(node_id),
                        code,
                        is_module,
                    });
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
            crate::browser::types::spawn_fetch_task(move || {
                let res = loader.fetch_stylesheet(&base, &href);
                let _ = tx.send((idx, res));
            });
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
                    let mut sheet = parse_stylesheet(&css_text);
                    resolve_font_faces_in_rules(&mut sheet.rules, &resolved);
                    for rule in &sheet.rules {
                        if let mango_css::parser::Rule::Import(import) = rule {
                            import_rules.push((resolved.clone(), import.url.clone()));
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
    while !pending_imports.is_empty() && !should_cancel() {
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
                crate::browser::types::spawn_fetch_task(move || {
                    let res = loader.fetch_stylesheet(&base, &href);
                    let _ = tx.send((idx, resolved, res));
                });
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
                    let mut sheet = parse_stylesheet(&css_text);
                    resolve_font_faces_in_rules(&mut sheet.rules, &resolved);
                    for rule in &sheet.rules {
                        if let mango_css::parser::Rule::Import(import) = rule {
                            pending_imports.push((resolved.clone(), import.url.clone()));
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

fn scan_es_module_imports(source: &str) -> Vec<String> {
    let mut imports = Vec::new();
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("import") || trimmed.starts_with("export") {
            if let Some(pos) = trimmed.find("from") {
                let rest = trimmed[pos + 4..].trim();
                if let Some(q) = rest.chars().next() {
                    if q == '\'' || q == '"' {
                        if let Some(end) = rest[1..].find(q) {
                            let path = &rest[1..1 + end];
                            if !path.is_empty() {
                                imports.push(path.to_string());
                            }
                        }
                    }
                }
            } else if let Some(rest) = trimmed.strip_prefix("import") {
                let rest = rest.trim();
                if let Some(q) = rest.chars().next() {
                    if q == '\'' || q == '"' {
                        if let Some(end) = rest[1..].find(q) {
                            let path = &rest[1..1 + end];
                            if !path.is_empty() {
                                imports.push(path.to_string());
                            }
                        }
                    }
                }
            }
        }
    }
    imports
}
