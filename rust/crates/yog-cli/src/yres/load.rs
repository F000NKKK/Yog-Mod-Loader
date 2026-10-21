//! Reading a mod's `.yres` files into sets: each one's neutral file and
//! its culture variants.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::names::{name_problem, valid_locale, valid_set_name};
use super::{Entry, Kind, NEUTRAL_LOCALE, find, parse};
use crate::shown_path;

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

/// A `.yres` read: its set, its locale (none for the neutral file), the file.
struct Read {
    set: String,
    locale: Option<String>,
    file: ResFile,
}

/// Reads and checks every `.yres` of the mod — errors are `file:line: message`.
pub(super) fn load(root: &Path, errors: &mut Vec<String>) -> BTreeMap<String, Set> {
    let mut sets: BTreeMap<String, Set> = BTreeMap::new();
    let mut cultures: Vec<Read> = Vec::new();
    for path in find(root) {
        let Some(read) = read_file(root, path, errors) else {
            continue;
        };
        match read.locale {
            None => add_neutral(root, &mut sets, read, errors),
            Some(_) => cultures.push(read),
        }
    }
    for read in cultures {
        add_culture(root, &mut sets, read, errors);
    }
    sets
}

/// The file at `path`: its name says its set and locale, its entries'
/// names are valid.
fn read_file(root: &Path, path: PathBuf, errors: &mut Vec<String>) -> Option<Read> {
    let shown = shown_path(root, &path);
    let file_name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let (set, locale) = yog_formats::yres::split_file_name(&file_name)?;
    if let Err(e) = check_name(set, locale) {
        errors.push(format!("{shown}: {e}"));
        return None;
    }
    let entries = match std::fs::read_to_string(&path)
        .map_err(|e| e.to_string())
        .and_then(|t| parse(&t))
    {
        Ok(entries) => entries,
        Err(e) => {
            errors.push(format!("{shown}:{}", e.trim_start_matches("line ")));
            return None;
        }
    };
    for e in &entries {
        if let Some(problem) = name_problem(&e.name) {
            errors.push(format!("{shown}:{}: {problem}", e.line));
        }
    }
    Some(Read {
        set: set.to_string(),
        locale: locale.map(str::to_string),
        file: ResFile { path, entries },
    })
}

/// What's wrong with a file's set name and locale, if anything.
fn check_name(set: &str, locale: Option<&str>) -> Result<(), String> {
    if !valid_set_name(set) {
        return Err(format!(
            "`{set}` isn't a valid resource set name — use an identifier like Strings or Icons"
        ));
    }
    match locale {
        Some(NEUTRAL_LOCALE) => Err(format!(
            "the neutral file {set}.yres is already {NEUTRAL_LOCALE} — put these texts there"
        )),
        Some(l) if !valid_locale(l) => Err(format!(
            "`{l}` isn't a Minecraft locale code (lower case, like ru_ru or pt_br)"
        )),
        _ => Ok(()),
    }
}

/// A set's neutral file: one a set.
fn add_neutral(
    root: &Path,
    sets: &mut BTreeMap<String, Set>,
    read: Read,
    errors: &mut Vec<String>,
) {
    if let Some(other) = sets.get(&read.set) {
        let (shown, other) = (
            shown_path(root, &read.file.path),
            shown_path(root, &other.neutral.path),
        );
        errors.push(format!(
            "{shown}: resource set {} is also defined by {other}",
            read.set
        ));
        return;
    }
    sets.insert(
        read.set.clone(),
        Set {
            name: read.set,
            neutral: read.file,
            cultures: BTreeMap::new(),
        },
    );
}

/// A culture variant: of a set with a neutral file, one a locale, texts only.
fn add_culture(
    root: &Path,
    sets: &mut BTreeMap<String, Set>,
    read: Read,
    errors: &mut Vec<String>,
) {
    let Read { set, locale, file } = read;
    let locale = locale.unwrap_or_default();
    let shown = shown_path(root, &file.path);
    let Some(target) = sets.get_mut(&set) else {
        errors.push(format!(
            "{shown}: no neutral {set}.yres for this culture variant"
        ));
        return;
    };
    if let Some(other) = target.cultures.get(&locale) {
        errors.push(format!(
            "{shown}: {set} {locale} is also defined by {}",
            shown_path(root, &other.path)
        ));
        return;
    }
    for e in file
        .entries
        .iter()
        .filter(|e| e.kind != Kind::String || e.key.is_some())
    {
        errors.push(format!("{shown}:{}: {:?}: culture variants hold texts only (no type other than string, no `key`)", e.line, e.name));
    }
    target.cultures.insert(locale, file);
}
