//! What a mod's content files may name: checks `@item:`, `@block:`,
//! `@entity:`, `@recipe:`, `@loot_table:` references against the mod's own content, every
//! Minecraft version Yog supports (`vanilla/ids.txt`, generated from their
//! client jars) and the other mods' ids it declares in `yog.toml`
//! (`[external.<namespace>]`).

use std::collections::{HashMap, HashSet};

pub(crate) use yog_formats::tags::{describe_hosts, hosts, tag_hosts, tags_known, yog_tag_for};
use yog_sync::LazyLock;

use yog_formats::book::refs::bare;

use crate::project::manifest::External;

/// What a reference names that the build can check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum RefKind {
    Item,
    Block,
    Entity,
    Recipe,
    LootTable,
}

impl RefKind {
    pub(crate) const ALL: [RefKind; 5] = [
        RefKind::Item,
        RefKind::Block,
        RefKind::Entity,
        RefKind::Recipe,
        RefKind::LootTable,
    ];

    /// Its prefix: `item` for `@item:`.
    pub(crate) fn word(self) -> &'static str {
        match self {
            RefKind::Item => yog_ids::kind::ITEM,
            RefKind::Block => yog_ids::kind::BLOCK,
            RefKind::Entity => yog_ids::kind::ENTITY,
            RefKind::Recipe => yog_ids::kind::RECIPE,
            RefKind::LootTable => yog_ids::kind::LOOT_TABLE,
        }
    }

    pub(crate) fn from_word(word: &str) -> Option<RefKind> {
        RefKind::ALL.into_iter().find(|k| k.word() == word)
    }
}

/// Vanilla ids: (kind, path) → the versions it's in (`None`: all); and
/// what Minecraft renamed (`vanilla/renames.txt`): (kind, a name) → the other.
struct Vanilla {
    versions: Vec<String>,
    ids: HashMap<(RefKind, String), Option<Vec<String>>>,
    renamed: HashMap<(RefKind, String), String>,
    potion_aliases: HashMap<String, Option<Vec<String>>>,
}

static VANILLA: LazyLock<Vanilla> = LazyLock::new(|| {
    let text = include_str!("../vanilla/ids.txt");
    let mut versions = Vec::new();
    let mut ids = HashMap::new();
    let mut potion_aliases = HashMap::new();
    for line in text.lines() {
        if let Some(v) = line.strip_prefix("# versions:") {
            versions = v.split_whitespace().map(str::to_string).collect();
            continue;
        }
        let mut p = line.split_whitespace();
        let (Some(kind), Some(path), Some(vs)) = (p.next(), p.next(), p.next()) else {
            continue;
        };
        let vs = (vs != "*").then(|| vs.split(',').map(str::to_string).collect());
        if kind == yog_ids::kind::POTION {
            potion_aliases.insert(path.to_string(), vs);
            continue;
        }
        let Some(kind) = RefKind::ALL.into_iter().find(|k| k.word() == kind) else {
            continue;
        };
        ids.insert((kind, path.to_string()), vs);
    }
    let mut renamed = HashMap::new();
    for line in include_str!("../vanilla/renames.txt")
        .lines()
        .filter(|l| !l.starts_with('#'))
    {
        let mut p = line.split_whitespace();
        let (Some(kind), Some(old), Some(new)) = (p.next(), p.next(), p.next()) else {
            continue;
        };
        let Some(kind) = RefKind::from_word(kind) else {
            continue;
        };
        renamed.insert((kind, old.to_string()), new.to_string());
        renamed.insert((kind, new.to_string()), old.to_string());
    }
    Vanilla {
        versions,
        ids,
        renamed,
        potion_aliases,
    }
});

/// The Minecraft versions Yog supports, oldest first.
pub(crate) fn versions() -> &'static [String] {
    &VANILLA.versions
}

/// Whether vanilla `path` (of `kind`) is in Minecraft `version`.
fn in_version(kind: RefKind, path: &str, version: &str) -> bool {
    VANILLA
        .ids
        .get(&(kind, path.to_string()))
        .is_some_and(|vs| vs.as_ref().is_none_or(|vs| vs.iter().any(|v| v == version)))
}

/// Whether `id` is an item or block of some Minecraft version Yog knows.
pub(crate) fn is_vanilla(id: &str) -> bool {
    let Some(path) = yog_ids::game_path(id) else {
        return false;
    };
    [RefKind::Item, RefKind::Block]
        .iter()
        .any(|k| VANILLA.ids.contains_key(&(*k, path.to_string())))
}

/// The game's potion Yog's alias `id` (`yog:potion/<name>`) stands for.
fn potion_of(id: &str) -> Option<&str> {
    bare(id).strip_prefix(yog_ids::yog::POTION_ALIAS)
}

/// Whether `id` is one of Yog's stable aliases for a vanilla potion item.
pub(crate) fn is_vanilla_potion(id: &str) -> bool {
    potion_of(id).is_some_and(|p| VANILLA.potion_aliases.contains_key(p))
}

/// Whether a stable Yog alias names a potion registered by this Minecraft version.
pub(crate) fn is_vanilla_potion_in(id: &str, version: &str) -> bool {
    potion_of(id)
        .and_then(|p| VANILLA.potion_aliases.get(p))
        .is_some_and(|versions| {
            versions
                .as_ref()
                .is_none_or(|versions| versions.iter().any(|v| v == version))
        })
}

/// What vanilla `id` (`minecraft:short_grass`) is on Minecraft `version`:
/// itself, what it's called there (`minecraft:grass` on 1.20.1), or `None`
/// when that version doesn't have it.
pub(crate) fn equivalent(kind: RefKind, id: &str, version: &str) -> Option<String> {
    let path = yog_ids::game_path(id)?;
    // An item may be a block's item.
    let kinds: &[RefKind] = if kind == RefKind::Item {
        &[RefKind::Item, RefKind::Block]
    } else {
        std::slice::from_ref(&kind)
    };
    if kinds.iter().any(|k| in_version(*k, path, version)) {
        return Some(id.to_string());
    }
    kinds.iter().find_map(|k| {
        let other = VANILLA.renamed.get(&(*k, path.to_string()))?;
        in_version(*k, other, version).then(|| format!("minecraft:{other}"))
    })
}

/// The ids a mod's references may name.
#[derive(Default)]
pub(crate) struct Known {
    /// The mod's namespace.
    pub(crate) ns: String,
    /// The Minecraft version whose content the mod names (its datapack).
    pub(crate) datapack: String,
    /// Its own content, by kind (full ids).
    pub(crate) own: HashMap<RefKind, HashSet<String>>,
    /// Other mods' ids it declared, by kind (full ids), and their namespaces.
    pub(crate) external: HashMap<RefKind, HashSet<String>>,
    pub(crate) external_ns: HashSet<String>,
}

impl Known {
    /// `datapack`: the version the mod names vanilla content by (`None`: the newest).
    pub(crate) fn new(ns: &str, externals: &[External], datapack: Option<&str>) -> Known {
        let datapack = datapack
            .map(str::to_string)
            .unwrap_or_else(|| VANILLA.versions.last().cloned().unwrap_or_default());
        let mut k = Known {
            ns: ns.to_string(),
            datapack,
            ..Default::default()
        };
        for e in externals {
            k.external_ns.insert(e.namespace.clone());
            for (kind, paths) in &e.ids {
                let set = k.external.entry(*kind).or_default();
                for p in paths {
                    set.insert(if p.contains(':') {
                        p.clone()
                    } else {
                        format!("{}:{p}", e.namespace)
                    });
                }
            }
        }
        k
    }

    pub(crate) fn add(&mut self, kind: RefKind, id: impl Into<String>) {
        self.own.entry(kind).or_default().insert(id.into());
    }

    /// Whether tag `id` of content `kind` exists somewhere: the game's, the
    /// loaders' and Yog's common tags are checked; others are the mod's or
    /// another mod's. Where it's only on some versions or loaders, what uses
    /// it is left out of the others (see [`tag_hosts`]).
    pub(crate) fn check_tag(&self, kind: &str, id: &str) -> Result<(), String> {
        let Some((ns, _)) = id.split_once(':') else {
            return Err(format!(
                "`{id}`: write the tag's namespace (`yog:{id}`, `{}:{id}`)",
                self.ns
            ));
        };
        if !tags_known(ns) || tag_hosts(yog_ids::registry_of(kind), id).is_some() {
            return Ok(());
        }
        Err(if ns == yog_ids::ns::YOG {
            format!("no {kind} tag {id} among Yog's common tags")
        } else {
            format!("no {kind} tag {id} in Minecraft or its loaders")
        })
    }

    /// Whether `id` (a reference of `kind`) names something that exists —
    /// for vanilla, in the mod's datapack version (other versions get what
    /// it's called there, see [`equivalent`]); why not, if it doesn't.
    pub(crate) fn check(&self, kind: RefKind, id: &str) -> Result<(), String> {
        let id = bare(id);
        let Some((ns, path)) = id.split_once(':') else {
            return Err(format!(
                "`{id}`: write its namespace (`minecraft:{id}`, `{}:{id}`)",
                self.ns
            ));
        };
        // Stable aliases for vanilla potion stacks; backends resolve these to
        // the version's Potion registry entry.
        if kind == RefKind::Item && potion_of(id).is_some() {
            return self.check_potion(id);
        }
        // An item may be a block's item.
        let kinds: &[RefKind] = if kind == RefKind::Item {
            &[RefKind::Item, RefKind::Block]
        } else {
            std::slice::from_ref(&kind)
        };
        if ns == self.ns {
            // The mod's loot tables are its blocks' and its data's.
            if kind == RefKind::LootTable
                || kinds
                    .iter()
                    .any(|k| self.own.get(k).is_some_and(|s| s.contains(id)))
            {
                return Ok(());
            }
            return Err(format!("no {} {id} in this mod", kind.word()));
        }
        if ns == yog_ids::ns::MINECRAFT {
            return self.check_vanilla(kind, kinds, id, path);
        }
        if kinds
            .iter()
            .any(|k| self.external.get(k).is_some_and(|s| s.contains(id)))
        {
            return Ok(());
        }
        Err(self.undeclared(kind, id, ns, path))
    }

    /// A vanilla potion alias: registered in the mod's datapack version?
    fn check_potion(&self, id: &str) -> Result<(), String> {
        if is_vanilla_potion_in(id, &self.datapack) {
            Ok(())
        } else if is_vanilla_potion(id) {
            Err(format!(
                "potion alias {id} isn't registered in Minecraft {}",
                self.datapack
            ))
        } else {
            Err(format!(
                "no item {id} among Yog's versioned default potion aliases"
            ))
        }
    }

    /// The game's `id` (of one of `kinds`): in the mod's datapack version?
    fn check_vanilla(
        &self,
        kind: RefKind,
        kinds: &[RefKind],
        id: &str,
        path: &str,
    ) -> Result<(), String> {
        let v = &*VANILLA;
        if !v.versions.contains(&self.datapack) {
            return Err(format!(
                "the mod's datapack is Minecraft {}, which Yog doesn't know ({})",
                self.datapack,
                v.versions.join(", ")
            ));
        }
        match kinds
            .iter()
            .find_map(|k| v.ids.get(&(*k, path.to_string())))
        {
            None => Err(format!("no {} {id} in Minecraft", kind.word())),
            Some(_) if kinds.iter().any(|k| in_version(*k, path, &self.datapack)) => Ok(()),
            Some(Some(only)) => {
                let hint = equivalent(kind, id, &self.datapack)
                    .map(|e| format!(" — there it's {e}"))
                    .unwrap_or_default();
                Err(format!(
                    "{id} isn't in Minecraft {} (the mod's datapack; it's in {}){hint}",
                    self.datapack,
                    only.join(", ")
                ))
            }
            Some(None) => Ok(()),
        }
    }

    /// Why another mod's `id` isn't known: not declared under its
    /// `[external.<ns>]`, and how to declare it there.
    fn undeclared(&self, kind: RefKind, id: &str, ns: &str, path: &str) -> String {
        let section = format!("[external.{ns}]");
        if self.external_ns.contains(ns) {
            return format!("{id} isn't declared under {section} in yog.toml");
        }
        match external_list(kind) {
            Some(list) => format!(
                "{id} is another mod's: declare it in yog.toml — {section} {list} = [\"{path}\"]"
            ),
            None => format!(
                "{id} is another mod's, and yog.toml can't declare its {}s",
                kind.word()
            ),
        }
    }
}

/// The `[external.<ns>]` list a kind's ids go in, if it has one.
fn external_list(kind: RefKind) -> Option<&'static str> {
    match kind {
        RefKind::Item => Some(yog_ids::word::ITEMS),
        RefKind::Block => Some(yog_ids::word::BLOCKS),
        RefKind::Entity => Some(yog_ids::word::ENTITIES),
        RefKind::Recipe => Some(yog_ids::word::RECIPES),
        RefKind::LootTable => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hosts_know_the_tags_the_build_does() {
        assert_eq!(
            yog_formats::tags::TABLE,
            include_str!("../../../../../java-common/src/main/resources/yog/tags.txt"),
            "java-common's yog/tags.txt is a copy of yog-formats/data/tags.txt"
        );
    }

    #[test]
    fn the_hosts_rename_as_the_build_does() {
        assert_eq!(
            include_str!("../vanilla/renames.txt"),
            include_str!("../../../../../java-common/src/main/resources/yog/renames.txt"),
            "java-common's yog/renames.txt is a copy of vanilla/renames.txt"
        );
    }

    #[test]
    fn references_are_checked_against_every_source() {
        let ext = External {
            namespace: "farmersdelight".into(),
            ids: vec![(RefKind::Item, vec!["cutting_board".into()])],
        };
        let mut k = Known::new("mymod", std::slice::from_ref(&ext), None);
        k.add(RefKind::Item, "mymod:ruby");
        assert!(k.check(RefKind::Item, "mymod:ruby").is_ok());
        assert!(
            k.check(RefKind::Item, "mymod:nope")
                .unwrap_err()
                .contains("no item mymod:nope")
        );
        assert!(k.check(RefKind::Item, "minecraft:stick").is_ok());
        assert!(
            k.check(RefKind::Item, "yog:potion/awkward").is_ok(),
            "Yog's stable vanilla potion item alias"
        );
        assert!(k.check(RefKind::Item, "yog:potion/long_swiftness").is_ok());
        assert!(k.check(RefKind::Item, "yog:potion/wind_charged").is_ok());
        assert!(
            k.check(RefKind::Item, "yog:potion/levitation").is_err(),
            "an effect name isn't a potion registry id"
        );
        assert!(
            k.check(RefKind::Item, "minecraft:stone").is_ok(),
            "a block's item"
        );
        assert!(
            k.check(RefKind::Block, "minecraft:furnace[facing=north]")
                .is_ok()
        );
        assert!(k.check(RefKind::Entity, "minecraft:allay").is_ok());
        assert!(
            k.check(RefKind::Item, "minecraft:short_grass").is_ok(),
            "the datapack is the newest by default"
        );
        let newest = versions().last().expect("supported versions");
        let wanted = format!(
            "isn't in Minecraft {newest} (the mod's datapack; it's in 1.20.1) — there it's minecraft:short_grass"
        );
        assert!(
            k.check(RefKind::Item, "minecraft:grass")
                .unwrap_err()
                .contains(&wanted)
        );
        assert_eq!(
            equivalent(RefKind::Item, "minecraft:chain", newest).as_deref(),
            Some("minecraft:iron_chain"),
            "renamed in 1.21.9"
        );
        let old = Known::new("mymod", &[ext], Some("1.20.1"));
        assert!(old.check(RefKind::Item, "yog:potion/awkward").is_ok());
        assert!(old.check(RefKind::Item, "yog:potion/wind_charged").is_err());
        assert!(old.check(RefKind::Item, "minecraft:grass").is_ok());
        assert!(
            old.check(RefKind::Block, "minecraft:crafter")
                .unwrap_err()
                .contains("isn't in Minecraft 1.20.1")
        );
        assert_eq!(
            equivalent(RefKind::Item, "minecraft:short_grass", "1.20.1").as_deref(),
            Some("minecraft:grass")
        );
        assert_eq!(
            equivalent(RefKind::Item, "minecraft:turtle_scute", "1.20.1").as_deref(),
            Some("minecraft:scute")
        );
        assert_eq!(
            equivalent(RefKind::Item, "minecraft:stick", "1.20.1").as_deref(),
            Some("minecraft:stick")
        );
        assert_eq!(
            equivalent(RefKind::Block, "minecraft:crafter", "1.20.1"),
            None,
            "1.20.1 has no crafter"
        );
        assert!(
            k.check(RefKind::Item, "minecraft:nope")
                .unwrap_err()
                .contains("in Minecraft")
        );
        assert!(
            k.check(RefKind::Item, "farmersdelight:cutting_board")
                .is_ok()
        );
        assert!(
            k.check(RefKind::Item, "farmersdelight:knife")
                .unwrap_err()
                .contains("isn't declared")
        );
        assert!(
            k.check(RefKind::Item, "create:wrench")
                .unwrap_err()
                .contains("[external.create]")
        );
        assert!(
            k.check(RefKind::Item, "stick")
                .unwrap_err()
                .contains("namespace")
        );
    }
}
