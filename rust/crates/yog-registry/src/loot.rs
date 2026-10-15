//! Loot: what a loot table gave — a chest's, a block's drops, a mob's —
//! as the mods see it before it's handed out ([`Loot`]), and what a mod
//! changes in it declaratively ([`LootModifier`], a `.ydef`).

/// A stack of items: its id, how many, and its custom data (SNBT, `{}` for none).
#[derive(yog_data::Schema, Debug, Clone, PartialEq)]
pub struct LootStack {
    pub id: String,
    pub count: u32,
    #[data(default)]
    pub nbt: String,
}

impl LootStack {
    pub fn new(id: impl Into<String>, count: u32) -> Self {
        Self { id: id.into(), count, nbt: String::new() }
    }
}

/// A loot table rolled: where and for whom, and what it gave. Handlers
/// change `items`; each sees the one before's result.
#[derive(yog_data::Schema, Debug, Clone, Default, PartialEq)]
pub struct Loot {
    /// The table (`minecraft:chests/simple_dungeon`, `minecraft:blocks/amethyst_cluster`).
    pub table: String,
    /// The dimension it's rolled in (`minecraft:overworld`).
    #[data(default)]
    pub dimension: String,
    /// Where: the chest, the broken block, the mob.
    pub pos: Option<[f64; 3]>,
    /// The entity it's for: the mob killed, the player opening the chest.
    pub entity: Option<String>,
    /// Who killed it (an entity's uuid).
    pub killer: Option<String>,
    /// The tool breaking the block.
    pub tool: Option<LootStack>,
    /// The tool's enchantments and levels (`minecraft:fortune`, 3).
    #[data(default)]
    pub enchantments: Vec<(String, u32)>,
    /// What it gave.
    #[data(default)]
    pub items: Vec<LootStack>,
}

impl Loot {
    /// The tool's level of `enchantment` (`minecraft:fortune`), 0 without it.
    pub fn enchantment(&self, enchantment: &str) -> u32 {
        self.enchantments.iter().find(|(e, _)| e == enchantment).map_or(0, |(_, l)| *l)
    }
}

/// Changes a mod makes to the loot of some tables: adds items, scales
/// some. A `.ydef` (`kind: "loot_modifier"`) or built in code.
#[derive(yog_data::Schema, Debug, Clone, Default, PartialEq)]
pub struct LootModifier {
    pub id: String,
    /// The tables it changes (`minecraft:chests/simple_dungeon`).
    pub tables: Vec<String>,
    /// Items added.
    #[data(default)]
    pub add: Vec<LootAdd>,
    /// Items whose counts are scaled.
    #[data(default)]
    pub scale: Vec<LootScale>,
}

/// An item a [`LootModifier`] adds.
#[derive(yog_data::Schema, Debug, Clone, PartialEq)]
pub struct LootAdd {
    pub item: String,
    /// How many: from `count[0]` to `count[1]`, evenly; below 1 adds none.
    #[data(default = "one_each")]
    pub count: [i32; 2],
    /// The chance it's added at all (0–1).
    #[data(default = "always")]
    #[data(range = "0..=1")]
    pub chance: f64,
    /// A loot function of the mod's code that finishes the stack (its
    /// custom data): registered by name.
    pub function: Option<String>,
}

/// Item counts a [`LootModifier`] scales.
#[derive(yog_data::Schema, Debug, Clone, PartialEq)]
pub struct LootScale {
    pub item: String,
    /// The factor; the count is rounded down (`0.5`: half, at least none).
    #[data(range = "0..=64")]
    pub by: f64,
}

fn one_each() -> [i32; 2] {
    [1, 1]
}

fn always() -> f64 {
    1.0
}

impl LootModifier {
    /// Applies it to `loot` (if its table is one of `tables`): `random`
    /// gives numbers in `[0, 1)`; `finish` runs a named loot function on a
    /// stack it adds.
    pub fn apply(&self, loot: &mut Loot, random: &mut dyn FnMut() -> f64, finish: &mut dyn FnMut(&str, &mut LootStack, &Loot)) {
        if !self.tables.iter().any(|t| *t == loot.table) {
            return;
        }
        for s in &self.scale {
            for stack in loot.items.iter_mut().filter(|st| st.id == s.item) {
                stack.count = (stack.count as f64 * s.by).floor() as u32;
            }
        }
        loot.items.retain(|st| st.count > 0);
        for a in &self.add {
            if a.chance < 1.0 && random() >= a.chance {
                continue;
            }
            let (lo, hi) = (a.count[0].min(a.count[1]), a.count[0].max(a.count[1]));
            let n = lo + (random() * (hi - lo + 1) as f64) as i32;
            if n < 1 {
                continue;
            }
            let mut stack = LootStack::new(a.item.clone(), n as u32);
            if let Some(f) = &a.function {
                finish(f, &mut stack, loot);
            }
            loot.items.push(stack);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_modifier_scales_and_adds_on_its_tables() {
        let m = LootModifier {
            id: "m".into(),
            tables: vec!["minecraft:chests/simple_dungeon".into()],
            add: vec![LootAdd { item: "mymod:scroll".into(), count: [-1, 1], chance: 1.0, function: Some("mymod:pattern".into()) }],
            scale: vec![LootScale { item: "minecraft:amethyst_shard".into(), by: 0.5 }],
        };
        let mut loot = Loot { table: "minecraft:chests/simple_dungeon".into(), items: vec![LootStack::new("minecraft:amethyst_shard", 3)], ..Default::default() };
        let mut r = [0.99].into_iter();
        m.apply(&mut loot, &mut || r.next().unwrap(), &mut |f, s, _| s.nbt = format!("{{f:\"{f}\"}}"));
        assert_eq!(loot.items[0].count, 1, "3 × 0.5, rounded down");
        assert_eq!((loot.items[1].id.as_str(), loot.items[1].count, loot.items[1].nbt.as_str()), ("mymod:scroll", 1, "{f:\"mymod:pattern\"}"));

        let mut none = Loot { table: "minecraft:chests/simple_dungeon".into(), ..Default::default() };
        m.apply(&mut none, &mut || 0.0, &mut |_, _, _| {});
        assert!(none.items.is_empty(), "-1 adds nothing");

        let mut other = Loot { table: "minecraft:chests/igloo_chest".into(), items: vec![LootStack::new("minecraft:amethyst_shard", 3)], ..Default::default() };
        m.apply(&mut other, &mut || 0.0, &mut |_, _, _| {});
        assert_eq!(other.items[0].count, 3, "another table");
    }

    #[test]
    fn loot_crosses_as_yog_data() {
        use yog_data::Schema;
        let loot = Loot {
            table: "minecraft:blocks/amethyst_cluster".into(),
            pos: Some([1.0, 2.0, 3.0]),
            tool: Some(LootStack::new("minecraft:diamond_pickaxe", 1)),
            enchantments: vec![("minecraft:fortune".into(), 3)],
            items: vec![LootStack::new("minecraft:amethyst_shard", 4)],
            ..Default::default()
        };
        let bytes = yog_data::binary::encode(&loot.write());
        let back = yog_data::read_bytes::<Loot>(&bytes).unwrap();
        assert_eq!(back, loot);
        assert_eq!(back.enchantment("minecraft:fortune"), 3);
    }
}
