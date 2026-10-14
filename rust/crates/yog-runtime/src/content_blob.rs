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

/// The registered content: `{ items, blocks, inventories, recipes, dimensions }`.
pub fn blob(h: &RuntimeHandlers) -> Vec<u8> {
    // Inventories' layouts are worked out first: the writer borrows them.
    let layouts: Vec<_> = h.inventories.iter().map(|d| d.resolved_layout()).collect();
    let mut w = Writer::new();
    w.map(5);

    w.key("items").list(h.items.len());
    for d in &h.items {
        // Always: id, max_stack, max_damage, fire_resistant, fuel_ticks, book.
        let n = 6 + d.name.is_some() as usize + d.tooltip.is_some() as usize + d.food.is_some() as usize;
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
        // slipperiness, connects, connect_groups, redstone.
        let n = 10 + d.name.is_some() as usize + d.shape.is_some() as usize + group.is_some() as usize + d.inventory_id.is_some() as usize;
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
    }

    w.key("inventories").list(h.inventories.len());
    for (d, layout) in h.inventories.iter().zip(&layouts) {
        // Always: id, slot_count, slots, include_player_inventory, player_inv, title.
        let n = 6 + d.background_texture.is_some() as usize + d.background_size.is_some() as usize;
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
    }

    w.key("recipes").list(h.recipes.len());
    for (ns, name, recipe) in &h.recipes {
        w.map(3);
        w.key("namespace").str(ns);
        w.key("name").str(name);
        w.key("recipe").value(recipe);
    }

    w.key("dimensions").list(h.dimensions.len());
    for (id, def) in &h.dimensions {
        w.map(2);
        w.key("id").str(id);
        w.key("def").value(def);
    }
    w.finish()
}
