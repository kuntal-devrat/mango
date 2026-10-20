//! Mathematical expression AST, calculation types, and thread-safe interned store.

pub mod calc;
pub mod expr;
pub mod store;

pub use calc::{AnchorSide, CalcLength};
pub use expr::{MathExpr, MathId, RoundStrategy};
pub use store::{
    clear_math_store, eval_math, get_interned_anchor_name, get_math, intern_anchor_name,
    intern_math, make_math_length,
};
