//! The game's texture atlases as data (`assets/atlases.ydata`): what a
//! `.ytexture` can be packed into.

use std::sync::LazyLock;

/// One of the game's atlases.
#[derive(yog_data::Schema, Debug, Clone, PartialEq)]
pub struct Atlas {
    /// `minecraft:blocks`: its definition is `assets/minecraft/atlases/blocks.json`.
    pub id: String,
    pub description: String,
    /// The first version that has it (every version Yog supports if none).
    pub since: Option<String>,
    /// The texture folders the game puts in it whole.
    #[data(default)]
    pub dirs: Vec<String>,
    /// Its sprites are named without their folder: `mob_effect/speed` is sprite `speed`.
    #[data(default)]
    pub bare: bool,
}

static ATLASES: LazyLock<Vec<Atlas>> = LazyLock::new(|| {
    yog_data::built_in::<Vec<Atlas>>(
        "yog-registry's atlases",
        include_str!("../assets/atlases.ydata"),
    )
});

impl Atlas {
    /// The game's.
    pub fn game() -> &'static [Atlas] {
        &ATLASES
    }

    /// The atlas a texture at `path` (`block/ruby`) belongs to by its folder.
    pub fn of_path(path: &str) -> Option<&'static Atlas> {
        ATLASES.iter().find(|a| a.dir_of(path).is_some())
    }

    /// Which of its folders `path` is in, and the rest of it.
    fn dir_of<'p>(&self, path: &'p str) -> Option<&'p str> {
        self.dirs.iter().find_map(|d| {
            path.strip_prefix(d.as_str())?
                .strip_prefix(yog_ids::sep::PATH)
        })
    }

    /// The sprite texture `path` is in this atlas: the path itself, or — a
    /// bare atlas's folder — the rest of it (`mob_effect/speed` → `speed`).
    pub fn sprite<'p>(&self, path: &'p str) -> &'p str {
        if self.bare {
            self.dir_of(path).unwrap_or(path)
        } else {
            path
        }
    }

    /// One of the game's, by id (`blocks` = `minecraft:blocks`).
    pub fn find(id: &str) -> Option<&'static Atlas> {
        let id = yog_ids::qualify(id);
        ATLASES.iter().find(|a| a.id == id)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_games_atlases_read() {
        assert!(super::Atlas::find("blocks").is_some());
        assert_eq!(
            super::Atlas::find("minecraft:gui").and_then(|a| a.since.as_deref()),
            Some("1.20.2")
        );
        assert_eq!(
            super::Atlas::of_path("item/staff/oak").map(|a| a.id.as_str()),
            Some("minecraft:blocks")
        );
        assert!(super::Atlas::of_path("entity/wisp").is_none());
        let effects = super::Atlas::of_path("mob_effect/clarity").unwrap();
        assert_eq!(effects.sprite("mob_effect/clarity"), "clarity");
        assert_eq!(
            super::Atlas::find("blocks").unwrap().sprite("block/ruby"),
            "block/ruby"
        );
        assert_eq!(
            super::Atlas::of_path("gui/sprites/hud/x").map(|a| a.id.as_str()),
            Some("minecraft:gui")
        );
    }
}
