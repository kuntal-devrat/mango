//! Serialization and formatting of selector AST into CSS syntax.

use super::types::{AttributeOperator, Combinator, CompoundSelector, SimpleSelector};

/// Formats a compound selector and combinator tail into a standard CSS selector string.
pub fn format_selector(head: &CompoundSelector, tail: &[(Combinator, CompoundSelector)]) -> String {
    let mut s = String::new();
    format_compound(head, &mut s);
    for (comb, compound) in tail {
        match comb {
            Combinator::Descendant => s.push(' '),
            Combinator::Child => s.push_str(" > "),
            Combinator::NextSibling => s.push_str(" + "),
            Combinator::SubsequentSibling => s.push_str(" ~ "),
        }
        format_compound(compound, &mut s);
    }
    s
}

/// Formats a single compound selector into a CSS string.
pub fn format_compound(compound: &CompoundSelector, s: &mut String) {
    for sel in &compound.simple_selectors {
        match sel {
            SimpleSelector::Universal => s.push('*'),
            SimpleSelector::Type(tag) => s.push_str(tag),
            SimpleSelector::Class(cls) => {
                s.push('.');
                s.push_str(cls);
            }
            SimpleSelector::Id(id) => {
                s.push('#');
                s.push_str(id);
            }
            SimpleSelector::Attribute {
                name,
                op,
                value,
                case_insensitive,
            } => {
                s.push('[');
                s.push_str(name);
                match op {
                    AttributeOperator::Exists => {}
                    AttributeOperator::Exact => {
                        s.push_str("=\"");
                        s.push_str(value);
                        s.push('"');
                    }
                    AttributeOperator::Includes => {
                        s.push_str("~=\"");
                        s.push_str(value);
                        s.push('"');
                    }
                    AttributeOperator::Prefix => {
                        s.push_str("^=\"");
                        s.push_str(value);
                        s.push('"');
                    }
                    AttributeOperator::Suffix => {
                        s.push_str("$=\"");
                        s.push_str(value);
                        s.push('"');
                    }
                    AttributeOperator::Substring => {
                        s.push_str("*=\"");
                        s.push_str(value);
                        s.push('"');
                    }
                    AttributeOperator::DashMatch => {
                        s.push_str("|=\"");
                        s.push_str(value);
                        s.push('"');
                    }
                }
                if *case_insensitive {
                    s.push_str(" i");
                }
                s.push(']');
            }
            SimpleSelector::PseudoClass(p) => {
                s.push(':');
                s.push_str(p);
            }
            SimpleSelector::PseudoElement(p) => {
                s.push_str("::");
                s.push_str(p);
            }
            SimpleSelector::Nesting => s.push('&'),
        }
    }
}
