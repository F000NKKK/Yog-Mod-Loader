//! What a mod's content files may name: checks `@item:`, `@block:`,
//! `@entity:`, `@recipe:` references against the mod's own content, every
//! Minecraft version Yog supports (`vanilla/ids.txt`, generated from their
//! client jars) and the other mods' ids it declares in `yog.toml`
//! (`[external.<namespace>]`).

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use yog_book::refs::bare;

use crate::manifest::External;

/// What a reference names that the build can check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RefKind {
    Item,
    Block,
    Entity,
    Recipe,
}

impl RefKind {
    pub const ALL: [RefKind; 4] = [RefKind::Item, RefKind::Block, RefKind::Entity, RefKind::Recipe];

    /// Its prefix: `item` for `@item:`.
    pub fn word(self) -> &'static str {
        match self {
            RefKind::Item => "item",
            RefKind::Block => "block",
            RefKind::Entity => "entity",
            RefKind::Recipe => "recipe",
        }
    }

    pub fn from_word(word: &str) -> Option<RefKind> {
        RefKind::ALL.into_iter().find(|k| k.word() == word)
    }
}

/// Vanilla ids: (kind, path) → the versions it's in (`None`: all).
struct Vanilla {
    versions: Vec<String>,
    ids: HashMap<(RefKind, String), Option<Vec<String>>>,
}

static VANILLA: LazyLock<Vanilla> = LazyLock::new(|| {
    let text = include_str!("../vanilla/ids.txt");
    let mut versions = Vec::new();
    let mut ids = HashMap::new();
    for line in text.lines() {
        if let Some(v) = line.strip_prefix("# versions:") {
            versions = v.split_whitespace().map(str::to_string).collect();
            continue;
        }
        let mut p = line.split_whitespace();
        let (Some(kind), Some(path), Some(vs)) = (p.next(), p.next(), p.next()) else { continue };
        let Some(kind) = RefKind::ALL.into_iter().find(|k| k.word() == kind) else { continue };
        let vs = (vs != "*").then(|| vs.split(',').map(str::to_string).collect());
        ids.insert((kind, path.to_string()), vs);
    }
    Vanilla { versions, ids }
});

/// The ids a mod's references may name.
#[derive(Default)]
pub struct Known {
    /// The mod's namespace.
    pub ns: String,
    /// Its own content, by kind (full ids).
    pub own: HashMap<RefKind, HashSet<String>>,
    /// Other mods' ids it declared, by kind (full ids), and their namespaces.
    pub external: HashMap<RefKind, HashSet<String>>,
    pub external_ns: HashSet<String>,
}

impl Known {
    pub fn new(ns: &str, externals: &[External]) -> Known {
        let mut k = Known { ns: ns.to_string(), ..Default::default() };
        for e in externals {
            k.external_ns.insert(e.namespace.clone());
            for (key, paths) in &e.ids {
                let kind = match key.as_str() {
                    "items" => RefKind::Item,
                    "blocks" => RefKind::Block,
                    "entities" => RefKind::Entity,
                    "recipes" => RefKind::Recipe,
                    _ => continue,
                };
                let set = k.external.entry(kind).or_default();
                for p in paths {
                    set.insert(if p.contains(':') { p.clone() } else { format!("{}:{p}", e.namespace) });
                }
            }
        }
        k
    }

    pub fn add(&mut self, kind: RefKind, id: impl Into<String>) {
        self.own.entry(kind).or_default().insert(id.into());
    }

    /// Whether `id` (a reference of `kind`) names something that exists —
    /// on every Minecraft version, for vanilla; why not, if it doesn't.
    pub fn check(&self, kind: RefKind, id: &str) -> Result<(), String> {
        let id = bare(id);
        let Some((ns, path)) = id.split_once(':') else {
            return Err(format!("`{id}`: write its namespace (`minecraft:{id}`, `{}:{id}`)", self.ns));
        };
        let what = kind.word();
        // An item may be a block's item.
        let kinds: &[RefKind] = if kind == RefKind::Item { &[RefKind::Item, RefKind::Block] } else { std::slice::from_ref(&kind) };
        if ns == self.ns {
            return if kinds.iter().any(|k| self.own.get(k).is_some_and(|s| s.contains(id))) {
                Ok(())
            } else {
                Err(format!("no {what} {id} in this mod"))
            };
        }
        if ns == "minecraft" {
            let v = &*VANILLA;
            let found = kinds.iter().find_map(|k| v.ids.get(&(*k, path.to_string())));
            return match found {
                None => Err(format!("no {what} {id} in Minecraft")),
                Some(None) => Ok(()),
                Some(Some(only)) => {
                    let missing: Vec<&str> = v.versions.iter().filter(|x| !only.contains(x)).map(String::as_str).collect();
                    Err(format!(
                        "{id} is only in Minecraft {} — not in {}",
                        only.join(", "),
                        missing.join(", ")
                    ))
                }
            };
        }
        if kinds.iter().any(|k| self.external.get(k).is_some_and(|s| s.contains(id))) {
            return Ok(());
        }
        let section = format!("[external.{ns}]");
        Err(if self.external_ns.contains(ns) {
            format!("{id} isn't declared under {section} in yog.toml")
        } else {
            let list = match kind {
                RefKind::Item => "items",
                RefKind::Block => "blocks",
                RefKind::Entity => "entities",
                _ => "recipes",
            };
            format!("{id} is another mod's: declare it in yog.toml — {section} {list} = [\"{path}\"]")
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn references_are_checked_against_every_source() {
        let ext = External { namespace: "farmersdelight".into(), ids: vec![("items".into(), vec!["cutting_board".into()])] };
        let mut k = Known::new("mymod", &[ext]);
        k.add(RefKind::Item, "mymod:ruby");
        assert!(k.check(RefKind::Item, "mymod:ruby").is_ok());
        assert!(k.check(RefKind::Item, "mymod:nope").unwrap_err().contains("no item mymod:nope"));
        assert!(k.check(RefKind::Item, "minecraft:stick").is_ok());
        assert!(k.check(RefKind::Item, "minecraft:stone").is_ok(), "a block's item");
        assert!(k.check(RefKind::Block, "minecraft:furnace[facing=north]").is_ok());
        assert!(k.check(RefKind::Entity, "minecraft:allay").is_ok());
        assert!(k.check(RefKind::Item, "minecraft:short_grass").unwrap_err().contains("not in 1.20.1"));
        assert!(k.check(RefKind::Item, "minecraft:nope").unwrap_err().contains("in Minecraft"));
        assert!(k.check(RefKind::Item, "farmersdelight:cutting_board").is_ok());
        assert!(k.check(RefKind::Item, "farmersdelight:knife").unwrap_err().contains("isn't declared"));
        assert!(k.check(RefKind::Item, "create:wrench").unwrap_err().contains("[external.create]"));
        assert!(k.check(RefKind::Item, "stick").unwrap_err().contains("namespace"));
    }
}
