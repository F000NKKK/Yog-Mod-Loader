//! `.ydef` definitions: ladders and levels, materials, sound groups, kinds
//! of recipe, tags, trees, ores, dimensions, damage types, loot, tables,
//! advancements, what the game registers — and the files they become.

mod advancement;
pub(in crate::content) mod data;
pub(in crate::content) mod ladder;
pub(in crate::content) mod worldgen;

use std::collections::{BTreeMap, HashMap};
use yog_data::Path as At;
use yog_data::schema::Read;
use yog_data::text::Doc;
use yog_data::{Diagnostic, Schema};
use yog_formats::def::{ArmorMaterial, Def, RecipeKindDef, SoundGroup, ToolMaterial};
use yog_ids::word::ID;

use crate::content::Compiled;
use crate::content::formats::table;
use crate::content::pipeline::ctx::Ctx;
use crate::content::pipeline::datapack;
use crate::content::pretty;
use crate::content::refs;
use crate::content::valid_path;
use worldgen::OreOut;

/// What the `.ydef`s define: every ladder, its levels bottom to top (the
/// game's tool ladder and the mod's), and the mod's sound groups.
#[derive(Default)]
pub(in crate::content) struct Defs {
    pub(in crate::content) order: BTreeMap<String, Vec<String>>,
    pub(in crate::content) sound_groups: Vec<SoundGroup>,
    pub(in crate::content) tool_materials: Vec<ToolMaterial>,
    pub(in crate::content) armor_materials: Vec<ArmorMaterial>,
    /// Placed features the mod adds to biomes (its ores), for the hosts that add them in code.
    pub(in crate::content) biome_features: Vec<(String, String, String)>,
    /// Its dimensions' ids (their data in `generated/dimensions/`).
    pub(in crate::content) dimensions: Vec<String>,
    /// Its tables (`kind: "table"`), each an enum in its code.
    pub(in crate::content) tables: Vec<table::Table>,
    /// Its loot modifiers' ids (their data in `generated/loot/`).
    pub(in crate::content) loot_modifiers: Vec<String>,
    /// Its attributes, effects and potions: (kind, id).
    pub(in crate::content) definitions: Vec<(&'static str, String)>,
    /// Its advancements' ids (their data in the datapack).
    pub(in crate::content) advancements: Vec<String>,
}

impl Defs {
    /// A level's ladder and place on it.
    pub(in crate::content) fn level(&self, id: &str) -> Option<(&str, usize)> {
        self.order
            .iter()
            .find_map(|(l, levels)| levels.iter().position(|x| x == id).map(|i| (l.as_str(), i)))
    }
}

/// A file's text and its document: where its values are.
pub(in crate::content) struct Located {
    text: String,
    doc: Doc,
}

impl Located {
    /// The line the value at `path` is on.
    pub(in crate::content) fn line(&self, path: &At) -> usize {
        self.doc.locate(&self.text, path).line as usize
    }

    /// The line of the top-level field `key`.
    pub(in crate::content) fn line_of(&self, key: &str) -> usize {
        self.line(&At::root().key(key))
    }
}

/// One `.ydef`, read through its schema.
pub(in crate::content) struct DefFile {
    pub(in crate::content) shown: String,
    pub(in crate::content) located: Located,
    pub(in crate::content) read: Read<Def>,
}

impl DefFile {
    /// Reads `text` of the file `cx` is on; what's wrong with it goes to `cx`.
    pub(in crate::content) fn read(cx: &mut Ctx, text: String) -> Option<DefFile> {
        let doc = match yog_data::text::parse(&text) {
            Ok(d) => d,
            Err(e) => {
                cx.err(e.at.line as usize, e.message);
                return None;
            }
        };
        match yog_data::schema::from_doc::<Def>(&text, &doc) {
            Ok(read) => Some(DefFile {
                shown: cx.file.clone(),
                located: Located { text, doc },
                read,
            }),
            Err(errors) => {
                cx.schema_errors(errors);
                None
            }
        }
    }
}

/// What the `.ydef`s checked against the mod's content make, besides
/// [`Compiled`]'s files.
#[derive(Default)]
pub(in crate::content) struct Made {
    /// The mod's kinds of recipe, by id.
    pub(in crate::content) recipe_kinds: HashMap<String, RecipeKindDef>,
    pub(in crate::content) ores: Vec<OreOut>,
    /// Dimensions: (id, their data).
    pub(in crate::content) dimensions: Vec<(String, Vec<u8>)>,
    pub(in crate::content) tables: Vec<table::Table>,
    /// Loot modifiers: (id, their data).
    pub(in crate::content) loot: Vec<(String, Vec<u8>)>,
    /// Attributes, effects, potions: (kind, id, their data).
    pub(in crate::content) definitions: Vec<(&'static str, String, Vec<u8>)>,
    /// Advancements' ids.
    pub(in crate::content) advancements: Vec<String>,
}

/// Whether what `.ydef` says is read with the ladders ([`ladder::read_defs`]),
/// not checked against the mod's content.
fn is_ladder_kind(def: &Def) -> bool {
    matches!(
        def,
        Def::Ladder { .. }
            | Def::Level { .. }
            | Def::SoundGroup(_)
            | Def::ToolMaterial(_)
            | Def::ArmorMaterial(_)
    )
}

/// Checks one `.ydef` against the mod's content (`known`): what it names
/// exists, its id is the mod's own; what it makes goes to `made` and `out`.
pub(in crate::content) fn compile_def(
    cx: &mut Ctx,
    file: DefFile,
    known: &refs::Known,
    made: &mut Made,
    out: &mut Compiled,
) {
    if is_ladder_kind(&file.read.value) {
        return;
    }
    let names_ok = names_known(cx, &file, known);
    let DefFile {
        located: at, read, ..
    } = file;
    let ns = cx.ns.clone();
    match read.value {
        Def::RecipeKind(k) => recipe_kind(cx, &at, k, names_ok, made),
        Def::Ore(o) => made.ores.push(worldgen::ore(o, &ns)),
        Def::Dimension(d) => made.dimensions.push(worldgen::dimension(d, &ns)),
        Def::Tag(t) => tag(cx, &at, &t, out),
        Def::DamageType(d) => damage_type(cx, &at, &d, out),
        Def::Tree(t) => tree(cx, &at, &t, out),
        Def::LootModifier(m) => {
            if let Some(id) = own_id(cx, &at, &m.id) {
                let m = yog_registry::LootModifier { id, ..m };
                made.loot
                    .push((m.id.clone(), yog_data::binary::encode(&m.write())));
            }
        }
        Def::LootTable(t) => loot_table(cx, &at, t, made, out),
        Def::Advancement(a) => advancement(cx, &at, &a, made, out),
        // Registered with the game: the host reads them from the content.
        Def::Attribute(mut a) => {
            if let Some(id) = own_id(cx, &at, &a.id) {
                a.id = id;
                registered(made, yog_ids::kind::ATTRIBUTE, &a.id, &a);
            }
        }
        Def::Effect(mut e) => {
            if let Some(id) = own_id(cx, &at, &e.id) {
                e.id = id;
                registered(made, yog_ids::kind::EFFECT, &e.id, &e);
            }
        }
        Def::Table(t) => match table::check(&cx.file, t, cx.refs, known) {
            Ok(t) => made.tables.push(t),
            Err(errors) => table_errors(cx, &at, errors),
        },
        Def::Ladder { .. }
        | Def::Level { .. }
        | Def::SoundGroup(_)
        | Def::ToolMaterial(_)
        | Def::ArmorMaterial(_) => {}
    }
}

/// A tag: its entries into the tag's file.
fn tag(cx: &mut Ctx, at: &Located, t: &yog_formats::def::TagDef, out: &mut Compiled) {
    match data::tag_entries(t, &cx.ns) {
        Ok((path, entries)) => out.tags.entry(path).or_default().extend(entries),
        Err(problems) => problems
            .into_iter()
            .for_each(|p| cx.err(at.line_of(yog_ids::word::VALUES), p)),
    }
}

/// A damage type: its file, and its id into the tags it joins.
fn damage_type(
    cx: &mut Ctx,
    at: &Located,
    d: &yog_formats::def::DamageTypeDef,
    out: &mut Compiled,
) {
    let Some(id) = own_id(cx, at, &d.id) else {
        return;
    };
    let (file, tags) = data::damage_type(d, &id);
    out.outputs.push(file);
    for (path, id) in tags {
        out.tags.entry(path).or_default().insert(id, true);
    }
}

/// A tree: its configured feature.
fn tree(cx: &mut Ctx, at: &Located, t: &yog_formats::def::TreeDef, out: &mut Compiled) {
    let Some(id) = own_id(cx, at, &t.id) else {
        return;
    };
    match worldgen::tree(t, &id) {
        Ok(file) => out.outputs.push(file),
        Err(e) => cx.err(at.line_of(ID), e),
    }
}

/// An advancement: its files, its id for the code.
fn advancement(
    cx: &mut Ctx,
    at: &Located,
    a: &yog_formats::def::AdvancementDef,
    made: &mut Made,
    out: &mut Compiled,
) {
    let Some(id) = own_id(cx, at, &a.id) else {
        return;
    };
    match advancement::advancement(cx, &mut out.lang, a, &id) {
        Ok(files) => {
            out.outputs.extend(files);
            if let Some(bg) = &a.background {
                out.loose.insert(advancement::background(bg));
            }
            made.advancements.push(id);
        }
        Err(e) => report_at(cx, at, e),
    }
}

/// A table's problems, each at its value's line.
fn table_errors(cx: &mut Ctx, at: &Located, errors: Vec<Diagnostic>) {
    for e in errors {
        report_at(cx, at, e);
    }
}

/// `e` reported at its value's line.
fn report_at(cx: &mut Ctx, at: &Located, e: Diagnostic) {
    let line = at.line(&e.path);
    cx.errors.push(e.in_file(&cx.file).at_line(line));
}

/// Whether what the file names exists: content and tags (a tag's
/// `optional` entries are other mods' — there or not).
fn names_known(cx: &mut Ctx, file: &DefFile, known: &refs::Known) -> bool {
    let optional = |path: &At| {
        matches!(file.read.value, Def::Tag(_))
            && path
                .to_string()
                .starts_with(yog_data::field!(yog_formats::def::TagDef, optional))
    };
    let mut ok = true;
    for f in file.read.refs.iter().filter(|f| !optional(&f.path)) {
        if let Some(kind) = refs::RefKind::from_word(&f.value.kind)
            && let Err(e) = known.check(kind, &f.value.id)
        {
            cx.err(file.located.line(&f.path), format!("{}: {e}", f.path));
            ok = false;
        }
    }
    for f in yog_data::found::tags(&file.read.refs)
        .iter()
        .filter(|f| !optional(&f.path))
    {
        if let Err(e) = known.check_tag(&f.value.kind, &f.value.id) {
            cx.err(file.located.line(&f.path), format!("{}: {e}", f.path));
            ok = false;
        }
    }
    ok
}

/// `id` in the mod's namespace, when it's the mod's own (`<ns>:<path>`);
/// an error at the file's `id` otherwise.
fn own_id(cx: &mut Ctx, at: &Located, id: &str) -> Option<String> {
    let id = yog_ids::qualify_in(&cx.ns, id);
    let (ns, path) = yog_ids::split(&id);
    if ns == cx.ns && valid_path(path) {
        return Some(id);
    }
    cx.err(
        at.line_of(ID),
        format!("{ID}: `{id}`: the mod's own, `{}:<name>`", cx.ns),
    );
    None
}

/// A kind of recipe: the mod's own, declared once; kept when what it names exists.
fn recipe_kind(cx: &mut Ctx, at: &Located, mut k: RecipeKindDef, names_ok: bool, made: &mut Made) {
    let Some(id) = own_id(cx, at, &k.id) else {
        return;
    };
    if made.recipe_kinds.contains_key(&id) {
        cx.err(
            at.line_of(ID),
            format!("recipe kind {id} is declared twice"),
        );
    }
    if names_ok {
        k.id = id.clone();
        made.recipe_kinds.insert(id, k);
    }
}

/// A table of its own: an empty one in the datapack (so it exists for
/// `/loot` and other tables), filled by a modifier on it.
fn loot_table(
    cx: &mut Ctx,
    at: &Located,
    t: yog_formats::def::LootTableDef,
    made: &mut Made,
    out: &mut Compiled,
) {
    use yog_ids::loot;
    let Some(id) = own_id(cx, at, &t.id) else {
        return;
    };
    let empty = serde_json::json!({ loot::TYPE: loot::GENERIC_TABLE, loot::POOLS: [] });
    out.outputs.push((
        datapack::data_file(yog_ids::kind::LOOT_TABLE, &id),
        pretty(&empty),
    ));
    let fill = yog_registry::LootModifier {
        id: id.clone(),
        tables: vec![id.clone()],
        add: t.add,
        scale: Vec::new(),
    };
    made.loot
        .push((id, yog_data::binary::encode(&fill.write())));
}

/// A definition the game registers (`attribute`, `effect`) under the mod's
/// own id: kept as data for the code to register.
fn registered<T: Schema>(made: &mut Made, kind: &'static str, id: &str, def: &T) {
    made.definitions
        .push((kind, id.to_string(), yog_data::binary::encode(&def.write())));
}
