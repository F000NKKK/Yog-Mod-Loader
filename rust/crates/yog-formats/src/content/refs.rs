//! How content files name the game's things by short name and a mod's by
//! reference: kinds of tool, materials, block tags, sound groups, levels.

use yog_data::{Reader, Schema, Shape, Value, ValueRef};
use yog_registry::{DIGGERS, TOOL_KINDS, ToolLadder};

use crate::def::{ArmorMaterialKind, LevelKind, SoundGroupKind, ToolMaterialKind};

/// A kind of tool: the game's by name, or a mod's mineable tag.
pub struct ToolKindRef;

impl yog_data::with::With<String> for ToolKindRef {
    fn shape() -> Shape {
        Shape::Tag(&[yog_ids::kind::BLOCK])
    }
    fn read(v: &ValueRef<'_>, cx: &mut Reader) -> Option<String> {
        match v {
            ValueRef::Str(s) if TOOL_KINDS.contains(s) => Some(s.to_string()),
            ValueRef::Str(s) if !s.starts_with('@') => {
                cx.problem(format!("`{s}`: one of {} — or a mod's kind of tool by its mineable tag, `@tag:block:mymod:mineable/hammer`", TOOL_KINDS.join(", ")));
                None
            }
            _ => yog_data::TagId::<yog_data::kinds::Block>::read(v, cx).map(|t| t.id),
        }
    }
    fn write(f: &String) -> Value {
        if TOOL_KINDS.contains(&f.as_str()) {
            Value::from(f.as_str())
        } else {
            Value::Tag(yog_data::Ref::new(yog_ids::kind::BLOCK, f.clone()))
        }
    }
}

/// A material: one of the game's by short name (built-in data), or a
/// reference to a mod's (`@tool_material:…`, `@armor_material:…`).
pub struct MaterialRef<K>(std::marker::PhantomData<K>);

/// Which built-in materials a kind of material has.
pub trait MaterialKind: yog_data::Kind {
    fn built_in(name: &str) -> Option<String>;
    fn names() -> Vec<String>;
}

impl MaterialKind for ToolMaterialKind {
    fn built_in(name: &str) -> Option<String> {
        yog_registry::ToolMaterial::built_in(name).map(|m| m.id.clone())
    }
    fn names() -> Vec<String> {
        yog_registry::ToolMaterial::built_ins()
            .iter()
            .map(|m| yog_ids::split(&m.id).1.to_string())
            .collect()
    }
}

impl MaterialKind for ArmorMaterialKind {
    fn built_in(name: &str) -> Option<String> {
        yog_registry::ArmorMaterial::built_in(name).map(|m| m.id.clone())
    }
    fn names() -> Vec<String> {
        yog_registry::ArmorMaterial::built_ins()
            .iter()
            .map(|m| yog_ids::split(&m.id).1.to_string())
            .collect()
    }
}

impl<K: MaterialKind> yog_data::with::With<String> for MaterialRef<K> {
    fn shape() -> Shape {
        Shape::Ref(<K as yog_data::Kinds>::NAMES)
    }
    fn read(v: &ValueRef<'_>, cx: &mut Reader) -> Option<String> {
        match v {
            ValueRef::Str(s) if !s.starts_with('@') => K::built_in(s).or_else(|| {
                cx.problem(format!(
                    "`{s}`: one of the game's ({}) — or a mod's, `@{}:mymod:…` (a .ydef)",
                    K::names().join(", "),
                    K::NAME
                ));
                None
            }),
            _ => yog_data::with::RefTo::<K>::read(v, cx),
        }
    }
    fn write(f: &String) -> Value {
        match yog_ids::game_path(f) {
            Some(s) if K::built_in(s).is_some() => Value::from(s),
            _ => Value::Ref(yog_data::Ref::new(K::NAME, f.clone())),
        }
    }
}

/// A block tag a block joins, as its full id. Tools and tool tiers are
/// tags in Minecraft, so a mod's own are written like any other tag; the
/// game's have short names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockTag(pub String);

/// `pickaxe` → `minecraft:mineable/pickaxe`, or any `@tag:block:mymod:mineable/hammer`.
pub struct ToolTag;

/// A sound group: the game's by short name (`stone`, `amethyst` =
/// `@sound_group:minecraft:amethyst`) or any `@sound_group:…` (a `.ydef`).
pub struct SoundRef;

/// A tool level: the game's by short name (`iron` = `@level:minecraft:iron`)
/// or any `@level:…`.
pub struct LevelRef;

fn block_tag(
    v: &ValueRef,
    cx: &mut Reader,
    short: &[&str],
    full: fn(&str) -> String,
    what: &str,
) -> Option<BlockTag> {
    match v {
        ValueRef::Str(s) if short.contains(s) => Some(BlockTag(full(s))),
        ValueRef::Str(s) if !s.starts_with('@') => {
            cx.problem(format!(
                "`{s}`: one of {} — or a tag of the mod's own {what}s, `@tag:block:mymod:…`",
                short.join(", ")
            ));
            None
        }
        _ => yog_data::TagId::<yog_data::kinds::Block>::read(v, cx).map(|t| BlockTag(t.id)),
    }
}

impl yog_data::with::With<BlockTag> for ToolTag {
    fn shape() -> Shape {
        Shape::Tag(&[yog_ids::kind::BLOCK])
    }
    fn read(v: &ValueRef<'_>, cx: &mut Reader) -> Option<BlockTag> {
        block_tag(
            v,
            cx,
            &DIGGERS,
            |s| format!("{}{s}", yog_ids::game::MINEABLE),
            "tool",
        )
    }
    fn write(f: &BlockTag) -> Value {
        match f.0.strip_prefix(yog_ids::game::MINEABLE) {
            Some(s) if DIGGERS.contains(&s) => Value::from(s),
            _ => Value::Tag(yog_data::Ref::new(yog_ids::kind::BLOCK, f.0.clone())),
        }
    }
}

impl yog_data::with::With<String> for LevelRef {
    fn shape() -> Shape {
        Shape::Ref(&[yog_ids::kind::LEVEL])
    }
    fn read(v: &ValueRef<'_>, cx: &mut Reader) -> Option<String> {
        match v {
            ValueRef::Str(s) if !s.starts_with(yog_ids::sep::TYPED) => {
                match ToolLadder::game().level(s) {
                    Some(l) => Some(l.id.clone()),
                    None => {
                        let names: Vec<&str> = ToolLadder::game()
                            .ids()
                            .map(|id| yog_ids::split(id).1)
                            .collect();
                        cx.problem(format!("`{s}`: one of {} — or a level of the tool ladder, `@level:mymod:…` (a .ydef)", names.join(", ")));
                        None
                    }
                }
            }
            _ => yog_data::with::RefTo::<LevelKind>::read(v, cx),
        }
    }
    fn write(f: &String) -> Value {
        match ToolLadder::game().level(f) {
            Some(l) => Value::from(yog_ids::split(&l.id).1),
            _ => Value::Ref(yog_data::Ref::new(yog_ids::kind::LEVEL, f.clone())),
        }
    }
}

impl yog_data::with::With<String> for SoundRef {
    fn shape() -> Shape {
        Shape::Ref(&[yog_ids::kind::SOUND_GROUP])
    }
    fn read(v: &ValueRef<'_>, cx: &mut Reader) -> Option<String> {
        match v {
            ValueRef::Str(s) if !s.starts_with('@') => {
                match yog_registry::SoundGroup::built_in(s) {
                    Some(g) => Some(g.id.clone()),
                    None => {
                        cx.problem(format!("`{s}`: not one of the game's sound groups (stone, wood, metal, amethyst…) — a mod's is `@sound_group:mymod:…` (a .ydef)"));
                        None
                    }
                }
            }
            _ => yog_data::with::RefTo::<SoundGroupKind>::read(v, cx),
        }
    }
    fn write(f: &String) -> Value {
        match yog_ids::game_path(f) {
            Some(s) if yog_registry::SoundGroup::built_in(s).is_some() => Value::from(s),
            _ => Value::Ref(yog_data::Ref::new(yog_ids::kind::SOUND_GROUP, f.clone())),
        }
    }
}

/// One value or a list of them, each through `W`.
pub struct OneOrMany<W>(std::marker::PhantomData<W>);

impl<F, W: yog_data::with::With<F>> yog_data::with::With<Vec<F>> for OneOrMany<W> {
    fn shape() -> Shape {
        Shape::List(W::shape)
    }
    fn read(v: &ValueRef<'_>, cx: &mut Reader) -> Option<Vec<F>> {
        match v {
            ValueRef::List(_) => yog_data::with::List::<W>::read(v, cx),
            _ => W::read(v, cx).map(|x| vec![x]),
        }
    }
    fn write(f: &Vec<F>) -> Value {
        match f.as_slice() {
            [one] => W::write(one),
            many => Value::List(many.iter().map(W::write).collect()),
        }
    }
    fn absent() -> Option<Vec<F>> {
        Some(Vec::new())
    }
    fn skip(f: &Vec<F>) -> bool {
        f.is_empty()
    }
}
