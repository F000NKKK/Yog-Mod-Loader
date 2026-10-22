//! Where files go in the pack the build writes: the game's data, and the
//! overlays a host applies on one loader and version
//! (`when/<loader|any>/<version>/<file>`, `yog_ids::pack`).

use yog_ids::pack::{ANY_LOADER, DATA, OVERLAYS, TAGS};

/// `file` in overlay `when` (`fabric/1.20`); `file` itself when `when` is empty.
pub(crate) fn overlay(when: &str, file: &str) -> String {
    if when.is_empty() {
        file.to_string()
    } else {
        format!("{OVERLAYS}/{when}/{file}")
    }
}

/// `file` in the overlay of every loader on `version`.
pub(crate) fn for_version(version: &str, file: &str) -> String {
    overlay(&format!("{ANY_LOADER}/{version}"), file)
}

/// The file of tag `tag` (`minecraft:needs_iron_tool`) of the content `kind`
/// (`block`, `item`, …).
pub(crate) fn tag_file(kind: &str, tag: &str) -> String {
    data_in(
        &[TAGS, yog_ids::registry_of(kind)].join(yog_ids::sep::PATH),
        tag,
    )
}

/// Data `id`'s file in folder `dir` of its namespace's data (`worldgen/placed_feature`).
pub(crate) fn data_in(dir: &str, id: &str) -> String {
    let (ns, path) = yog_ids::split(id);
    format!("{DATA}/{ns}/{dir}/{path}{}", yog_ids::res::JSON)
}

/// The file data `id` of content `kind` is written to: the current folder's.
pub(crate) fn data_file(kind: &str, id: &str) -> String {
    data_in(yog_ids::registry_of(kind), id)
}

/// The files data `id` of content `kind` may be in: the current folder's
/// first, then 1.20's plural one (`loot_table`, `loot_tables`).
pub(crate) fn data_files(kind: &str, id: &str) -> Vec<String> {
    yog_ids::registry_dirs(kind)
        .iter()
        .map(|dir| data_in(dir, id))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_are_where_the_hosts_look() {
        assert_eq!(
            tag_file("block", "needs_iron_tool"),
            "data/minecraft/tags/block/needs_iron_tool.json"
        );
        assert_eq!(
            overlay("fabric/1.20", "data/x.json"),
            "when/fabric/1.20/data/x.json"
        );
        assert_eq!(
            for_version("1.21", "data/x.json"),
            "when/any/1.21/data/x.json"
        );
        assert_eq!(overlay("", "data/x.json"), "data/x.json");
    }
}
