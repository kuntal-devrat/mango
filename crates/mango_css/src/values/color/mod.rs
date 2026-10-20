pub mod named;
pub mod parse;
pub mod relative;
pub mod spaces;

pub use named::parse_named_color;
pub use parse::parse_color;
pub use spaces::*;
