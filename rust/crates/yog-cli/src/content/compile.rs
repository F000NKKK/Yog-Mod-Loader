//! Compiling a mod's content files: each one read and checked, then what
//! they name checked against everything they define, then the code and the
//! data made of them.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use yog_data::Schema;
use yog_ids::Format;
use yog_ids::ns::MINECRAFT;
use yog_registry::{Brew, PotionDef};

use crate::content::block_item::{compile_file, inventory_screens};
use crate::content::body::{compile_body, compile_entity, compile_font};
use crate::content::book::{BookAgainst, compile_book, recipe_layouts};
use crate::content::ctx::{Deferred, Shared, Sink, Wanted};
use crate::content::ctx::{caveat, problem};
use crate::content::defs::data::{definition_path, loot_path};
use crate::content::defs::ladder::{read_defs, tool_level_tags};
use crate::content::defs::worldgen::dimension_path;
use crate::content::defs::{DefFile, Defs, Made, compile_def};
use crate::content::entry::Entry;
use crate::content::files::{BodyFile, BookFile, EntityFile, FontFile};
use crate::content::outputs::version_overlays;
use crate::content::recipe::registration::Recipe;
use crate::content::recipe::{Against, compile_recipe};
use crate::content::source::{Content, mentions_content_macro, registrations, rust_source};
use crate::content::{
    CONTENT_DIR, Compiled, content_format, datapack, find, looks, particles, refs, sounds,
    textures, valid_namespace, videos,
};
use crate::diagnostics::Diagnostic;
use crate::yres;

/// Ids defined so far, each by the file defining it: a second one is a mistake.
#[derive(Default)]
struct Ids(HashMap<String, String>);

impl Ids {
    /// `id` (a `what`) defined by `file`.
    fn claim(&mut self, errors: &mut Vec<Diagnostic>, file: &str, what: &str, id: &str) {
        if let Some(other) = self.0.insert(id.to_string(), file.to_string()) {
            errors.push(problem(format!("{what} {id} is also defined by {other}")).in_file(file));
        }
    }
}

/// Files read and checked later, once what they name is known: (file, text).
#[derive(Default)]
struct Pending {
    books: Vec<(String, String)>,
    recipes: Vec<(String, String, PathBuf)>,
    defs: Vec<DefFile>,
}

/// Everything checked so far.
struct Build<'a> {
    shared: Shared<'a>,
    /// The mod's namespace, when its id is one.
    mod_ns: Option<&'a str>,
    sink: Sink,
    out: Compiled,
    /// The models the files write, by path: one file each.
    generated: BTreeMap<String, String>,
    entries: Vec<Entry>,
    potions: BTreeMap<String, PotionDef>,
    entities: Vec<EntityFile>,
    bodies: Vec<BodyFile>,
    fonts: Vec<FontFile>,
    recipes: Vec<Recipe>,
    books: Vec<BookFile>,
    /// Blocks', items' and entities' ids, then each other kind's.
    ids: Ids,
    model_ids: Ids,
    anim_ids: Ids,
    font_ids: Ids,
    recipe_ids: Ids,
    book_ids: Ids,
    pending: Pending,
}

/// Compiles the mod's content files (see the module docs). Any error fails
/// the whole thing, listing every problem.
pub fn compile(root: &Path, mod_id: &str, refs: &yres::Refs) -> Result<Compiled, String> {
    let files = find(root);
    let mut sink = Sink::default();
    let own = textures::compile(root, mod_id, refs, &mut sink.errors);
    let mut videos = videos::compile(root, mod_id, refs, &mut sink.errors);
    let events = std::mem::take(&mut videos.events);
    let sounds = sounds::compile(root, mod_id, refs, events, &mut sink.errors);
    let particles = particles::compile(root, mod_id, &own.ids, &mut sink.errors);
    if files.is_empty() && particles.is_empty() && videos.is_empty() {
        return media_only(own, sounds, sink);
    }
    let mod_ns = valid_namespace(mod_id).then_some(mod_id);
    let shared = Shared {
        root,
        refs,
        ns: mod_ns.unwrap_or(MINECRAFT),
        own_textures: &own.ids,
    };
    let mut b = Build::new(shared, mod_ns, sink);
    for path in &files {
        b.read(path);
    }
    let mut known = b.known();
    let mut defs = read_defs(&b.pending.defs, b.shared.ns, &mut b.sink.errors);
    let mut made = b.defs(&known);
    made.definitions.extend(particles.definitions);
    made.definitions.extend(videos.definitions);
    let layouts = b.recipes(&mut known, &made);
    b.check_deferred(&defs, &known);
    b.tool_levels(&defs);
    b.books(&known, &layouts);
    b.inventories();
    b.out.packed = own.packed_but(&b.out.loose);
    b.out.outputs.extend(own.outputs);
    b.out.outputs.extend(sounds.output);
    b.out.outputs.extend(sounds.rendered);
    b.out.outputs.extend(particles.outputs);
    b.out.outputs.extend(videos.outputs);
    b.finish(&mut defs, made, &known)
}

/// A mod with no content files: its textures and sounds alone.
fn media_only(
    own: textures::Textures,
    sounds: sounds::Sounds,
    sink: Sink,
) -> Result<Compiled, String> {
    if !sink.errors.is_empty() {
        crate::diagnostics::report_all(&sink.errors);
        return Err(format!(
            "invalid textures/sounds in {}/:{}{}",
            crate::yres::RESOURCES_DIR,
            crate::diagnostics::LIST_SEP,
            crate::diagnostics::listed(&sink.errors)
        ));
    }
    let mut out = Compiled::default();
    out.outputs.extend(own.outputs);
    out.outputs.extend(sounds.output);
    out.outputs.extend(sounds.rendered);
    out.packed = own.packed;
    Ok(out)
}

impl<'a> Build<'a> {
    fn new(shared: Shared<'a>, mod_ns: Option<&'a str>, sink: Sink) -> Build<'a> {
        Build {
            shared,
            mod_ns,
            sink,
            out: Compiled::default(),
            generated: BTreeMap::new(),
            entries: Vec::new(),
            potions: BTreeMap::new(),
            entities: Vec::new(),
            bodies: Vec::new(),
            fonts: Vec::new(),
            recipes: Vec::new(),
            books: Vec::new(),
            ids: Ids::default(),
            model_ids: Ids::default(),
            anim_ids: Ids::default(),
            font_ids: Ids::default(),
            recipe_ids: Ids::default(),
            book_ids: Ids::default(),
            pending: Pending::default(),
        }
    }

    /// Reads the content file at `path`: checked now, or kept for when
    /// what it names is known.
    fn read(&mut self, path: &Path) {
        let shown = crate::shown_path(self.shared.root, path);
        let Some(kind) = content_format(path) else {
            return;
        };
        let text = match crate::files::read_source(path) {
            Ok(t) => t,
            Err(e) => {
                return self
                    .sink
                    .errors
                    .push(problem(format!("{e}")).in_file(&shown));
            }
        };
        let errors = &mut self.sink.errors;
        match kind {
            Format::Model | Format::Anim => {
                let Some(b) = compile_body(&self.shared, &shown, &text, kind, path, errors) else {
                    return;
                };
                let ids = if b.model {
                    &mut self.model_ids
                } else {
                    &mut self.anim_ids
                };
                ids.claim(errors, &shown, kind.noun(), &b.id);
                self.bodies.push(b);
            }
            Format::Font => {
                let Some(font) = compile_font(&self.shared, &shown, &text, path, errors) else {
                    return;
                };
                self.font_ids
                    .claim(errors, &shown, yog_ids::kind::FONT, &font.id);
                self.fonts.push(font);
            }
            // Checked once every block, item and recipe is known.
            Format::Book => self.pending.books.push((shown, text)),
            // Checked once every block and item is known.
            Format::Recipe => self.pending.recipes.push((shown, text, path.to_path_buf())),
            Format::Def => {
                let mut cx = self.sink.cx(&self.shared, &shown);
                if let Some(f) = DefFile::read(&mut cx, text) {
                    self.pending.defs.push(f);
                }
            }
            Format::Entity => {
                let mut cx = self.sink.cx(&self.shared, &shown);
                let Some(e) = compile_entity(&mut cx, &text, path, self.mod_ns, &mut self.out)
                else {
                    return;
                };
                self.ids
                    .claim(&mut self.sink.errors, &shown, yog_ids::word::ID, &e.id);
                self.entities.push(e);
            }
            _ => {
                let mut cx = self.sink.cx(&self.shared, &shown);
                let Some(entry) = compile_file(
                    &mut cx,
                    &text,
                    kind,
                    path,
                    self.mod_ns,
                    &mut self.out,
                    &mut self.generated,
                ) else {
                    return;
                };
                self.ids
                    .claim(&mut self.sink.errors, &shown, yog_ids::word::ID, &entry.id);
                match entry.potion.clone() {
                    Some(p) => {
                        self.potions.insert(entry.id, p);
                    }
                    None => self.entries.push(entry),
                }
            }
        }
    }

    /// What the files may name: the mod's content, vanilla's and other mods'
    /// declared ids. A model or clips of this mod an entity names must be
    /// among its `.ymodel`/`.yanim`.
    fn known(&mut self) -> refs::Known {
        // A manifest that doesn't read was reported when the build prepared.
        let manifest =
            crate::manifest::YogToml::read(&self.shared.root.join(yog_ids::file::MANIFEST)).ok();
        let externals = manifest
            .as_ref()
            .map(|t| t.externals.clone())
            .unwrap_or_default();
        let datapack = manifest.as_ref().and_then(|t| t.datapack.clone());
        let mut known = refs::Known::new(self.shared.ns, &externals, datapack.as_deref());
        for e in &self.entries {
            known.add(
                if e.kind == Format::Block {
                    refs::RefKind::Block
                } else {
                    refs::RefKind::Item
                },
                e.id.clone(),
            );
        }
        for id in self.potions.keys() {
            known.add(refs::RefKind::Item, id.clone());
        }
        for e in &self.entities {
            known.add(refs::RefKind::Entity, e.id.clone());
            for (wanted, kind) in [(&e.model, Format::Model), (&e.animations, Format::Anim)] {
                if let Some(id) = wanted
                    && self.mod_ns.is_some_and(|ns| yog_ids::split(id).0 == ns)
                    && !self
                        .bodies
                        .iter()
                        .any(|b| b.model == (kind == Format::Model) && &b.id == id)
                {
                    self.sink.errors.push(problem(format!(
                        "{}: {} `{id}`: no such file in {CONTENT_DIR}/ (`.{}`)",
                        e.file,
                        kind.noun(),
                        kind.ext()
                    )));
                }
            }
        }
        known
    }

    /// The `.ydef`s checked against what the files name (`known`).
    fn defs(&mut self, known: &refs::Known) -> Made {
        let mut made = Made::default();
        for f in std::mem::take(&mut self.pending.defs) {
            let shown = f.shown.clone();
            let mut cx = self.sink.cx(&self.shared, &shown);
            compile_def(&mut cx, f, known, &mut made, &mut self.out);
        }
        made
    }

    /// The `.yrecipe`s, against the mod's kinds of recipe and how they're
    /// shown (`.yui` `recipe_layout` components, returned); their brewing
    /// recipes onto the potions they make.
    fn recipes(
        &mut self,
        known: &mut refs::Known,
        made: &Made,
    ) -> Vec<yog_formats::book::layout::RecipeLayout> {
        let layouts = recipe_layouts(
            self.shared.root,
            self.shared.ns,
            &made.recipe_kinds,
            known,
            &mut self.sink.errors,
        );
        let mut brews: Vec<(String, Brew)> = Vec::new();
        let mut against = Against {
            mod_ns: self.mod_ns,
            known,
            kinds: &made.recipe_kinds,
            brews: &mut brews,
        };
        for (shown, text, path) in std::mem::take(&mut self.pending.recipes) {
            let mut cx = self.sink.cx(&self.shared, &shown);
            let Some(r) = compile_recipe(&mut cx, &text, &path, &mut against) else {
                continue;
            };
            self.recipe_ids
                .claim(&mut self.sink.errors, &shown, yog_ids::kind::RECIPE, &r.id);
            self.recipes.push(r);
        }
        for r in &self.recipes {
            known.add(refs::RefKind::Recipe, r.id.clone());
        }
        for (to, brew) in brews {
            self.brew(to, brew);
        }
        layouts
    }

    /// A brewing recipe making potion `to`: the mod's potion or a Yog
    /// default potion alias, from one.
    fn brew(&mut self, to: String, brew: Brew) {
        let defined = |p: &BTreeMap<String, PotionDef>, id: &str| {
            p.contains_key(id) || refs::is_vanilla_potion(id)
        };
        if !defined(&self.potions, &brew.from) {
            return self.sink.errors.push(problem(format!("brewing recipe input `{}`: no potion `.yitem` or Yog default potion alias defines it", brew.from)));
        }
        if !defined(&self.potions, &to) {
            return self.sink.errors.push(problem(format!("brewing recipe result `{to}`: no potion `.yitem` or Yog default potion alias defines it")));
        }
        self.potions
            .entry(to.clone())
            .or_insert_with(|| PotionDef {
                id: to,
                name: None,
                effects: Vec::new(),
                brewed: Vec::new(),
            })
            .brewed
            .push(brew);
    }

    /// What blocks and items name (drops, levels), known now.
    fn check_deferred(&mut self, defs: &Defs, known: &refs::Known) {
        for Deferred {
            file,
            line,
            path,
            wanted,
        } in std::mem::take(&mut self.sink.deferred)
        {
            if let Err(e) = check_wanted(&wanted, defs, known) {
                self.sink
                    .errors
                    .push(problem(format!("{path}: {e}")).in_file(file).at_line(line));
            }
        }
    }

    /// The tags that make blocks need their tool level.
    fn tool_levels(&mut self, defs: &Defs) {
        for e in &self.entries {
            let Some((level, line)) = &e.block.tool_level else {
                continue;
            };
            let at = |d: Diagnostic| d.in_file(&e.file).at_line(*line);
            let field = yog_data::field!(yog_formats::content::BlockProps, tool_level);
            match tool_level_tags(&e.id, level, defs, &mut self.out) {
                Ok(None) => {}
                Ok(Some(m)) => self
                    .sink
                    .warnings
                    .push(at(caveat(format!("block.{field}: {m}")))),
                Err(m) => self
                    .sink
                    .errors
                    .push(at(problem(format!("block.{field}: {m}")))),
            }
        }
    }

    /// The `.ybook`s, once every block, item and recipe is known.
    fn books(&mut self, known: &refs::Known, layouts: &[yog_formats::book::layout::RecipeLayout]) {
        let recipe_kinds: HashMap<String, String> = self
            .recipes
            .iter()
            .map(|r| (r.id.clone(), r.ty.clone()))
            .collect();
        // The mod's own kinds of book page (`.yui` components with `book_page`).
        let page_types =
            crate::book_page::find(self.shared.root, self.shared.ns, &mut self.sink.errors);
        let against = BookAgainst {
            known,
            layouts,
            recipe_kinds: &recipe_kinds,
            page_types: &page_types,
        };
        for (shown, text) in std::mem::take(&mut self.pending.books) {
            let mut cx = self.sink.cx(&self.shared, &shown);
            let Some(b) = compile_book(&mut cx, &text, &against) else {
                continue;
            };
            self.book_ids
                .claim(&mut self.sink.errors, &shown, yog_ids::kind::BOOK, &b.id);
            self.books.push(b);
        }
    }

    /// The inventory screens blocks of this mod open must be its screens
    /// (another mod's ids can't be checked here).
    fn inventories(&mut self) {
        let Some(ns) = self.mod_ns else { return };
        let screens = inventory_screens(&self.shared.root.join(yog_ids::project::SRC));
        for (file, line, id) in &self.sink.inventories {
            if yog_ids::split(id).0 == ns && !screens.contains(id) {
                self.sink.errors.push(
                    problem(format!(
                        "block.inventory `{id}`: no .yui <Screen id=\"{id}\"> with <Slot>s in src/"
                    ))
                    .in_file(file)
                    .at_line(*line),
                );
            }
        }
    }

    /// Everything made, unless something was wrong: then every problem.
    fn finish(
        mut self,
        defs: &mut Defs,
        mut made: Made,
        known: &refs::Known,
    ) -> Result<Compiled, String> {
        // Armor's worn look where 1.21.4+ looks for it.
        for m in &defs.armor_materials {
            looks::equipment(
                self.shared.root,
                &mut self.out,
                &m.texture,
                &mut self.sink.warnings,
            );
        }
        self.out.warnings = std::mem::take(&mut self.sink.warnings);
        if !self.sink.errors.is_empty() {
            crate::diagnostics::report_all(&self.sink.errors);
            return Err(format!(
                "invalid blocks/items in {CONTENT_DIR}/:{}{}",
                crate::diagnostics::LIST_SEP,
                crate::diagnostics::listed(&self.sink.errors)
            ));
        }
        let potions = std::mem::take(&mut self.potions);
        made.definitions
            .extend(potions.into_iter().map(|(id, potion)| {
                (
                    yog_ids::kind::POTION,
                    id,
                    yog_data::binary::encode(&potion.write()),
                )
            }));
        self.count();
        self.data_files(&made);
        self.out.registrations =
            registrations(&self.entries, &self.entities, &made.dimensions, defs);
        self.made_into(defs, made);
        let content = Content {
            entries: &self.entries,
            recipes: &self.recipes,
            books: &self.books,
            bodies: &self.bodies,
            entities: &self.entities,
            fonts: &self.fonts,
            defs,
            textures: self.shared.own_textures,
        };
        if content.registers_anything()
            && !mentions_content_macro(&self.shared.root.join(yog_ids::project::SRC))
        {
            self.out.warnings.push(caveat("the mod has content (blocks, items, particles, videos, …), but src/ never calls `yog_api::content!();` — it isn't registered"));
        }
        self.out.source = Some(rust_source(&content));
        version_overlays(&mut self.out, &known.datapack);
        Ok(self.out)
    }

    /// How many of each the build says it made.
    fn count(&mut self) {
        self.out.blocks = self
            .entries
            .iter()
            .filter(|e| e.kind == Format::Block)
            .count();
        self.out.items = self.entries.len() - self.out.blocks;
        self.out.recipes = self.recipes.len();
        self.out.books = self.books.len();
    }

    /// The data the code includes: books, bodies, fonts, entities, the mod's
    /// own kinds of recipe, dimensions, loot modifiers, definitions.
    fn data_files(&mut self, made: &Made) {
        let files = &mut self.out.data_files;
        files.extend(self.books.iter().map(|b| (b.path(), b.data.clone())));
        files.extend(self.bodies.iter().map(|b| (b.path(), b.data.clone())));
        files.extend(self.fonts.iter().map(|f| (f.path(), f.data.clone())));
        files.extend(self.entities.iter().map(|e| (e.path(), e.data.clone())));
        files.extend(self.recipes.iter().filter_map(Recipe::data_file));
        files.extend(
            made.dimensions
                .iter()
                .map(|(id, data)| (dimension_path(id), data.clone())),
        );
        files.extend(
            made.loot
                .iter()
                .map(|(id, data)| (loot_path(id), data.clone())),
        );
        files.extend(
            made.definitions
                .iter()
                .map(|(kind, id, data)| (definition_path(kind, id), data.clone())),
        );
    }

    /// What the `.ydef`s made, into what the code is generated from; ores'
    /// files and a tool level above the game's tag of what its tools can't mine.
    fn made_into(&mut self, defs: &mut Defs, made: Made) {
        for (files, feature) in made.ores {
            self.out.outputs.extend(files);
            defs.biome_features.push(feature);
        }
        for m in &defs.tool_materials {
            if yog_ids::split(&m.incorrect).0 != MINECRAFT {
                let file = datapack::tag_file(yog_ids::kind::BLOCK, &m.incorrect);
                self.out
                    .tags
                    .entry(datapack::overlay(
                        &yog_registry::ToolLadder::game().incorrect_when,
                        &file,
                    ))
                    .or_default();
            }
        }
        defs.definitions = made
            .definitions
            .into_iter()
            .map(|(kind, id, _)| (kind, id))
            .collect();
        defs.loot_modifiers = made.loot.into_iter().map(|(id, _)| id).collect();
        defs.dimensions = made.dimensions.into_iter().map(|(id, _)| id).collect();
        defs.advancements = made.advancements;
        defs.tables = made.tables;
    }
}

/// Whether what a file names exists: a level, ladder or material of the
/// game's or the `.ydef`s'; a tag of one of its kinds; content.
fn check_wanted(wanted: &Wanted, defs: &Defs, known: &refs::Known) -> Result<(), String> {
    use yog_formats::def::{ArmorMaterial, SoundGroup, ToolMaterial};
    use yog_ids::kind;
    let found = |there: bool, what: &str, id: &str| {
        if there {
            Ok(())
        } else {
            Err(format!("no {what} {id} — the game's or a .ydef's"))
        }
    };
    match wanted {
        Wanted::Tag { kinds, id } => {
            let mut found = kinds.iter().map(|k| known.check_tag(k, id));
            let first = found.next().unwrap_or(Ok(()));
            if first.is_ok() || found.any(|f| f.is_ok()) {
                Ok(())
            } else {
                first
            }
        }
        Wanted::Ref(r) => match r.kind.as_str() {
            kind::LEVEL => found(defs.level(&r.id).is_some(), "level", &r.id),
            kind::LADDER => found(defs.order.contains_key(&r.id), "ladder", &r.id),
            kind::TOOL_MATERIAL => found(
                defs.tool_materials.iter().any(|m| m.id == r.id)
                    || ToolMaterial::built_in(&r.id).is_some(),
                "tool material",
                &r.id,
            ),
            kind::ARMOR_MATERIAL => found(
                defs.armor_materials.iter().any(|m| m.id == r.id)
                    || ArmorMaterial::built_in(&r.id).is_some(),
                "armor material",
                &r.id,
            ),
            kind::SOUND_GROUP => found(
                defs.sound_groups.iter().any(|g| g.id == r.id)
                    || SoundGroup::built_in(&r.id).is_some(),
                "sound group",
                &r.id,
            ),
            other => refs::RefKind::from_word(other).map_or(Ok(()), |k| known.check(k, &r.id)),
        },
    }
}
