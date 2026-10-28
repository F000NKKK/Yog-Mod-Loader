//! A model's: a parent that's a template (of its kind: an item's sprites
//! are an item's) or a model id, the template's slots given, plain slot
//! names, a template's shape or its own elements (not both, not neither).

use yog_data::{Diagnostic, Path, field};
use yog_ids::model as m;

use super::{Rules, error, warning};
use crate::content::vocab::{TEMPLATES, Template, template};
use crate::content::{BlockFile, Connect, Model, SLOT_MARK, States};

/// What a model is for: a block's shape, or an item's (sprites allowed).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ModelOf {
    Block,
    Item,
}

impl Rules for BlockFile {
    fn rules(&self, out: &mut Vec<Diagnostic>) {
        let main = Path::root().key(field!(BlockFile, model));
        match self
            .kind
            .as_deref()
            .and_then(crate::content::BlockKind::built_in)
        {
            Some(kind) => super::kind::kind_model_rules(kind, &self.model, main, out),
            None => self.model.rules_at(main, ModelOf::Block, out),
        }
        if let Some(model) = &self.item_model {
            model.rules_at(
                Path::root().key(field!(BlockFile, item_model)),
                ModelOf::Item,
                out,
            );
        }
        let Some(states) = &self.states else { return };
        let at = Path::root().key(field!(BlockFile, states));
        states.rules_at(at.clone(), out);
        if let Some(c) = &states.connect {
            c.arm.rules_at(
                at.key(field!(States, connect)).key(field!(Connect, arm)),
                ModelOf::Block,
                out,
            );
        }
        for (name, model) in &states.models.0 {
            model.rules_at(
                at.key(field!(States, models)).key(name),
                ModelOf::Block,
                out,
            );
        }
    }
}

impl Model {
    /// Its rules, it being at `at`, a model `of` a block or an item.
    pub(crate) fn rules_at(&self, at: Path, of: ModelOf, out: &mut Vec<Diagnostic>) {
        for slot in self.textures.0.iter().map(|(s, _)| s) {
            if slot.is_empty() || slot.starts_with(SLOT_MARK) {
                out.push(error(
                    at.key(field!(Model, textures)).key(slot),
                    "slot names are plain words (`all`, `side`, `0`)",
                ));
            }
        }
        let parent = self.parent.as_deref();
        let tpl = parent.and_then(template);
        if let Some(name) = parent {
            parent_rules(name, tpl, at.key(field!(Model, parent)), of, out);
        }
        // A material gives textures this file can't see.
        let own_slots = self.material.is_none();
        if let Some(t) = tpl.filter(|_| own_slots) {
            self.template_slot_rules(t, &at, out);
        }
        match (&self.elements, tpl) {
            (Some(_), Some(t)) => out.push(error(
                at.key(field!(Model, elements)),
                format!(
                    "elements replace the shape of parent {:?} — remove one of them",
                    t.name
                ),
            )),
            (None, None) if parent.is_none() => {
                out.push(error(at.clone(), "no parent and no elements — give a template (parent: \"cube_all\") or elements"));
            }
            _ => {}
        }
        if let Some(els) = &self.elements {
            // A model by id as parent has slots this file can't see.
            let raw_parent = (parent.is_some() && tpl.is_none()) || !own_slots;
            for (i, el) in els.iter().enumerate() {
                super::element::element_rules(
                    el,
                    &self.textures,
                    raw_parent,
                    at.key(field!(Model, elements)).index(i),
                    out,
                );
            }
        }
    }

    /// A template's slots given: a sprite's layers in a row from `layer0`, a
    /// block template's every slot.
    fn template_slot_rules(&self, t: &Template, at: &Path, out: &mut Vec<Diagnostic>) {
        let has = |slot: &str| self.textures.0.iter().any(|(s, _)| s == slot);
        if !t.sprite {
            for slot in t.slots.iter().filter(|s| !has(s)) {
                out.push(error(
                    at.clone(),
                    format!("parent {:?} needs textures.{slot}", t.name),
                ));
            }
            return;
        }
        if !has(m::LAYER0) {
            out.push(error(
                at.clone(),
                format!("parent {:?} needs textures.{}", t.name, m::LAYER0),
            ));
        }
        for (slot, _) in &self.textures.0 {
            let here = at.key(field!(Model, textures)).key(slot);
            match slot
                .strip_prefix(m::LAYER)
                .and_then(|n| n.parse::<usize>().ok())
            {
                Some(n) if n == 0 || has(&format!("{}{}", m::LAYER, n - 1)) => {}
                Some(_) => out.push(error(here, "without the layer before it")),
                None if slot == m::PARTICLE => {}
                None => out.push(warning(
                    here,
                    "isn't used by an item sprite (layer0, layer1, ...)",
                )),
            }
        }
    }
}

/// Parent `name` (template `tpl` when it's one), at `at`: a template of the
/// model's kind, or a model id.
fn parent_rules(
    name: &str,
    tpl: Option<&Template>,
    at: Path,
    of: ModelOf,
    out: &mut Vec<Diagnostic>,
) {
    match tpl {
        Some(t) if t.sprite && of == ModelOf::Block => {
            out.push(error(
                at,
                format!("{name:?} is an item sprite — block models use cube_all, cube_column, ..."),
            ));
        }
        Some(_) => {}
        None if name.contains(yog_ids::sep::NAMESPACE) && yog_ids::valid_id(name) => {}
        None => {
            let names: Vec<&str> = TEMPLATES
                .iter()
                .filter(|t| of == ModelOf::Item || !t.sprite)
                .map(|t| t.name)
                .collect();
            out.push(error(
                at,
                format!(
                    "{name:?} isn't a template ({}) or a model id like `minecraft:block/stairs`",
                    names.join(", ")
                ),
            ));
        }
    }
}
