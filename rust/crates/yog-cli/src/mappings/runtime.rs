//! The game's obfuscated names → a host's run-time names, read from
//! Fabric's intermediary (tiny v2) or Forge's SRG (tsrg2).

use std::collections::HashMap;

use yog_ids::mapping_ns;

/// Obfuscated → run-time: classes, fields by (class, name), methods by
/// (class, name, descriptor), all obfuscated.
#[derive(Debug, Default)]
pub struct Runtime {
    pub classes: HashMap<String, String>,
    pub fields: HashMap<(String, String), String>,
    pub methods: HashMap<(String, String, String), String>,
}

/// What separates a line's columns: tiny's tabs, tsrg's spaces.
const TAB: char = '\t';
const SPACE: char = ' ';
/// Tiny's line kinds.
const TINY_CLASS: &str = "c";
const TINY_FIELD: &str = "f";
const TINY_METHOD: &str = "m";
/// Columns before a tiny header's namespaces (`tiny 2 0`).
const TINY_HEADER: usize = 3;

/// Fabric's intermediary (tiny v2: `official` → `intermediary`).
pub fn tiny(text: &str) -> Result<Runtime, String> {
    let mut lines = text.lines();
    let header: Vec<&str> = lines.next().unwrap_or_default().split(TAB).collect();
    let column = |ns: &str| {
        header
            .iter()
            .skip(TINY_HEADER)
            .position(|h| *h == ns)
            .ok_or_else(|| format!("the tiny file has no `{ns}` names"))
    };
    let (from, to) = (
        column(mapping_ns::OFFICIAL)?,
        column(mapping_ns::INTERMEDIARY)?,
    );
    let mut out = Runtime::default();
    let mut class = String::new();
    for line in lines {
        let depth = line.len() - line.trim_start_matches(TAB).len();
        let cols: Vec<&str> = line.trim_start_matches(TAB).split(TAB).collect();
        match (depth, cols.first().copied()) {
            (0, Some(TINY_CLASS)) => {
                let names = &cols[1..];
                class = names.get(from).copied().unwrap_or_default().to_owned();
                if let Some(rt) = names.get(to) {
                    out.classes.insert(class.clone(), (*rt).to_owned());
                }
            }
            (1, Some(kind @ (TINY_FIELD | TINY_METHOD))) if cols.len() > 2 => {
                let (desc, names) = (cols[1], &cols[2..]);
                let (Some(obf), Some(rt)) = (names.get(from), names.get(to)) else {
                    continue;
                };
                if kind == TINY_FIELD {
                    out.fields
                        .insert((class.clone(), (*obf).to_owned()), (*rt).to_owned());
                } else {
                    let key = (class.clone(), (*obf).to_owned(), desc.to_owned());
                    out.methods.insert(key, (*rt).to_owned());
                }
            }
            _ => {}
        }
    }
    Ok(out)
}

/// What starts a tsrg2 method's descriptor.
const DESC_START: char = '(';

/// Forge's SRG (tsrg2: `obf srg id`).
pub fn tsrg(text: &str) -> Runtime {
    let mut out = Runtime::default();
    let mut class = String::new();
    for line in text.lines().skip(1) {
        let depth = line.len() - line.trim_start_matches(TAB).len();
        let cols: Vec<&str> = line.trim_start_matches(TAB).split(SPACE).collect();
        match (depth, cols.as_slice()) {
            (0, [obf, srg, ..]) => {
                class = (*obf).to_owned();
                out.classes.insert(class.clone(), (*srg).to_owned());
            }
            (1, [obf, desc, srg, ..]) if desc.starts_with(DESC_START) => {
                let key = (class.clone(), (*obf).to_owned(), (*desc).to_owned());
                out.methods.insert(key, (*srg).to_owned());
            }
            (1, [obf, srg, ..]) => {
                out.fields
                    .insert((class.clone(), (*obf).to_owned()), (*srg).to_owned());
            }
            _ => {}
        }
    }
    out
}
