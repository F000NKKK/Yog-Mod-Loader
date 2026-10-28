//! `.yblock` / `.yitem` content files — a mod's blocks and items: their
//! properties, 3D model, block states and item form, in one JSON file each,
//! edited in Yog-IDLE's 3D model editor — and `.yrecipe` files, one recipe
//! each (crafting, shapeless, smelting), edited in its recipe editor. See
//! Yog-IDLE's `docs/blocks-items.md` for the formats.
//!
//! `yog build` reads `content/**/*.yblock` and `content/**/*.yitem` and
//!
//! - writes Minecraft's JSON for them: block models, blockstates, item
//!   models, 1.21.4+ item definitions (`assets/<ns>/items/`), block loot
//!   tables, the `mineable/<tool>` and `needs_<level>_tool` block tags;
//! - puts their names and tooltips into the lang files (a `@<Set>.<name>`
//!   text of a `.yres` file under the key Minecraft looks up, a literal into
//!   `en_us`);
//! - builds the mod's textures (`.ytexture`, see `textures`);
//! - generates `obj/generated/yog-content.g.rs` — `Blocks`/`Items`
//!   with an id constant and a `BlockDef`/`ItemDef` builder per entry and
//!   a `register` function — pulled in by `yog_api::content!();`, which
//!   also registers them (unless an entry says `"register": "manual"`) and
//!   every recipe.

pub(crate) mod formats;
pub(crate) mod media;
pub(crate) mod pipeline;

mod import;
pub(crate) use import::font_copy_path;
pub(crate) mod refs;

pub use import::{Imported, import_command};
pub use pipeline::compile::compile;
pub use pipeline::outputs::{OutputSet, TAGS_FILE, TagLists, merge_outputs, read_tags};
pub use pipeline::source::{GENERATED_PARTS_DIR, Source, write_source};
pub(crate) use yog_formats::content::vocab::TEMPLATES;

use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use yog_ids::Format;

/// Where content files live, under the mod root.
pub const CONTENT_DIR: &str = yog_ids::project::CONTENT;
/// The generated Rust code, under the mod root.
pub const GENERATED_FILE: &str = yog_ids::concatcp!(
    yog_ids::project::GENERATED,
    yog_ids::sep::PATH,
    "yog-content.g.rs"
);

/// The generated Yog Script, under the mod root: the content's ids by name
/// for the mod's `.ys` (one of its sources when it's built).
pub const GENERATED_SCRIPT: &str = yog_ids::concatcp!(
    yog_ids::project::GENERATED,
    yog_ids::sep::PATH,
    "yog-content.g.",
    yog_ids::Format::Script.ext()
);

/// A generated file: path inside the `.yog` and its bytes.
pub(crate) type Output = (String, Vec<u8>);

/// The formats a content file under `content/` is written in.
const CONTENT: [Format; 9] = [
    Format::Block,
    Format::Item,
    Format::Recipe,
    Format::Book,
    Format::Def,
    Format::Model,
    Format::Anim,
    Format::Entity,
    Format::Font,
];

/// The content format of the file at `path`.
fn content_format(path: &Path) -> Option<Format> {
    Format::of_path(path).filter(|f| CONTENT.contains(f))
}

/// Everything `yog build` makes of a mod's content files.
#[derive(Debug, Default)]
pub struct Compiled {
    /// Models, blockstates, item definitions, loot tables, textures.
    pub outputs: Vec<Output>,
    /// Lang entries: `(namespace, locale)` → key → text, merged into the
    /// lang files by [`merge_outputs`].
    lang: Lang,
    /// `.yres` text keys to copy under another key: (from, to).
    lang_aliases: Vec<(String, String)>,
    /// Tag path (`data/minecraft/tags/block/...json`) → its entries.
    tags: BTreeMap<String, formats::defs::data::TagEntries>,
    /// The generated Rust code; `None` when the mod has no content files.
    pub source: Option<Source>,
    /// The generated Yog Script ([`GENERATED_SCRIPT`]); `None` when the
    /// content has no ids to name.
    pub script: Option<String>,
    /// Data the code includes (`include_bytes!`), by path under
    /// `obj/generated/`: the books, as Yog Data.
    pub data_files: Vec<(String, Vec<u8>)>,
    /// Each block, item and entity file's (kind, id): what the mod's
    /// Yog Script content declarations bind to.
    pub ids: Vec<(String, String)>,
    /// What's suspect but builds.
    pub warnings: Vec<crate::diagnostics::Diagnostic>,
    /// [`yog_registry::REGISTRATIONS_FILE`]: what the mod registers at the
    /// game's start — packaged by dev builds only, for hot reload.
    pub registrations: String,
    /// The pictures (`.yres` images' files) an atlas took in: left out of the build.
    pub packed: BTreeSet<String>,
    /// Textures the game loads as files of their own as well (a tab's
    /// background): their pictures stay in the build though packed.
    loose: BTreeSet<String>,
    /// How many blocks, items, recipes and books it compiled.
    pub blocks: usize,
    /// See [`Compiled::blocks`].
    pub items: usize,
    /// See [`Compiled::blocks`].
    pub recipes: usize,
    /// See [`Compiled::blocks`].
    pub books: usize,
}

pub(crate) use yog_ids::{valid_id, valid_namespace, valid_path};

/// Every content file (`CONTENT`) under `content/` of the mod at `root`, sorted.
pub(crate) fn find(root: &Path) -> Vec<PathBuf> {
    yog_analysis::files::under(&root.join(CONTENT_DIR), &|p| content_format(p).is_some())
}

fn pretty(v: &Value) -> Vec<u8> {
    let mut s = serde_json::to_string_pretty(v).unwrap_or_default();
    s.push('\n');
    s.into_bytes()
}

/// The mod's lang: (namespace, locale) → key → text.
type Lang = BTreeMap<(String, String), BTreeMap<String, String>>;

#[cfg(test)]
mod tests;

/// Whole numbers below this are written as integers.
const WHOLE_BELOW: f64 = 1e15;

/// A number in the game's JSON: integral values without a fraction (`16`, not `16.0`).
pub(crate) fn num_value(n: f64) -> serde_json::Value {
    if n.fract() == 0.0 && n.abs() < WHOLE_BELOW {
        serde_json::Value::from(n as i64)
    } else {
        serde_json::Number::from_f64(n)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null)
    }
}
