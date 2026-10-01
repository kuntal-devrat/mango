//! HTML element classification and metadata according to WHATWG specification.

/// Returns `true` if the given tag is a void element.
///
/// Void elements must not have any content and can never have a closing tag:
/// `area`, `base`, `br`, `col`, `embed`, `hr`, `img`, `input`, `link`, `meta`,
/// `param`, `source`, `track`, `wbr`.
pub fn is_void_element(tag: &str) -> bool {
    matches!(
        tag.to_ascii_lowercase().as_str(),
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
            | "frame"
    )
}

/// Returns `true` if the tag content is treated as raw text (no tag parsing inside).
///
/// Per WHATWG HTML5 §13.2.6.4.7:
/// Example: `<script>`, `<style>`, `<iframe>`, `<noembed>`, `<noframes>`, `<xmp>`, `<plaintext>`.
pub fn is_raw_text_element(tag: &str) -> bool {
    matches!(
        tag.to_ascii_lowercase().as_str(),
        "script" | "style" | "iframe" | "noembed" | "noframes" | "xmp" | "plaintext"
    )
}

/// Returns `true` if the tag content is escapable raw text (entities are decoded, but no child tags).
///
/// Example: `<textarea>`, `<title>`.
pub fn is_escapable_raw_text_element(tag: &str) -> bool {
    matches!(tag.to_ascii_lowercase().as_str(), "textarea" | "title")
}

/// Returns `true` if the element is an active formatting element under the WHATWG Adoption Agency Algorithm.
///
/// Per WHATWG HTML5 §13.2.4.3:
/// `a`, `b`, `big`, `code`, `em`, `font`, `i`, `nobr`, `s`, `small`, `strike`, `strong`, `tt`, `u`.
pub fn is_active_formatting_element(tag: &str) -> bool {
    matches!(
        tag.to_ascii_lowercase().as_str(),
        "a" | "b"
            | "big"
            | "code"
            | "em"
            | "font"
            | "i"
            | "nobr"
            | "s"
            | "small"
            | "strike"
            | "strong"
            | "tt"
            | "u"
    )
}

/// Returns `true` if the element is an inline formatting element.
pub fn is_formatting_element(tag: &str) -> bool {
    matches!(
        tag.to_ascii_lowercase().as_str(),
        "a" | "b"
            | "big"
            | "code"
            | "em"
            | "font"
            | "i"
            | "nobr"
            | "s"
            | "small"
            | "strike"
            | "strong"
            | "tt"
            | "u"
            | "mark"
            | "span"
    )
}

/// Returns `true` if the element is in the WHATWG "special" category.
///
/// Special elements close preceding implied elements (e.g. `<p>`) and alter
/// tree building insertion modes.
pub fn is_special_element(tag: &str) -> bool {
    matches!(
        tag.to_ascii_lowercase().as_str(),
        "address"
            | "applet"
            | "area"
            | "article"
            | "aside"
            | "base"
            | "basefont"
            | "bgsound"
            | "blockquote"
            | "body"
            | "br"
            | "button"
            | "caption"
            | "center"
            | "col"
            | "colgroup"
            | "dd"
            | "details"
            | "dialog"
            | "dir"
            | "div"
            | "dl"
            | "dt"
            | "embed"
            | "fieldset"
            | "figcaption"
            | "figure"
            | "footer"
            | "form"
            | "frame"
            | "frameset"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "head"
            | "header"
            | "hgroup"
            | "hr"
            | "html"
            | "iframe"
            | "img"
            | "input"
            | "li"
            | "link"
            | "listing"
            | "main"
            | "marquee"
            | "menu"
            | "meta"
            | "nav"
            | "noembed"
            | "noframes"
            | "noscript"
            | "object"
            | "ol"
            | "optgroup"
            | "option"
            | "p"
            | "param"
            | "plaintext"
            | "pre"
            | "script"
            | "section"
            | "select"
            | "source"
            | "style"
            | "summary"
            | "table"
            | "tbody"
            | "td"
            | "template"
            | "textarea"
            | "tfoot"
            | "th"
            | "thead"
            | "title"
            | "tr"
            | "track"
            | "ul"
            | "wbr"
            | "xmp"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_void_elements() {
        assert!(is_void_element("img"));
        assert!(is_void_element("BR"));
        assert!(is_void_element("input"));
        assert!(!is_void_element("div"));
        assert!(!is_void_element("span"));
    }

    #[test]
    fn test_raw_text_elements() {
        assert!(is_raw_text_element("script"));
        assert!(is_raw_text_element("STYLE"));
        assert!(is_raw_text_element("iframe"));
        assert!(is_raw_text_element("IFRAME"));
        assert!(!is_raw_text_element("p"));
    }
}
