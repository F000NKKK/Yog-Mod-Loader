//! A model's: a template gives its shape, so not its own elements too.

use yog_data::{Diagnostic, Path, field};

use super::{Rules, error};
use crate::content::vocab::template;
use crate::content::{BlockFile, Connect, Model, States};

impl Rules for BlockFile {
    fn rules(&self, out: &mut Vec<Diagnostic>) {
        self.model
            .rules_at(Path::root().key(field!(BlockFile, model)), out);
        if let Some(m) = &self.item_model {
            m.rules_at(Path::root().key(field!(BlockFile, item_model)), out);
        }
        let Some(states) = &self.states else { return };
        let at = Path::root().key(field!(BlockFile, states));
        if let Some(c) = &states.connect {
            c.arm.rules_at(
                at.key(field!(States, connect)).key(field!(Connect, arm)),
                out,
            );
        }
        for (name, m) in &states.models.0 {
            m.rules_at(at.key(field!(States, models)).key(name), out);
        }
    }
}

impl Model {
    /// Its rules, it being at `at`.
    pub(crate) fn rules_at(&self, at: Path, out: &mut Vec<Diagnostic>) {
        if let (Some(t), Some(_)) = (self.parent.as_deref().and_then(template), &self.elements) {
            out.push(error(
                at.key(field!(Model, elements)),
                format!(
                    "elements replace the shape of parent {:?} — remove one of them",
                    t.name
                ),
            ));
        }
    }
}
