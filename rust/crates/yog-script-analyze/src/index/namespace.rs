//! The game's classes under the `Game` namespace, as C# names them: a
//! package's segments in PascalCase (`net/minecraft/world/entity/player/Player`
//! is `Game.World.Entity.Player.Player`, `com/mojang/blaze3d/…` is
//! `Game.Mojang.Blaze3d.…`), a nested class with its outer one
//! (`OutgoingChatMessage.Player`).

use super::NESTED;

/// The game's packages and what each is under `Game`: `net/minecraft/` is
/// the namespace itself, `com/mojang/` its `Mojang`.
pub const GAME_PACKAGES: [(&str, &str); 2] = [("net/minecraft/", ""), ("com/mojang/", "mojang/")];
/// What separates a package's segments, and a namespace's.
const PACKAGE_SEP: char = '/';
const NAMESPACE_SEP: char = '.';

/// Whether `internal` is one of the game's own classes.
pub fn is_game_class(internal: &str) -> bool {
    GAME_PACKAGES
        .iter()
        .any(|(root, _)| internal.starts_with(root))
}

/// Class `internal` as `Game`'s: its namespace under `Game` (`World`,
/// `Entity`, …) and its name (`Player`, `OutgoingChatMessage.Player`).
pub fn of_class(internal: &str) -> Option<(Vec<String>, String)> {
    let (root, under) = GAME_PACKAGES
        .iter()
        .find(|(root, _)| internal.starts_with(root))?;
    let path = format!("{under}{}", &internal[root.len()..]);
    let (package, name) = path.rsplit_once(PACKAGE_SEP).unwrap_or(("", &path));
    let namespace = package
        .split(PACKAGE_SEP)
        .filter(|s| !s.is_empty())
        .map(yog_codegen::pascal)
        .collect();
    Some((namespace, name.replace(NESTED, &NAMESPACE_SEP.to_string())))
}

/// The class `name` (`Player`, `OutgoingChatMessage.Player`) of namespace
/// `namespace` under `Game` (`["World", "Entity", "Player"]`), as its
/// internal name — the packages' segments are lower case.
pub fn class_of(namespace: &[String], name: &str) -> String {
    let package: String = namespace
        .iter()
        .map(|s| format!("{}{PACKAGE_SEP}", s.to_lowercase()))
        .collect();
    let name = name.replace(NAMESPACE_SEP, &NESTED.to_string());
    let (root, rest) = GAME_PACKAGES
        .iter()
        .find(|(_, under)| !under.is_empty() && package.starts_with(under))
        .map_or((GAME_PACKAGES[0].0, package.as_str()), |(root, under)| {
            (*root, &package[under.len()..])
        });
    format!("{root}{rest}{name}")
}

#[cfg(test)]
mod tests;
