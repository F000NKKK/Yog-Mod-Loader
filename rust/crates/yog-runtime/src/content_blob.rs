//! What the hosts register — items (and which open a book), blocks (with
//! their sound groups whole), inventories, recipes, dimensions — as one
//! Yog Data blob: `yog_event::content::Content`, which the hosts read
//! through the Java records generated from it (`YogTypes.Content`).

use yog_event::content::{
    Armor, Block, Content, Dimension, Hangs, Inventory, InventorySlot, Item, Machine, Recipe,
    Tool,
};

use crate::handlers::RuntimeHandlers;

/// The registered content, encoded.
pub fn blob(h: &RuntimeHandlers) -> Vec<u8> {
    yog_data::to_bytes(&content(h))
}

/// The registered content.
fn content(h: &RuntimeHandlers) -> Content {
    // A block made from another (stairs from planks) comes after it: the host makes it of it.
    let (bases, made): (Vec<_>, Vec<_>) = h.blocks.iter().partition(|d| d.base.is_none());
    Content {
        items: h.items.iter().map(|d| item(h, d)).collect(),
        blocks: bases.into_iter().chain(made).map(|d| block(h, d)).collect(),
        inventories: h.inventories.iter().map(inventory).collect(),
        recipes: h
            .recipes
            .iter()
            .map(|(ns, name, recipe)| Recipe {
                namespace: ns.clone(),
                name: name.clone(),
                recipe: recipe.clone(),
            })
            .collect(),
        biome_features: h.biome_features.clone(),
        entities: h
            .entities
            .iter()
            .map(|e| e.clone().with_default_sounds())
            .collect(),
        dimensions: h
            .dimensions
            .iter()
            .map(|(id, def)| Dimension {
                id: id.clone(),
                def: def.clone(),
            })
            .collect(),
        fonts: crate::fonts::atlases(),
        attributes: h.attributes.clone(),
        effects: h.effects.clone(),
        potions: h.potions.clone(),
    }
}

/// `id` of a mod's (`mods`) or the game's (`built_in`); `None` (logged) for neither.
fn named<'a, T>(
    what: (&str, &str),
    id: &str,
    mods: &'a std::collections::HashMap<String, T>,
    built_in: fn(&str) -> Option<&'static T>,
) -> Option<&'a T> {
    let found = mods.get(id).or_else(|| built_in(id));
    if found.is_none() {
        let (kind, owner) = what;
        yog_logging::warn!("{owner}: no {kind} {id} — the game's or a mod's");
    }
    found
}

/// An item, its materials whole.
fn item(h: &RuntimeHandlers, d: &yog_registry::ItemDef) -> Item {
    let tool = d.tool.as_ref().and_then(|t| {
        let m = named(
            ("tool material", &d.id),
            &t.material,
            &h.tool_materials,
            yog_registry::ToolMaterial::built_in,
        )?;
        Some(Tool {
            kind: t.kind.clone(),
            attack_damage: t.attack_damage,
            attack_speed: t.attack_speed,
            game_tool: m.game_tool(&t.kind),
            material: m.clone(),
        })
    });
    let armor = d.armor.as_ref().and_then(|a| {
        let m = named(
            ("armor material", &d.id),
            &a.material,
            &h.armor_materials,
            yog_registry::ArmorMaterial::built_in,
        )?;
        Some(Armor {
            slot: a.slot.clone(),
            material: m.clone(),
        })
    });
    Item {
        id: d.id.clone(),
        max_stack: d.max_stack.into(),
        name: d.name.clone().unwrap_or_default(),
        tooltip: d.tooltip.clone().unwrap_or_default(),
        max_damage: d.max_damage as i32,
        fire_resistant: d.fire_resistant,
        fuel_ticks: d.fuel_ticks as i32,
        food: d.food.clone(),
        book: h.books.contains(&d.id),
        tool,
        armor,
        wearable: d.wearable.clone(),
        hangs: d.hangs.as_ref().map(|hangs| hanging(h, hangs)),
    }
}

/// What an item hangs: its own size, else the entity type's (at least one block each way).
fn hanging(h: &RuntimeHandlers, hangs: &yog_registry::Hangs) -> Hangs {
    let [width, height] = hangs.size.map(|s| s.map(|n| n as i32)).unwrap_or_else(|| {
        let def = h.entities.iter().find(|e| e.id == hangs.entity);
        let blocks = |size: fn(&yog_registry::EntityDef) -> f32| {
            def.map_or(1, |e| size(e).round().max(1.0) as i32)
        };
        [blocks(|e| e.width), blocks(|e| e.height)]
    });
    Hangs {
        entity: hangs.entity.clone(),
        width,
        height,
    }
}

/// A block, its sound group whole.
fn block(h: &RuntimeHandlers, d: &yog_registry::BlockDef) -> Block {
    let sound = d.sound.as_deref().and_then(|s| {
        named(
            ("sound group", &d.id),
            s,
            &h.sound_groups,
            yog_registry::SoundGroup::built_in,
        )
    });
    Block {
        id: d.id.clone(),
        hardness: d.hardness,
        resistance: d.resistance,
        name: d.name.clone().unwrap_or_default(),
        shape: d.shape.map(Vec::from),
        light: d.light_level.into(),
        sound: sound.cloned(),
        requires_tool: d.requires_tool,
        no_collision: d.no_collision,
        slipperiness: d.slipperiness,
        connects: d.connects,
        connect_groups: d.connect_groups.clone(),
        inventory: d.inventory_id.clone(),
        redstone: d.redstone_source,
        facing: d.facing.clone(),
        placement: d.placement.clone(),
        attach: d.attach,
        waterlogged: d.waterlogged,
        properties: d.properties.clone(),
        powered: d.powered.clone(),
        comparator: d.comparator.clone(),
        accepts_items: d.accepts_items,
        no_occlusion: d.no_occlusion,
        kind: d.kind.clone(),
        base: d.base.clone(),
        set: d.set.clone(),
        press_ticks: d.press_ticks as i32,
        mobs_only: d.mobs_only,
    }
}

/// An inventory with its slots where its layout puts them.
fn inventory(d: &yog_inventory::InventoryDef) -> Inventory {
    let (x, y) = d.player_inv_offset;
    Inventory {
        id: d.id.clone(),
        slot_count: d.slot_count as i32,
        slots: d
            .resolved_layout()
            .into_iter()
            .map(|s| InventorySlot {
                at: vec![s.x, s.y],
                accepts: s.accepts,
            })
            .collect(),
        include_player_inventory: d.include_player_inventory,
        player_inv: vec![x, y],
        background_texture: d.background_texture.clone(),
        title: d.title.clone(),
        background_size: d.background_size.map(|(w, h)| vec![w, h]),
        machine: d.machine.as_ref().map(|m| Machine {
            places: m.places.iter().map(|p| p.inv as i32).collect(),
            result: m.result as i32,
        }),
    }
}
