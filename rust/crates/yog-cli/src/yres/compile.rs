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
use crate::rust_code::is_keyword;
use crate::shown_path;

/// A set's texts: name → (translation key, parameters) — what its
/// culture variants are checked against.
type Texts = HashMap<String, (String, Vec<String>)>;

/// One entry's accessor as it's being written: where the entry is, its
/// set, its accessor's name and doc, its `@<Set>.<name>`.
struct Accessor<'a> {
    at: String,
    set: &'a str,
    acc: String,
    doc: String,
    reference: String,
}

/// Everything the sets compile into so far.
struct Build<'a> {
    root: &'a Path,
    mod_id: &'a str,
    errors: Vec<String>,
    c: Compiled,
    /// Locale → (key, text), in set order then file order.
    lang: BTreeMap<String, Vec<(String, String)>>,
    /// Each translation key's entry: where it's from.
    key_origin: HashMap<String, String>,
    /// Bundled path → the entry it comes from, and its bytes.
    bundled: BTreeMap<String, (String, Vec<u8>)>,
    /// Sound event → its `sounds.json` definition.
    sounds: BTreeMap<String, serde_json::Value>,
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
        sounds: BTreeMap::new(),
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
            let at = format!("{shown}:{}", e.line);
            let acc = accessor_name(&e.name);
            self.check_accessor(&at, e, &acc, &mut names);
            let a = Accessor {
                reference: format!("{}.{}", set.name, e.name),
                doc: accessors::doc(e, &shown),
                at,
                set: &set.name,
                acc,
            };
            if let Some(problem) = value_problem(e) {
                self.errors.push(format!("{}: {problem}", a.at));
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
        at: &str,
        e: &Entry,
        acc: &str,
        names: &mut HashMap<String, usize>,
    ) {
        if is_keyword(acc) {
            self.errors.push(format!(
                "{at}: {:?} gives the accessor `{acc}`, a Rust keyword — rename it",
                e.name
            ));
        }
        if let Some(first) = names.insert(acc.to_string(), e.line) {
            self.errors.push(format!(
                "{at}: {:?} gives the same accessor `{acc}` as the entry on line {first}",
                e.name
            ));
        }
    }

    /// A text: its key (its own, or the set's default) used once, its
    /// placeholders the accessor's parameters, its words into the neutral lang.
    fn text(&mut self, e: &Entry, a: &Accessor, texts: &mut Texts) -> Option<String> {
        let key = e
            .key
            .clone()
            .unwrap_or_else(|| default_key(a.set, self.mod_id, &e.name));
        if let Some(first) = self.key_origin.insert(key.clone(), a.at.clone()) {
            self.errors.push(format!(
                "{}: translation key {key:?} is also used at {first}",
                a.at
            ));
        }
        let read = segments(e.text()).and_then(|segs| params(&segs).map(|ps| (segs, ps)));
        let (segs, ps) = read
            .map_err(|err| self.errors.push(format!("{}: {:?}: {err}", a.at, e.name)))
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
        let path = e.name.replace(yog_ids::sep::EXT, yog_ids::sep::PATH);
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
            self.errors.push(format!(
                "{}: {:?} goes to {target}, like {:?}",
                a.at, e.name, other.0
            ));
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
            .map_err(|err| self.errors.push(format!("{}: {:?}: {err}", a.at, e.name)))
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

    /// A sound: an Ogg file, and its event (its own, or its name) in `sounds.json`.
    fn sound(
        &mut self,
        e: &Entry,
        a: &Accessor,
        bytes: &[u8],
        path: &str,
    ) -> (String, String, Vec<u8>) {
        let event = e
            .event
            .clone()
            .unwrap_or_else(|| e.name.to_ascii_lowercase());
        let mut def =
            serde_json::json!({ yog_ids::res::SOUNDS_WORD: [yog_ids::join(self.mod_id, path)] });
        if let Some(sub) = &e.subtitle {
            def["subtitle"] = serde_json::Value::String(sub.clone());
        }
        if self.sounds.insert(event.clone(), def).is_some() {
            self.errors
                .push(format!("{}: sound event {event:?} is defined twice", a.at));
        }
        let file = format!("{}{path}{}", yog_ids::res::SOUNDS, yog_ids::res::OGG);
        (
            yog_ids::asset_file(self.mod_id, &file),
            yog_ids::join(self.mod_id, &event),
            bytes.to_vec(),
        )
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
                    Err(err) => self
                        .errors
                        .push(format!("{shown}:{}: {:?}: {err}", t.line, t.name)),
                }
            }
            if !missing.is_empty() {
                self.c.warnings.push(format!(
                    "{shown}: {} text(s) not translated (shown in {NEUTRAL_LOCALE}): {}",
                    missing.len(),
                    preview(&missing)
                ));
            }
            if !extra.is_empty() {
                self.c.warnings.push(format!(
                    "{shown}: {} text(s) not in {}.yres, left out: {}",
                    extra.len(),
                    set.name,
                    preview(&extra)
                ));
            }
        }
    }

    /// The lang files, the media, `sounds.json` — unless something was
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
        if !self.sounds.is_empty() {
            match outputs::sounds_json(self.root, self.mod_id, std::mem::take(&mut self.sounds)) {
                Ok(out) => self.c.outputs.push(out),
                Err(e) => self.errors.extend(e),
            }
        }
        if !self.errors.is_empty() {
            return Err(format!(
                "invalid .yres resources:\n  {}",
                self.errors.join("\n  ")
            ));
        }
        Ok(self.c)
    }
}
