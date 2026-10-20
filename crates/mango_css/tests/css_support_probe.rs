//! Objective measurement of CSS property support.
//!
//! For each candidate property we compute the style of a probe element twice:
//!   1. with no stylesheet  -> `baseline`
//!   2. with a stylesheet declaring only that property -> `styled`
//!
//! If the two computed styles differ, the property reached the computed-value
//! stage and is therefore *supported*. If they are identical the declaration was
//! either dropped at parse time or ignored by the apply layer.

use mango_css::computed::compute_style;
use mango_css::parser::parse_stylesheet;
use mango_html::dom::Document;
use mango_html::parse_html;

/// (property, distinctive value) pairs. The value is chosen so that it is
/// distinguishable from the initial value of the property.
const CANDIDATES: &[(&str, &str)] = &[
    // --- Layout / box model ---
    ("display", "flex"),
    ("position", "absolute"),
    ("float", "left"),
    ("clear", "both"),
    ("box-sizing", "border-box"),
    ("width", "321px"),
    ("height", "123px"),
    ("min-width", "111px"),
    ("min-height", "112px"),
    ("max-width", "222px"),
    ("max-height", "223px"),
    ("margin-top", "11px"),
    ("margin-right", "12px"),
    ("margin-bottom", "13px"),
    ("margin-left", "14px"),
    ("padding-top", "21px"),
    ("padding-right", "22px"),
    ("padding-bottom", "23px"),
    ("padding-left", "24px"),
    ("border-top-width", "5px"),
    ("border-right-width", "6px"),
    ("border-bottom-width", "7px"),
    ("border-left-width", "8px"),
    ("border-top-color", "rgb(1, 2, 3)"),
    ("border-top-style", "dashed"),
    ("border-top-left-radius", "9px"),
    ("border-top-right-radius", "10px"),
    ("border-bottom-left-radius", "11px"),
    ("border-bottom-right-radius", "12px"),
    ("overflow", "hidden"),
    ("overflow-x", "scroll"),
    ("overflow-y", "auto"),
    ("visibility", "hidden"),
    ("opacity", "0.42"),
    ("z-index", "77"),
    ("vertical-align", "middle"),
    ("order", "3"),
    // --- Color / background ---
    ("color", "rgb(10, 20, 30)"),
    ("background-color", "rgb(40, 50, 60)"),
    ("background-image", "linear-gradient(red, blue)"),
    ("background-repeat", "no-repeat"),
    ("background-position", "center center"),
    ("background-size", "cover"),
    ("background-attachment", "fixed"),
    ("background-clip", "content-box"),
    ("background-origin", "padding-box"),
    ("opacity-bg", "0.5"),
    // --- Font / text ---
    ("font-family", "Comic Sans MS"),
    ("font-size", "23px"),
    ("font-style", "italic"),
    ("font-weight", "bold"),
    ("font-variant", "small-caps"),
    ("font-stretch", "condensed"),
    ("font-feature-settings", "\"liga\" 1"),
    ("font-variation-settings", "\"wght\" 400"),
    ("line-height", "2.5"),
    ("letter-spacing", "3px"),
    ("word-spacing", "4px"),
    ("text-align", "center"),
    ("text-align-last", "justify"),
    ("text-decoration", "underline"),
    ("text-decoration-line", "overline"),
    ("text-decoration-color", "rgb(7, 7, 7)"),
    ("text-decoration-style", "wavy"),
    ("text-decoration-thickness", "2px"),
    ("text-transform", "uppercase"),
    ("text-indent", "5px"),
    ("text-overflow", "ellipsis"),
    ("text-shadow", "1px 1px 2px rgb(0,0,0)"),
    ("text-wrap", "balance"),
    ("white-space", "pre"),
    ("word-break", "break-all"),
    ("overflow-wrap", "break-word"),
    ("hyphens", "auto"),
    ("tab-size", "8"),
    ("direction", "rtl"),
    ("unicode-bidi", "bidi-override"),
    ("writing-mode", "vertical-rl"),
    ("list-style-type", "square"),
    ("list-style-position", "inside"),
    ("list-style-image", "none"),
    ("quotes", "\"<\" \">\""),
    ("content", "\"x\""),
    // --- Flexbox ---
    ("flex-direction", "column-reverse"),
    ("flex-wrap", "wrap-reverse"),
    ("flex-grow", "3"),
    ("flex-shrink", "0"),
    ("flex-basis", "55px"),
    ("order-flex", "4"),
    ("align-items", "flex-end"),
    ("align-self", "center"),
    ("align-content", "space-around"),
    ("justify-content", "space-between"),
    ("justify-items", "center"),
    ("justify-self", "end"),
    ("gap", "7px 8px"),
    ("row-gap", "9px"),
    ("column-gap", "10px"),
    // --- Grid ---
    ("grid-template-columns", "1fr 2fr"),
    ("grid-template-rows", "auto 100px"),
    ("grid-template-areas", "\"a b\""),
    ("grid-auto-flow", "dense"),
    ("grid-auto-rows", "50px"),
    ("grid-auto-columns", "60px"),
    ("grid-column-start", "2"),
    ("grid-row-start", "3"),
    ("grid-area", "1 / 2 / 3 / 4"),
    ("place-items", "center"),
    ("place-content", "start"),
    // --- Multi-column ---
    ("column-count", "3"),
    ("column-width", "120px"),
    ("column-gap-mc", "11px"),
    ("column-rule", "1px solid red"),
    ("column-span", "all"),
    ("column-fill", "balance"),
    // --- Transform / effects ---
    ("transform", "translate(10px, 20px)"),
    ("transform-origin", "25% 75%"),
    ("transform-style", "preserve-3d"),
    ("perspective", "500px"),
    ("filter", "blur(2px)"),
    ("backdrop-filter", "blur(1px)"),
    ("mix-blend-mode", "multiply"),
    ("background-blend-mode", "overlay"),
    ("clip-path", "inset(10px)"),
    ("mask-image", "linear-gradient(red, blue)"),
    ("isolation", "isolate"),
    ("box-shadow", "2px 2px 4px rgb(0,0,0)"),
    ("outline", "2px solid green"),
    ("outline-offset", "3px"),
    ("outline-color", "rgb(9, 9, 9)"),
    ("outline-style", "dotted"),
    ("outline-width", "4px"),
    ("cursor", "pointer"),
    ("caret-color", "red"),
    ("accent-color", "red"),
    ("appearance", "none"),
    ("resize", "both"),
    ("user-select", "none"),
    ("pointer-events", "none"),
    ("touch-action", "none"),
    // --- Transitions / animations ---
    ("transition", "color 1s ease"),
    ("transition-property", "opacity"),
    ("transition-duration", "2s"),
    ("transition-delay", "1s"),
    ("transition-timing-function", "ease-in"),
    ("animation", "spin 1s linear infinite"),
    ("animation-name", "spin"),
    ("animation-duration", "3s"),
    ("animation-delay", "1s"),
    ("animation-iteration-count", "3"),
    ("animation-direction", "reverse"),
    ("animation-fill-mode", "both"),
    ("animation-play-state", "paused"),
    ("animation-timing-function", "ease-out"),
    // --- Fragmentation / misc ---
    ("break-before", "page"),
    ("break-after", "always"),
    ("break-inside", "avoid"),
    ("page-break-before", "always"),
    ("contain", "paint"),
    ("content-visibility", "auto"),
    ("container-type", "inline-size"),
    ("container-name", "card"),
    ("object-fit", "cover"),
    ("object-position", "25% 75%"),
    ("will-change", "transform"),
    ("aspect-ratio", "16 / 9"),
    ("inset", "1px 2px 3px 4px"),
    ("scroll-behavior", "smooth"),
    ("scroll-snap-type", "x mandatory"),
    ("scrollbar-width", "thin"),
    ("scrollbar-color", "red blue"),
    ("view-transition-name", "hero"),
    ("anchor-name", "--a"),
    ("position-try-fallbacks", "flip-block"),
    // --- Logical properties ---
    ("margin-inline-start", "12px"),
    ("padding-inline-end", "13px"),
    ("border-inline-start-width", "3px"),
    ("inset-inline-start", "4px"),
    ("inline-size", "15px"),
    ("block-size", "16px"),
    // --- CSS-wide keywords (functional check) ---
    ("color", "inherit"),
];

#[test]
fn probe_css_property_support() {
    let html = "<html><body><div id=\"probe\">x</div></body></html>";
    let doc: Document = parse_html(html);
    let root = doc.root();
    let probe = doc
        .find_element_by_id(root, "probe")
        .expect("probe element should exist");

    let baseline = compute_style(probe, &doc, &[], None);

    let mut supported: Vec<&str> = Vec::new();
    let mut unsupported: Vec<(&str, &str)> = Vec::new();

    for (prop, value) in CANDIDATES {
        let css = format!("#probe {{ {}: {}; }}", prop, value);
        let sheet = parse_stylesheet(&css);
        let styled = compute_style(probe, &doc, &[&sheet], None);
        if styled != baseline {
            supported.push(prop);
        } else {
            unsupported.push((prop, value));
        }
    }

    println!("=== CSS PROPERTY SUPPORT ===");
    println!("supported: {}/{}", supported.len(), CANDIDATES.len());
    println!(
        "coverage: {:.1}%",
        100.0 * supported.len() as f32 / CANDIDATES.len() as f32
    );
    println!("--- SUPPORTED ---");
    for p in &supported {
        println!("  {p}");
    }
    println!("--- NOT APPLIED ---");
    for (p, v) in &unsupported {
        println!("  {p}: {v}");
    }

    assert!(
        supported.len() >= 149,
        "CSS property support regressed: {} < 149",
        supported.len()
    );
}
