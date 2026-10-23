//! How the game draws items whose look isn't their textures alone — heads,
//! spawn eggs, tinted potions and leather — as data
//! (`assets/item_looks.ydata`), for tools that show items as the game does.

use yog_sync::LazyLock;

use yog_data::Ordered;
use yog_graphics::Color;

/// The models items are drawn through in a GUI.
#[derive(yog_data::Schema, Debug, Clone, PartialEq, Default)]
pub struct LookModels {
    /// A potion's (Yog's potion aliases are drawn through it).
    pub potion: String,
    /// The template every spawn egg's model is.
    pub spawn_egg: String,
    /// Models drawn as their mob's head.
    pub heads: Vec<String>,
}

/// A colour the game tints an item's first layer with: these items, or
/// every item whose id starts with `prefix`.
#[derive(yog_data::Schema, Debug, Clone, PartialEq)]
pub struct ItemTint {
    pub color: Color,
    #[data(default)]
    pub items: Vec<String>,
    #[data(default)]
    pub prefix: String,
}

/// The game's item looks.
#[derive(yog_data::Schema, Debug, Clone, PartialEq, Default)]
pub struct ItemLooks {
    pub models: LookModels,
    /// Items drawn as a mob's head, by item path: the mob's texture.
    pub heads: Ordered<String>,
    pub tints: Vec<ItemTint>,
    /// Spawn eggs' colours (base, spots), by mob.
    pub spawn_eggs: Ordered<Vec<Color>>,
}

static ITEM_LOOKS: LazyLock<ItemLooks> = LazyLock::new(|| {
    yog_data::built_in::<ItemLooks>(
        "yog-registry's item looks",
        include_str!("../../assets/item_looks.ydata"),
    )
});

impl ItemLooks {
    /// The game's.
    pub fn game() -> &'static ItemLooks {
        &ITEM_LOOKS
    }

    /// The texture of the mob whose head item `path` (`zombie_head`) is.
    pub fn head(&self, path: &str) -> Option<&str> {
        self.heads
            .0
            .iter()
            .find(|(p, _)| p == path)
            .map(|(_, t)| t.as_str())
    }

    /// Mob `mob`'s spawn egg colours: base, spots.
    pub fn spawn_egg(&self, mob: &str) -> Option<(Color, Color)> {
        match self
            .spawn_eggs
            .0
            .iter()
            .find(|(m, _)| m == mob)?
            .1
            .as_slice()
        {
            [base, spots] => Some((*base, *spots)),
            _ => None,
        }
    }

    /// The tint of item `id`'s first layer.
    pub fn tint(&self, id: &str) -> Option<Color> {
        self.tints
            .iter()
            .find(|t| {
                t.items.iter().any(|i| i == id)
                    || (!t.prefix.is_empty() && id.starts_with(&t.prefix))
            })
            .map(|t| t.color)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_game_looks_read() {
        let l = ItemLooks::game();
        assert_eq!(
            l.head("zombie_head"),
            Some("minecraft:entity/zombie/zombie")
        );
        assert_eq!(
            l.spawn_egg("creeper"),
            Some((Color(0xFF0D_A70B), Color(0xFF00_0000)))
        );
        assert_eq!(l.tint("minecraft:potion"), Some(Color(0xFF38_5DC6)));
        assert_eq!(l.tint("minecraft:leather_boots"), Some(Color(0xFFA0_6540)));
    }
}
