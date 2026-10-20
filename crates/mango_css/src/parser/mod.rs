pub mod declarations;
pub mod rules;
pub mod selectors;
pub mod stream;
pub mod types;

#[cfg(test)]
mod tests;

pub use declarations::animation::is_transition_animation_property;
pub use stream::{
    CssParser, parse_declaration_list, parse_math_length_from_str, parse_selectors,
    parse_stylesheet,
};
pub use types::*;
