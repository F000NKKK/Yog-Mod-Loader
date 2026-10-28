//! An item's: a tool or armor (not both), armor already worn, a hanging
//! thing's size, a durable item's stack.

use yog_data::{Diagnostic, Path, field};

use super::{Rules, error, warning};
use crate::content::{HangsProps, ItemFile, ItemProps};

impl Rules for ItemFile {
    fn rules(&self, out: &mut Vec<Diagnostic>) {
        if let Some(model) = &self.model {
            model.rules_at(Path::root().key(field!(ItemFile, model)), out);
        }
        if let Some(item) = &self.item {
            item.rules_at(Path::root().key(field!(ItemFile, item)), out);
        }
    }
}

impl ItemProps {
    /// Its rules, it being at `at`.
    pub(crate) fn rules_at(&self, at: Path, out: &mut Vec<Diagnostic>) {
        if self
            .hangs
            .as_ref()
            .and_then(|h| h.size)
            .is_some_and(|[w, h]| w == 0 || h == 0)
        {
            let size = at
                .key(field!(ItemProps, hangs))
                .key(field!(HangsProps, size));
            out.push(error(
                size,
                "a hanging thing is at least a block wide and high",
            ));
        }
        if self.wearable.is_some() && self.armor.is_some() {
            out.push(error(
                at.key(field!(ItemProps, wearable)),
                "armor is worn already — leave wearable out",
            ));
        }
        if self.tool.is_some() && self.armor.is_some() {
            out.push(error(
                at.key(field!(ItemProps, armor)),
                "an item is a tool or armor, not both",
            ));
        }
        if self.durability.is_some_and(|d| d > 0) && self.stack_size.is_some_and(|s| s > 1) {
            out.push(warning(
                at.key(field!(ItemProps, stack_size)),
                "ignored: an item with durability stacks to 1",
            ));
        }
    }
}
