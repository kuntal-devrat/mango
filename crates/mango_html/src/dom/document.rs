//! DOM Document root, arena management, node factory, and shadow root registry.

use std::collections::{HashMap, HashSet};

use mango_core::{Arena, InternedString, StringInterner};

use super::node::{DomError, ElementData, Node, NodeData, NodeId, QuirksMode};

/// A complete DOM document containing all allocated nodes.
#[derive(Debug, Clone)]
pub struct Document {
    pub(crate) arena: Arena<Node>,
    pub(crate) root: NodeId,
    pub quirks_mode: QuirksMode,
    /// String interner for deduplicating tag names and attribute names.
    pub interner: StringInterner,
    /// Set of DOM nodes marked dirty due to attribute/class/DOM mutations.
    pub dirty_nodes: HashSet<NodeId>,
    /// The document's character encoding (canonical name e.g. "UTF-8", "windows-1252").
    pub character_set: String,
    /// The document's MIME type (e.g. "text/html").
    pub content_type: String,
    /// The document's active target fragment identifier for the :target selector.
    pub target_id: Option<String>,
    /// Mapping from shadow root fragment `NodeId` to host element `NodeId`.
    pub shadow_hosts: HashMap<NodeId, NodeId>,
    /// Monotonically increasing mutation version counter.
    pub version: u64,
}

impl Document {
    /// Creates a new empty document with a root `Document` node and pre-populated string interner.
    pub fn new() -> Self {
        let mut arena = Arena::new();
        // Allocate root placeholder
        let root = arena.alloc_with(|id| Node {
            id,
            parent: None,
            first_child: None,
            last_child: None,
            prev_sibling: None,
            next_sibling: None,
            data: NodeData::Document,
        });

        Self {
            arena,
            root,
            quirks_mode: QuirksMode::NoQuirks,
            interner: StringInterner::with_common_strings(),
            dirty_nodes: HashSet::new(),
            character_set: "UTF-8".to_string(),
            content_type: "text/html".to_string(),
            target_id: None,
            shadow_hosts: HashMap::new(),
            version: 0,
        }
    }

    /// Sets or clears the active target fragment identifier for the `:target` CSS pseudo-class.
    pub fn set_target_id(&mut self, target: Option<String>) {
        self.target_id = target;
    }

    /// Marks a DOM node as dirty for incremental style recalculation.
    pub fn mark_dirty(&mut self, node_id: NodeId) {
        self.dirty_nodes.insert(node_id);
        self.version = self.version.wrapping_add(1);
    }

    /// Returns a reference to the set of dirty nodes.
    pub fn dirty_nodes(&self) -> &HashSet<NodeId> {
        &self.dirty_nodes
    }

    /// Returns `true` if any nodes are currently marked dirty.
    pub fn has_dirty_nodes(&self) -> bool {
        !self.dirty_nodes.is_empty()
    }

    /// Takes the set of dirty nodes, clearing the internal tracker.
    pub fn take_dirty_nodes(&mut self) -> HashSet<NodeId> {
        std::mem::take(&mut self.dirty_nodes)
    }

    /// Clears all dirty flags.
    pub fn clear_dirty(&mut self) {
        self.dirty_nodes.clear();
    }

    /// Sets an attribute on an element, automatically marking the node dirty.
    pub fn set_attribute(&mut self, node_id: NodeId, name: &str, value: &str) {
        if let Some(node) = self.arena.get_mut(node_id)
            && let NodeData::Element(elem) = &mut node.data
        {
            if let Some(pos) = elem
                .attributes
                .iter()
                .position(|(k, _)| k.eq_ignore_ascii_case(name))
            {
                elem.attributes[pos].1 = value.to_string();
            } else {
                elem.attributes
                    .push((name.to_ascii_lowercase(), value.to_string()));
            }
            self.dirty_nodes.insert(node_id);
            self.version = self.version.wrapping_add(1);
        }
    }

    /// Removes an attribute from an element, marking the node dirty.
    pub fn remove_attribute(&mut self, node_id: NodeId, name: &str) {
        if let Some(node) = self.arena.get_mut(node_id)
            && let NodeData::Element(elem) = &mut node.data
        {
            elem.attributes
                .retain(|(k, _)| !k.eq_ignore_ascii_case(name));
            self.dirty_nodes.insert(node_id);
            self.version = self.version.wrapping_add(1);
        }
    }

    /// Adds a CSS class to an element if not already present, marking the node dirty.
    pub fn add_class(&mut self, node_id: NodeId, class_name: &str) {
        if let Some(node) = self.arena.get_mut(node_id)
            && let NodeData::Element(elem) = &mut node.data
        {
            let existing = elem.get_attribute("class").unwrap_or("").to_string();
            let classes: Vec<&str> = existing.split_ascii_whitespace().collect();
            if !classes.contains(&class_name) {
                let new_class = if existing.is_empty() {
                    class_name.to_string()
                } else {
                    format!("{existing} {class_name}")
                };
                if let Some(pos) = elem
                    .attributes
                    .iter()
                    .position(|(k, _)| k.eq_ignore_ascii_case("class"))
                {
                    elem.attributes[pos].1 = new_class;
                } else {
                    elem.attributes.push(("class".to_string(), new_class));
                }
                self.dirty_nodes.insert(node_id);
                self.version = self.version.wrapping_add(1);
            }
        }
    }

    /// Removes a CSS class from an element, marking the node dirty.
    pub fn remove_class(&mut self, node_id: NodeId, class_name: &str) {
        if let Some(node) = self.arena.get_mut(node_id)
            && let NodeData::Element(elem) = &mut node.data
            && let Some(class_val) = elem.get_attribute("class")
        {
            let updated: Vec<&str> = class_val
                .split_ascii_whitespace()
                .filter(|&c| c != class_name)
                .collect();
            let new_class = updated.join(" ");
            if let Some(pos) = elem
                .attributes
                .iter()
                .position(|(k, _)| k.eq_ignore_ascii_case("class"))
            {
                elem.attributes[pos].1 = new_class;
            }
            self.dirty_nodes.insert(node_id);
            self.version = self.version.wrapping_add(1);
        }
    }

    /// Toggles a CSS class on an element, returning `true` if added, `false` if removed.
    pub fn toggle_class(&mut self, node_id: NodeId, class_name: &str) -> bool {
        let has = if let Some(node) = self.get(node_id) {
            if let NodeData::Element(elem) = &node.data {
                elem.has_class(class_name)
            } else {
                false
            }
        } else {
            false
        };

        if has {
            self.remove_class(node_id, class_name);
            false
        } else {
            self.add_class(node_id, class_name);
            true
        }
    }

    /// Interns a string into the document's string interner.
    pub fn intern(&mut self, s: &str) -> InternedString {
        self.interner.intern(s)
    }

    /// Resolves an interned handle back to its string slice.
    pub fn resolve_atom(&self, atom: InternedString) -> &str {
        self.interner.resolve(atom)
    }

    /// Returns the root document node ID.
    pub fn root(&self) -> NodeId {
        self.root
    }

    /// Gets a reference to a node by its ID.
    pub fn get(&self, id: NodeId) -> Option<&Node> {
        self.arena.get(id)
    }

    /// Gets a mutable reference to a node by its ID.
    pub fn get_mut(&mut self, id: NodeId) -> Option<&mut Node> {
        self.arena.get_mut(id)
    }

    /// Creates an Element node in the document arena, automatically interning
    /// its tag name and attribute keys into the document's interner.
    ///
    /// Tag names and attribute keys are stored lowercased per HTML spec
    /// (foreign SVG/MathML case adjustments are re-applied so `viewBox`,
    /// `linearGradient`, etc. keep their canonical casing).
    pub fn create_element(&mut self, tag_name: &str, attributes: Vec<(String, String)>) -> NodeId {
        let lower_tag = tag_name.to_ascii_lowercase();
        // Re-apply SVG tag-name adjustment so foreign elements keep canonical case.
        let stored_tag = match lower_tag.as_str() {
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
            _ => lower_tag.clone(),
        };
        let tag_atom = self.interner.intern(&stored_tag.to_ascii_lowercase());
        // Lowercase attribute keys, re-applying SVG attribute adjustments.
        let mut lowered_attrs: Vec<(String, String)> = Vec::with_capacity(attributes.len());
        for (k, v) in attributes {
            let lk = k.to_ascii_lowercase();
            let stored_k = match lk.as_str() {
                "attributename" => "attributeName".to_string(),
                "attributetype" => "attributeType".to_string(),
                "basefrequency" => "baseFrequency".to_string(),
                "baseprofile" => "baseProfile".to_string(),
                "calcmode" => "calcMode".to_string(),
                "clippathunits" => "clipPathUnits".to_string(),
                "diffuseconstant" => "diffuseConstant".to_string(),
                "edgemode" => "edgeMode".to_string(),
                "filterunits" => "filterUnits".to_string(),
                "glyphref" => "glyphRef".to_string(),
                "gradienttransform" => "gradientTransform".to_string(),
                "gradientunits" => "gradientUnits".to_string(),
                "kernelmatrix" => "kernelMatrix".to_string(),
                "kernelunitlength" => "kernelUnitLength".to_string(),
                "keypoints" => "keyPoints".to_string(),
                "keysplines" => "keySplines".to_string(),
                "keytimes" => "keyTimes".to_string(),
                "lengthadjust" => "lengthAdjust".to_string(),
                "limitingconeangle" => "limitingConeAngle".to_string(),
                "markerheight" => "markerHeight".to_string(),
                "markerunits" => "markerUnits".to_string(),
                "markerwidth" => "markerWidth".to_string(),
                "maskcontentunits" => "maskContentUnits".to_string(),
                "maskunits" => "maskUnits".to_string(),
                "numoctaves" => "numOctaves".to_string(),
                "pathlength" => "pathLength".to_string(),
                "patterncontentunits" => "patternContentUnits".to_string(),
                "patterntransform" => "patternTransform".to_string(),
                "patternunits" => "patternUnits".to_string(),
                "pointsatx" => "pointsAtX".to_string(),
                "pointsaty" => "pointsAtY".to_string(),
                "pointsatz" => "pointsAtZ".to_string(),
                "preservealpha" => "preserveAlpha".to_string(),
                "preserveaspectratio" => "preserveAspectRatio".to_string(),
                "primitiveunits" => "primitiveUnits".to_string(),
                "refx" => "refX".to_string(),
                "refy" => "refY".to_string(),
                "repeatcount" => "repeatCount".to_string(),
                "repeatdur" => "repeatDur".to_string(),
                "requiredextensions" => "requiredExtensions".to_string(),
                "requiredfeatures" => "requiredFeatures".to_string(),
                "specularconstant" => "specularConstant".to_string(),
                "specularexponent" => "specularExponent".to_string(),
                "spreadmethod" => "spreadMethod".to_string(),
                "startoffset" => "startOffset".to_string(),
                "stddeviation" => "stdDeviation".to_string(),
                "stitchtiles" => "stitchTiles".to_string(),
                "surfacescale" => "surfaceScale".to_string(),
                "systemlanguage" => "systemLanguage".to_string(),
                "tablevalues" => "tableValues".to_string(),
                "targetx" => "targetX".to_string(),
                "targety" => "targetY".to_string(),
                "textlength" => "textLength".to_string(),
                "viewbox" => "viewBox".to_string(),
                "viewtarget" => "viewTarget".to_string(),
                "xchannelselector" => "xChannelSelector".to_string(),
                "ychannelselector" => "yChannelSelector".to_string(),
                "zoomandpan" => "zoomAndPan".to_string(),
                _ => lk.clone(),
            };
            self.interner.intern(&lk);
            lowered_attrs.push((stored_k, v));
        }
        let template_contents = if stored_tag.to_ascii_lowercase() == "template" {
            Some(self.create_fragment())
        } else {
            None
        };
        self.arena.alloc_with(|id| Node {
            id,
            parent: None,
            first_child: None,
            last_child: None,
            prev_sibling: None,
            next_sibling: None,
            data: NodeData::Element(ElementData {
                tag_name: stored_tag,
                tag_atom: Some(tag_atom),
                attributes: lowered_attrs,
                shadow_root: None,
                shadow_root_mode: None,
                shadow_root_clonable: false,
                shadow_root_delegates_focus: false,
                template_contents,
                is_hovered: false,
                is_active: false,
            }),
        })
    }

    /// Creates a `DocumentFragment` node in the document arena.
    pub fn create_fragment(&mut self) -> NodeId {
        self.arena.alloc_with(|id| Node {
            id,
            parent: None,
            first_child: None,
            last_child: None,
            prev_sibling: None,
            next_sibling: None,
            data: NodeData::DocumentFragment,
        })
    }

    /// Returns the template content `DocumentFragment` for a `<template>` node, if any.
    pub fn template_contents(&self, node_id: NodeId) -> Option<NodeId> {
        if let Some(node) = self.get(node_id)
            && let NodeData::Element(el) = &node.data
        {
            return el.template_contents;
        }
        None
    }

    /// Gets or creates a template content `DocumentFragment` for a `<template>` element.
    pub fn get_or_create_template_contents(&mut self, node_id: NodeId) -> NodeId {
        if let Some(contents) = self.template_contents(node_id) {
            return contents;
        }
        let frag = self.create_fragment();
        if let Some(node) = self.get_mut(node_id)
            && let NodeData::Element(el) = &mut node.data
        {
            el.template_contents = Some(frag);
        }
        frag
    }

    /// Sets the template content `DocumentFragment` for a `<template>` element.
    pub fn set_template_contents(&mut self, node_id: NodeId, contents: NodeId) {
        if let Some(node) = self.get_mut(node_id)
            && let NodeData::Element(el) = &mut node.data
        {
            el.template_contents = Some(contents);
        }
    }

    /// Returns `true` when the node is a `DocumentFragment`.
    pub fn is_document_fragment(&self, node_id: NodeId) -> bool {
        matches!(
            self.get(node_id).map(|n| &n.data),
            Some(NodeData::DocumentFragment)
        )
    }

    /// Attaches a shadow root (as a DocumentFragment) to an element node.
    ///
    /// Returns `None` (error) if the host already has a shadow root,
    /// is not an element, or is unknown — it never overwrites an existing root.
    pub fn attach_shadow(&mut self, host_id: NodeId) -> Option<NodeId> {
        self.attach_shadow_with_options(host_id, "open", false, false)
    }

    /// Attaches a shadow root with mode ("open" | "closed") and options flags.
    pub fn attach_shadow_with_options(
        &mut self,
        host_id: NodeId,
        mode: &str,
        clonable: bool,
        delegates_focus: bool,
    ) -> Option<NodeId> {
        // Check host exists and is an element without an existing shadow root.
        match self.get(host_id) {
            None => return None,
            Some(n) => match &n.data {
                NodeData::Element(el) => {
                    if el.shadow_root.is_some() {
                        return None;
                    }
                }
                _ => return None,
            },
        }
        let shadow_fragment = self.create_fragment();
        if let Some(host_node) = self.get_mut(host_id)
            && let NodeData::Element(el) = &mut host_node.data
        {
            // Re-check under mutable borrow (no overwrite on race).
            if el.shadow_root.is_some() {
                return None;
            }
            let actual_host_id = host_node.id;
            el.shadow_root = Some(shadow_fragment);
            el.shadow_root_mode = Some(mode.to_ascii_lowercase());
            el.shadow_root_clonable = clonable;
            el.shadow_root_delegates_focus = delegates_focus;
            self.shadow_hosts.insert(shadow_fragment, actual_host_id);
            self.dirty_nodes.insert(actual_host_id);
            self.version = self.version.wrapping_add(1);
            return Some(shadow_fragment);
        }
        None
    }

    /// Result-based variant returning [`DomError`] on failure.
    pub fn try_attach_shadow(&mut self, host_id: NodeId) -> Result<NodeId, DomError> {
        match self.get(host_id) {
            None => Err(DomError::NotFoundError),
            Some(n) => match &n.data {
                NodeData::Element(el) if el.shadow_root.is_some() => {
                    Err(DomError::HierarchyRequestError)
                }
                NodeData::Element(_) => Ok(self
                    .attach_shadow(host_id)
                    .ok_or(DomError::HierarchyRequestError)?),
                _ => Err(DomError::HierarchyRequestError),
            },
        }
    }

    /// Returns the shadow root attached to an element node, if any.
    pub fn get_shadow_root(&self, host_id: NodeId) -> Option<NodeId> {
        if let Some(node) = self.get(host_id)
            && let NodeData::Element(el) = &node.data
        {
            return el.shadow_root;
        }
        None
    }

    /// Returns the shadow root mode attached to an element node, if any.
    pub fn get_shadow_root_mode(&self, host_id: NodeId) -> Option<&str> {
        if let Some(node) = self.get(host_id)
            && let NodeData::Element(el) = &node.data
        {
            return el.shadow_root_mode.as_deref();
        }
        None
    }

    /// Returns the host element for a given shadow root fragment ID, if any.
    pub fn get_host_for_shadow_root(&self, sr_id: NodeId) -> Option<NodeId> {
        self.shadow_hosts.get(&sr_id).copied().or_else(|| {
            self.shadow_hosts.iter().find_map(|(sr, host)| {
                if sr.raw() == sr_id.raw() {
                    Some(*host)
                } else {
                    None
                }
            })
        })
    }

    /// Finds the nearest enclosing shadow root fragment ID for a given node, if it is inside a shadow tree.
    pub fn get_enclosing_shadow_root(&self, mut curr: NodeId) -> Option<NodeId> {
        while let Some(parent) = self.get(curr).and_then(|n| n.parent) {
            curr = parent;
        }
        if self.is_document_fragment(curr) && self.shadow_hosts.contains_key(&curr) {
            Some(curr)
        } else {
            None
        }
    }

    /// Returns `true` if the node resides within a shadow DOM tree.
    pub fn is_in_shadow_tree(&self, node_id: NodeId) -> bool {
        self.get_enclosing_shadow_root(node_id).is_some()
    }

    /// Creates a Text node in the document arena.
    pub fn create_text(&mut self, text: &str) -> NodeId {
        self.arena.alloc_with(|id| Node {
            id,
            parent: None,
            first_child: None,
            last_child: None,
            prev_sibling: None,
            next_sibling: None,
            data: NodeData::Text(text.to_string()),
        })
    }

    /// Creates a Comment node in the document arena.
    pub fn create_comment(&mut self, comment: &str) -> NodeId {
        self.arena.alloc_with(|id| Node {
            id,
            parent: None,
            first_child: None,
            last_child: None,
            prev_sibling: None,
            next_sibling: None,
            data: NodeData::Comment(comment.to_string()),
        })
    }

    /// Creates a DocumentType node in the document arena.
    pub fn create_doctype(&mut self, name: &str, public_id: &str, system_id: &str) -> NodeId {
        self.arena.alloc_with(|id| Node {
            id,
            parent: None,
            first_child: None,
            last_child: None,
            prev_sibling: None,
            next_sibling: None,
            data: NodeData::DocumentType {
                name: name.to_string(),
                public_id: public_id.to_string(),
                system_id: system_id.to_string(),
            },
        })
    }
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}
