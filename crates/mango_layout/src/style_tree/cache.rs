//! Owns a page stylesheet set and the index borrowing it.
//!
//! `RuleIndex` intentionally borrows parsed stylesheet rules. `PageStyleIndex`
//! keeps both objects together so the browser can reuse the index across
//! relayouts without leaking stylesheet memory or rebuilding the index every
//! frame.

use mango_css::{MediaEnvironment, Origin, RuleIndex, Stylesheet};
use self_cell::self_cell;

self_cell! {
    pub struct PageStyleIndex {
        owner: Vec<Stylesheet>,

        #[covariant]
        dependent: RuleIndex,
    }
}

impl PageStyleIndex {
    pub fn from_stylesheets(stylesheets: Vec<Stylesheet>, env: MediaEnvironment) -> Self {
        Self::new(stylesheets, move |sheets| {
            let refs: Vec<&Stylesheet> = sheets.iter().collect();
            let mut order = 1000;
            RuleIndex::from_stylesheets_with_env(&refs, Origin::Author, &mut order, env)
        })
    }

    pub fn index(&self) -> &RuleIndex<'_> {
        self.borrow_dependent()
    }

    pub fn stylesheet_count(&self) -> usize {
        self.borrow_owner().len()
    }

    pub fn indexed_entry_count(&self) -> usize {
        self.borrow_dependent().indexed_entry_count()
    }
}
