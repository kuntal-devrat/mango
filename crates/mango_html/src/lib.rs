//! # mango_html
//!
//! HTML5 tokenizer and tree builder. Parses raw HTML into a DOM tree.
//!
//! Implements the [WHATWG HTML parsing specification](https://html.spec.whatwg.org/multipage/parsing.html):
//! - **Tokenizer**: State machine that produces tokens (start tags, end tags, text, comments)
//! - **Tree Builder**: Consumes tokens and constructs the DOM tree with proper error recovery
//! - **DOM**: Arena-allocated node graph for performant memory management
//! - **Entities**: Character entity reference resolution

pub mod dom;
pub mod elements;
pub mod entities;
pub mod tokenizer;
pub mod tree_builder;

pub use dom::{ChildrenIter, Document, ElementData, Node, NodeData, NodeId, QuirksMode};
pub use entities::decode_entities;
pub use tokenizer::{Token, Tokenizer};
pub use tree_builder::{InsertionMode, TreeBuilder, parse_html};
