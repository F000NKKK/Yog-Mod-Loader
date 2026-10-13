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
use std::path::{Component, Path, PathBuf};

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

/// An entry's type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    String,
    Image,
    File,
    Sound,
    Color,
    Number,
    Bool,
}

impl Kind {
    fn parse(s: &str) -> Option<Kind> {
        Some(match s {
            "string" => Kind::String,
            "image" => Kind::Image,
            "file" => Kind::File,
            "sound" => Kind::Sound,
            "color" => Kind::Color,
            "number" => Kind::Number,
            "bool" => Kind::Bool,
            _ => return None,
        })
    }

    fn name(self) -> &'static str {
        match self {
            Kind::String => "string",
            Kind::Image => "image",
            Kind::File => "file",
            Kind::Sound => "sound",
            Kind::Color => "color",
            Kind::Number => "number",
            Kind::Bool => "bool",
        }
    }
}

/// A JSON scalar as written: a string, a number (its literal) or a bool.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Str(String),
    Num(String),
    Bool(bool),
}

/// One entry of a `.yres` file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub kind: Kind,
    pub value: Value,
    pub comment: Option<String>,
    pub key: Option<String>,
    pub texture: Option<String>,
    pub event: Option<String>,
    pub subtitle: Option<String>,
    /// 1-based line of the name.
    pub line: usize,
}

impl Entry {
    fn text(&self) -> &str {
        match &self.value {
            Value::Str(s) => s,
            _ => "",
        }
    }
}

/// Parses `.yres` text. Errors are `line N: message`. Duplicate names are
/// an error; names aren't validated here (see [`valid_name`]).
pub fn parse(text: &str) -> Result<Vec<Entry>, String> {
    let mut p = Parser {
        s: text.as_bytes(),
        i: 0,
        line: 1,
    };
    // A UTF-8 BOM is tolerated, as editors on Windows like to add one.
    if p.s.starts_with(b"\xEF\xBB\xBF") {
        p.i = 3;
    }
    p.ws();
    if !p.eat(b'{') {
        return Err(p.err("expected `{` — a .yres file is one JSON object of named resources"));
    }
    let mut entries: Vec<Entry> = Vec::new();
    let mut seen: HashMap<String, usize> = HashMap::new();
    p.ws();
    if !p.eat(b'}') {
        loop {
            p.ws();
            let line = p.line;
            let name = p
                .string()
                .map_err(|e| format!("{e} (expected a resource name)"))?;
            if let Some(first) = seen.get(&name) {
                return Err(format!(
                    "line {line}: duplicate name {name:?} (first on line {first})"
                ));
            }
            p.ws();
            if !p.eat(b':') {
                return Err(p.err("expected `:` after the name"));
            }
            p.ws();
            let mut entry = Entry {
                name: name.clone(),
                kind: Kind::String,
                value: Value::Str(String::new()),
                comment: None,
                key: None,
                texture: None,
                event: None,
                subtitle: None,
                line,
            };
            match p.peek() {
                Some(b'{') => p.entry_object(&mut entry)?,
                _ => {
                    entry.value = p.scalar().map_err(|e| {
                        format!("{e} — {name:?} must be a string or {{ \"value\": ..., \"type\": ..., \"comment\": ... }}")
                    })?;
                    entry.kind = match entry.value {
                        Value::Str(_) => Kind::String,
                        Value::Num(_) => Kind::Number,
                        Value::Bool(_) => Kind::Bool,
                    };
                }
            }
            seen.insert(name, line);
            entries.push(entry);
            p.ws();
            if p.eat(b',') {
                continue;
            }
            if p.eat(b'}') {
                break;
            }
            return Err(p.err("expected `,` or `}`"));
        }
    }
    p.ws();
    if p.i < p.s.len() {
        return Err(p.err("unexpected text after the closing `}`"));
    }
    Ok(entries)
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
    line: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }

    fn eat(&mut self, c: u8) -> bool {
        if self.peek() == Some(c) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn eat_word(&mut self, word: &str) -> bool {
        if self.s[self.i..].starts_with(word.as_bytes()) {
            self.i += word.len();
            true
        } else {
            false
        }
    }

    fn ws(&mut self) {
        while let Some(c) = self.peek() {
            match c {
                b'\n' => self.line += 1,
                b' ' | b'\t' | b'\r' => {}
                _ => return,
            }
            self.i += 1;
        }
    }

    fn err(&self, msg: &str) -> String {
        match self.peek() {
            None => format!("line {}: {msg}, found the end of the file", self.line),
            Some(_) => format!("line {}: {msg}", self.line),
        }
    }

    /// A string, number or `true`/`false`.
    fn scalar(&mut self) -> Result<Value, String> {
        match self.peek() {
            Some(b'"') => Ok(Value::Str(self.string()?)),
            Some(b't') if self.eat_word("true") => Ok(Value::Bool(true)),
            Some(b'f') if self.eat_word("false") => Ok(Value::Bool(false)),
            Some(c) if c == b'-' || c.is_ascii_digit() => {
                let start = self.i;
                while let Some(c) = self.peek() {
                    if c.is_ascii_digit() || matches!(c, b'-' | b'+' | b'.' | b'e' | b'E') {
                        self.i += 1;
                    } else {
                        break;
                    }
                }
                let lit = String::from_utf8_lossy(&self.s[start..self.i]).into_owned();
                if lit.parse::<f64>().is_err() {
                    return Err(format!("line {}: invalid number `{lit}`", self.line));
                }
                Ok(Value::Num(lit))
            }
            _ => Err(self.err("expected a value")),
        }
    }

    /// A JSON string literal, unescaped.
    fn string(&mut self) -> Result<String, String> {
        if !self.eat(b'"') {
            return Err(self.err("expected a string"));
        }
        let mut out: Vec<u8> = Vec::new();
        loop {
            let Some(c) = self.peek() else {
                return Err(self.err("unterminated string"));
            };
            self.i += 1;
            match c {
                b'"' => break,
                b'\n' => {
                    return Err(format!(
                        "line {}: unterminated string (strings can't span lines — use \\n)",
                        self.line
                    ))
                }
                b'\\' => {
                    let Some(e) = self.peek() else {
                        return Err(self.err("unterminated string"));
                    };
                    self.i += 1;
                    let ch = match e {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'/' => '/',
                        b'b' => '\u{8}',
                        b'f' => '\u{c}',
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        b'u' => self.unicode_escape()?,
                        other => {
                            return Err(format!(
                                "line {}: invalid escape `\\{}`",
                                self.line, other as char
                            ))
                        }
                    };
                    let mut buf = [0u8; 4];
                    out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                }
                c if c < 0x20 => {
                    return Err(format!(
                        "line {}: control character in a string — escape it",
                        self.line
                    ))
                }
                c => out.push(c),
            }
        }
        String::from_utf8(out)
            .map_err(|_| format!("line {}: the string isn't valid UTF-8", self.line))
    }

    fn hex4(&mut self) -> Result<u32, String> {
        let digits = self
            .s
            .get(self.i..self.i + 4)
            .ok_or_else(|| self.err("truncated \\u escape"))?;
        let text = std::str::from_utf8(digits).map_err(|_| self.err("invalid \\u escape"))?;
        let v = u32::from_str_radix(text, 16)
            .map_err(|_| format!("line {}: invalid \\u escape `\\u{text}`", self.line))?;
        self.i += 4;
        Ok(v)
    }

    fn unicode_escape(&mut self) -> Result<char, String> {
        let hi = self.hex4()?;
        let code = if (0xD800..0xDC00).contains(&hi) {
            if !(self.eat(b'\\') && self.eat(b'u')) {
                return Err(format!(
                    "line {}: lone surrogate in a \\u escape",
                    self.line
                ));
            }
            let lo = self.hex4()?;
            if !(0xDC00..0xE000).contains(&lo) {
                return Err(format!(
                    "line {}: invalid surrogate pair in a \\u escape",
                    self.line
                ));
            }
            0x10000 + ((hi - 0xD800) << 10) + (lo - 0xDC00)
        } else {
            hi
        };
        char::from_u32(code).ok_or_else(|| format!("line {}: invalid \\u escape", self.line))
    }

    /// `{ "type": ..., "value": ..., "comment": ..., ... }` into `entry`.
    fn entry_object(&mut self, entry: &mut Entry) -> Result<(), String> {
        let name = entry.name.clone();
        let start = self.line;
        self.eat(b'{');
        let mut value = None;
        let mut kind = None;
        let mut seen: Vec<String> = Vec::new();
        self.ws();
        if !self.eat(b'}') {
            loop {
                self.ws();
                let field_line = self.line;
                let field = self.string()?;
                self.ws();
                if !self.eat(b':') {
                    return Err(self.err("expected `:`"));
                }
                self.ws();
                if seen.contains(&field) {
                    return Err(format!(
                        "line {field_line}: {name:?}: duplicate field `{field}`"
                    ));
                }
                let v = self.scalar()?;
                let text = |v: Value| match v {
                    Value::Str(s) => Ok(s),
                    _ => Err(format!(
                        "line {field_line}: {name:?}: `{field}` must be a string"
                    )),
                };
                match field.as_str() {
                    "value" => value = Some(v),
                    "type" => {
                        let t = text(v)?;
                        kind = Some(Kind::parse(&t).ok_or_else(|| {
                            format!(
                                "line {field_line}: {name:?}: unknown type `{t}` (string, image, file, sound, color, number or bool)"
                            )
                        })?);
                    }
                    "comment" => entry.comment = Some(text(v)?),
                    "key" => entry.key = Some(text(v)?),
                    "texture" => entry.texture = Some(text(v)?),
                    "event" => entry.event = Some(text(v)?),
                    "subtitle" => entry.subtitle = Some(text(v)?),
                    other => {
                        return Err(format!(
                            "line {field_line}: {name:?}: unknown field `{other}` (value, type, comment, key, texture, event, subtitle)"
                        ))
                    }
                }
                seen.push(field);
                self.ws();
                if self.eat(b',') {
                    continue;
                }
                if self.eat(b'}') {
                    break;
                }
                return Err(self.err("expected `,` or `}`"));
            }
        }
        let value = value.ok_or_else(|| format!("line {start}: {name:?}: missing `value`"))?;
        let kind = kind.unwrap_or(match value {
            Value::Str(_) => Kind::String,
            Value::Num(_) => Kind::Number,
            Value::Bool(_) => Kind::Bool,
        });
        let fits = matches!(
            (kind, &value),
            (Kind::Number, Value::Num(_))
                | (Kind::Bool, Value::Bool(_))
                | (
                    Kind::String | Kind::Image | Kind::File | Kind::Sound | Kind::Color,
                    Value::Str(_)
                )
        );
        if !fits {
            return Err(format!(
                "line {start}: {name:?}: a {} needs a {} value",
                kind.name(),
                match kind {
                    Kind::Number => "number",
                    Kind::Bool => "true/false",
                    _ => "string",
                }
            ));
        }
        let misplaced = [
            ("key", entry.key.is_some(), Kind::String),
            ("texture", entry.texture.is_some(), Kind::Image),
            ("event", entry.event.is_some(), Kind::Sound),
            ("subtitle", entry.subtitle.is_some(), Kind::Sound),
        ];
        for (field, present, only) in misplaced {
            if present && kind != only {
                return Err(format!(
                    "line {start}: {name:?}: `{field}` is only for {} entries",
                    only.name()
                ));
            }
        }
        entry.kind = kind;
        entry.value = value;
        Ok(())
    }
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
    !name.is_empty() && name.split('.').all(ident_part)
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

/// `resources/.../Strings.ru_ru.yres` → `("Strings", Some("ru_ru"))`.
pub fn split_file_name(file_name: &str) -> Option<(&str, Option<&str>)> {
    let stem = file_name.strip_suffix(".yres")?;
    Some(match stem.split_once('.') {
        Some((set, locale)) => (set, Some(locale)),
        None => (stem, None),
    })
}

// ── Placeholders ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
enum Segment {
    Text(String),
    /// `{name}` or `{0}`.
    Arg(String),
}

/// Splits a text into literal parts and `{arg}` placeholders.
fn segments(text: &str) -> Result<Vec<Segment>, String> {
    let mut out = Vec::new();
    let mut lit = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' if chars.peek() == Some(&'{') => {
                chars.next();
                lit.push('{');
            }
            '}' if chars.peek() == Some(&'}') => {
                chars.next();
                lit.push('}');
            }
            '{' => {
                let mut arg = String::new();
                loop {
                    match chars.next() {
                        Some('}') => break,
                        Some(c) => arg.push(c),
                        None => {
                            return Err("`{` without `}` (write `{{` for a literal brace)".into())
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
                return Err(format!("placeholder {{{i}}} is missing (numbered placeholders start at {{0}}, no gaps)"));
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
            } else if path.extension().is_some_and(|e| e == "yres") {
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
        let Some((set, locale)) = split_file_name(&file_name) else {
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

/// A path of an entry, relative to its `.yres` file → the path relative to
/// `resources/` (`/`-separated, checked to stay inside and to exist).
fn resource_path(root: &Path, yres: &Path, value: &str) -> Result<(String, PathBuf), String> {
    let base = yres.parent().unwrap_or(root);
    let mut parts: Vec<String> = base
        .strip_prefix(root.join(RESOURCES_DIR))
        .unwrap_or(Path::new(""))
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    for c in Path::new(&value.replace('\\', "/")).components() {
        match c {
            Component::Normal(s) => parts.push(s.to_string_lossy().into_owned()),
            Component::CurDir => {}
            Component::ParentDir if !parts.is_empty() => {
                parts.pop();
            }
            _ => return Err(format!("`{value}` must stay inside {RESOURCES_DIR}/")),
        }
    }
    let rel = parts.join("/");
    if rel.is_empty() {
        return Err("the path is empty".into());
    }
    let full = root.join(RESOURCES_DIR).join(&rel);
    if !full.is_file() {
        return Err(format!("{RESOURCES_DIR}/{rel} doesn't exist"));
    }
    if !rel.bytes().all(|b| {
        b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'_' | b'-' | b'.' | b'/')
    }) {
        return Err(format!(
            "{RESOURCES_DIR}/{rel}: Minecraft resource paths are lower case — only a-z, 0-9, `_`, `-`, `.` and `/`"
        ));
    }
    Ok((rel, full))
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
    // Bundled path → the file it comes from.
    let mut bundled: BTreeMap<String, PathBuf> = BTreeMap::new();
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
                    Kind::Image => format!("The image `{}`.", doc_line(e.text())),
                    Kind::File => format!("The file `{}`.", doc_line(e.text())),
                    Kind::Sound => format!("The sound `{}`.", doc_line(e.text())),
                    Kind::Color => format!("The color `{}`.", doc_line(e.text())),
                    Kind::Number | Kind::Bool => match &e.value {
                        Value::Num(n) => format!("The number `{n}`."),
                        Value::Bool(b) => format!("The flag `{b}`."),
                        Value::Str(v) => format!("The value `{}`.", doc_line(v)),
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
                    let (rel, full) = match resource_path(root, &set.neutral.path, e.text()) {
                        Ok(p) => p,
                        Err(err) => {
                            errors.push(format!("{at}: {:?}: {err}", e.name));
                            continue;
                        }
                    };
                    let (target, id) = match e.kind {
                        Kind::Image => {
                            let Some(stem) = rel.strip_suffix(".png") else {
                                errors
                                    .push(format!("{at}: {:?}: images must be .png files", e.name));
                                continue;
                            };
                            let texture = e
                                .texture
                                .as_deref()
                                .map(|t| t.trim_matches('/').trim_end_matches(".png").to_string())
                                .unwrap_or_else(|| stem.to_string());
                            let source = format!("{mod_id}:{texture}");
                            c.refs.images.insert(reference, source.clone());
                            (format!("assets/{mod_id}/textures/{texture}.png"), source)
                        }
                        Kind::Sound => {
                            let Some(stem) = rel.strip_suffix(".ogg") else {
                                errors
                                    .push(format!("{at}: {:?}: sounds must be .ogg files", e.name));
                                continue;
                            };
                            let event = e
                                .event
                                .clone()
                                .unwrap_or_else(|| e.name.to_ascii_lowercase());
                            let mut def =
                                serde_json::json!({ "sounds": [format!("{mod_id}:{stem}")] });
                            if let Some(sub) = &e.subtitle {
                                def["subtitle"] = serde_json::Value::String(sub.clone());
                            }
                            if sounds.insert(event.clone(), def).is_some() {
                                errors
                                    .push(format!("{at}: sound event {event:?} is defined twice"));
                            }
                            (
                                format!("assets/{mod_id}/sounds/{stem}.ogg"),
                                format!("{mod_id}:{event}"),
                            )
                        }
                        _ => (format!("assets/{mod_id}/{rel}"), format!("{mod_id}:{rel}")),
                    };
                    if let Some(other) = bundled.insert(target.clone(), full.clone()) {
                        if other != full {
                            errors.push(format!(
                                "{at}: {:?} goes to {target}, like {}",
                                e.name,
                                display(root, &other)
                            ));
                        }
                    }
                    let _ = writeln!(
                        code,
                        "{doc}    pub const fn {acc}() -> &'static str {{\n        {id:?}\n    }}"
                    );
                }
                Kind::Color => match parse_color(e.text()) {
                    Some(v) => {
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
    for (target, source) in bundled {
        match std::fs::read(&source) {
            Ok(bytes) => c.outputs.push((target, bytes)),
            Err(e) => errors.push(format!("{}: {e}", display(root, &source))),
        }
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

fn write_if_changed(path: &Path, code: &str) -> Result<(), String> {
    if std::fs::read_to_string(path).ok().as_deref() == Some(code) {
        return Ok(()); // unchanged: cargo needn't rebuild
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    std::fs::write(path, code).map_err(|e| format!("{}: {e}", path.display()))
}

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
        write_if_changed(&dir.join(format!("{set}.g.rs")), &file)?;
        let _ = writeln!(
            index,
            "include!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/generated/resources/{set}.g.rs\"));"
        );
    }
    write_if_changed(&root.join(GENERATED_FILE), &index)
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
    use super::*;

    #[test]
    fn parses_all_shapes_in_order() {
        let entries = parse(
            "{\n  \"b\": \"B\",\n  \"a\": { \"value\": \"A \\u00e9\", \"comment\": \"note\", \"key\": \"x.y\" },\n  \"n\": 5,\n  \"f\": false,\n  \"c\": { \"type\": \"color\", \"value\": \"#FF0000\" },\n  \"r\": { \"type\": \"number\", \"value\": -1.5e2 }\n}\n",
        )
        .unwrap();
        assert_eq!(entries.len(), 6);
        assert_eq!(entries[0].name, "b");
        assert_eq!(entries[1].line, 3);
        assert_eq!(entries[1].value, Value::Str("A é".into()));
        assert_eq!(entries[1].key.as_deref(), Some("x.y"));
        assert_eq!(entries[2].kind, Kind::Number);
        assert_eq!(entries[3].value, Value::Bool(false));
        assert_eq!(entries[4].kind, Kind::Color);
        assert_eq!(entries[5].value, Value::Num("-1.5e2".into()));
    }

    #[test]
    fn errors_carry_lines() {
        let err = parse("{\n  \"a\": \"1\",\n  \"a\": \"2\"\n}").unwrap_err();
        assert!(
            err.starts_with("line 3: duplicate name \"a\" (first on line 2)"),
            "{err}"
        );
        let err = parse("{\n  \"a\": { \"type\": \"bool\", \"value\": \"x\" }\n}").unwrap_err();
        assert!(
            err.starts_with("line 2:") && err.contains("true/false"),
            "{err}"
        );
        let err = parse("{\n\"a\": { \"value\": \"x\", \"note\": \"y\" }\n}").unwrap_err();
        assert!(err.contains("unknown field `note`"), "{err}");
        let err = parse("{\n\"a\": { \"type\": \"image\", \"value\": \"x\", \"key\": \"k\" }\n}")
            .unwrap_err();
        assert!(err.contains("`key` is only for string"), "{err}");
        let err = parse("{\n  \"a\": \"1\"\n  \"b\": \"2\"\n}").unwrap_err();
        assert!(err.starts_with("line 3: expected `,` or `}`"), "{err}");
        assert!(parse("[]").unwrap_err().starts_with("line 1: expected `{`"));
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
            split_file_name("Strings.ru_ru.yres"),
            Some(("Strings", Some("ru_ru")))
        );
        assert_eq!(split_file_name("Icons.yres"), Some(("Icons", None)));
    }

    #[test]
    fn placeholders_become_minecraft_format() {
        let segs = segments("{shown} of {total}: 100% {{ok}}").unwrap();
        let ps = params(&segs).unwrap();
        assert_eq!(ps, ["shown", "total"]);
        assert_eq!(lang_text(&segs, &ps).unwrap(), "%1$s of %2$s: 100%% {ok}");
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
        std::fs::create_dir_all(res.join("images")).unwrap();
        std::fs::create_dir_all(res.join("sounds")).unwrap();
        std::fs::write(res.join("images/logo.png"), b"png").unwrap();
        std::fs::write(res.join("sounds/click.ogg"), b"ogg").unwrap();
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
            r##"{"logo": {"type": "image", "value": "images/logo.png"}, "accent": {"type": "color", "value": "#FF8800"}, "click": {"type": "sound", "value": "sounds/click.ogg"}, "max": 3, "on": true}"##,
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
        assert_eq!(out["assets/m/textures/images/logo.png"], "png");
        assert_eq!(out["assets/m/sounds/sounds/click.ogg"], "ogg");
        assert!(out["assets/m/sounds.json"].contains("\"m:sounds/click\""));
        assert_eq!(c.refs.images["Icons.logo"], "m:images/logo");
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
            icons.contains("pub const fn logo() -> &'static str {\n        \"m:images/logo\""),
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
            resolve_image(&c.refs, "@Icons.logo").unwrap(),
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
            "{\n  \"a\": \"A\",\n  \"b\": { \"value\": \"B\", \"key\": \"m.a\" },\n  \"img\": { \"type\": \"image\", \"value\": \"../x.png\" },\n  \"bad name\": \"x\",\n  \"col\": { \"type\": \"color\", \"value\": \"red\" }\n}",
        )
        .unwrap();
        std::fs::write(res.join("Strings.en_us.yres"), "{}").unwrap();
        std::fs::write(res.join("Strings.de_de.yres"), "{\n  \"a\": 1\n}").unwrap();
        std::fs::write(res.join("Other.ru_ru.yres"), "{}").unwrap();
        let err = compile(&root, "m").unwrap_err();
        for needle in [
            "resources/Strings.yres:3: translation key \"m.a\" is also used at resources/Strings.yres:2",
            "resources/Strings.yres:4: \"img\": `../x.png` must stay inside resources/",
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
