//! The game's kinds of block with a class of their own (`kind: "stairs"`).

use yog_data::{Ordered, Reader, Schema, Shape, Value, ValueRef};

/// A kind of block the game has a class for — `kind: "stairs"` on a
/// `.yblock`: the texture slots its models take, the game's template models
/// it's drawn with, which state shows which, its item's model and its drops.
/// The game's kinds are data (`data/block_kinds.ydata`); the hosts make each
/// of the game's class by name.
#[derive(Schema, Debug, Clone)]
pub struct BlockKind {
    pub name: String,
    /// The texture slots of its models, given by the `.yblock`'s `model.textures`.
    pub slots: Vec<String>,
    /// Its models: a name → the game's template model. `""` is the block's
    /// own model (`block/<id>`), another name `block/<id>_<name>`.
    pub models: Ordered<String>,
    /// The model its item shows (a name of `models`); none: the `.yblock`
    /// gives `item_model`.
    pub item: Option<String>,
    /// The `block` fields it takes of `base`, `set`, `press_ticks`, `triggered_by`.
    #[data(default)]
    pub takes: Vec<String>,
    /// Its default for `block.occludes`.
    pub occludes: Option<bool>,
    /// Its default for `block.collision`.
    pub collision: Option<bool>,
    /// It drops only in states with these properties (`half=lower`: a door
    /// drops once, from its bottom half).
    pub drops_when: Option<String>,
    /// It drops two in states with these properties (`type=double`).
    pub drops_two_when: Option<String>,
    /// A state's properties (`facing=east,half=bottom`) → the model it shows.
    #[data(default)]
    pub variants: Ordered<KindModel>,
    /// Or models drawn together, each when its properties match (a fence's
    /// post and arms).
    #[data(default)]
    pub multipart: Vec<KindPart>,
}

/// A model a kind's state shows: a name of its `models` (`base`: the
/// model of `block.base`), turned.
#[derive(Schema, Debug, Clone)]
pub struct KindModel {
    pub model: String,
    pub x: Option<u32>,
    pub y: Option<u32>,
    #[data(default)]
    pub uvlock: bool,
}

/// A part of a kind's multipart model: drawn always, or `when` its state
/// has these properties (`north=true`).
#[derive(Schema, Debug, Clone)]
pub struct KindPart {
    pub apply: KindModel,
    pub when: Option<String>,
}

/// The `block` fields only some kinds take (`BlockKind::takes`).
pub const KIND_FIELDS: [&str; 4] = ["base", "set", "press_ticks", "triggered_by"];

static BUILT_IN_KINDS: yog_sync::LazyLock<Vec<BlockKind>> = yog_sync::LazyLock::new(|| {
    yog_data::built_in(
        "yog-formats' built-in block kinds",
        include_str!("../../data/block_kinds.ydata"),
    )
});

impl BlockKind {
    /// The game's kinds: stairs, slabs, doors, trapdoors, buttons, pressure
    /// plates, fences, fence gates.
    pub fn built_ins() -> &'static [BlockKind] {
        &BUILT_IN_KINDS
    }

    pub fn built_in(name: &str) -> Option<&'static BlockKind> {
        Self::built_ins().iter().find(|k| k.name == name)
    }

    pub fn takes(&self, field: &str) -> bool {
        self.takes.iter().any(|t| t == field)
    }
}

/// A kind of block by name, one of [`BlockKind::built_ins`].
pub struct KindRef;

impl yog_data::with::With<String> for KindRef {
    fn shape() -> Shape {
        Shape::Any
    }
    fn read(v: &ValueRef<'_>, cx: &mut Reader) -> Option<String> {
        let s = String::read(v, cx)?;
        if BlockKind::built_in(&s).is_none() {
            let names: Vec<&str> = BlockKind::built_ins()
                .iter()
                .map(|k| k.name.as_str())
                .collect();
            cx.problem(format!(
                "`{s}`: one of the game's kinds of block — {}",
                names.join(", ")
            ));
            return None;
        }
        Some(s)
    }
    fn write(f: &String) -> Value {
        Value::from(f.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_game_kinds_of_block_read_and_name_their_own_models() {
        let kinds = BlockKind::built_ins();
        assert_eq!(kinds.len(), 8);
        for k in kinds {
            let names = k
                .variants
                .0
                .iter()
                .map(|(_, m)| m)
                .chain(k.multipart.iter().map(|p| &p.apply))
                .map(|m| m.model.as_str());
            for name in names.chain(k.item.as_deref()) {
                assert!(
                    name == "base" && k.takes("base") || k.models.0.iter().any(|(n, _)| n == name),
                    "{}: no model `{name}`",
                    k.name
                );
            }
            for (n, _) in &k.models.0 {
                assert_ne!(n, "base", "{}: `base` is block.base's model", k.name);
            }
            assert!(
                k.takes.iter().all(|t| KIND_FIELDS.contains(&t.as_str())),
                "{}",
                k.name
            );
            assert!(
                k.variants.0.is_empty() != k.multipart.is_empty(),
                "{}: variants or multipart",
                k.name
            );
        }
    }
}
