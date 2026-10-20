//! WHATWG HTML5 token dispatch and insertion modes.

pub mod foreign;
pub mod in_body;
pub mod in_select;
pub mod in_table;
pub mod in_template;
pub mod initial;

use crate::tokenizer::Token;
use crate::tree_builder::types::{InsertionMode, TreeBuilder};

impl<'a> TreeBuilder<'a> {
    pub(crate) fn process_token(&mut self, token: Token) {
        if self.is_current_node_foreign() && self.process_token_foreign(token.clone()) {
            return;
        }

        match self.mode {
            InsertionMode::Initial => self.process_token_initial(token),
            InsertionMode::BeforeHtml => self.process_token_before_html(token),
            InsertionMode::BeforeHead => self.process_token_before_head(token),
            InsertionMode::InHead => self.process_token_in_head(token),
            InsertionMode::AfterHead => self.process_token_after_head(token),
            InsertionMode::InBody => self.process_token_in_body(token),
            InsertionMode::InTable => self.process_token_in_table(token),
            InsertionMode::InTableText => self.process_token_in_table(token),
            InsertionMode::InCaption => self.process_token_in_caption(token),
            InsertionMode::InColumnGroup => self.process_token_in_column_group(token),
            InsertionMode::InTableBody => self.process_token_in_table_body(token),
            InsertionMode::InRow => self.process_token_in_row(token),
            InsertionMode::InCell => self.process_token_in_cell(token),
            InsertionMode::InSelect => self.process_token_in_select(token),
            InsertionMode::InSelectInTable => self.process_token_in_select_in_table(token),
            InsertionMode::InTemplate => self.process_token_in_template(token),
            InsertionMode::InFrameset => self.process_token_in_frameset(token),
            InsertionMode::AfterFrameset => self.process_token_after_frameset(token),
            InsertionMode::AfterBody => self.process_token_after_body(token),
            InsertionMode::AfterAfterBody => self.process_token_after_after_body(token),
        }
    }
}
