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
//! - bundles image/file/sound entries' files under `assets/<mod id>/...`,
//!   plus a `sounds.json` for the sounds;
//! - generates `.yog-build/generated/resources/<Set>.g.rs` — a type per
//!   set with a typed accessor per entry — and `resources.g.rs` including
//!   them, pulled in as `crate::res` by `yog_api::resources!();`;
//! - resolves `@<Set>.<name>` references for `.yui` screens and `yog.toml`.

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use yog_data::{Reader, Schema, Seg, Shape, ValueRef};

/// The locale of the neutral `<Set>.yres` files.
pub const NEUTRAL_LOCALE: &str = "en_us";
/// Where resource files live, under the mod root.
pub const RESOURCES_DIR: &str = "resources";
/// Where the generated accessors go, under the mod root.
/// The file `yog_api::resources!()` includes; it includes one
/// `resources/<Set>.g.rs` per set (shown under its `.yres` in Yog-IDLE).
pub const GENERATED_FILE: &str = ".yog-build/generated/resources.g.rs";
/// Where the per-set `<Set>.g.rs` files go.
pub const GENERATED_DIR: &str = ".yog-build/generated/resources";

/// A generated file: path inside the `.yog` and its bytes.
pub type Output = (String, Vec<u8>);

// ── Parsing ───────────────────────────────────────────────────────────────────

pub use yog_formats::yres::{EntryObject, Kind};

/// The type of an entry that doesn't say, by its value.
fn kind_of(value: &Value) -> Kind {
    match value {
        Value::Str(_) => Kind::String,
        Value::Num(_) => Kind::Number,
        Value::Bool(_) => Kind::Bool,
        Value::Bytes(b) if b.starts_with(OGG_MAGIC) => Kind::Sound,
        Value::Bytes(b) if yog_media::image::decode(b).is_ok() => Kind::Image,
        Value::Bytes(_) => Kind::File,
    }
}

/// What an Ogg stream starts with — the game's sound format.
const OGG_MAGIC: &[u8] = b"OggS";

/// A scalar as written: a string, a number (its literal), a bool, or bytes
/// (a picture, a sound, a file — `@bytes:<base64>`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Str(String),
    Num(String),
    Bool(bool),
    Bytes(Vec<u8>),
}

/// One entry of a `.yres` file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub kind: Kind,
    pub value: Value,
    pub comment: Option<String>,
    pub key: Option<String>,
    pub event: Option<String>,
    pub subtitle: Option<String>,
    /// 1-based line of the name.
    pub line: usize,
}

impl Entry {
    /// An image entry named `name` holding the picture `bytes`.
    pub fn picture(name: &str, bytes: Vec<u8>) -> Entry {
        Entry { name: name.to_string(), kind: Kind::Image, value: Value::Bytes(bytes), comment: None, key: None, event: None, subtitle: None, line: 0 }
    }

    fn text(&self) -> &str {
        match &self.value {
            Value::Str(s) => s,
            _ => "",
        }
    }
}

/// What bytes are written as in text: `@bytes:<base64>`.
const BYTES_PREFIX: &str = yog_ids::concatcp!(yog_ids::sep::TYPED, yog_ids::typed::BYTES, yog_ids::sep::NAMESPACE);

/// A whole `.yres` file: named entries in file order.
struct YresFile(Vec<Entry>);

/// `v` as an entry's scalar value.
fn scalar(v: &ValueRef, cx: &mut Reader) -> Option<Value> {
    Some(match v {
        ValueRef::Bytes(b) => Value::Bytes(b.to_vec()),
        ValueRef::Str(s) if s.starts_with(BYTES_PREFIX) => match yog_data::typed::classify(s) {
            Ok(yog_data::Value::Bytes(b)) => Value::Bytes(b),
            Ok(_) => unreachable!("`@bytes:` classifies as bytes"),
            Err(e) => {
                cx.problem(e);
                return None;
            }
        },
        ValueRef::Str(_) => Value::Str(String::read(v, cx)?),
        ValueRef::Int(n) => Value::Num(n.to_string()),
        ValueRef::Float(f) => Value::Num(format!("{f:?}")),
        ValueRef::Bool(b) => Value::Bool(*b),
        other => {
            cx.expected("a string, a number or true/false", other);
            return None;
        }
    })
}

fn read_entry(name: &str, v: &ValueRef, cx: &mut Reader) -> Option<Entry> {
    let mut entry = Entry {
        name: name.to_string(),
        kind: Kind::String,
        value: Value::Str(String::new()),
        comment: None,
        key: None,
        event: None,
        subtitle: None,
        line: 0,
    };
    if !matches!(v, ValueRef::Map(_)) {
        entry.value = scalar(v, cx)?;
        entry.kind = kind_of(&entry.value);
        return Some(entry);
    }
    let o = EntryObject::read(v, cx);
    // The value is checked even when another field is wrong: every problem at once.
    let value = v.get("value").and_then(|val| cx.at(Seg::Key("value".into()), |cx| scalar(val, cx)));
    let (o, value) = (o?, value?);
    let kind = o.kind.unwrap_or(kind_of(&value));
    let fits = matches!(
        (kind, &value),
        (Kind::Number, Value::Num(_)) | (Kind::Bool, Value::Bool(_)) | (Kind::String | Kind::Color, Value::Str(_)) | (Kind::Image | Kind::File | Kind::Sound, Value::Bytes(_))
    );
    if !fits {
        let needs = match kind {
            Kind::Number => "number",
            Kind::Bool => "true/false",
            Kind::Image | Kind::File | Kind::Sound => "bytes (`@bytes:…`)",
            _ => "string",
        };
        cx.at(Seg::Key("value".into()), |cx| cx.problem(format!("a {} needs a {needs} value", kind.name())));
        return None;
    }
    let misplaced = [("key", o.key.is_some(), Kind::String), ("event", o.event.is_some(), Kind::Sound), ("subtitle", o.subtitle.is_some(), Kind::Sound)];
    for (field, present, only) in misplaced {
        if present && kind != only {
            cx.at(Seg::Key(field.into()), |cx| cx.problem(format!("only {} entries have a `{field}`", only.name())));
            return None;
        }
    }
    entry.kind = kind;
    entry.value = value;
    (entry.comment, entry.key, entry.event, entry.subtitle) = (o.comment, o.key, o.event, o.subtitle);
    Some(entry)
}

impl Schema for YresFile {
    fn shape() -> Shape {
        Shape::Map(EntryObject::shape)
    }

    fn read(v: &ValueRef<'_>, cx: &mut Reader) -> Option<Self> {
        let ValueRef::Map(map) = v else {
            cx.problem("expected `{` — a .yres file is one JSON object of named resources");
            return None;
        };
        let mut entries = Vec::with_capacity(map.len());
        let mut ok = true;
        // `$…` keys are the file's metadata (`$schema`); every other key names a resource.
        for (name, v) in map.iter().filter(|(k, _)| !k.starts_with('$')) {
            match cx.at(Seg::Key(name.to_string()), |cx| read_entry(name, v, cx)) {
                Some(e) => entries.push(e),
                None => ok = false,
            }
        }
        ok.then_some(YresFile(entries))
    }

    fn write(&self) -> yog_data::Value {
        let mut map = yog_data::Map::new();
        for e in &self.0 {
            let value = match &e.value {
                Value::Str(s) => yog_data::Value::Str(s.clone()),
                Value::Num(n) => n.parse::<i64>().map(yog_data::Value::Int).unwrap_or_else(|_| yog_data::Value::Float(n.parse().unwrap_or_default())),
                Value::Bool(b) => yog_data::Value::Bool(*b),
                Value::Bytes(b) => yog_data::Value::Bytes(b.clone()),
            };
            let plain = e.kind == kind_of(&e.value) && e.comment.is_none() && e.key.is_none() && e.event.is_none() && e.subtitle.is_none();
            if plain {
                map.insert(e.name.clone(), value);
                continue;
            }
            let o = EntryObject {
                kind: (e.kind != kind_of(&e.value)).then_some(e.kind),
                value,
                comment: e.comment.clone(),
                key: e.key.clone(),
                event: e.event.clone(),
                subtitle: e.subtitle.clone(),
            };
            map.insert(e.name.clone(), o.write());
        }
        yog_data::Value::Map(map)
    }
}

/// Parses `.yres` text. Every problem is reported, `line:column: name: message`
/// one per line; names aren't validated here (see [`valid_name`]).
pub fn parse(text: &str) -> Result<Vec<Entry>, String> {
    let doc = yog_data::text::parse(text).map_err(|e| e.to_string())?;
    let mut entries = yog_data::schema::from_doc::<YresFile>(text, &doc).map_err(|e| e.to_string())?.value.0;
    // Each name's line: the spans are in the same order as the names.
    let mut lines = HashMap::new();
    if let yog_data::Value::Map(map) = &doc.value {
        let mut line = 1;
        let mut at = 0;
        for ((name, _), spans) in map.iter().zip(&doc.spans.children) {
            let start = spans.key.unwrap_or(spans.span).start as usize;
            line += text.as_bytes()[at..start].iter().filter(|&&b| b == b'\n').count();
            at = start;
            lines.insert(name, line);
        }
    }
    for e in &mut entries {
        e.line = lines.get(e.name.as_str()).copied().unwrap_or(0);
    }
    Ok(entries)
}

/// `.yres` text for `entries`, as the IDE writes it.
pub fn write(entries: &[Entry]) -> String {
    yog_data::text::to_string_pretty(&YresFile(entries.to_vec()).write())
}

// ── Names ─────────────────────────────────────────────────────────────────────

const RUST_KEYWORDS: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern",
    "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub",
    "ref", "return", "self", "static", "struct", "super", "trait", "true", "type", "unsafe", "use",
    "where", "while", "abstract", "become", "box", "do", "final", "gen", "macro", "override",
    "priv", "try", "typeof", "unsized", "virtual", "yield",
];

fn ident_part(s: &str) -> bool {
    let mut chars = s.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// An entry name: identifiers joined by `.` (`title`, `screen.title`).
pub fn valid_name(name: &str) -> bool {
    yog_data::typed::is_res_key(name)
}

/// A resource set name: one identifier (`Strings`, `Icons`).
pub fn valid_set_name(name: &str) -> bool {
    ident_part(name) && !RUST_KEYWORDS.contains(&name)
}

/// Minecraft's locale codes: lower case, `en_us`, `pt_br`, `lol_us`, ...
pub fn valid_locale(locale: &str) -> bool {
    let Some((lang, region)) = locale.split_once('_') else {
        return false;
    };
    (2..=3).contains(&lang.len())
        && lang.bytes().all(|b| b.is_ascii_lowercase())
        && (2..=4).contains(&region.len())
        && region
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
}

/// The accessor for entry `name`: `screen.title` → `screen_title`,
/// `ModName` → `mod_name`.
pub fn accessor_name(name: &str) -> String {
    let mut out = String::new();
    for (n, part) in name.split('.').enumerate() {
        if n > 0 {
            out.push('_');
        }
        let mut prev_lower = false;
        for c in part.chars() {
            if c.is_ascii_uppercase() && prev_lower {
                out.push('_');
            }
            prev_lower = c.is_ascii_lowercase() || c.is_ascii_digit();
            out.push(c.to_ascii_lowercase());
        }
    }
    out
}

/// The default translation key of text `name` in set `set`.
pub fn default_key(set: &str, mod_id: &str, name: &str) -> String {
    match set {
        "Items" => format!("item.{mod_id}.{name}"),
        "Blocks" => format!("block.{mod_id}.{name}"),
        "Entities" => format!("entity.{mod_id}.{name}"),
        _ => format!("{mod_id}.{name}"),
    }
}

// ── Placeholders ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
enum Segment {
    Text(String),
    /// `{name}` or `{0}`.
    Arg(String),
}

/// Splits a text into literal parts and `{arg}` placeholders. Yog rich
/// text (`{b|…}`, `{#rrggbb|…}`: an opening with a `|` before any brace)
/// and its `\` escapes stay in the text as they are.
fn segments(text: &str) -> Result<Vec<Segment>, String> {
    let mut out = Vec::new();
    let mut lit = String::new();
    let mut spans = 0usize;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' if {
                let rest: String = chars.clone().take_while(|&x| x != '{' && x != '}').collect();
                rest.contains('|')
            } =>
            {
                lit.push('{');
                spans += 1;
            }
            '}' if spans > 0 => {
                lit.push('}');
                spans -= 1;
            }
            '{' if chars.peek() == Some(&'{') => {
                chars.next();
                lit.push_str("{{");
            }
            '}' if chars.peek() == Some(&'}') => {
                chars.next();
                lit.push_str("}}");
            }
            '{' => {
                let mut arg = String::new();
                loop {
                    match chars.next() {
                        Some('}') => break,
                        Some(c) => arg.push(c),
                        None => {
                            return Err("`{` without `}` (write `{{` for a literal brace)".into());
                        }
                    }
                }
                let arg = arg.trim().to_string();
                let numeric = !arg.is_empty() && arg.bytes().all(|b| b.is_ascii_digit());
                if !numeric && (!ident_part(&arg) || RUST_KEYWORDS.contains(&arg.as_str())) {
                    return Err(format!(
                        "placeholder `{{{arg}}}` must be a number or an identifier (write `{{{{` for a literal brace)"
                    ));
                }
                if !lit.is_empty() {
                    out.push(Segment::Text(std::mem::take(&mut lit)));
                }
                out.push(Segment::Arg(arg));
            }
            '}' => return Err("`}` without `{` (write `}}` for a literal brace)".into()),
            c => lit.push(c),
        }
    }
    if !lit.is_empty() {
        out.push(Segment::Text(lit));
    }
    Ok(out)
}

/// The parameters of a neutral text, in order: `{0}`, `{1}`, … by number
/// (no gaps), named ones by first appearance; one style per text.
fn params(segs: &[Segment]) -> Result<Vec<String>, String> {
    let args: Vec<&String> = segs
        .iter()
        .filter_map(|s| match s {
            Segment::Arg(a) => Some(a),
            Segment::Text(_) => None,
        })
        .collect();
    let numeric = args
        .iter()
        .filter(|a| a.bytes().all(|b| b.is_ascii_digit()))
        .count();
    if numeric > 0 && numeric < args.len() {
        return Err("mixes numbered `{0}` and named `{name}` placeholders — use one style".into());
    }
    let mut out: Vec<String> = Vec::new();
    if numeric > 0 {
        let mut nums: Vec<usize> = args.iter().filter_map(|a| a.parse().ok()).collect();
        nums.sort_unstable();
        nums.dedup();
        for (i, n) in nums.iter().enumerate() {
            if *n != i {
                return Err(format!(
                    "placeholder {{{i}}} is missing (numbered placeholders start at {{0}}, no gaps)"
                ));
            }
            out.push(n.to_string());
        }
    } else {
        for a in args {
            if !out.contains(a) {
                out.push(a.clone());
            }
        }
    }
    Ok(out)
}

/// Minecraft's form of a text: `%` → `%%`, the placeholder for parameter
/// `i` → `%{i+1}$s`. Errors on a placeholder not in `params`.
fn lang_text(segs: &[Segment], params: &[String]) -> Result<String, String> {
    let mut out = String::new();
    for s in segs {
        match s {
            Segment::Text(t) => out.push_str(&t.replace('%', "%%")),
            Segment::Arg(a) => {
                let pos = params
                    .iter()
                    .position(|p| p == a)
                    .ok_or_else(|| format!("placeholder `{{{a}}}` isn't in the neutral text"))?;
                let _ = write!(out, "%{}$s", pos + 1);
            }
        }
    }
    Ok(out)
}

fn param_ident(p: &str) -> String {
    if p.bytes().all(|b| b.is_ascii_digit()) {
        format!("arg{p}")
    } else {
        accessor_name(p)
    }
}

// ── Loading a mod's resources ────────────────────────────────────────────────

/// One `.yres` file.
#[derive(Debug, Clone)]
pub struct ResFile {
    pub path: PathBuf,
    pub entries: Vec<Entry>,
}

/// A resource set: its neutral file and culture variants.
#[derive(Debug, Clone)]
pub struct Set {
    pub name: String,
    pub neutral: ResFile,
    pub cultures: BTreeMap<String, ResFile>,
}

/// What `.yui` screens and `yog.toml` can refer to as `@<Set>.<name>`.
#[derive(Debug, Clone, Default)]
pub struct Refs {
    /// `Set.name` → (translation key, neutral text).
    pub texts: HashMap<String, (String, String)>,
    /// `Set.name` → image source (`<mod id>:<texture>`).
    pub images: HashMap<String, String>,
    /// `Set.name` → color (`0xAARRGGBB`).
    pub colors: HashMap<String, u32>,
    /// `Set.name` → an image's picture, as the build writes it.
    pub pictures: HashMap<String, Picture>,
}

/// An image's picture: its PNG and the file the build writes it to — which
/// a texture packed into an atlas takes the place of.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Picture {
    pub png: Vec<u8>,
    pub output: String,
}

/// Whether `path` is a valid resource path: lower case a-z, 0-9, `_`, `-`, `.`, `/`.
fn valid_resource_path(path: &str) -> bool {
    !path.is_empty() && path.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'_' | b'-' | b'.' | b'/'))
}

/// Everything `yog build` makes of a mod's resources.
#[derive(Debug, Default)]
pub struct Compiled {
    /// Files to add to the `.yog` (lang JSON, images, sounds, sounds.json) —
    /// they replace a hand-written asset at the same path (merged into them).
    pub outputs: Vec<Output>,
    /// `<Set>` → generated Rust source.
    pub sources: BTreeMap<String, String>,
    pub refs: Refs,
    pub warnings: Vec<String>,
    /// Number of `.yres` files read.
    pub files: usize,
}

/// Every `resources/**/*.yres` of the mod at `root`, sorted.
pub fn find(root: &Path) -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if yog_ids::Format::Res.is(&path) {
                out.push(path);
            }
        }
    }
    let mut out = Vec::new();
    walk(&root.join(RESOURCES_DIR), &mut out);
    out.sort();
    out
}

fn display(root: &Path, p: &Path) -> String {
    p.strip_prefix(root)
        .unwrap_or(p)
        .to_string_lossy()
        .replace('\\', "/")
}

/// Reads and checks every `.yres` of the mod — errors are `file:line: message`.
fn load(root: &Path, errors: &mut Vec<String>) -> BTreeMap<String, Set> {
    let mut neutral: BTreeMap<String, ResFile> = BTreeMap::new();
    let mut cultures: Vec<(String, String, ResFile)> = Vec::new();
    for path in find(root) {
        let shown = display(root, &path);
        let file_name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let Some((set, locale)) = yog_formats::yres::split_file_name(&file_name) else {
            continue;
        };
        if !valid_set_name(set) {
            errors.push(format!(
                "{shown}: `{set}` isn't a valid resource set name — use an identifier like Strings or Icons"
            ));
            continue;
        }
        if let Some(locale) = locale {
            if locale == NEUTRAL_LOCALE {
                errors.push(format!(
                    "{shown}: the neutral file {set}.yres is already {NEUTRAL_LOCALE} — put these texts there"
                ));
                continue;
            }
            if !valid_locale(locale) {
                errors.push(format!(
                    "{shown}: `{locale}` isn't a Minecraft locale code (lower case, like ru_ru or pt_br)"
                ));
                continue;
            }
        }
        let entries = match std::fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|t| parse(&t))
        {
            Ok(entries) => entries,
            Err(e) => {
                errors.push(format!("{shown}:{}", e.trim_start_matches("line ")));
                continue;
            }
        };
        for e in &entries {
            if !valid_name(&e.name) {
                errors.push(format!(
                    "{shown}:{}: {:?} isn't a valid name — identifiers joined by `.`, like screen.title",
                    e.line, e.name
                ));
            }
        }
        let file = ResFile { path, entries };
        match locale {
            None => {
                if let Some(other) = neutral.get(set) {
                    errors.push(format!(
                        "{shown}: resource set {set} is also defined by {}",
                        display(root, &other.path)
                    ));
                    continue;
                }
                neutral.insert(set.to_string(), file);
            }
            Some(locale) => cultures.push((set.to_string(), locale.to_string(), file)),
        }
    }
    let mut sets: BTreeMap<String, Set> = neutral
        .into_iter()
        .map(|(name, neutral)| {
            (
                name.clone(),
                Set {
                    name,
                    neutral,
                    cultures: BTreeMap::new(),
                },
            )
        })
        .collect();
    for (set, locale, file) in cultures {
        let shown = display(root, &file.path);
        let Some(target) = sets.get_mut(&set) else {
            errors.push(format!(
                "{shown}: no neutral {set}.yres for this culture variant"
            ));
            continue;
        };
        if let Some(other) = target.cultures.get(&locale) {
            errors.push(format!(
                "{shown}: {set} {locale} is also defined by {}",
                display(root, &other.path)
            ));
            continue;
        }
        for e in &file.entries {
            if e.kind != Kind::String || e.key.is_some() {
                errors.push(format!(
                    "{shown}:{}: {:?}: culture variants hold texts only (no type other than string, no `key`)",
                    e.line, e.name
                ));
            }
        }
        target.cultures.insert(locale, file);
    }
    sets
}

fn parse_color(s: &str) -> Option<u32> {
    let hex = s.trim().strip_prefix('#')?;
    let v = u32::from_str_radix(hex, 16).ok()?;
    match hex.len() {
        6 => Some(0xFF00_0000 | v),
        8 => Some(v),
        _ => None,
    }
}

/// The lang JSON of `entries` (key, text), order kept.
fn lang_json(entries: &[(String, String)]) -> String {
    let mut out = String::from("{");
    for (n, (k, v)) in entries.iter().enumerate() {
        out.push_str(if n == 0 { "\n  " } else { ",\n  " });
        out.push_str(&serde_json::to_string(k).unwrap_or_default());
        out.push_str(": ");
        out.push_str(&serde_json::to_string(v).unwrap_or_default());
    }
    out.push_str(if entries.is_empty() { "}\n" } else { "\n}\n" });
    out
}

fn preview(names: &[&str]) -> String {
    const SHOWN: usize = 5;
    let mut text = names
        .iter()
        .take(SHOWN)
        .copied()
        .collect::<Vec<_>>()
        .join(", ");
    if names.len() > SHOWN {
        let _ = write!(text, ", … (+{})", names.len() - SHOWN);
    }
    text
}

fn doc_line(s: &str) -> String {
    let one: String = s
        .chars()
        .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
        .collect();
    one.replace("*/", "* /")
}

/// Compiles the mod's resources (see the module docs). Any error fails the
/// whole thing, listing every problem.
pub fn compile(root: &Path, mod_id: &str) -> Result<Compiled, String> {
    let mut errors = Vec::new();
    let sets = load(root, &mut errors);
    let mut c = Compiled {
        files: sets.values().map(|s| 1 + s.cultures.len()).sum(),
        ..Compiled::default()
    };
    // locale → (key, text), in set order then file order.
    let mut lang: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    let mut key_origin: HashMap<String, String> = HashMap::new();
    // Bundled path → the entry it comes from, and its bytes.
    let mut bundled: BTreeMap<String, (String, Vec<u8>)> = BTreeMap::new();
    let mut sounds: BTreeMap<String, serde_json::Value> = BTreeMap::new();

    for set in sets.values() {
        let shown = display(root, &set.neutral.path);
        let mut code = String::new();
        let mut accessors: HashMap<String, usize> = HashMap::new();
        // Neutral text params per name, for checking the cultures.
        let mut text_params: HashMap<&str, (String, Vec<String>)> = HashMap::new();
        let _ = writeln!(
            code,
            "/// The resources of `{shown}`.\n#[allow(dead_code)]\npub struct {name};\n\n#[allow(dead_code, non_snake_case, clippy::all)]\nimpl {name} {{",
            name = set.name
        );
        for e in &set.neutral.entries {
            let at = format!("{shown}:{}", e.line);
            let acc = accessor_name(&e.name);
            if RUST_KEYWORDS.contains(&acc.as_str()) {
                errors.push(format!(
                    "{at}: {:?} gives the accessor `{acc}`, a Rust keyword — rename it",
                    e.name
                ));
            }
            if let Some(first) = accessors.insert(acc.clone(), e.line) {
                errors.push(format!(
                    "{at}: {:?} gives the same accessor `{acc}` as the entry on line {first}",
                    e.name
                ));
            }
            let upper = acc.to_ascii_uppercase();
            // Like Resources.Designer.cs: what the entry holds, then its comment.
            let doc = {
                let what = match e.kind {
                    Kind::String => format!(
                        "Looks up a localized string similar to \"{}\".",
                        doc_line(e.text())
                    ),
                    Kind::Image => format!("The image `{}`.", e.name),
                    Kind::File => format!("The file `{}`.", e.name),
                    Kind::Sound => format!("The sound `{}`.", e.name),
                    Kind::Color => format!("The color `{}`.", doc_line(e.text())),
                    Kind::Number | Kind::Bool => match &e.value {
                        Value::Num(n) => format!("The number `{n}`."),
                        Value::Bool(b) => format!("The flag `{b}`."),
                        Value::Str(v) => format!("The value `{}`.", doc_line(v)),
                        Value::Bytes(b) => format!("{} bytes.", b.len()),
                    },
                };
                let mut doc = format!("    /// {what}\n    ///\n    /// `{}`", e.name);
                if let Some(comment) = &e.comment {
                    let _ = write!(doc, " — {}", doc_line(comment));
                }
                let _ = writeln!(doc, " ({shown}:{})", e.line);
                doc
            };
            let reference = format!("{}.{}", set.name, e.name);
            match e.kind {
                Kind::String => {
                    let key = e
                        .key
                        .clone()
                        .unwrap_or_else(|| default_key(&set.name, mod_id, &e.name));
                    if let Some(first) = key_origin.insert(key.clone(), at.clone()) {
                        errors.push(format!(
                            "{at}: translation key {key:?} is also used at {first}"
                        ));
                    }
                    let segs = match segments(e.text()) {
                        Ok(s) => s,
                        Err(err) => {
                            errors.push(format!("{at}: {:?}: {err}", e.name));
                            continue;
                        }
                    };
                    let ps = match params(&segs) {
                        Ok(p) => p,
                        Err(err) => {
                            errors.push(format!("{at}: {:?}: {err}", e.name));
                            continue;
                        }
                    };
                    if let Ok(text) = lang_text(&segs, &ps) {
                        lang.entry(NEUTRAL_LOCALE.into())
                            .or_default()
                            .push((key.clone(), text));
                    }
                    let _ = writeln!(
                        code,
                        "    /// Translation key of [`Self::{acc}`]: `{key}`.\n    pub const {upper}_KEY: &'static str = {key:?};"
                    );
                    code.push_str(&doc);
                    if ps.is_empty() {
                        let _ = writeln!(
                            code,
                            "    pub fn {acc}() -> ::std::string::String {{\n        ::yog_api::tr(Self::{upper}_KEY)\n    }}"
                        );
                    } else {
                        let idents: Vec<String> = ps.iter().map(|p| param_ident(p)).collect();
                        let sig = idents
                            .iter()
                            .map(|p| format!("{p}: impl ::std::fmt::Display"))
                            .collect::<Vec<_>>()
                            .join(", ");
                        let args = idents
                            .iter()
                            .map(|p| format!("&{p}"))
                            .collect::<Vec<_>>()
                            .join(", ");
                        let _ = writeln!(
                            code,
                            "    pub fn {acc}({sig}) -> ::std::string::String {{\n        ::yog_api::tr_args(Self::{upper}_KEY, &[{args}])\n    }}"
                        );
                    }
                    c.refs
                        .texts
                        .insert(reference, (key.clone(), e.text().to_string()));
                    text_params.insert(&e.name, (key, ps));
                }
                Kind::Image | Kind::File | Kind::Sound => {
                    let Value::Bytes(bytes) = &e.value else { continue };
                    // A medium's path is its name, dotted: `block.ruby` → `block/ruby`.
                    let path = e.name.replace('.', yog_ids::sep::PATH);
                    if !valid_resource_path(&path) {
                        errors.push(format!("{at}: {:?}: Minecraft resource paths are lower case — only a-z, 0-9, `_`, `-`, `.`", e.name));
                        continue;
                    }
                    let (target, id, data) = match e.kind {
                        Kind::Image => {
                            let png = match yog_media::image::decode(bytes).and_then(|img| yog_media::image::encode_png(&img)) {
                                Ok(png) => png,
                                Err(err) => {
                                    errors.push(format!("{at}: {:?}: {err}", e.name));
                                    continue;
                                }
                            };
                            let source = yog_ids::join(mod_id, &path);
                            let target = format!("{}/{mod_id}/{}", yog_ids::pack::ASSETS, yog_ids::texture_file(&path));
                            c.refs.images.insert(reference.clone(), source.clone());
                            c.refs.pictures.insert(reference, Picture { png: png.clone(), output: target.clone() });
                            (target, source, png)
                        }
                        Kind::Sound => {
                            if !bytes.starts_with(OGG_MAGIC) {
                                errors.push(format!("{at}: {:?}: sounds are Ogg Vorbis", e.name));
                                continue;
                            }
                            let event = e
                                .event
                                .clone()
                                .unwrap_or_else(|| e.name.to_ascii_lowercase());
                            let mut def =
                                serde_json::json!({ "sounds": [yog_ids::join(mod_id, &path)] });
                            if let Some(sub) = &e.subtitle {
                                def["subtitle"] = serde_json::Value::String(sub.clone());
                            }
                            if sounds.insert(event.clone(), def).is_some() {
                                errors
                                    .push(format!("{at}: sound event {event:?} is defined twice"));
                            }
                            (
                                format!("{}/{mod_id}/sounds/{path}.ogg", yog_ids::pack::ASSETS),
                                yog_ids::join(mod_id, &event),
                                bytes.clone(),
                            )
                        }
                        _ => (format!("{}/{mod_id}/{path}", yog_ids::pack::ASSETS), yog_ids::join(mod_id, &path), bytes.clone()),
                    };
                    if let Some(other) = bundled.insert(target.clone(), (e.name.clone(), data))
                        && other.0 != e.name
                    {
                        errors.push(format!("{at}: {:?} goes to {target}, like {:?}", e.name, other.0));
                    }
                    let _ = writeln!(
                        code,
                        "{doc}    pub const fn {acc}() -> &'static str {{\n        {id:?}\n    }}"
                    );
                }
                Kind::Color => match parse_color(e.text()) {
                    Some(v) => {
                        c.refs.colors.insert(reference, v);
                        let _ = writeln!(code, "{doc}    pub const {upper}: u32 = 0x{v:08X};");
                    }
                    None => errors.push(format!(
                        "{at}: {:?}: `{}` isn't a color — #RRGGBB or #AARRGGBB",
                        e.name,
                        e.text()
                    )),
                },
                Kind::Number => {
                    let Value::Num(lit) = &e.value else { continue };
                    let int = !lit.contains(['.', 'e', 'E']);
                    match (int, lit.parse::<i64>()) {
                        (true, Ok(v)) => {
                            let _ = writeln!(code, "{doc}    pub const {upper}: i64 = {v};");
                        }
                        _ => {
                            let v: f64 = lit.parse().unwrap_or_default();
                            let _ = writeln!(code, "{doc}    pub const {upper}: f64 = {v:?};");
                        }
                    }
                }
                Kind::Bool => {
                    let v = matches!(e.value, Value::Bool(true));
                    let _ = writeln!(code, "{doc}    pub const {upper}: bool = {v};");
                }
            }
        }
        code.push_str("}\n");
        c.sources.insert(set.name.clone(), code);

        for (locale, file) in &set.cultures {
            let shown = display(root, &file.path);
            let mut missing: Vec<&str> = Vec::new();
            for e in &set.neutral.entries {
                if e.kind == Kind::String
                    && !file
                        .entries
                        .iter()
                        .any(|t| t.name == e.name && !t.text().is_empty())
                {
                    missing.push(&e.name);
                }
            }
            let mut extra: Vec<&str> = Vec::new();
            for t in &file.entries {
                let Some((key, ps)) = text_params.get(t.name.as_str()) else {
                    extra.push(&t.name);
                    continue;
                };
                if t.text().is_empty() {
                    continue;
                }
                match segments(t.text()).and_then(|segs| lang_text(&segs, ps)) {
                    Ok(text) => lang
                        .entry(locale.clone())
                        .or_default()
                        .push((key.clone(), text)),
                    Err(err) => errors.push(format!("{shown}:{}: {:?}: {err}", t.line, t.name)),
                }
            }
            if !missing.is_empty() {
                c.warnings.push(format!(
                    "{shown}: {} text(s) not translated (shown in {NEUTRAL_LOCALE}): {}",
                    missing.len(),
                    preview(&missing)
                ));
            }
            if !extra.is_empty() {
                c.warnings.push(format!(
                    "{shown}: {} text(s) not in {}.yres, left out: {}",
                    extra.len(),
                    set.name,
                    preview(&extra)
                ));
            }
        }
    }

    // Lang files, merged into hand-written ones.
    for (locale, mut entries) in lang {
        let path = format!("assets/{mod_id}/lang/{locale}.json");
        if let Ok(text) = std::fs::read_to_string(root.join(&path)) {
            match parse(&text) {
                Ok(hand) => {
                    let mut merged: Vec<(String, String)> = Vec::new();
                    for h in hand {
                        if let Some(at) = key_origin.get(&h.name) {
                            errors.push(format!(
                                "{path}:{}: {:?} is also defined at {at}",
                                h.line, h.name
                            ));
                        }
                        merged.push((h.name.clone(), h.text().to_string()));
                    }
                    merged.append(&mut entries);
                    entries = merged;
                }
                Err(e) => errors.push(format!("{path}:{}", e.trim_start_matches("line "))),
            }
        }
        c.outputs.push((path, lang_json(&entries).into_bytes()));
    }
    for (target, (_, bytes)) in bundled {
        c.outputs.push((target, bytes));
    }
    if !sounds.is_empty() {
        let path = format!("assets/{mod_id}/sounds.json");
        let mut all = serde_json::Map::new();
        if let Ok(text) = std::fs::read_to_string(root.join(&path)) {
            match serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&text) {
                Ok(hand) => all = hand,
                Err(e) => errors.push(format!("{path}: {e}")),
            }
        }
        for (event, def) in sounds {
            if all.insert(event.clone(), def).is_some() {
                errors.push(format!(
                    "{path}: sound event {event:?} is also defined by a .yres sound"
                ));
            }
        }
        let json =
            serde_json::to_string_pretty(&serde_json::Value::Object(all)).unwrap_or_default();
        c.outputs.push((path, format!("{json}\n").into_bytes()));
    }

    if !errors.is_empty() {
        return Err(format!(
            "invalid .yres resources:\n  {}",
            errors.join("\n  ")
        ));
    }
    Ok(c)
}

const BANNER: &str = "// <auto-generated>\n//     Generated by `yog build` from {source} — don't edit,\n//     it's rewritten on every build.\n// </auto-generated>\n\n";

/// Writes `.yog-build/generated/resources/<Set>.g.rs` per set and
/// `.yog-build/generated/resources.g.rs` including them all (what
/// `yog_api::resources!()` pulls in as `crate::res`); deletes those of sets
/// that are gone.
pub fn write_sources(root: &Path, compiled: &Compiled) -> Result<(), String> {
    let dir = root.join(GENERATED_DIR);
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name
                .strip_suffix(".g.rs")
                .is_some_and(|set| !compiled.sources.contains_key(set))
            {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
    let mut index = BANNER.replace("{source}", "resources/**/*.yres");
    index.push_str("// Included by `yog_api::resources!()` as `crate::res`.\n");
    for (set, code) in &compiled.sources {
        let source = code
            .lines()
            .next()
            .and_then(|l| l.split('`').nth(1))
            .unwrap_or("resources/");
        let mut file = BANNER.replace("{source}", source);
        file.push_str(code);
        crate::write_if_changed(&dir.join(format!("{set}.g.rs")), &file)?;
        let _ = writeln!(
            index,
            "include!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/generated/resources/{set}.g.rs\"));"
        );
    }
    crate::write_if_changed(&root.join(GENERATED_FILE), &index)
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
mod tests {

    #[test]
    fn rich_text_spans_are_not_placeholders() {
        let seg = segments("{#8d6acc|Media} and {b|x} for {name}").unwrap();
        assert_eq!(seg, [Segment::Text("{#8d6acc|Media} and {b|x} for ".into()), Segment::Arg("name".into())]);
    }

    use super::*;

    #[test]
    fn parses_all_shapes_in_order() {
        let text = "{\n  // Comments are fine.\n  \"b\": \"B\",\n  \"a\": { \"value\": \"A \\u00e9\", \"comment\": \"note\", \"key\": \"x.y\" },\n  \"n\": 5,\n  \"f\": false,\n  \"c\": { \"type\": \"color\", \"value\": \"#FF0000\" },\n  \"r\": { \"type\": \"number\", \"value\": -1.5e2 },\n  \"at\": \"@@everyone\",\n}\n";
        let entries = parse(text).unwrap();
        assert_eq!(entries.len(), 7);
        assert_eq!(entries[0].name, "b");
        assert_eq!(entries[1].line, 4);
        assert_eq!(entries[1].value, Value::Str("A é".into()));
        assert_eq!(entries[1].key.as_deref(), Some("x.y"));
        assert_eq!(entries[2].kind, Kind::Number);
        assert_eq!(entries[3].value, Value::Bool(false));
        assert_eq!(entries[4].kind, Kind::Color);
        assert_eq!(entries[5].value, Value::Num("-150.0".into()));
        assert_eq!(entries[6].value, Value::Str("@everyone".into()));
        // Written back, it reads the same.
        let again = parse(&write(&entries)).unwrap();
        assert_eq!(again.iter().map(|e| (&e.name, e.kind, &e.value, &e.key)).collect::<Vec<_>>(), entries.iter().map(|e| (&e.name, e.kind, &e.value, &e.key)).collect::<Vec<_>>());
    }

    #[test]
    fn errors_carry_lines() {
        let err = parse("{\n  \"a\": \"1\",\n  \"a\": \"2\"\n}").unwrap_err();
        assert_eq!(err, "3:3: `a` is already set in this object");
        let err = parse("{\n  \"a\": { \"type\": \"bool\", \"value\": \"x\" }\n}").unwrap_err();
        assert_eq!(err, "2:35: a.value: a bool needs a true/false value");
        let err = parse("{\n\"a\": { \"value\": \"x\", \"note\": \"y\" }\n}").unwrap_err();
        assert!(err.contains("2:22: a.note: unknown field `note`"), "{err}");
        let err = parse("{\n\"a\": { \"type\": \"image\", \"value\": \"@bytes:AAAA\", \"key\": \"k\" }\n}").unwrap_err();
        assert!(err.contains("a.key: only string entries have a `key`"), "{err}");
        let err = parse("{\n  \"a\": \"1\"\n  \"b\": \"2\"\n}").unwrap_err();
        assert_eq!(err, "3:3: expected `,` or `}`");
        // Every problem at once.
        let err = parse("{\n  \"a\": [],\n  \"b\": { \"type\": \"img\", \"value\": \"@Strings.x\" }\n}").unwrap_err();
        assert_eq!(err.lines().collect::<Vec<_>>(), [
            "2:8: a: expected a string, a number or true/false, found a list",
            "3:18: b.type: unknown `img` — did you mean `image`?",
            "3:34: b.value: expected plain text, found a resource (write `@@` for a literal `@`)",
        ]);
        assert!(parse("[]").unwrap_err().contains("expected `{`"));
        assert!(parse("\u{FEFF}{ }").unwrap().is_empty());
    }

    #[test]
    fn names_and_keys() {
        assert_eq!(accessor_name("screen.title"), "screen_title");
        assert_eq!(accessor_name("ModName"), "mod_name");
        assert!(valid_name("ruby.tooltip") && !valid_name("1a") && !valid_name("a..b"));
        assert!(valid_locale("pt_br") && valid_locale("lol_us") && !valid_locale("EN_US"));
        assert_eq!(default_key("Items", "m", "ruby"), "item.m.ruby");
        assert_eq!(default_key("Strings", "m", "title"), "m.title");
        assert_eq!(
            yog_formats::yres::split_file_name("Strings.ru_ru.yres"),
            Some(("Strings", Some("ru_ru")))
        );
        assert_eq!(yog_formats::yres::split_file_name("Icons.yres"), Some(("Icons", None)));
    }

    #[test]
    fn placeholders_become_minecraft_format() {
        let segs = segments("{shown} of {total}: 100% {{ok}}").unwrap();
        let ps = params(&segs).unwrap();
        assert_eq!(ps, ["shown", "total"]);
        assert_eq!(lang_text(&segs, &ps).unwrap(), "%1$s of %2$s: 100%% {{ok}}");
        let ru = segments("из {total} показано {shown}").unwrap();
        assert_eq!(lang_text(&ru, &ps).unwrap(), "из %2$s показано %1$s");
        assert!(lang_text(&segments("{other}").unwrap(), &ps).is_err());
        let segs = segments("{1} and {0}").unwrap();
        assert_eq!(params(&segs).unwrap(), ["0", "1"]);
        assert!(params(&segments("{0} {name}").unwrap()).is_err());
        assert!(params(&segments("{1}").unwrap()).is_err());
        assert!(segments("a { b").is_err());
        assert!(segments("a } b").is_err());
    }

    fn temp_mod(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("yog-yres-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(RESOURCES_DIR)).unwrap();
        dir
    }

    #[test]
    fn compiles_a_mod() {
        let root = temp_mod("ok");
        let res = root.join(RESOURCES_DIR);
        std::fs::write(
            res.join("Strings.yres"),
            r#"{"title": {"value": "Mods", "comment": "c"}, "count": "{n} mods", "gone": "G"}"#,
        )
        .unwrap();
        std::fs::write(
            res.join("Strings.ru_ru.yres"),
            r#"{"title": "Моды", "count": "Модов: {n}", "x": "X"}"#,
        )
        .unwrap();
        std::fs::write(
            res.join("Icons.yres"),
            r##"{"images.logo": "@bytes:iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==", "accent": {"type": "color", "value": "#FF8800"}, "sounds.click": {"value": "@bytes:T2dnUw==", "event": "click"}, "max": 3, "on": true}"##,
        )
        .unwrap();
        std::fs::write(res.join("Items.yres"), r#"{"ruby": "Ruby"}"#).unwrap();
        std::fs::create_dir_all(root.join("assets/m/lang")).unwrap();
        std::fs::write(
            root.join("assets/m/lang/en_us.json"),
            r#"{"hand.key": "Hand"}"#,
        )
        .unwrap();

        let c = compile(&root, "m").unwrap();
        let out: BTreeMap<&str, String> = c
            .outputs
            .iter()
            .map(|(p, b)| (p.as_str(), String::from_utf8_lossy(b).into_owned()))
            .collect();
        assert_eq!(
            out["assets/m/lang/en_us.json"],
            "{\n  \"hand.key\": \"Hand\",\n  \"item.m.ruby\": \"Ruby\",\n  \"m.title\": \"Mods\",\n  \"m.count\": \"%1$s mods\",\n  \"m.gone\": \"G\"\n}\n"
        );
        assert_eq!(
            out["assets/m/lang/ru_ru.json"],
            "{\n  \"m.title\": \"Моды\",\n  \"m.count\": \"Модов: %1$s\"\n}\n"
        );
        assert!(out["assets/m/textures/images/logo.png"].starts_with("\u{89}PNG") || out.contains_key("assets/m/textures/images/logo.png"));
        assert_eq!(out["assets/m/sounds/sounds/click.ogg"], "OggS");
        assert!(out["assets/m/sounds.json"].contains("\"m:sounds/click\""));
        assert_eq!(c.refs.images["Icons.images.logo"], "m:images/logo");
        assert!(c.refs.pictures.contains_key("Icons.images.logo"));
        assert_eq!(
            c.refs.texts["Strings.title"],
            ("m.title".to_string(), "Mods".to_string())
        );
        let strings = &c.sources["Strings"];
        assert!(
            strings.contains("pub const TITLE_KEY: &'static str = \"m.title\";"),
            "{strings}"
        );
        assert!(
            strings.contains("pub fn count(n: impl ::std::fmt::Display)"),
            "{strings}"
        );
        assert!(
            strings.contains("::yog_api::tr_args(Self::COUNT_KEY, &[&n])"),
            "{strings}"
        );
        let icons = &c.sources["Icons"];
        assert!(
            icons.contains("pub const fn images_logo() -> &'static str {\n        \"m:images/logo\""),
            "{icons}"
        );
        assert!(
            icons.contains("pub const ACCENT: u32 = 0xFFFF8800;"),
            "{icons}"
        );
        assert!(icons.contains("pub const MAX: i64 = 3;"), "{icons}");
        assert!(icons.contains("pub const ON: bool = true;"), "{icons}");
        assert!(icons.contains("\"m:click\""), "{icons}");
        assert_eq!(c.warnings.len(), 2, "{:?}", c.warnings);
        assert!(
            c.warnings[0].contains("1 text(s) not translated") && c.warnings[0].contains("gone")
        );
        assert!(c.warnings[1].contains("left out: x"));

        write_sources(&root, &c).unwrap();
        let icons_rs =
            std::fs::read_to_string(root.join(GENERATED_DIR).join("Icons.g.rs")).unwrap();
        assert!(icons_rs.starts_with("// <auto-generated>"), "{icons_rs}");
        assert!(icons_rs.contains("pub struct Icons;"), "{icons_rs}");
        let index = std::fs::read_to_string(root.join(GENERATED_FILE)).unwrap();
        assert!(
            index.contains("/generated/resources/Icons.g.rs\"));"),
            "{index}"
        );
        assert!(
            index.contains("/generated/resources/Strings.g.rs\"));"),
            "{index}"
        );
        let (name, key) = resolve_text(&c.refs, "@Strings.title").unwrap();
        assert_eq!((name.as_str(), key.as_deref()), ("Mods", Some("m.title")));
        assert!(resolve_text(&c.refs, "@Strings.nope").is_err());
        assert_eq!(
            resolve_image(&c.refs, "@Icons.images.logo").unwrap(),
            "m:images/logo"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn reports_every_problem_with_file_and_line() {
        let root = temp_mod("errors");
        let res = root.join(RESOURCES_DIR);
        std::fs::write(
            res.join("Strings.yres"),
            "{\n  \"a\": \"A\",\n  \"b\": { \"value\": \"B\", \"key\": \"m.a\" },\n  \"img\": { \"type\": \"image\", \"value\": \"@bytes:AAAA\" },\n  \"bad name\": \"x\",\n  \"col\": { \"type\": \"color\", \"value\": \"red\" }\n}",
        )
        .unwrap();
        std::fs::write(res.join("Strings.en_us.yres"), "{}").unwrap();
        std::fs::write(res.join("Strings.de_de.yres"), "{\n  \"a\": 1\n}").unwrap();
        std::fs::write(res.join("Other.ru_ru.yres"), "{}").unwrap();
        let err = compile(&root, "m").unwrap_err();
        for needle in [
            "resources/Strings.yres:3: translation key \"m.a\" is also used at resources/Strings.yres:2",
            "resources/Strings.yres:4: \"img\": not a picture this reads",
            "resources/Strings.yres:5: \"bad name\" isn't a valid name",
            "resources/Strings.yres:6: \"col\": `red` isn't a color",
            "Strings.en_us.yres: the neutral file Strings.yres is already en_us",
            "resources/Strings.de_de.yres:2: \"a\": culture variants hold texts only",
            "resources/Other.ru_ru.yres: no neutral Other.yres",
        ] {
            assert!(err.contains(needle), "missing {needle:?} in:\n{err}");
        }
        let _ = std::fs::remove_dir_all(&root);
    }
}
