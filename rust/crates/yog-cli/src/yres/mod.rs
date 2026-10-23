//! `.yres` resource files — a mod's texts, images, sounds, files, colors,
//! numbers and flags, like a C# project's `Resources/*.resx`. See
//! Yog-IDLE's `docs/yres.md` for the format.
//!
//! `yog build` reads `resources/**/*.yres`: `<Set>.yres` is the neutral
//! (en_us) file of resource set `<Set>`, `<Set>.<locale>.yres` its culture
//! variants (texts only). It
//!
//! - writes the texts into `assets/<mod id>/lang/<locale>.json` (merged with
//!   a hand-written one), `{name}`/`{0}` placeholders turned into `%1$s`;
//! - bundles image/file/sound entries' files under `assets/<mod id>/...`
//!   (sounds as Ogg Vorbis: `.ysound` events name them);
//! - generates `.yog-build/generated/resources/<Set>.g.rs` — a type per
//!   set with a typed accessor per entry — and `resources.g.rs` including
//!   them, pulled in as `crate::res` by `yog_api::resources!();`;
//! - resolves `@<Set>.<name>` references for `.yui` screens and `yog.toml`.

mod accessors;
mod compile;
mod load;
mod names;
mod outputs;
mod placeholders;
mod resource_ids;

pub use compile::compile;
pub(crate) use names::{accessor_name, default_key, name_problem};
pub use outputs::write_sources;

use crate::diagnostics::Diagnostic;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

/// The locale of the neutral `<Set>.yres` files.
pub const NEUTRAL_LOCALE: &str = "en_us";
/// Where resource files live, under the mod root.
pub(crate) const RESOURCES_DIR: &str = yog_ids::project::RESOURCES;
/// Where the generated accessors go, under the mod root.
/// The file `yog_api::resources!()` includes; it includes one
/// `resources/<Set>.g.rs` per set (shown under its `.yres` in Yog-IDLE).
pub const GENERATED_FILE: &str = ".yog-build/generated/resources.g.rs";
/// Where the per-set `<Set>.g.rs` files go.
pub const GENERATED_DIR: &str = ".yog-build/generated/resources";

/// A generated file: path inside the `.yog` and its bytes.
pub(crate) type Output = (String, Vec<u8>);

pub use yog_formats::yres::{Entry, Kind, Value, parse, write};

/// What `.yui` screens and `yog.toml` can refer to as `@<Set>.<name>`.
#[derive(Debug, Clone, Default)]
pub struct Refs {
    /// `Set.name` → (translation key, neutral text).
    pub(crate) texts: HashMap<String, (String, String)>,
    /// `Set.name` → image source (`<mod id>:<texture>`).
    pub(crate) images: HashMap<String, String>,
    /// `Set.name` → color (`0xAARRGGBB`).
    pub(crate) colors: HashMap<String, u32>,
    /// `Set.name` → an image's picture, as the build writes it.
    pub(crate) pictures: HashMap<String, Picture>,
    /// `Set.name` → a sound's file as `sounds.json` names it (`<mod id>:<path>`).
    pub(crate) sounds: HashMap<String, String>,
    /// `Set.name` → a video's source bytes, which `.yvideo` files render.
    pub(crate) videos: HashMap<String, Vec<u8>>,
}

impl Refs {
    /// Text `r`: its translation key and neutral words.
    pub(crate) fn text(&self, r: &yog_data::Res) -> Result<&(String, String), String> {
        self.texts
            .get(&r.name())
            .ok_or_else(|| format!("`{r}`: no text `{}` in resources/*.yres", r.name()))
    }

    /// Sound `r`: its file as `sounds.json` names it.
    pub(crate) fn sound(&self, r: &yog_data::Res) -> Result<&str, String> {
        self.sounds
            .get(&r.name())
            .map(String::as_str)
            .ok_or_else(|| format!("`{r}`: no sound `{}` in resources/*.yres", r.name()))
    }

    /// Video `r`: its source's bytes.
    pub(crate) fn video(&self, r: &yog_data::Res) -> Result<&[u8], String> {
        self.videos
            .get(&r.name())
            .map(Vec::as_slice)
            .ok_or_else(|| format!("`{r}`: no video `{}` in resources/*.yres", r.name()))
    }

    /// Color `r` (`0xAARRGGBB`).
    pub fn color(&self, r: &yog_data::Res) -> Result<u32, String> {
        self.colors
            .get(&r.name())
            .copied()
            .ok_or_else(|| format!("`{r}`: no color `{}` in resources/*.yres", r.name()))
    }

    /// The translation key text `t` names — `@lang:key`, a thing's name, a
    /// `.yres` text; none for words, which aren't one.
    pub(crate) fn key_of(&self, t: &yog_data::Text) -> Result<Option<String>, String> {
        use yog_data::Text;
        Ok(match t {
            Text::Literal(_) => None,
            Text::Lang(k) => Some(k.clone()),
            Text::NameOf(r) => Some(yog_ids::lang_key(
                &r.kind,
                yog_formats::book::refs::bare(&r.id),
            )),
            Text::Res(r) => Some(self.text(r)?.0.clone()),
        })
    }
}

/// An image's picture: its PNG and the file the build writes it to — which
/// a texture packed into an atlas takes the place of.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Picture {
    pub(crate) png: Vec<u8>,
    pub(crate) output: String,
}

/// Everything `yog build` makes of a mod's resources.
#[derive(Debug, Default)]
pub struct Compiled {
    /// Files to add to the `.yog` (lang JSON, images, sounds, sounds.json) —
    /// they replace a hand-written asset at the same path (merged into them).
    /// The files it made: `(path in the .yog, bytes)`.
    pub outputs: Vec<Output>,
    /// `<Set>` → generated Rust source.
    pub sources: BTreeMap<String, String>,
    /// What screens and `yog.toml` can refer to (`@<Set>.<name>`).
    pub refs: Refs,
    /// What to warn of.
    pub warnings: Vec<Diagnostic>,
    /// Number of `.yres` files read.
    pub files: usize,
}

/// Every `resources/**/*.yres` of the mod at `root`, sorted.
pub(crate) fn find(root: &Path) -> Vec<PathBuf> {
    crate::files::under(&root.join(RESOURCES_DIR), &|p| yog_ids::Format::Res.is(p))
}

/// What's wrong with `e`'s value for its type, if anything — the build's
/// rule, which editors show too.
pub(crate) fn value_problem(e: &Entry) -> Option<String> {
    let media = |what: &str| format!("{:?}: {what} are bytes (`@bytes:…`)", e.name);
    match (e.kind, &e.value) {
        (Kind::Image | Kind::File | Kind::Sound | Kind::Video, Value::Bytes(_))
            if !yog_ids::valid_path(&e.name.replace('.', yog_ids::sep::PATH)) =>
        {
            Some(format!(
                "{:?}: Minecraft resource paths are lower case — only a-z, 0-9, `_`, `-`, `.`",
                e.name
            ))
        }
        (Kind::Image | Kind::File | Kind::Sound | Kind::Video, Value::Bytes(_)) => None,
        (Kind::Image, _) => Some(media("images")),
        (Kind::File, _) => Some(media("files")),
        (Kind::Sound, _) => Some(media("sounds")),
        (Kind::Video, _) => Some(media("videos")),
        (Kind::Color, _) => parse_color(e.text()).is_none().then(|| {
            format!(
                "{:?}: `{}` isn't a color — #RRGGBB or #AARRGGBB",
                e.name,
                e.text()
            )
        }),
        (Kind::Number, Value::Num(_))
        | (Kind::Bool, Value::Bool(_))
        | (Kind::String, Value::Str(_)) => None,
        (Kind::Number, _) => Some(format!("{:?}: a number", e.name)),
        (Kind::Bool, _) => Some(format!("{:?}: true or false", e.name)),
        (Kind::String, _) => Some(format!("{:?}: a text", e.name)),
    }
}

/// A color entry's value as ARGB, when it is one.
pub(crate) fn color_of(e: &Entry) -> Option<u32> {
    (e.kind == Kind::Color)
        .then(|| parse_color(e.text()))
        .flatten()
}

/// `#RRGGBB` / `#AARRGGBB`: a `.yres` color is written with its `#`.
fn parse_color(s: &str) -> Option<u32> {
    crate::yui::color(s).filter(|_| s.trim().starts_with(crate::yui::HEX_MARK))
}

/// Resolves a `yog.toml` value: `@<Set>.<name>` of a text → (neutral text,
/// Some(key)); anything else as is. Errors on an unknown reference.
pub fn resolve_text(refs: &Refs, value: &str) -> Result<(String, Option<String>), String> {
    match value.strip_prefix('@') {
        Some(rest) if rest.starts_with('@') => Ok((rest.to_string(), None)),
        Some(rest) => match refs.texts.get(rest) {
            Some((key, text)) => Ok((text.clone(), Some(key.clone()))),
            None => Err(format!("`{value}`: no text `{rest}` in resources/*.yres")),
        },
        None => Ok((value.to_string(), None)),
    }
}

/// Resolves a `yog.toml` image value: `@<Set>.<name>` → its image source.
pub fn resolve_image(refs: &Refs, value: &str) -> Result<String, String> {
    match value.strip_prefix('@') {
        Some(rest) => refs
            .images
            .get(rest)
            .cloned()
            .ok_or_else(|| format!("`{value}`: no image `{rest}` in resources/*.yres")),
        None => Ok(value.to_string()),
    }
}

#[cfg(test)]
mod tests;

/// A resources error (`YB0003`), where it is to be said by the caller.
pub(crate) fn problem(msg: impl std::fmt::Display) -> crate::diagnostics::Diagnostic {
    crate::diagnostics::Diagnostic::error(yog_ids::build_problem::RESOURCES, msg)
}

/// A resources warning (`YB0003`).
pub(crate) fn caveat(msg: impl std::fmt::Display) -> crate::diagnostics::Diagnostic {
    crate::diagnostics::Diagnostic::warning(yog_ids::build_problem::RESOURCES, msg)
}
