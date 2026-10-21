//! One `.yblock` / `.yitem`: its properties, model, states, drops, tags and
//! texts checked and turned into the game's files.

mod block;
mod item;
mod loot;
mod screens;
mod states;

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::Value;
use yog_data::Path as At;
use yog_data::field;
use yog_formats::content::{BlockFile, ItemFile, Model, Register};
use yog_ids::{Format, content_field};

use crate::content::ctx::{Ctx, Loc, Wanted};
use crate::content::entry::{BlockProps, Entry, ItemProps};
use crate::content::file::{check_format, file_id};
use crate::content::{Compiled, Lang, datapack, pretty};
use crate::yres;

pub(super) use loot::{drops_table, kind_drops};
pub(super) use screens::inventory_screens;

/// What a translation key adds for an item's or block's description.
const DESCRIPTION: &str = ".desc";

/// What a `.yitem` and a `.yblock` both say, and what only one of them does.
struct Parts {
    format: Option<u32>,
    id: Option<String>,
    name: Option<yog_data::Text>,
    tooltip: Option<yog_data::Text>,
    register: Option<Register>,
    model: Option<Model>,
    item_model: Option<Model>,
    block: Option<yog_formats::content::BlockProps>,
    states: Option<yog_formats::content::States>,
    item: Option<yog_formats::content::ItemProps>,
    /// A block's item form.
    block_item: Option<yog_formats::content::BlockItemProps>,
    requires: Vec<String>,
    tags: Vec<String>,
    /// One of the game's kinds of block.
    kind: Option<String>,
}

impl From<ItemFile> for Parts {
    fn from(f: ItemFile) -> Parts {
        Parts {
            format: f.format,
            id: f.id,
            name: f.name,
            tooltip: f.tooltip,
            register: f.register,
            model: f.model,
            item_model: None,
            block: None,
            states: None,
            item: f.item,
            block_item: None,
            requires: f.requires,
            tags: f.tags,
            kind: None,
        }
    }
}

impl From<BlockFile> for Parts {
    fn from(f: BlockFile) -> Parts {
        Parts {
            format: f.format,
            id: f.id,
            name: f.name,
            tooltip: f.tooltip,
            register: f.register,
            model: Some(f.model),
            item_model: f.item_model,
            block: f.block,
            states: f.states,
            item: None,
            block_item: f.item,
            requires: f.requires,
            tags: f.tags,
            kind: f.kind,
        }
    }
}

/// The file's outputs under `assets/<ns>/`, each written by one file only.
pub(super) struct Emit<'a> {
    pub(super) ns: String,
    /// The id's path: what its files are named by.
    pub(super) p: String,
    pub(super) out: &'a mut Compiled,
    generated: &'a mut BTreeMap<String, String>,
}

impl Emit<'_> {
    /// `value` as `assets/<ns>/<rel>`.
    pub(super) fn asset(&mut self, cx: &mut Ctx, rel: String, value: &Value) {
        let full = yog_ids::asset_file(&self.ns, &rel);
        if cx.root.join(&full).is_file() {
            cx.err(1, format!("{full} is written by this file — delete the hand-written one (or bring it in with `yog import model`)"));
        }
        if let Some(other) = self.generated.insert(full.clone(), cx.file.clone()) {
            cx.err(1, format!("{full} is also written by {other}"));
        }
        self.out.outputs.push((full, pretty(value)));
    }

    /// The item's model and its definition (1.21.4+) naming it.
    fn item_model(&mut self, cx: &mut Ctx, model: &Value) {
        let path = format!("{}{}", yog_ids::res::ITEM_DIR, self.p);
        self.asset(cx, yog_ids::model_file(&path), model);
        let id = yog_ids::join(&self.ns, &path);
        let def = serde_json::json!({ crate::consts::item_def::MODEL: { crate::consts::item_def::TYPE: crate::consts::item_def::PLAIN, crate::consts::item_def::MODEL: id } });
        self.asset(cx, yog_ids::item_definition_file(&self.p), &def);
    }
}

/// Checks one `.yitem` or `.yblock` (`content::files`) and writes what the
/// game reads: models, block states, item definitions, loot, lang, tags.
pub(super) fn compile_file(
    cx: &mut Ctx,
    text: &str,
    kind: Format,
    path: &Path,
    mod_ns: Option<&str>,
    out: &mut Compiled,
    generated: &mut BTreeMap<String, String>,
) -> Option<Entry> {
    let doc = match yog_data::text::parse(text) {
        Ok(d) => d,
        Err(e) => {
            cx.err(e.at.line as usize, e.message);
            return None;
        }
    };
    let loc_fn = |p: &At| doc.locate(text, p).line as usize;
    let loc: Loc = &loc_fn;
    let mut parts = read_parts(cx, text, &doc, kind, mod_ns)?;
    let root = At::root();
    check_format(cx, loc, &root.key(field!(ItemFile, format)), parts.format);
    let (ns, p) = file_id(
        cx,
        loc(&root.key(field!(ItemFile, id))),
        parts.id.as_deref(),
        path,
        mod_ns,
    )?;
    cx.ns = ns.clone();
    apply_materials(cx, loc, &mut parts, kind, &ns);
    let id = yog_ids::join(&ns, &p);
    let potion_only = parts.item.as_ref().is_some_and(|it| it.potion.is_some());
    if !potion_only {
        lang_texts(cx, loc, &parts, out, yog_ids::lang_key(kind.noun(), &id));
    }
    let mut entry = Entry {
        kind,
        id: id.clone(),
        file: cx.file.clone(),
        auto: parts.register.unwrap_or_default() == Register::Auto,
        requires: parts.requires.clone(),
        block: BlockProps::default(),
        item: ItemProps::default(),
        potion: parts
            .item
            .as_ref()
            .and_then(|it| it.potion.as_ref())
            .map(|p| yog_registry::PotionDef {
                id: id.clone(),
                name: p.name.clone(),
                effects: p.effects.clone(),
                brewed: Vec::new(),
            }),
    };
    join_tags(cx, &parts.tags, kind, &id, out);
    // A block's item form: its stack, fire and fuel.
    if let Some(it) = &parts.block_item {
        entry.item.stack_size = it.stack_size;
        entry.item.fire_resistant = it.fire_resistant;
        entry.item.fuel_ticks = it.fuel_ticks.unwrap_or(0);
    }
    let mut emit = Emit {
        ns,
        p,
        out,
        generated,
    };
    if kind == Format::Item {
        item::compile(cx, loc, &parts, &mut entry, &mut emit)?;
    } else {
        block::compile(cx, loc, &parts, &mut entry, &mut emit)?;
    }
    Some(entry)
}

/// The file read through its schema; what it names (drops, tags) queued to
/// be checked once every file is read.
fn read_parts(
    cx: &mut Ctx,
    text: &str,
    doc: &yog_data::text::Doc,
    kind: Format,
    mod_ns: Option<&str>,
) -> Option<Parts> {
    let loc = |p: &At| doc.locate(text, p).line as usize;
    let read = match kind {
        Format::Item => yog_data::schema::from_doc::<ItemFile>(text, doc)
            .map(|r| (Parts::from(r.value), r.refs, r.tags)),
        _ => yog_data::schema::from_doc::<BlockFile>(text, doc)
            .map(|r| (Parts::from(r.value), r.refs, r.tags)),
    };
    let (parts, refs, tag_refs) = match read {
        Ok(r) => r,
        Err(e) => {
            cx.schema_errors(e);
            return None;
        }
    };
    for f in refs {
        cx.defer(loc(&f.path), &f.path, Wanted::Ref(f.value));
    }
    for f in tag_refs {
        cx.defer(
            loc(&f.path),
            &f.path,
            Wanted::Tag {
                kinds: vec![f.value.kind],
                id: f.value.id,
            },
        );
    }
    // Its tags (checked like the rest): a block's are block or item tags.
    let kinds: &[&str] = if kind == Format::Block {
        &[yog_ids::kind::BLOCK, yog_ids::kind::ITEM]
    } else {
        &[yog_ids::kind::ITEM]
    };
    for (i, tag) in parts.tags.iter().enumerate() {
        let at = At::root().key(field!(ItemFile, tags)).index(i);
        let tag = yog_ids::qualify_in(
            mod_ns.unwrap_or(yog_ids::ns::MINECRAFT),
            tag.trim_start_matches(yog_ids::sep::TAG),
        );
        cx.defer(
            loc(&at),
            &at,
            Wanted::Tag {
                kinds: kinds.iter().map(|k| k.to_string()).collect(),
                id: tag,
            },
        );
    }
    Some(parts)
}

/// Materials: their textures under the models' own; a block's render layer,
/// when it gives none, its model's material's.
fn apply_materials(cx: &mut Ctx, loc: Loc, parts: &mut Parts, kind: Format, ns: &str) {
    for (field, is_block_model) in [
        (content_field::MODEL, true),
        (content_field::ITEM_MODEL, false),
    ] {
        let model = if is_block_model {
            parts.model.as_mut()
        } else {
            parts.item_model.as_mut()
        };
        let Some(model) = model else { continue };
        let Some(material) = model.material.clone() else {
            continue;
        };
        match super::material::load(cx.root, ns, &material) {
            Ok(m) => {
                let layer = super::material::apply(model, &m);
                if let (true, Some(layer)) = (is_block_model && kind == Format::Block, layer) {
                    parts
                        .block
                        .get_or_insert_with(Default::default)
                        .render_layer
                        .get_or_insert(layer);
                }
            }
            Err(e) => cx.err_at(loc, &At::root().key(field).key(content_field::MATERIAL), e),
        }
    }
}

/// Name and tooltip → lang, under the keys the game looks up.
/// `key`: the game's translation key of the item or block.
fn lang_texts(cx: &mut Ctx, loc: Loc, parts: &Parts, out: &mut Compiled, key: String) {
    let ns = cx.ns.clone();
    let texts = [
        (field!(ItemFile, name), &parts.name, key.clone()),
        (
            field!(ItemFile, tooltip),
            &parts.tooltip,
            format!("{key}{DESCRIPTION}"),
        ),
    ];
    for (field, text, target) in texts {
        let Some(t) = text else { continue };
        match text_key(cx, &mut out.lang, t, &ns, target.clone()) {
            Ok(from) if from != target => out.lang_aliases.push((from, target)),
            Ok(_) => {}
            Err(e) => cx.err_at(loc, &At::root().key(field), e),
        }
    }
}

/// Its tags: a block's block tag, and the item tag of the same name for its item form.
fn join_tags(cx: &Ctx, tags: &[String], kind: Format, id: &str, out: &mut Compiled) {
    for tag in tags {
        let tag = yog_ids::qualify_in(&cx.ns, tag.trim_start_matches(yog_ids::sep::TAG_SHOWN));
        if kind == Format::Block {
            out.tags
                .entry(datapack::tag_file(yog_ids::kind::BLOCK, &tag))
                .or_default()
                .insert(id.to_string(), true);
        }
        out.tags
            .entry(datapack::tag_file(yog_ids::kind::ITEM, &tag))
            .or_default()
            .insert(id.to_string(), true);
    }
}

/// The translation key a text names (`@Lang.…`, `@lang:…`, an item's name),
/// or for words, `key` with the words put in the mod's lang.
pub(super) fn text_key(
    cx: &mut Ctx,
    lang: &mut Lang,
    t: &yog_data::Text,
    ns: &str,
    key: String,
) -> Result<String, String> {
    if let yog_data::Text::Literal(words) = t {
        lang.entry((ns.to_string(), yres::NEUTRAL_LOCALE.to_string()))
            .or_default()
            .insert(key.clone(), words.clone());
        return Ok(key);
    }
    Ok(cx.refs.key_of(t)?.unwrap_or(key))
}
