//! URL unwrapping, tracking redirect removal, and `<meta http-equiv="refresh">` handling.

use mango_html::dom::{Document, NodeData, NodeId};
use mango_net::Url;

/// Unwraps tracking redirects (e.g. DuckDuckGo `/l/?uddg=...` or Google `/url?q=...`)
/// into direct destination URLs so navigation does not get stalled by intermediate
/// empty redirect interstitials.
pub fn unwrap_tracking_redirect(url_str: &str) -> Option<String> {
    let parsed = Url::parse(url_str).ok()?;
    let host = parsed.host.as_deref().unwrap_or("");
    let query = parsed.query.as_deref().unwrap_or("");
    let is_ddg =
        (host.ends_with("duckduckgo.com") || host == "ddg.gg") && parsed.path.starts_with("/l/");
    let is_google = host.contains("google.") && parsed.path.starts_with("/url");

    if is_ddg || is_google {
        for param in query.split('&') {
            let val = if is_ddg {
                param.strip_prefix("uddg=")
            } else {
                param
                    .strip_prefix("q=")
                    .or_else(|| param.strip_prefix("url="))
            };
            if let Some(v) = val {
                let decoded = crate::browser::navigation::anchor::percent_decode(v);
                if decoded.starts_with("http://") || decoded.starts_with("https://") {
                    return Some(decoded);
                }
            }
        }
    }
    None
}

/// Checks for a `<meta http-equiv="refresh" content="...">` tag in the document.
/// Returns `Some((delay_seconds, target_url))` if present.
pub fn check_meta_refresh(doc: &Document, base_url: Option<&Url>) -> Option<(u64, String)> {
    fn search_meta(doc: &Document, node_id: NodeId) -> Option<(u64, String)> {
        let node = doc.get(node_id)?;
        if let NodeData::Element(elem) = &node.data
            && elem.tag_name.eq_ignore_ascii_case("meta")
            && elem
                .get_attribute("http-equiv")
                .is_some_and(|v| v.trim().eq_ignore_ascii_case("refresh"))
            && let Some(content) = elem.get_attribute("content")
        {
            let trimmed = content.trim();
            let (delay_str, url_part) = trimmed
                .split_once(';')
                .map(|(d, u)| (d.trim(), Some(u.trim())))
                .unwrap_or((trimmed, None));
            let delay = delay_str.parse::<u64>().unwrap_or(0);
            let mut target_url = String::new();
            if let Some(u) = url_part {
                let raw = match u.to_ascii_lowercase().find("url=") {
                    Some(idx) => &u[idx + 4..],
                    None => u,
                }
                .trim()
                .trim_matches(|c| matches!(c, '\'' | '"' | ' '));
                if !raw.is_empty() {
                    target_url = raw.to_string();
                }
            }
            return Some((delay, target_url));
        }
        for child in doc.children(node_id) {
            if let Some(found) = search_meta(doc, child.id) {
                return Some(found);
            }
        }
        None
    }

    let (delay, target) = search_meta(doc, doc.root())?;
    if target.is_empty() {
        return None;
    }
    let resolved = base_url
        .and_then(|base| base.resolve(&target).ok().map(|u| u.to_string()))
        .unwrap_or(target);
    Some((delay, resolved))
}
