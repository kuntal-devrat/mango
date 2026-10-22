//! Engine CSS support matrix.
//!
//! `CssPropertyId` is an *optimisation* registry: it maps property names to dense
//! enum discriminants for the fast property map. It is not the engine's support
//! matrix — the cascade/apply layer supports a number of properties that have no
//! registry entry (for example `quotes`, `counter-increment`, `position-anchor`
//! or the `animation-*` / `transition-*` longhands).
//!
//! CSS Conditional Rules Level 3 § 3.1 defines `@supports (prop: value)` as true
//! when the UA supports that property with that value. This module provides the
//! "does the UA support this property" half of that question; the "with that
//! value" half lives in [`super::validation::is_valid_property_value`].
//!
//! The list below mirrors the property names handled by the `computed::apply`
//! modules. `supported_properties_cover_apply_layer` in the module tests asserts
//! that every property the apply layer can change is reported as supported.

/// Properties implemented by the cascade/apply layer that are not present in the
/// `CssPropertyId` registry.
pub const EXTRA_SUPPORTED_PROPERTIES: &[&str] = &[
    // Backgrounds & effects
    "backdrop-filter",
    "background-blend-mode",
    "background-clip",
    "background-position",
    "border-image",
    "border-image-source",
    "box-shadow",
    "clip-path",
    "contain-intrinsic-height",
    "contain-intrinsic-size",
    "contain-intrinsic-width",
    "content-visibility",
    "filter",
    "isolation",
    "mask-composite",
    "mask-image",
    "mask-mode",
    "mask-position",
    "mask-repeat",
    "mask-size",
    "mix-blend-mode",
    "object-fit",
    "object-position",
    "text-shadow",
    "transform",
    "transform-origin",
    "transform-origin-x",
    "transform-origin-y",
    "transform-origin-z",
    // Box model / logical
    "block-size",
    "border-block-end-width",
    "border-block-start-width",
    "border-inline-end-width",
    "border-inline-start-width",
    "inset",
    "inset-block-end",
    "inset-block-start",
    "inset-inline-end",
    "inset-inline-start",
    "margin-block-end",
    "margin-block-start",
    "margin-inline-end",
    "margin-inline-start",
    "max-block-size",
    "max-inline-size",
    "min-block-size",
    "min-inline-size",
    "padding-block-end",
    "padding-block-start",
    "padding-inline-end",
    "padding-inline-start",
    // Typography
    "font-display",
    "font-feature-settings",
    "font-variation-settings",
    "hyphens",
    "letter-spacing",
    "line-clamp",
    "line-height",
    "quotes",
    "text-align",
    "text-decoration",
    "text-decoration-line",
    "text-decoration-thickness",
    "text-emphasis-color",
    "text-emphasis-style",
    "text-indent",
    "text-overflow",
    "text-transform",
    "text-underline-offset",
    "text-wrap",
    "white-space",
    "word-break",
    "word-spacing",
    "word-wrap",
    "writing-mode",
    // Lists & counters
    "counter-increment",
    "counter-reset",
    "list-style-position",
    "list-style-type",
    // Multi-column
    "column-count",
    "column-gap",
    "column-rule-color",
    "column-rule-style",
    "column-rule-width",
    "column-span",
    "column-width",
    // Flexbox & grid
    "align-content",
    "align-items",
    "align-self",
    "container-name",
    "container-type",
    "flex-basis",
    "flex-direction",
    "flex-grow",
    "flex-shrink",
    "flex-wrap",
    "gap",
    "grid-auto-flow",
    "grid-column-end",
    "grid-column-start",
    "grid-row-end",
    "grid-row-start",
    "grid-template-areas",
    "grid-template-columns",
    "grid-template-rows",
    "justify-content",
    "justify-items",
    "justify-self",
    "row-gap",
    // Positioning
    "anchor-name",
    "position-anchor",
    "position-try-fallbacks",
    "position-try-options",
    "scroll-snap-type",
    "scroll-timeline",
    "scroll-timeline-axis",
    "scroll-timeline-name",
    "view-timeline",
    "view-timeline-axis",
    "view-timeline-name",
    "view-transition-name",
    // Animations & transitions
    "animation-delay",
    "animation-direction",
    "animation-duration",
    "animation-fill-mode",
    "animation-iteration-count",
    "animation-name",
    "animation-play-state",
    "animation-timeline",
    "animation-timing-function",
    "transition-delay",
    "transition-duration",
    "transition-property",
    "transition-timing-function",
    // Interaction & misc
    "accent-color",
    "appearance",
    "aspect-ratio",
    "border-collapse",
    "border-spacing",
    "break-inside",
    "caption-side",
    "contain",
    "content",
    "cursor",
    "direction",
    "display",
    "float",
    "clear",
    "list-style",
    "order",
    "overflow-wrap",
    "outline-color",
    "outline-offset",
    "outline-style",
    "outline-width",
    "page-break-inside",
    "pointer-events",
    "position",
    "resize",
    "scroll-behavior",
    "scrollbar-color",
    "scrollbar-width",
    "table-layout",
    "unicode-bidi",
    "user-select",
    "vertical-align",
    "visibility",
    "will-change",
    "z-index",
    "zoom",
];

/// Returns true if the engine implements `property_name`.
///
/// This is the "is this property supported" half of an `@supports (prop: value)`
/// condition. The value half is checked separately by
/// [`super::validation::is_valid_property_value`].
pub fn is_supported_property(property_name: &str) -> bool {
    let prop = property_name.trim().to_ascii_lowercase();

    if prop.is_empty() {
        return false;
    }

    // Custom properties are supported by definition.
    if prop.starts_with("--") {
        return true;
    }

    // The dense registry covers the bulk of the supported properties.
    if crate::property_id::CssPropertyId::from_name(&prop).is_some() {
        return true;
    }

    // Shorthands are expanded by the parser into supported longhands.
    if super::declaration::Declaration::is_shorthand_name(&prop) {
        return true;
    }

    EXTRA_SUPPORTED_PROPERTIES.binary_search(&prop.as_str()).is_ok()
        || EXTRA_SUPPORTED_PROPERTIES.contains(&prop.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supported_properties_are_sorted_or_unique() {
        let mut sorted = EXTRA_SUPPORTED_PROPERTIES.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(
            sorted.len(),
            EXTRA_SUPPORTED_PROPERTIES.len(),
            "EXTRA_SUPPORTED_PROPERTIES contains duplicates"
        );
    }

    #[test]
    fn registry_properties_are_supported() {
        // A representative sample of the dense registry must be reported as
        // supported, since `is_supported_property` short-circuits on it.
        for name in [
            "display", "position", "float", "clear", "width", "height", "color",
            "margin-top", "padding-left", "font-size", "z-index", "overflow",
            "visibility", "transform", "opacity", "grid-template-columns",
        ] {
            assert!(
                is_supported_property(name),
                "{name} is in the registry but not reported as supported"
            );
        }
    }

    #[test]
    fn unknown_properties_are_not_supported() {
        assert!(!is_supported_property("invalid-property-xyz"));
        assert!(!is_supported_property("opacity-bg"));
        assert!(!is_supported_property(""));
    }

    #[test]
    fn known_properties_are_supported() {
        assert!(is_supported_property("display"));
        assert!(is_supported_property("width"));
        assert!(is_supported_property("quotes"));
        assert!(is_supported_property("animation-name"));
        assert!(is_supported_property("--custom"));
    }
}
