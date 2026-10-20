//! What the hosts register — items (and which open a book), blocks (with
//! their sound groups whole), inventories, recipes, dimensions — as one
//! Yog Data blob (`yog_data::binary`), read by `dev.yog.data.YogContent` in
//! java-common: one call and one format instead of a TSV dialect per kind.
//!
//! Written straight from the runtime's definitions with a
//! `yog_data::binary::Writer`: no tree is built and nothing is copied.

use yog_data::binary::Writer;

use crate::RuntimeHandlers;

/// A block's sound group: a mod's by id, else the game's (`stone`, `minecraft:stone`).
fn sound_group<'a>(h: &'a RuntimeHandlers, name: &str) -> Option<&'a yog_registry::SoundGroup> {
    h.sound_groups.get(name).or_else(|| yog_registry::SoundGroup::built_in(name))
}

/// How many blocks wide and high an item hangs its entity: its own size,
/// else the entity type's (at least one block each way).
fn hanging_size(h: &RuntimeHandlers, hangs: &yog_registry::Hangs) -> [u32; 2] {
    hangs.size.unwrap_or_else(|| {
        let def = h.entities.iter().find(|e| e.get("id").and_then(|v| v.as_str()) == Some(hangs.entity.as_str()));
        let blocks = |key: &str| def.and_then(|e| e.get(key)?.as_f64()).map_or(1, |n| n.round().max(1.0) as u32);
        [blocks("width"), blocks("height")]
    })
}

fn tool_material<'a>(h: &'a RuntimeHandlers, id: &str) -> Option<&'a yog_registry::ToolMaterial> {
    h.tool_materials.get(id).or_else(|| yog_registry::ToolMaterial::built_in(id))
}

fn armor_material<'a>(h: &'a RuntimeHandlers, id: &str) -> Option<&'a yog_registry::ArmorMaterial> {
    h.armor_materials.get(id).or_else(|| yog_registry::ArmorMaterial::built_in(id))
}

/// A tool material, whole: the host makes its tier from it.
fn write_tool_material<'v>(w: &mut Writer<'v>, m: &'v yog_registry::ToolMaterial) {
    w.map(8);
    w.key("id").str(&m.id);
    w.key("durability").int(m.durability as i64);
    w.key("speed").float(m.speed as f64);
    w.key("attack_damage").float(m.attack_damage as f64);
    w.key("enchantability").int(m.enchantability as i64);
    w.key("repair").str(&m.repair);
    w.key("level_index").int(m.level_index as i64);
    w.key("incorrect").str(&m.incorrect);
}

/// An armor material, whole.
fn write_armor_material<'v>(w: &mut Writer<'v>, m: &'v yog_registry::ArmorMaterial) {
    w.map(9);
    w.key("id").str(&m.id);
    w.key("durability").int(m.durability as i64);
    w.key("defense").map(4);
    for (k, v) in [("head", m.defense.head), ("chest", m.defense.chest), ("legs", m.defense.legs), ("feet", m.defense.feet)] {
        w.key(k).int(v as i64);
    }
    w.key("enchantability").int(m.enchantability as i64);
    w.key("equip_sound").str(&m.equip_sound);
    w.key("toughness").float(m.toughness as f64);
    w.key("knockback_resistance").float(m.knockback_resistance as f64);
    w.key("repair").str(&m.repair);
    w.key("texture").str(&m.texture);
}

fn floats(w: &mut Writer, fs: &[f32]) {
    w.list(fs.len());
    for f in fs {
        w.float(*f as f64);
    }
}

fn strings<'v>(w: &mut Writer<'v>, ss: &'v [String]) {
    w.list(ss.len());
    for s in ss {
        w.str(s);
    }
}

/// The whole group, so the host builds it without a list of its own.
fn sound<'v>(w: &mut Writer<'v>, g: &'v yog_registry::SoundGroup) {
    w.map(7);
    for (k, v) in [("break", &g.break_sound), ("step", &g.step), ("place", &g.place), ("hit", &g.hit), ("fall", &g.fall)] {
        w.key(k).str(v);
    }
    w.key("volume").float(g.volume as f64);
    w.key("pitch").float(g.pitch as f64);
}

/// The registered content: `{ items, blocks, inventories, recipes, biome_features, entities, dimensions, fonts, attributes, effects, potions }`.
pub fn blob(h: &RuntimeHandlers) -> Vec<u8> {
    // Inventories' layouts are worked out first: the writer borrows them.
    let layouts: Vec<_> = h.inventories.iter().map(|d| d.resolved_layout()).collect();
    let fonts = crate::fonts::atlases();
    let mut w = Writer::new();
    w.map(11);

    w.key("items").list(h.items.len());
    for d in &h.items {
        let tool = d.tool.as_ref().and_then(|t| {
            let m = tool_material(h, &t.material);
            if m.is_none() {
                yog_logging::warn!("item {}: no tool material {} — the game's or a mod's", d.id, t.material);
            }
            Some((t, m?))
        });
        let armor = d.armor.as_ref().and_then(|a| {
            let m = armor_material(h, &a.material);
            if m.is_none() {
                yog_logging::warn!("item {}: no armor material {} — the game's or a mod's", d.id, a.material);
            }
            Some((a, m?))
        });
        // Always: id, max_stack, max_damage, fire_resistant, fuel_ticks, book.
        let n = 6
            + d.name.is_some() as usize
            + d.tooltip.is_some() as usize
            + d.food.is_some() as usize
            + tool.is_some() as usize
            + armor.is_some() as usize
            + d.wearable.is_some() as usize
            + d.hangs.is_some() as usize;
        w.map(n);
        w.key("id").str(&d.id);
        w.key("max_stack").int(d.max_stack as i64);
        if let Some(n) = &d.name {
            w.key("name").str(n);
        }
        if let Some(t) = &d.tooltip {
            w.key("tooltip").str(t);
        }
        w.key("max_damage").int(d.max_damage as i64);
        w.key("fire_resistant").bool(d.fire_resistant);
        w.key("fuel_ticks").int(d.fuel_ticks as i64);
        if let Some(f) = &d.food {
            w.key("food").map(3);
            w.key("nutrition").int(f.nutrition as i64);
            w.key("saturation").float(f.saturation as f64);
            w.key("always_edible").bool(f.can_always_eat);
        }
        if let Some((t, m)) = tool {
            w.key("tool").map(4);
            w.key("kind").str(&t.kind);
            w.key("attack_damage").float(t.attack_damage as f64);
            w.key("attack_speed").float(t.attack_speed as f64);
            w.key("material");
            write_tool_material(&mut w, m);
        }
        if let Some((a, m)) = armor {
            w.key("armor").map(2);
            w.key("slot").str(&a.slot);
            w.key("material");
            write_armor_material(&mut w, m);
        }
        if let Some(slot) = &d.wearable {
            w.key("wearable").str(slot);
        }
        if let Some(hangs) = &d.hangs {
            let [width, height] = hanging_size(h, hangs);
            w.key("hangs").map(3);
            w.key("entity").str(&hangs.entity);
            w.key("width").int(width as i64);
            w.key("height").int(height as i64);
        }
        // A book registered under the item's id: the host makes it open the book.
        w.key("book").bool(h.books.contains(&d.id));
    }

    w.key("blocks").list(h.blocks.len());
    for d in &h.blocks {
        let group = d.sound.as_deref().and_then(|s| {
            let g = sound_group(h, s);
            if g.is_none() {
                yog_logging::warn!("block {}: no sound group {s} — the game's or a mod's", d.id);
            }
            g
        });
        // Always: id, hardness, resistance, light, requires_tool, no_collision,
        // slipperiness, connects, connect_groups, redstone, facing, placement,
        // attach, waterlogged, properties, accepts_items, no_occlusion, kind,
        // press_ticks, mobs_only.
        let n = 20
            + d.name.is_some() as usize
            + d.base.is_some() as usize
            + d.set.is_some() as usize
            + d.shape.is_some() as usize
            + group.is_some() as usize
            + d.inventory_id.is_some() as usize
            + d.powered.is_some() as usize
            + d.comparator.is_some() as usize;
        w.map(n);
        w.key("id").str(&d.id);
        w.key("hardness").float(d.hardness as f64);
        w.key("resistance").float(d.resistance as f64);
        if let Some(n) = &d.name {
            w.key("name").str(n);
        }
        if let Some(sh) = &d.shape {
            w.key("shape");
            floats(&mut w, sh);
        }
        w.key("light").int(d.light_level as i64);
        if let Some(g) = group {
            w.key("sound");
            sound(&mut w, g);
        }
        w.key("requires_tool").bool(d.requires_tool);
        w.key("no_collision").bool(d.no_collision);
        w.key("slipperiness").float(d.slipperiness as f64);
        w.key("connects").bool(d.connects);
        w.key("connect_groups");
        strings(&mut w, &d.connect_groups);
        if let Some(inv) = &d.inventory_id {
            w.key("inventory").str(inv);
        }
        w.key("redstone").bool(d.redstone_source);
        w.key("facing").str(&d.facing);
        w.key("placement").str(&d.placement);
        w.key("attach").bool(d.attach);
        w.key("waterlogged").bool(d.waterlogged);
        w.key("accepts_items").bool(d.accepts_items);
        w.key("properties").list(d.properties.len());
        for p in &d.properties {
            w.map(2);
            w.key("name").str(&p.name);
            w.key("values");
            strings(&mut w, &p.values);
        }
        if let Some(p) = &d.powered {
            w.key("powered").str(p);
        }
        if let Some(c) = &d.comparator {
            w.key("comparator").str(c);
        }
        w.key("no_occlusion").bool(d.no_occlusion);
        w.key("kind").str(&d.kind);
        if let Some(b) = &d.base {
            w.key("base").str(b);
        }
        if let Some(s) = &d.set {
            w.key("set").str(s);
        }
        w.key("press_ticks").int(d.press_ticks as i64);
        w.key("mobs_only").bool(d.mobs_only);
    }

    w.key("inventories").list(h.inventories.len());
    for (d, layout) in h.inventories.iter().zip(&layouts) {
        // Always: id, slot_count, slots, include_player_inventory, player_inv, title.
        let n = 6 + d.background_texture.is_some() as usize + d.background_size.is_some() as usize + d.machine.is_some() as usize;
        w.map(n);
        w.key("id").str(&d.id);
        w.key("slot_count").int(d.slot_count as i64);
        w.key("slots").list(layout.len());
        for s in layout {
            w.map(2);
            w.key("at");
            floats(&mut w, &[s.x, s.y]);
            w.key("accepts");
            strings(&mut w, &s.accepts);
        }
        w.key("include_player_inventory").bool(d.include_player_inventory);
        w.key("player_inv");
        floats(&mut w, &[d.player_inv_offset.0, d.player_inv_offset.1]);
        if let Some(t) = &d.background_texture {
            w.key("background_texture").str(t);
        }
        w.key("title").str(&d.title);
        if let Some((bw, bh)) = d.background_size {
            w.key("background_size");
            floats(&mut w, &[bw, bh]);
        }
        // A machine: its ingredients' slots and its result's (the recipe is the runtime's to find).
        if let Some(m) = &d.machine {
            w.key("machine").map(2);
            w.key("places").list(m.places.len());
            for p in &m.places {
                w.int(p.inv as i64);
            }
            w.key("result").int(m.result as i64);
        }
    }

    w.key("recipes").list(h.recipes.len());
    for (ns, name, recipe) in &h.recipes {
        w.map(3);
        w.key("namespace").str(ns);
        w.key("name").str(name);
        w.key("recipe").value(recipe);
    }

    w.key("biome_features").list(h.biome_features.len());
    for f in &h.biome_features {
        w.map(3);
        w.key("feature").str(&f.feature);
        w.key("biomes").str(&f.biomes);
        w.key("step").str(&f.step);
    }

    w.key("entities").list(h.entities.len());
    for e in &h.entities {
        w.value(e);
    }

    w.key("dimensions").list(h.dimensions.len());
    for (id, def) in &h.dimensions {
        w.map(2);
        w.key("id").str(id);
        w.key("def").value(def);
    }

    w.key("fonts").list(fonts.len());
    for f in &fonts {
        w.map(5);
        w.key("id").str(&f.id);
        w.key("png").bytes(&f.png);
        w.key("rows");
        strings(&mut w, &f.rows);
        w.key("ascent").int(f.ascent as i64);
        w.key("height").int(f.height as i64);
    }

    for (key, defs) in [("attributes", &h.attributes), ("effects", &h.effects), ("potions", &h.potions)] {
        w.key(key).list(defs.len());
        for d in defs {
            w.value(d);
        }
    }
    w.finish()
}
