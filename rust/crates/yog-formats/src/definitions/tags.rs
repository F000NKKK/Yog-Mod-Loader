//! The tags of every Minecraft version and loader Yog supports, and Yog's
//! common tags (`yog:<path>`) over them: `data/tags.txt`, generated from
//! the game's and the loaders' jars (`data/gen_tags.py`) — what the build
//! checks references with, the editors show, and the hosts (a copy in
//! java-common) make `yog:` tags of.

use std::collections::{BTreeSet, HashMap};
use yog_sync::LazyLock;

/// The table, as generated.
pub const TABLE: &str = include_str!("../../data/tags.txt");

/// Where a tag is: every Minecraft version and loader (`None`), or these
/// hosts (`<loader>/<version>`).
pub type Hosts = Option<BTreeSet<String>>;

/// The tags of every version and loader Yog supports (`vanilla/tags.txt`,
/// generated from their jars): (registry, id) → where; and Yog's common
/// tags: (registry, path) → the tags `yog:<path>` is, the game's first.
struct Tags {
    hosts: Vec<String>,
    tags: HashMap<(String, String), Hosts>,
    yog: HashMap<(String, String), Vec<String>>,
    /// What the loaders' tags hold (on any version), by (registry, id).
    has: HashMap<(String, String), Vec<String>>,
    /// The game's content a `yog:` tag holds on every host, by (registry, path).
    base: HashMap<(String, String), Vec<String>>,
    /// The namespaces of the tags it knows, and Yog's.
    namespaces: BTreeSet<String>,
}

/// Where a tag is everywhere.
use yog_ids::tag_table::EVERYWHERE;
/// The table's records: a tag and where it is, a `yog:` tag and the tags
/// it is, what the game's content a `yog:` tag holds, what a loader's holds.
use yog_ids::tag_table::{BASE, HAS, TAG, YOG};

static TAGS: LazyLock<Tags> = LazyLock::new(|| {
    let mut hosts: Vec<String> = Vec::new();
    let (mut tags, mut yog, mut has, mut base) = (
        HashMap::new(),
        HashMap::new(),
        HashMap::new(),
        HashMap::new(),
    );
    for line in TABLE.lines() {
        if let Some(h) = line.strip_prefix(yog_ids::tag_table::HOSTS) {
            hosts = h.split_whitespace().map(str::to_string).collect();
        }
        let p: Vec<&str> = line.split_whitespace().collect();
        match p.as_slice() {
            [TAG, reg, id, place] => {
                let on = (*place != EVERYWHERE).then(|| {
                    place
                        .split(',')
                        .flat_map(|w| {
                            if w.contains('/') {
                                vec![w.to_string()]
                            } else {
                                let of_version = format!("{}{w}", yog_ids::sep::PATH);
                                hosts
                                    .iter()
                                    .filter(|h| h.ends_with(&of_version))
                                    .cloned()
                                    .collect()
                            }
                        })
                        .collect()
                });
                tags.insert((reg.to_string(), id.to_string()), on);
            }
            [YOG, reg, path, of @ ..] => {
                yog.insert(
                    (reg.to_string(), path.to_string()),
                    of.iter().map(|t| t.to_string()).collect(),
                );
            }
            [BASE, reg, path, entries @ ..] => {
                base.insert(
                    (reg.to_string(), path.to_string()),
                    entries.iter().map(|t| t.to_string()).collect(),
                );
            }
            [HAS, reg, id, entries @ ..] => {
                has.insert(
                    (reg.to_string(), id.to_string()),
                    entries.iter().map(|t| t.to_string()).collect(),
                );
            }
            _ => {}
        }
    }
    let namespaces = tags
        .keys()
        .map(|(_, id): &(String, String)| yog_ids::split(id).0.to_string())
        .chain([yog_ids::ns::YOG.to_string()])
        .collect();
    Tags {
        hosts,
        tags,
        yog,
        has,
        base,
        namespaces,
    }
});

/// Every host Yog supports: `<loader>/<version>`.
pub fn hosts() -> &'static [String] {
    &TAGS.hosts
}

/// Host `<loader>/<version>`'s version.
pub fn host_version(host: &str) -> Option<&str> {
    host.split_once(yog_ids::sep::PATH).map(|(_, v)| v)
}

/// Whether the tables know tags of namespace `ns` (the game's, the loaders', Yog's).
pub fn tags_known(ns: &str) -> bool {
    TAGS.namespaces.contains(ns)
}

/// Where tag `id` (of `registry`) is, if the tables know it: a `yog:` tag
/// is wherever one of its tags is. `None` for a tag they don't know.
pub fn tag_hosts(registry: &str, id: &str) -> Option<Hosts> {
    let (ns, path) = id.split_once(yog_ids::sep::NAMESPACE)?;
    if ns == yog_ids::ns::YOG {
        let key = (registry.to_string(), path.to_string());
        let of = TAGS.yog.get(&key)?;
        // It holds the game's content everywhere.
        if TAGS.base.contains_key(&key) {
            return Some(None);
        }
        let mut on = BTreeSet::new();
        for t in of {
            match TAGS.tags.get(&(registry.to_string(), t.clone()))? {
                None => return Some(None),
                Some(h) => on.extend(h.iter().cloned()),
            }
        }
        return Some((on.len() < TAGS.hosts.len()).then_some(on));
    }
    TAGS.tags
        .get(&(registry.to_string(), id.to_string()))
        .cloned()
}

/// The `yog:` tag that is `id` (`c:iron_ingots` → `yog:ingots/iron`).
pub fn yog_tag_for(registry: &str, id: &str) -> Option<String> {
    TAGS.yog
        .iter()
        .find(|((r, _), of)| r == registry && of.iter().any(|t| t == id))
        .map(|((_, p), _)| yog_ids::join(yog_ids::ns::YOG, p))
}

/// `hosts` for people: `Fabric 1.20.1, Minecraft 1.21.1` (every loader of a version).
pub fn describe_hosts(hosts: &BTreeSet<String>) -> String {
    let versions: BTreeSet<&str> = TAGS.hosts.iter().filter_map(|h| host_version(h)).collect();
    let mut out = Vec::new();
    for v in versions {
        let all: Vec<&String> = TAGS
            .hosts
            .iter()
            .filter(|h| host_version(h) == Some(v))
            .collect();
        let on: Vec<&String> = all.iter().copied().filter(|h| hosts.contains(*h)).collect();
        if on.len() == all.len() {
            out.push(format!("Minecraft {v}"));
        } else {
            out.extend(on.iter().map(|h| h.replace('/', " ")));
        }
    }
    out.join(", ")
}

/// The tags `yog:<path>` of `registry` is (the game's first), if it's one of Yog's.
pub fn yog_tag(registry: &str, path: &str) -> Option<&'static [String]> {
    TAGS.yog
        .get(&(registry.to_string(), path.to_string()))
        .map(Vec::as_slice)
}

/// Yog's common tags of `registry`: each path, and the tags it is.
pub fn yog_tags(registry: &str) -> Vec<(&'static str, &'static [String])> {
    let mut out: Vec<_> = TAGS
        .yog
        .iter()
        .filter(|((r, _), _)| r == registry)
        .map(|((_, p), of)| (p.as_str(), of.as_slice()))
        .collect();
    out.sort_by_key(|(p, _)| *p);
    out
}

/// The game's content `yog:<path>` holds on every host, whatever tags it has there.
pub fn yog_base(registry: &str, path: &str) -> &'static [String] {
    TAGS.base
        .get(&(registry.to_string(), path.to_string()))
        .map_or(&[], Vec::as_slice)
}

/// What a loader's tag (`c:ingots/iron`) holds — ids and `#tags` — on any
/// version; the game's own tags are read from its jars instead.
pub fn loader_tag(registry: &str, id: &str) -> Option<&'static [String]> {
    TAGS.has
        .get(&(registry.to_string(), id.to_string()))
        .map(Vec::as_slice)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yog_tags_are_the_game_and_loader_tags_of_each_host() {
        assert_eq!(tag_hosts("item", "yog:ingots/iron"), Some(None));
        assert_eq!(
            tag_hosts("item", "minecraft:music_discs")
                .flatten()
                .map(|h| h.len()),
            Some(5),
            "1.19.2 and 1.20.1, each on every loader it has"
        );
        assert_eq!(
            yog_tag_for("item", "c:iron_ingots").as_deref(),
            Some("yog:ingots/iron")
        );
        assert_eq!(
            yog_tag_for("item", "forge:cobblestone").as_deref(),
            Some("yog:cobblestones")
        );
        assert_eq!(tag_hosts("item", "yog:nope"), None);
        assert_eq!(
            tag_hosts("item", "yog:nuggets/gold"),
            Some(None),
            "Fabric 1.20.1 has no such tag, but its base is the game's nugget"
        );
        assert_eq!(
            yog_base("item", "nuggets/gold"),
            &["minecraft:gold_nugget".to_string()][..]
        );
        assert_eq!(
            yog_tag("item", "planks"),
            Some(&["minecraft:planks".to_string()][..])
        );
        assert!(
            loader_tag("item", "c:iron_ingots")
                .is_some_and(|e| e.iter().any(|i| i == "minecraft:iron_ingot"))
        );
    }
}
