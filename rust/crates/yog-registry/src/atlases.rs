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
}

static ATLASES: LazyLock<Vec<Atlas>> = LazyLock::new(|| {
    yog_data::from_text::<Vec<Atlas>>(include_str!("../assets/atlases.ydata"))
        .unwrap_or_else(|e| panic!("yog-registry's atlases: {e}"))
        .value
});

impl Atlas {
    /// The game's.
    pub fn game() -> &'static [Atlas] {
        &ATLASES
    }

    /// The atlas a texture at `path` (`block/ruby`) belongs to by its folder.
    pub fn of_path(path: &str) -> Option<&'static Atlas> {
        let dir = path.split(yog_ids::sep::PATH).next()?;
        ATLASES.iter().find(|a| a.dirs.iter().any(|d| d == dir))
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
        assert_eq!(super::Atlas::find("minecraft:gui").and_then(|a| a.since.as_deref()), Some("1.20.2"));
        assert_eq!(super::Atlas::of_path("item/staff/oak").map(|a| a.id.as_str()), Some("minecraft:blocks"));
        assert!(super::Atlas::of_path("entity/wisp").is_none());
    }
}
