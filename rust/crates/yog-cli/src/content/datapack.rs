//! Where files go in the pack the build writes: the game's data, and the
//! overlays a host applies on one loader and version
//! (`when/<loader|any>/<version>/<file>`, `yog_ids::pack`).

use yog_ids::pack::{ANY_LOADER, DATA, OVERLAYS, TAGS};

/// `file` in overlay `when` (`fabric/1.20`); `file` itself when `when` is empty.
pub fn overlay(when: &str, file: &str) -> String {
    if when.is_empty() { file.to_string() } else { format!("{OVERLAYS}/{when}/{file}") }
}

/// `file` in the overlay of every loader on `version`.
pub fn for_version(version: &str, file: &str) -> String {
    overlay(&format!("{ANY_LOADER}/{version}"), file)
}

/// The file of tag `tag` (`minecraft:needs_iron_tool`) of the content `kind`
/// (`block`, `item`, …).
pub fn tag_file(kind: &str, tag: &str) -> String {
    let (ns, path) = yog_ids::split(tag);
    format!("{DATA}/{ns}/{TAGS}/{}/{path}.json", yog_ids::registry_of(kind))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_are_where_the_hosts_look() {
        assert_eq!(tag_file("block", "needs_iron_tool"), "data/minecraft/tags/block/needs_iron_tool.json");
        assert_eq!(overlay("fabric/1.20", "data/x.json"), "when/fabric/1.20/data/x.json");
        assert_eq!(for_version("1.21", "data/x.json"), "when/any/1.21/data/x.json");
        assert_eq!(overlay("", "data/x.json"), "data/x.json");
    }
}
