//! Compiling a mod's resources: every set's code, its texts into the lang
//! files, its media into the pack.

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::path::Path;

use super::load::{Set, load};
use super::names::{accessor_name, default_key};
use super::placeholders::{lang_text, params, segments};
use super::{
    Compiled, Entry, Kind, NEUTRAL_LOCALE, Picture, Value, accessors, outputs, value_problem,
};
use crate::diagnostics::Diagnostic;
use crate::rust_code::is_keyword;
use crate::shown_path;
use crate::yres::{caveat, problem};

/// A set's texts: name → (translation key, parameters) — what its
/// culture variants are checked against.
type Texts = HashMap<String, (String, Vec<String>)>;

/// One entry's accessor as it's being written: where the entry is, its
/// set, its accessor's name and doc, its `@<Set>.<name>`.
struct Accessor<'a> {
    at: Place,
    set: &'a str,
    acc: String,
    doc: String,
    reference: String,
}

/// Where an entry is: its file and its line.
#[derive(Debug, Clone)]
struct Place {
    file: String,
    line: usize,
}

impl Place {
    /// A problem with the entry here.
    fn problem(&self, msg: impl std::fmt::Display) -> Diagnostic {
        problem(msg).in_file(&self.file).at_line(self.line)
    }
}

/// As the text says it: `file:line`.
impl std::fmt::Display for Place {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.file, self.line)
    }
}

/// Everything the sets compile into so far.
struct Build<'a> {
    root: &'a Path,
    mod_id: &'a str,
    errors: Vec<Diagnostic>,
    c: Compiled,
    /// Locale → (key, text), in set order then file order.
    lang: BTreeMap<String, Vec<(String, String)>>,
    /// Each translation key's entry: where it's from.
    key_origin: HashMap<String, String>,
    /// Bundled path → the entry it comes from, and its bytes.
    bundled: BTreeMap<String, (String, Vec<u8>)>,
}

/// Compiles the mod's resources (see the module docs). Any error fails the
/// whole thing, listing every problem.
pub fn compile(root: &Path, mod_id: &str) -> Result<Compiled, String> {
    let mut errors = Vec::new();
    let sets = load(root, &mut errors);
    let c = Compiled {
        files: sets.values().map(|s| 1 + s.cultures.len()).sum(),
        ..Compiled::default()
    };
    let mut b = Build {
        root,
        mod_id,
        errors,
        c,
        lang: BTreeMap::new(),
        key_origin: HashMap::new(),
        bundled: BTreeMap::new(),
    };
    for set in sets.values() {
        let texts = b.set(set);
        b.cultures(set, &texts);
    }
    b.finish()
}

/// Up to [`PREVIEW`] of `names`, and how many more.
fn preview(names: &[&str]) -> String {
    let mut text = names
        .iter()
        .take(PREVIEW)
        .copied()
        .collect::<Vec<_>>()
        .join(", ");
    if names.len() > PREVIEW {
        let _ = write!(text, ", … (+{})", names.len() - PREVIEW);
    }
    text
}

/// How many names a warning shows.
const PREVIEW: usize = 5;

impl Build<'_> {
    /// Set `set`'s code; its texts.
    fn set(&mut self, set: &Set) -> Texts {
        let shown = shown_path(self.root, &set.neutral.path);
        let mut code = accessors::set_header(&shown, &set.name);
        let mut names: HashMap<String, usize> = HashMap::new();
        let mut texts = Texts::new();
        for e in &set.neutral.entries {
            let at = Place {
                file: shown.clone(),
                line: e.line,
            };
            let acc = accessor_name(&e.name);
            self.check_accessor(&at, e, &acc, &mut names);
            let a = Accessor {
                reference: format!("{}.{}", set.name, e.name),
                doc: accessors::doc(e, &shown),
                at,
                set: &set.name,
                acc,
            };
            if let Some(found) = value_problem(e) {
                self.errors.push(a.at.problem(found));
                continue;
            }
            let written = match e.kind {
                Kind::String => self.text(e, &a, &mut texts),
                Kind::Image | Kind::File | Kind::Sound => self.medium(e, &a),
                Kind::Color | Kind::Number | Kind::Bool => self.value(e, &a),
            };
            code.push_str(&written.unwrap_or_default());
        }
        code.push_str("}\n");
        self.c.sources.insert(set.name.clone(), code);
        texts
    }

    /// An accessor's name: not a keyword, one entry's.
    fn check_accessor(
        &mut self,
        at: &Place,
        e: &Entry,
        acc: &str,
        names: &mut HashMap<String, usize>,
    ) {
        if is_keyword(acc) {
            self.errors.push(at.problem(format!(
                "{:?} gives the accessor `{acc}`, a Rust keyword — rename it",
                e.name
            )));
        }
        if let Some(first) = names.insert(acc.to_string(), e.line) {
            self.errors.push(at.problem(format!(
                "{:?} gives the same accessor `{acc}` as the entry on line {first}",
                e.name
            )));
        }
    }

    /// A text: its key (its own, or the set's default) used once, its
    /// placeholders the accessor's parameters, its words into the neutral lang.
    fn text(&mut self, e: &Entry, a: &Accessor, texts: &mut Texts) -> Option<String> {
        let key = e
            .key
            .clone()
            .unwrap_or_else(|| default_key(a.set, self.mod_id, &e.name));
        if let Some(first) = self.key_origin.insert(key.clone(), a.at.to_string()) {
            self.errors
                .push(a.at.problem(format!("translation key {key:?} is also used at {first}")));
        }
        let read = segments(e.text()).and_then(|segs| params(&segs).map(|ps| (segs, ps)));
        let (segs, ps) = read
            .map_err(|err| {
                self.errors
                    .push(a.at.problem(format!("{:?}: {err}", e.name)))
            })
            .ok()?;
        if let Ok(text) = lang_text(&segs, &ps) {
            self.lang
                .entry(NEUTRAL_LOCALE.into())
                .or_default()
                .push((key.clone(), text));
        }
        let code = accessors::text(&a.acc, &key, &ps, &a.doc);
        self.c
            .refs
            .texts
            .insert(a.reference.clone(), (key.clone(), e.text().to_string()));
        texts.insert(e.name.clone(), (key, ps));
        Some(code)
    }

    /// An image, file or sound: its bytes into the pack where its name
    /// (dotted: `block.ruby` → `block/ruby`) says, its accessor its id.
    fn medium(&mut self, e: &Entry, a: &Accessor) -> Option<String> {
        let Value::Bytes(bytes) = &e.value else {
            return None;
        };
        let path = e.name.replace(yog_ids::sep::DOT, yog_ids::sep::PATH);
        let (target, id, data) = match e.kind {
            Kind::Image => self.image(e, a, bytes, &path)?,
            Kind::Sound => self.sound(e, a, bytes, &path),
            _ => (
                yog_ids::asset_file(self.mod_id, &path),
                yog_ids::join(self.mod_id, &path),
                bytes.clone(),
            ),
        };
        if let Some(other) = self.bundled.insert(target.clone(), (e.name.clone(), data))
            && other.0 != e.name
        {
            self.errors
                .push(a.at.problem(format!("{:?} goes to {target}, like {:?}", e.name, other.0)));
        }
        Some(accessors::medium(&a.acc, &id, &a.doc))
    }

    /// An image: as a PNG texture, its picture kept for what draws it.
    fn image(
        &mut self,
        e: &Entry,
        a: &Accessor,
        bytes: &[u8],
        path: &str,
    ) -> Option<(String, String, Vec<u8>)> {
        let png =
            yog_media::image::decode(bytes).and_then(|img| yog_media::image::encode_png(&img));
        let png = png
            .map_err(|err| {
                self.errors
                    .push(a.at.problem(format!("{:?}: {err}", e.name)))
            })
            .ok()?;
        let source = yog_ids::join(self.mod_id, path);
        let target = yog_ids::asset_file(self.mod_id, &yog_ids::texture_file(path));
        self.c
            .refs
            .images
            .insert(a.reference.clone(), source.clone());
        self.c.refs.pictures.insert(
            a.reference.clone(),
            Picture {
                png: png.clone(),
                output: target.clone(),
            },
        );
        Some((target, source, png))
    }

    /// A sound: an Ogg Vorbis file (converted when it isn't one), named
    /// by `.ysound` events (`refs.sounds`).
    fn sound(
        &mut self,
        e: &Entry,
        a: &Accessor,
        bytes: &[u8],
        path: &str,
    ) -> (String, String, Vec<u8>) {
        let ogg = crate::audio::to_ogg(bytes).unwrap_or_else(|err| {
            self.errors
                .push(a.at.problem(format!("{:?}: {err}", e.name)));
            Vec::new()
        });
        let id = yog_ids::join(self.mod_id, path);
        self.c.refs.sounds.insert(a.reference.clone(), id.clone());
        let file = format!("{}{path}{}", yog_ids::res::SOUNDS, yog_ids::res::OGG);
        (yog_ids::asset_file(self.mod_id, &file), id, ogg)
    }

    /// A color, number or flag: a const.
    fn value(&mut self, e: &Entry, a: &Accessor) -> Option<String> {
        if e.kind == Kind::Color {
            self.c
                .refs
                .colors
                .insert(a.reference.clone(), super::color_of(e)?);
        }
        accessors::value(e, &a.acc.to_ascii_uppercase(), &a.doc)
    }

    /// Set `set`'s culture variants: their texts into their langs, with the
    /// neutral texts' parameters; what's missing or extra said.
    fn cultures(&mut self, set: &Set, texts: &Texts) {
        for (locale, file) in &set.cultures {
            let shown = shown_path(self.root, &file.path);
            let translated = |name: &str| {
                file.entries
                    .iter()
                    .any(|t| t.name == name && !t.text().is_empty())
            };
            let missing: Vec<&str> = set
                .neutral
                .entries
                .iter()
                .filter(|e| e.kind == Kind::String && !translated(&e.name))
                .map(|e| e.name.as_str())
                .collect();
            let extra: Vec<&str> = file
                .entries
                .iter()
                .filter(|t| !texts.contains_key(&t.name))
                .map(|t| t.name.as_str())
                .collect();
            for t in file.entries.iter().filter(|t| !t.text().is_empty()) {
                let Some((key, ps)) = texts.get(&t.name) else {
                    continue;
                };
                match segments(t.text()).and_then(|segs| lang_text(&segs, ps)) {
                    Ok(text) => self
                        .lang
                        .entry(locale.clone())
                        .or_default()
                        .push((key.clone(), text)),
                    Err(err) => self.errors.push(
                        problem(format!("{:?}: {err}", t.name))
                            .in_file(&shown)
                            .at_line(t.line),
                    ),
                }
            }
            if !missing.is_empty() {
                self.c.warnings.push(
                    caveat(format!(
                        "{} text(s) not translated (shown in {NEUTRAL_LOCALE}): {}",
                        missing.len(),
                        preview(&missing)
                    ))
                    .in_file(&shown),
                );
            }
            if !extra.is_empty() {
                self.c.warnings.push(
                    caveat(format!(
                        "{} text(s) not in {}.yres, left out: {}",
                        extra.len(),
                        set.name,
                        preview(&extra)
                    ))
                    .in_file(&shown),
                );
            }
        }
    }

    /// The lang files and the media — unless something was
    /// wrong: then every problem.
    fn finish(mut self) -> Result<Compiled, String> {
        for (locale, entries) in std::mem::take(&mut self.lang) {
            match outputs::lang_file(self.root, self.mod_id, &locale, entries, &self.key_origin) {
                Ok(out) => self.c.outputs.push(out),
                Err(e) => self.errors.extend(e),
            }
        }
        for (target, (_, bytes)) in std::mem::take(&mut self.bundled) {
            self.c.outputs.push((target, bytes));
        }
        if !self.errors.is_empty() {
            crate::diagnostics::report_all(&self.errors);
            return Err(format!(
                "invalid .yres resources:{}{}",
                crate::diagnostics::LIST_SEP,
                crate::diagnostics::listed(&self.errors)
            ));
        }
        Ok(self.c)
    }
}
