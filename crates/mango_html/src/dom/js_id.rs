//! Generation-safe encoding of DOM node handles crossing the JS boundary.
//!
//! Internally a [`NodeId`] is an arena index plus a generation counter. If the
//! index were exposed to JavaScript directly, a stale handle kept by a page
//! would silently alias whatever node later reused that arena slot. Packing the
//! generation into the same `u32` prevents that: recycled slots receive a new
//! generation, so old JS handles resolve to `None` instead of the new node.
//!
//! Layout: `[ 20 bits index | 11 bits generation ]`.
//! - Up to 1,048,575 live slot positions per document.
//! - Up to 2,047 reuses per slot. After that the slot is retired rather than
//!   reused, keeping the generation field unambiguous.
//! - The packed value always fits in a positive `i32`, so legacy traversal ops
//!   that use `-1` as a null sentinel can keep their existing JS contract.

use super::NodeId;

/// Number of low bits reserved for the arena generation.
pub const JS_ID_GENERATION_BITS: u32 = 11;
/// Mask for the generation bits.
pub const JS_ID_GENERATION_MASK: u32 = (1 << JS_ID_GENERATION_BITS) - 1;
/// Maximum arena index representable in a JS handle. One bit is reserved so
/// the packed handle is always a positive `i32` (legacy ops use `-1` as null).
pub const JS_ID_MAX_INDEX: u32 = (1 << (31 - JS_ID_GENERATION_BITS)) - 1;
/// Maximum generation that can be handed to JavaScript.
pub const JS_ID_MAX_GENERATION: u32 = JS_ID_GENERATION_MASK;

/// Sentinel returned when a handle cannot be represented. It decodes to an
/// index beyond [`JS_ID_MAX_INDEX`], so every arena lookup fails closed. It is
/// also `-1` when passed through legacy `i32` null-sentinel ops.
const INVALID_JS_ID: u32 = u32::MAX;

/// Packs an arena node id into a generation-safe JavaScript handle.
///
/// Returns [`INVALID_JS_ID`] if the arena index or generation does not fit the
/// packed representation. This only happens for documents with more than
/// [`JS_ID_MAX_INDEX`] simultaneously allocated slot positions, or after a slot
/// has been retired at [`JS_ID_MAX_GENERATION`].
pub fn node_id_to_js(id: NodeId) -> u32 {
    let index = id.raw();
    let generation = id.generation();
    // The document root is represented by the legacy `0` sentinel so the JS
    // shim can return the singleton `document` object instead of wrapping the
    // root node as an element.
    if index == 0 {
        return 0;
    }
    if index > JS_ID_MAX_INDEX || generation == 0 || generation > JS_ID_MAX_GENERATION {
        log::warn!(
            "DOM handle cannot be represented in 31 bits (index={index}, generation={generation})"
        );
        return INVALID_JS_ID;
    }
    (index << JS_ID_GENERATION_BITS) | generation
}

/// Unpacks a JavaScript handle into an arena node id.
///
/// `0` is the legacy document/root sentinel used throughout the DOM shim.
/// It maps to a wildcard id for arena slot 0 (the document root), matching the
/// pre-packing behavior. Concrete handles are never `0` because generations
/// start at 1.
pub fn node_id_from_js(raw: u32) -> NodeId {
    NodeId::from_raw_parts(raw >> JS_ID_GENERATION_BITS, raw & JS_ID_GENERATION_MASK)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_index_and_generation() {
        for (index, generation) in [(1, 1), (42, 7), (JS_ID_MAX_INDEX, JS_ID_MAX_GENERATION)] {
            let id = NodeId::from_raw_parts(index, generation);
            let packed = node_id_to_js(id);
            assert_ne!(packed, 0);
            assert_eq!(node_id_from_js(packed), id);
        }
    }

    #[test]
    fn document_root_uses_the_zero_sentinel() {
        let root = NodeId::from_raw_parts(0, 1);
        assert_eq!(node_id_to_js(root), 0);
        assert_eq!(node_id_from_js(0), NodeId::from_raw(0));
    }

    #[test]
    fn zero_is_the_document_sentinel() {
        let id = node_id_from_js(0);
        assert_eq!(id.raw(), 0);
        assert_eq!(id.generation(), 0);
    }

    #[test]
    fn unrepresentable_id_fails_closed() {
        let id = NodeId::from_raw_parts(JS_ID_MAX_INDEX + 1, 1);
        let packed = node_id_to_js(id);
        assert_eq!(packed, INVALID_JS_ID);
        let decoded = node_id_from_js(packed);
        assert!(decoded.raw() > JS_ID_MAX_INDEX);
    }
}
