//! The game's tool ladder as data (`assets/tool_levels.ydata`): its levels
//! bottom to top, and the block tags that make a block need each level on
//! every loader and version.

use std::sync::LazyLock;

/// A block tag, and where it applies: `<loader|any>/<version>`, or
/// everywhere when empty.
#[derive(yog_data::Schema, Debug, Clone, PartialEq)]
pub struct LevelTag {
    pub tag: String,
    #[data(default)]
    pub when: String,
}

/// One of the game's tool levels.
#[derive(yog_data::Schema, Debug, Clone, PartialEq)]
pub struct ToolLevel {
    /// `minecraft:stone`.
    pub id: String,
    /// 1.21's tag of blocks a tool of this level can't mine for drops.
    pub incorrect: String,
    /// What makes a block need this level.
    pub needs: Vec<LevelTag>,
}

/// What a block needing a level above the game's top one gets.
#[derive(yog_data::Schema, Debug, Clone, PartialEq, Default)]
pub struct AboveTop {
    pub needs: Vec<LevelTag>,
    /// What doesn't hold everywhere, for the build to warn about.
    pub caveat: String,
}

/// The game's tool ladder.
#[derive(yog_data::Schema, Debug, Clone, PartialEq, Default)]
pub struct ToolLadder {
    /// Its id: `@ladder:minecraft:tool`.
    pub ladder: String,
    /// Where the game has [`ToolLevel::incorrect`] tags: `any/1.21`.
    pub incorrect_when: String,
    /// Bottom to top.
    pub levels: Vec<ToolLevel>,
    pub above: AboveTop,
}

static TOOL_LADDER: LazyLock<ToolLadder> = LazyLock::new(|| {
    yog_data::built_in::<ToolLadder>(
        "yog-registry's tool levels",
        include_str!("../assets/tool_levels.ydata"),
    )
});

impl ToolLadder {
    /// The game's.
    pub fn game() -> &'static ToolLadder {
        &TOOL_LADDER
    }

    /// One of its levels by short name or id (`iron` = `minecraft:iron`).
    pub fn level(&self, name: &str) -> Option<&ToolLevel> {
        let id = yog_ids::qualify(name);
        self.levels.iter().find(|l| l.id == id)
    }

    /// Its levels' ids, bottom to top.
    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.levels.iter().map(|l| l.id.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_game_ladder_reads() {
        let l = ToolLadder::game();
        assert_eq!(
            l.ids().collect::<Vec<_>>(),
            [
                "minecraft:wood",
                "minecraft:stone",
                "minecraft:iron",
                "minecraft:diamond",
                "minecraft:netherite"
            ]
        );
        assert_eq!(
            l.level("iron").unwrap().needs[0].tag,
            "minecraft:needs_iron_tool"
        );
        assert!(l.above.needs.iter().any(|t| t.when == "fabric/1.20"));
    }
}
