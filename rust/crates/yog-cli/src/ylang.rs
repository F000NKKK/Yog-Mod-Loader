//! `.ylang` translation files → Minecraft lang JSON.
//!
//! A `.ylang` file is a JSON object of translation keys, in the order they
//! should appear. A value is either the text itself or an object carrying
//! the text and a design-time comment for translators (dropped at build):
//!
//! ```json
//! {
//!   "item.mymod.ruby": "Ruby",
//!   "mymod.screen.title": { "value": "My Mod", "comment": "Header of the settings screen" }
//! }
//! ```
//!
//! The file name is the locale (`en_us.ylang`, `ru_ru.ylang`). Two places
//! are picked up:
//!
//! - `lang/<locale>.ylang` at the mod root → namespace = the mod id;
//! - `assets/<ns>/lang/<locale>.ylang` → namespace `<ns>`.
//!
//! `yog build` turns each into `assets/<ns>/lang/<locale>.json` inside the
//! `.yog` (strings only, key order kept); the `.ylang` itself isn't bundled.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

/// The locale every other one is compared against.
pub const REFERENCE_LOCALE: &str = "en_us";

/// One translation: key, text, the design-time comment, and the 1-based
/// line the key is on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub key: String,
    pub value: String,
    pub comment: Option<String>,
    pub line: usize,
}

/// Parses `.ylang` text. Errors are `line N: message`.
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
        return Err(p.err("expected `{` — a .ylang file is one JSON object of translation keys"));
    }
    let mut entries: Vec<Entry> = Vec::new();
    let mut seen: HashMap<String, usize> = HashMap::new();
    p.ws();
    if !p.eat(b'}') {
        loop {
            p.ws();
            let line = p.line;
            let key = p
                .string()
                .map_err(|e| format!("{e} (expected a translation key)"))?;
            if let Some(first) = seen.get(&key) {
                return Err(format!(
                    "line {line}: duplicate key {key:?} (first on line {first})"
                ));
            }
            p.ws();
            if !p.eat(b':') {
                return Err(p.err("expected `:` after the key"));
            }
            p.ws();
            let (value, comment) = match p.peek() {
                Some(b'"') => (p.string()?, None),
                Some(b'{') => p.value_object(&key)?,
                _ => {
                    return Err(p.err(&format!(
                        "{key:?}: the value must be a string or {{ \"value\": \"...\", \"comment\": \"...\" }}"
                    )))
                }
            };
            seen.insert(key.clone(), line);
            entries.push(Entry {
                key,
                value,
                comment,
                line,
            });
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

/// The Minecraft lang JSON for `entries`: a flat object of strings,
/// key order kept, one key per line.
pub fn to_lang_json(entries: &[Entry]) -> String {
    let mut out = String::from("{");
    for (n, e) in entries.iter().enumerate() {
        out.push_str(if n == 0 { "\n  " } else { ",\n  " });
        out.push_str(&serde_json::to_string(&e.key).unwrap_or_default());
        out.push_str(": ");
        out.push_str(&serde_json::to_string(&e.value).unwrap_or_default());
    }
    out.push_str(if entries.is_empty() { "}\n" } else { "\n}\n" });
    out
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

    /// `{ "value": "...", "comment": "..." }`.
    fn value_object(&mut self, key: &str) -> Result<(String, Option<String>), String> {
        let start = self.line;
        self.eat(b'{');
        let (mut value, mut comment) = (None, None);
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
                if self.peek() != Some(b'"') {
                    return Err(self.err(&format!("{key:?}: `{field}` must be a string")));
                }
                let text = self.string()?;
                let slot = match field.as_str() {
                    "value" => &mut value,
                    "comment" => &mut comment,
                    other => {
                        return Err(format!(
                            "line {field_line}: {key:?}: unknown field `{other}` (expected `value` and optionally `comment`)"
                        ))
                    }
                };
                if slot.replace(text).is_some() {
                    return Err(format!(
                        "line {field_line}: {key:?}: duplicate field `{field}`"
                    ));
                }
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
        let value = value.ok_or_else(|| format!("line {start}: {key:?}: missing `value`"))?;
        Ok((value, comment))
    }
}

/// A generated file: path inside the `.yog` and its bytes.
pub type Output = (String, Vec<u8>);

/// One `.ylang` found in a mod: where it is and what it compiles to.
#[derive(Debug, Clone)]
pub struct LangFile {
    pub path: PathBuf,
    pub namespace: String,
    pub locale: String,
}

/// Every `.ylang` of the mod at `root`: `lang/*.ylang` (namespace
/// `mod_id`) and `assets/<ns>/lang/*.ylang`, sorted by path.
pub fn find(root: &Path, mod_id: &str) -> Vec<LangFile> {
    let mut out = Vec::new();
    let mut scan = |dir: &Path, namespace: &str| {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && path.extension().is_some_and(|e| e == "ylang") {
                let locale = path
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned();
                out.push(LangFile {
                    path,
                    namespace: namespace.to_string(),
                    locale,
                });
            }
        }
    };
    scan(&root.join("lang"), mod_id);
    if let Ok(namespaces) = std::fs::read_dir(root.join("assets")) {
        for ns in namespaces.flatten() {
            if ns.path().is_dir() {
                scan(&ns.path().join("lang"), &ns.file_name().to_string_lossy());
            }
        }
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

fn valid_locale(locale: &str) -> bool {
    !locale.is_empty()
        && locale
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

/// Parses and checks every `.ylang` of the mod: returns the lang JSON
/// files to bundle and warnings (keys missing from / extra to a locale,
/// compared with `en_us`). Any parse error fails the whole thing.
pub fn compile_all(root: &Path, mod_id: &str) -> Result<(Vec<Output>, Vec<String>), String> {
    let display = |p: &Path| p.strip_prefix(root).unwrap_or(p).display().to_string();
    let mut errors = Vec::new();
    // (namespace, locale) → files and their entries.
    type Parsed = Vec<(PathBuf, Vec<Entry>)>;
    let mut parsed: BTreeMap<(String, String), Parsed> = BTreeMap::new();
    for file in find(root, mod_id) {
        if !valid_locale(&file.locale) {
            errors.push(format!(
                "{}: the file name must be a lower-case locale such as en_us or ru_ru",
                display(&file.path)
            ));
            continue;
        }
        let text = match std::fs::read_to_string(&file.path) {
            Ok(t) => t,
            Err(e) => {
                errors.push(format!("{}: {e}", display(&file.path)));
                continue;
            }
        };
        match parse(&text) {
            Ok(entries) => parsed
                .entry((file.namespace.clone(), file.locale.clone()))
                .or_default()
                .push((file.path.clone(), entries)),
            Err(e) => errors.push(format!(
                "{}:{}",
                display(&file.path),
                e.trim_start_matches("line ")
            )),
        }
    }

    let mut outputs = Vec::new();
    let mut by_namespace: BTreeMap<String, BTreeMap<String, Vec<Entry>>> = BTreeMap::new();
    for ((namespace, locale), files) in parsed {
        let output = format!("assets/{namespace}/lang/{locale}.json");
        if root.join(&output).is_file() {
            errors.push(format!(
                "{output} exists next to a {locale}.ylang for the same namespace — keep one of them"
            ));
            continue;
        }
        // `lang/x.ylang` and `assets/<mod id>/lang/x.ylang` merge; a key in both is an error.
        let mut merged: Vec<Entry> = Vec::new();
        let mut origin: HashMap<String, String> = HashMap::new();
        for (path, entries) in files {
            for e in entries {
                let here = format!("{}:{}", display(&path), e.line);
                if let Some(first) = origin.get(&e.key) {
                    errors.push(format!(
                        "{here}: duplicate key {:?} (also at {first})",
                        e.key
                    ));
                    continue;
                }
                origin.insert(e.key.clone(), here);
                merged.push(e);
            }
        }
        outputs.push((output, to_lang_json(&merged).into_bytes()));
        by_namespace
            .entry(namespace)
            .or_default()
            .insert(locale, merged);
    }
    if !errors.is_empty() {
        return Err(format!(
            "invalid .ylang translations:\n  {}",
            errors.join("\n  ")
        ));
    }
    Ok((outputs, missing_key_warnings(&by_namespace)))
}

/// Keys of `en_us` missing in another locale, and keys only that locale has.
fn missing_key_warnings(
    by_namespace: &BTreeMap<String, BTreeMap<String, Vec<Entry>>>,
) -> Vec<String> {
    let mut warnings = Vec::new();
    for (namespace, locales) in by_namespace {
        let Some(reference) = locales.get(REFERENCE_LOCALE) else {
            if !locales.is_empty() {
                warnings.push(format!("{namespace}: no {REFERENCE_LOCALE}.ylang — Minecraft falls back to it for missing translations"));
            }
            continue;
        };
        for (locale, entries) in locales {
            if locale == REFERENCE_LOCALE {
                continue;
            }
            let has = |list: &[Entry], key: &str| list.iter().any(|e| e.key == key);
            let missing: Vec<&str> = reference
                .iter()
                .filter(|e| !has(entries, &e.key))
                .map(|e| e.key.as_str())
                .collect();
            let extra: Vec<&str> = entries
                .iter()
                .filter(|e| !has(reference, &e.key))
                .map(|e| e.key.as_str())
                .collect();
            if !missing.is_empty() {
                warnings.push(format!(
                    "{namespace}/{locale}: {} key(s) not translated (shown in {REFERENCE_LOCALE}): {}",
                    missing.len(),
                    preview(&missing)
                ));
            }
            if !extra.is_empty() {
                warnings.push(format!(
                    "{namespace}/{locale}: {} key(s) not in {REFERENCE_LOCALE}: {}",
                    extra.len(),
                    preview(&extra)
                ));
            }
        }
    }
    warnings
}

fn preview(keys: &[&str]) -> String {
    const SHOWN: usize = 5;
    let mut text = keys
        .iter()
        .take(SHOWN)
        .copied()
        .collect::<Vec<_>>()
        .join(", ");
    if keys.len() > SHOWN {
        text.push_str(&format!(", … (+{})", keys.len() - SHOWN));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_strings_and_commented_values_in_order() {
        let entries = parse(
            "{\n  \"b.key\": \"B\",\n  \"a.key\": { \"value\": \"A \\\"quoted\\\" \\u00e9\", \"comment\": \"note\" }\n}\n",
        )
        .unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].key, "b.key");
        assert_eq!(entries[0].line, 2);
        assert_eq!(entries[1].value, "A \"quoted\" é");
        assert_eq!(entries[1].comment.as_deref(), Some("note"));
        assert_eq!(entries[1].line, 3);
    }

    #[test]
    fn empty_object_and_bom() {
        assert!(parse("\u{FEFF}{ }").unwrap().is_empty());
        assert_eq!(to_lang_json(&[]), "{}\n");
    }

    #[test]
    fn surrogate_pairs() {
        let e = parse(r#"{"k": "\ud83d\ude00"}"#).unwrap();
        assert_eq!(e[0].value, "😀");
    }

    #[test]
    fn errors_carry_lines() {
        let err = parse("{\n  \"a\": \"1\",\n  \"a\": \"2\"\n}").unwrap_err();
        assert!(
            err.starts_with("line 3: duplicate key \"a\" (first on line 2)"),
            "{err}"
        );
        let err = parse("{\n  \"a\": 5\n}").unwrap_err();
        assert!(
            err.starts_with("line 2:") && err.contains("must be a string"),
            "{err}"
        );
        let err = parse("{\n  \"a\": \"1\"\n  \"b\": \"2\"\n}").unwrap_err();
        assert!(err.starts_with("line 3: expected `,` or `}`"), "{err}");
        let err = parse("{\n\"a\": { \"comment\": \"x\" }\n}").unwrap_err();
        assert!(err.contains("missing `value`"), "{err}");
        let err = parse("{\n\"a\": { \"value\": \"x\", \"note\": \"y\" }\n}").unwrap_err();
        assert!(err.contains("unknown field `note`"), "{err}");
        let err = parse("[]").unwrap_err();
        assert!(err.starts_with("line 1: expected `{`"), "{err}");
        let err = parse("{\"a\": \"1\"} x").unwrap_err();
        assert!(err.contains("after the closing"), "{err}");
    }

    #[test]
    fn lang_json_drops_comments_and_escapes() {
        let entries =
            parse(r#"{"z": {"value": "line\nnext", "comment": "c"}, "a": "\"q\""}"#).unwrap();
        assert_eq!(
            to_lang_json(&entries),
            "{\n  \"z\": \"line\\nnext\",\n  \"a\": \"\\\"q\\\"\"\n}\n"
        );
    }

    fn temp_mod(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("yog-ylang-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn compiles_both_locations_and_warns_on_missing_keys() {
        let root = temp_mod("both");
        std::fs::create_dir_all(root.join("lang")).unwrap();
        std::fs::create_dir_all(root.join("assets/other/lang")).unwrap();
        std::fs::write(
            root.join("lang/en_us.ylang"),
            r#"{"m.a": "A", "m.b": {"value": "B", "comment": "c"}}"#,
        )
        .unwrap();
        std::fs::write(root.join("lang/ru_ru.ylang"), r#"{"m.a": "А", "m.x": "X"}"#).unwrap();
        std::fs::write(
            root.join("assets/other/lang/en_us.ylang"),
            r#"{"o.a": "OA"}"#,
        )
        .unwrap();
        let (outputs, warnings) = compile_all(&root, "mymod").unwrap();
        let names: Vec<&str> = outputs.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(
            names,
            [
                "assets/mymod/lang/en_us.json",
                "assets/mymod/lang/ru_ru.json",
                "assets/other/lang/en_us.json"
            ]
        );
        assert_eq!(
            String::from_utf8_lossy(&outputs[0].1),
            "{\n  \"m.a\": \"A\",\n  \"m.b\": \"B\"\n}\n"
        );
        assert_eq!(warnings.len(), 2, "{warnings:?}");
        assert!(
            warnings[0].contains("mymod/ru_ru: 1 key(s) not translated")
                && warnings[0].contains("m.b")
        );
        assert!(warnings[1].contains("not in en_us: m.x"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn reports_file_and_line_and_cross_file_duplicates() {
        let root = temp_mod("errors");
        std::fs::create_dir_all(root.join("lang")).unwrap();
        std::fs::create_dir_all(root.join("assets/mymod/lang")).unwrap();
        std::fs::write(root.join("lang/en_us.ylang"), "{\n  \"k\": \"1\"\n}").unwrap();
        std::fs::write(
            root.join("assets/mymod/lang/en_us.ylang"),
            "{\n\n  \"k\": \"2\"\n}",
        )
        .unwrap();
        std::fs::write(root.join("lang/de_de.ylang"), "{\n  \"k\": true\n}").unwrap();
        std::fs::write(root.join("lang/Bad.ylang"), "{}").unwrap();
        let err = compile_all(&root, "mymod").unwrap_err();
        assert!(err.contains("lang/de_de.ylang:2:"), "{err}");
        assert!(
            err.contains("Bad.ylang: the file name must be a lower-case locale"),
            "{err}"
        );
        assert!(err.contains("duplicate key \"k\""), "{err}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn json_next_to_ylang_is_an_error() {
        let root = temp_mod("clash");
        std::fs::create_dir_all(root.join("assets/mymod/lang")).unwrap();
        std::fs::write(root.join("assets/mymod/lang/en_us.ylang"), r#"{"k": "1"}"#).unwrap();
        std::fs::write(root.join("assets/mymod/lang/en_us.json"), r#"{"k": "1"}"#).unwrap();
        let err = compile_all(&root, "mymod").unwrap_err();
        assert!(err.contains("keep one of them"), "{err}");
        let _ = std::fs::remove_dir_all(&root);
    }
}
