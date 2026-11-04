//! SVG path parser delegating directly to `usvg` and `tiny-skia`.

use tiny_skia::Path;

/// Parses an SVG path data string `d` into a `tiny_skia::Path` using `usvg`.
pub fn parse_svg_path(d: &str) -> Option<Path> {
    if d.trim().is_empty() {
        return None;
    }
    let svg = format!(r#"<svg xmlns="http://www.w3.org/2000/svg"><path d="{d}"/></svg>"#);
    let opt = usvg::Options::default();
    let tree = usvg::Tree::from_str(&svg, &opt).ok()?;
    for node in tree.root().children() {
        if let usvg::Node::Path(path) = node {
            return Some((*path.data()).clone());
        }
    }
    None
}
