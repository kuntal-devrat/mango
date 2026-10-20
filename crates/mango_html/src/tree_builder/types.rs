//! Core types and data structures for the HTML5 tree builder.

use crate::dom::{Document, NodeId};
use crate::tokenizer::Tokenizer;

/// An entry in the WHATWG list of active formatting elements.
#[derive(Debug, Clone, PartialEq)]
pub enum FormattingEntry {
    Element {
        tag_name: String,
        attributes: Vec<(String, String)>,
        node_id: NodeId,
    },
    Marker,
}

/// HTML tree builder insertion mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InsertionMode {
    Initial,
    BeforeHtml,
    BeforeHead,
    InHead,
    AfterHead,
    InBody,
    InTable,
    InTableText,
    InCaption,
    InColumnGroup,
    InTableBody,
    InRow,
    InCell,
    InSelect,
    InSelectInTable,
    InTemplate,
    InFrameset,
    AfterFrameset,
    AfterBody,
    AfterAfterBody,
}

/// The HTML tree builder state machine.
pub struct TreeBuilder<'a> {
    pub(crate) tokenizer: Tokenizer<'a>,
    pub(crate) doc: Document,
    pub(crate) mode: InsertionMode,
    /// Stack of open elements (NodeIds in the document).
    pub(crate) open_elements: Vec<NodeId>,
    /// List of active formatting elements (WHATWG HTML5 §13.2.4.3).
    pub(crate) active_formatting_elements: Vec<FormattingEntry>,
    /// Pointer to the `<head>` element node, if created.
    pub(crate) head_element: Option<NodeId>,
    /// Buffer for accumulating adjacent character tokens.
    pub(crate) text_buffer: String,
    /// Stack of template insertion modes (WHATWG HTML5 §13.2.4.2).
    pub(crate) template_insertion_modes: Vec<InsertionMode>,
    /// Original insertion mode (e.g. for text / select).
    #[allow(dead_code)]
    pub(crate) original_insertion_mode: Option<InsertionMode>,
    /// Flag indicating whether foster parenting is currently active.
    pub(crate) foster_parenting: bool,
    /// Frameset-ok flag (§13.2.4.1). Initialized to true, set to false on non-whitespace text / elements.
    pub(crate) frameset_ok: bool,
}
