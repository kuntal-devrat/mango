//! Foreign content handling (SVG and MathML integration in HTML5).

use crate::dom::NodeData;
use crate::tokenizer::Token;
use crate::tree_builder::types::TreeBuilder;

impl<'a> TreeBuilder<'a> {
    pub(crate) fn is_current_node_foreign(&self) -> bool {
        if let Some(&node_id) = self.open_elements.last()
            && let Some(node) = self.doc.get(node_id)
            && let NodeData::Element(el) = &node.data
        {
            if el.tag_name.eq_ignore_ascii_case("foreignObject") {
                return false;
            }
            return self.is_inside_foreign_content();
        }
        false
    }

    pub(crate) fn is_inside_foreign_content(&self) -> bool {
        for &id in self.open_elements.iter().rev() {
            if let Some(node) = self.doc.get(id)
                && let NodeData::Element(el) = &node.data
            {
                if el.tag_name.eq_ignore_ascii_case("foreignObject") {
                    return false;
                }
                if el.tag_name.eq_ignore_ascii_case("svg")
                    || el.tag_name.eq_ignore_ascii_case("math")
                {
                    return true;
                }
                if matches!(el.tag_name.as_str(), "html" | "body" | "table") {
                    return false;
                }
            }
        }
        false
    }

    pub(crate) fn process_token_foreign(&mut self, token: Token) -> bool {
        match token {
            Token::Character(c) => {
                self.text_buffer.push(c);
                true
            }
            Token::Comment(c) => {
                self.insert_comment(&c);
                true
            }
            Token::StartTag {
                name,
                mut attributes,
                self_closing,
            } => {
                if matches!(
                    name.to_ascii_lowercase().as_str(),
                    "b" | "big"
                        | "blockquote"
                        | "body"
                        | "br"
                        | "center"
                        | "code"
                        | "dd"
                        | "div"
                        | "dl"
                        | "dt"
                        | "em"
                        | "embed"
                        | "h1"
                        | "h2"
                        | "h3"
                        | "h4"
                        | "h5"
                        | "h6"
                        | "head"
                        | "hr"
                        | "i"
                        | "img"
                        | "li"
                        | "menu"
                        | "meta"
                        | "nobr"
                        | "ol"
                        | "p"
                        | "pre"
                        | "ruby"
                        | "s"
                        | "small"
                        | "span"
                        | "strong"
                        | "strike"
                        | "sub"
                        | "sup"
                        | "table"
                        | "tt"
                        | "u"
                        | "ul"
                        | "var"
                ) {
                    while let Some(&top_id) = self.open_elements.last() {
                        if let Some(node) = self.doc.get(top_id)
                            && let NodeData::Element(el) = &node.data
                            && (el.tag_name.eq_ignore_ascii_case("svg")
                                || el.tag_name.eq_ignore_ascii_case("math"))
                        {
                            self.open_elements.pop();
                            break;
                        }
                        self.open_elements.pop();
                    }
                    return false;
                }

                let adjusted_name = adjust_svg_tag_name(&name);
                adjust_svg_attributes(&mut attributes);
                let elem = self.insert_element(&adjusted_name, attributes);
                if !self_closing {
                    self.open_elements.push(elem);
                }
                true
            }
            Token::EndTag { name } => {
                let name_lower = name.to_ascii_lowercase();
                let mut found = false;
                for &id in self.open_elements.iter().rev() {
                    if let Some(node) = self.doc.get(id)
                        && let NodeData::Element(el) = &node.data
                    {
                        if el.tag_name.eq_ignore_ascii_case(&name_lower) {
                            found = true;
                            break;
                        }
                        if el.tag_name.eq_ignore_ascii_case("svg")
                            || el.tag_name.eq_ignore_ascii_case("math")
                        {
                            break;
                        }
                    }
                }
                if found {
                    while let Some(top_id) = self.open_elements.pop() {
                        if let Some(node) = self.doc.get(top_id)
                            && let NodeData::Element(el) = &node.data
                            && el.tag_name.eq_ignore_ascii_case(&name_lower)
                        {
                            break;
                        }
                    }
                    if name_lower == "svg" || name_lower == "math" {
                        self.reset_insertion_mode();
                    }
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    }
}

/// Adjusts SVG tag name case per WHATWG HTML §13.2.6.2.
pub(crate) fn adjust_svg_tag_name(name: &str) -> String {
    match name.to_ascii_lowercase().as_str() {
        "altglyph" => "altGlyph".to_string(),
        "altglyphdef" => "altGlyphDef".to_string(),
        "altglyphitem" => "altGlyphItem".to_string(),
        "animatecolor" => "animateColor".to_string(),
        "animatemotion" => "animateMotion".to_string(),
        "animatetransform" => "animateTransform".to_string(),
        "clippath" => "clipPath".to_string(),
        "feblend" => "feBlend".to_string(),
        "fecolormatrix" => "feColorMatrix".to_string(),
        "fecomponenttransfer" => "feComponentTransfer".to_string(),
        "fecomposite" => "feComposite".to_string(),
        "feconvolvematrix" => "feConvolveMatrix".to_string(),
        "fediffuselighting" => "feDiffuseLighting".to_string(),
        "fedisplacementmap" => "feDisplacementMap".to_string(),
        "fedistantlight" => "feDistantLight".to_string(),
        "fedropshadow" => "feDropShadow".to_string(),
        "feflood" => "feFlood".to_string(),
        "fefunca" => "feFuncA".to_string(),
        "fefuncb" => "feFuncB".to_string(),
        "fefuncg" => "feFuncG".to_string(),
        "fefuncr" => "feFuncR".to_string(),
        "fegaussianblur" => "feGaussianBlur".to_string(),
        "feimage" => "feImage".to_string(),
        "femerge" => "feMerge".to_string(),
        "femergenode" => "feMergeNode".to_string(),
        "femorphology" => "feMorphology".to_string(),
        "feoffset" => "feOffset".to_string(),
        "fepointlight" => "fePointLight".to_string(),
        "fespecularlighting" => "feSpecularLighting".to_string(),
        "fespotlight" => "feSpotLight".to_string(),
        "fetile" => "feTile".to_string(),
        "feturbulence" => "feTurbulence".to_string(),
        "foreignobject" => "foreignObject".to_string(),
        "glyphref" => "glyphRef".to_string(),
        "lineargradient" => "linearGradient".to_string(),
        "radialgradient" => "radialGradient".to_string(),
        "textpath" => "textPath".to_string(),
        _ => name.to_string(),
    }
}

/// Adjusts SVG attribute name case per WHATWG HTML §13.2.6.2.
pub(crate) fn adjust_svg_attributes(attributes: &mut [(String, String)]) {
    for (key, _) in attributes.iter_mut() {
        let replacement = match key.to_ascii_lowercase().as_str() {
            "attributename" => "attributeName",
            "attributetype" => "attributeType",
            "basefrequency" => "baseFrequency",
            "baseprofile" => "baseProfile",
            "calcmode" => "calcMode",
            "clippathunits" => "clipPathUnits",
            "diffuseconstant" => "diffuseConstant",
            "edgemode" => "edgeMode",
            "filterunits" => "filterUnits",
            "glyphref" => "glyphRef",
            "gradienttransform" => "gradientTransform",
            "gradientunits" => "gradientUnits",
            "kernelmatrix" => "kernelMatrix",
            "kernelunitlength" => "kernelUnitLength",
            "keypoints" => "keyPoints",
            "keysplines" => "keySplines",
            "keytimes" => "keyTimes",
            "lengthadjust" => "lengthAdjust",
            "limitingconeangle" => "limitingConeAngle",
            "markerheight" => "markerHeight",
            "markerunits" => "markerUnits",
            "markerwidth" => "markerWidth",
            "maskcontentunits" => "maskContentUnits",
            "maskunits" => "maskUnits",
            "numoctaves" => "numOctaves",
            "pathlength" => "pathLength",
            "patterncontentunits" => "patternContentUnits",
            "patterntransform" => "patternTransform",
            "patternunits" => "patternUnits",
            "pointsatx" => "pointsAtX",
            "pointsaty" => "pointsAtY",
            "pointsatz" => "pointsAtZ",
            "preservealpha" => "preserveAlpha",
            "preserveaspectratio" => "preserveAspectRatio",
            "primitiveunits" => "primitiveUnits",
            "refx" => "refX",
            "refy" => "refY",
            "repeatcount" => "repeatCount",
            "repeatdur" => "repeatDur",
            "requiredextensions" => "requiredExtensions",
            "requiredfeatures" => "requiredFeatures",
            "specularconstant" => "specularConstant",
            "specularexponent" => "specularExponent",
            "spreadmethod" => "spreadMethod",
            "startoffset" => "startOffset",
            "stddeviation" => "stdDeviation",
            "stitchtiles" => "stitchTiles",
            "surfacescale" => "surfaceScale",
            "systemlanguage" => "systemLanguage",
            "tablevalues" => "tableValues",
            "targetx" => "targetX",
            "targety" => "targetY",
            "textlength" => "textLength",
            "viewbox" => "viewBox",
            "viewtarget" => "viewTarget",
            "xchannelselector" => "xChannelSelector",
            "ychannelselector" => "yChannelSelector",
            "zoomandpan" => "zoomAndPan",
            _ => continue,
        };
        *key = replacement.to_string();
    }
}
