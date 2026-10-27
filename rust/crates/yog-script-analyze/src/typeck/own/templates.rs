//! A method's message template parameter (`[StructuredMessageTemplate(
//! nameof(message))]`), read off its declaration (`typeck/templates.rs`).

use yog_script_ids::system::STRUCTURED_MESSAGE_TEMPLATE;

use super::threads::{attributes, text};
use crate::ast::Decl;

/// The parameter `d` marks a message template, if it marks one.
pub(super) fn template(d: &Decl) -> Option<String> {
    let a = attributes(d, STRUCTURED_MESSAGE_TEMPLATE).next()?;
    text(&a.args.first()?.value.kind)
}
