//! The runtime's registration and dispatch, without a game: `api_*`
//! functions write into a local `RuntimeHandlers`, callbacks are this
//! test's own `extern "C"` functions, the "mod" is the test process.

use std::ffi::c_void;
use std::sync::{Mutex, RwLock};

use yog_data::Schema;
use yog_hot_reload::{GenerationAllocator, LoadedModule};

use super::*;

/// `CURRENT_REGISTRATION` is global: one registering test at a time.
static SERIAL: Mutex<()> = Mutex::new(());

fn module() -> LoadedModule {
    LoadedModule::this_process(GenerationAllocator::new().next())
}

/// Runs `f` as a mod's `yog_mod_register` would: with a registration
/// context, against fresh handlers it returns.
fn registering(f: impl FnOnce(*mut c_void)) -> Box<RwLock<RuntimeHandlers>> {
    let _serial = SERIAL.locked();
    let handlers = Box::new(RwLock::new(RuntimeHandlers::default()));
    *CURRENT_REGISTRATION.locked() = Some(module());
    f(&*handlers as *const RwLock<RuntimeHandlers> as *mut c_void);
    *CURRENT_REGISTRATION.locked() = None;
    handlers
}

/// A blob of content as `content/register` hands it over, into the handlers at `ctx`.
unsafe fn blob(ctx: *mut c_void, kind: &str, id: &str, bytes: &[u8]) {
    let handlers = unsafe { &*(ctx as *const RwLock<RuntimeHandlers>) };
    register_blob(&mut handlers.write_locked(), kind, id.to_owned(), bytes);
}

/// Content as a mod registers it: the definition's blob.
unsafe fn content(ctx: *mut c_void, kind: &str, id: &str, def: &yog_data::Value) {
    unsafe { blob(ctx, kind, id, &yog_data::binary::encode(def)) }
}

fn json(text: &str) -> yog_data::Value {
    yog_data::json::from_json_text(text).unwrap()
}

#[test]
fn items_arrive_as_their_definition() {
    let h = registering(|ctx| unsafe {
        let mut def = yog_registry::ItemDef::new("mymod:ruby").max_stack(16).name("").tooltip("Shiny").food(yog_registry::FoodDef::new(4, 0.3));
        def.fire_resistant = true;
        content(ctx, "item", "mymod:ruby", &def.write());
    });
    let h = h.read_locked();
    let item = &h.items[0];
    assert_eq!(item.id, "mymod:ruby");
    assert_eq!(item.max_stack, 16);
    assert_eq!(item.name, None, "an empty name is no name");
    assert_eq!(item.tooltip.as_deref(), Some("Shiny"));
    assert!(item.fire_resistant);
    assert_eq!(item.food.as_ref().map(|f| f.nutrition), Some(4));
}

#[test]
fn startup_grants_keep_their_items() {
    let h = registering(|ctx| unsafe {
        let g = yog_registry::StartupGrant { id: "mymod:starter".into(), items: vec!["mymod:staff".into(), "minecraft:book".into()], book: Some(String::new()), command: Some("say hi".into()) };
        content(ctx, "startup_grant", "mymod:starter", &g.write());
    });
    let h = h.read_locked();
    let g = &h.startup_grants[0];
    assert_eq!(g.items, ["mymod:staff", "minecraft:book"]);
    assert_eq!(g.book, None);
    assert_eq!(g.command.as_deref(), Some("say hi"));
}

#[test]
fn books_get_the_recipes_registered_before_and_after_them() {
    let recipe = json(r#"{"type":"shapeless","ingredients":["minecraft:stick"],"result":"minecraft:torch"}"#);
    let book = json(r#"{"id":"mymod:guide","name":"Guide","categories":[],"entries":[]}"#);
    let h = registering(|ctx| unsafe {
        content(ctx, "recipe", "mymod:a", &recipe);
        content(ctx, "book", "mymod:guide", &book);
        content(ctx, "recipe", "mymod:b", &recipe);
        content(ctx, "book", "mymod:broken", &json(r#"{"categories": "none"}"#));
        // Not Yog Data, an unknown kind: told, nothing stored.
        blob(ctx, "book", "mymod:junk", b"junk");
        content(ctx, "spell", "mymod:fireball", &book);
    });
    let h = h.read_locked();
    assert_eq!(h.recipes.len(), 2);
    assert_eq!(h.recipes[1].1, "b");
    assert!(h.book_renderers.locked().contains_key("mymod:guide"));
    assert!(!h.books.contains("mymod:broken"), "not a book: none for the host either");
    assert!(!h.book_renderers.locked().contains_key("mymod:broken"));
    assert!(!h.books.contains("mymod:junk"), "not Yog Data: not a book");
}

#[test]
fn blocks_and_inventories_arrive_as_their_definitions() {
    let h = registering(|ctx| unsafe {
        let mut b = yog_registry::BlockDef::new("mymod:box");
        b.inventory_id = Some("mymod:box".into());
        b.shape = Some([0.0, 0.0, 0.0, 16.0, 8.0, 16.0]);
        content(ctx, "block", "mymod:box", &b.write());
        let mut inv = yog_inventory::InventoryDef::new("mymod:box", 3);
        inv.background_size = Some((176.0, 120.0));
        content(ctx, "inventory", "mymod:box", &inv.write());
    });
    let h = h.read_locked();
    assert_eq!(h.blocks[0].shape, Some([0.0, 0.0, 0.0, 16.0, 8.0, 16.0]));
    assert_eq!(h.blocks[0].inventory_id.as_deref(), Some("mymod:box"));
    assert_eq!((h.inventories[0].slot_count, h.inventories[0].background_size), (3, Some((176.0, 120.0))));
}

#[test]
fn the_hosts_get_content_as_one_yog_data_blob() {
    let h = registering(|ctx| unsafe {
        let def = yog_registry::ItemDef::new("mymod:apple").max_stack(16).name("Apple").tooltip("").fuel(200).food(yog_registry::FoodDef::new(4, 1.5).can_always_eat());
        content(ctx, "item", "mymod:apple", &def.write());
        content(ctx, "item", "mymod:guide", &yog_registry::ItemDef::new("mymod:guide").write());
        content(ctx, "book", "mymod:guide", &json(r#"{"id":"mymod:guide","name":"Guide","categories":[],"entries":[]}"#));
    });
    let bytes = crate::content_blob::blob(&h.read_locked());
    let v = yog_data::binary::decode(&bytes).unwrap();
    let item = &v.get("items").unwrap().as_list().unwrap()[0];
    assert_eq!(item.get("id").and_then(|x| x.as_str()), Some("mymod:apple"));
    assert_eq!(item.get("max_stack").and_then(|x| x.as_i64()), Some(16));
    assert!(item.get("tooltip").is_none(), "an empty tooltip is no tooltip");
    let book = |i: usize| v.get("items").unwrap().as_list().unwrap()[i].get("book").and_then(|x| x.as_bool());
    assert_eq!((book(0), book(1)), (Some(false), Some(true)), "the item of a book's id opens it");
    let food = item.get("food").unwrap();
    assert_eq!((food.get("nutrition").and_then(|x| x.as_i64()), food.get("always_edible").and_then(|x| x.as_bool())), (Some(4), Some(true)));
    assert_eq!(v.get("blocks").unwrap().as_list().unwrap().len(), 0);
}

#[test]
fn tools_and_armor_arrive_with_their_materials_whole() {
    use yog_registry::{ItemDef, ToolMaterial};
    let h = registering(|ctx| unsafe {
        let mythril = ToolMaterial::like("mymod:mythril", "diamond").unwrap();
        content(ctx, "tool_material", "mymod:mythril", &mythril.write());
        content(ctx, "item", "mymod:pick", &ItemDef::new("mymod:pick").tool("pickaxe", "mymod:mythril", 1.0, -2.8).write());
        content(ctx, "item", "mymod:helm", &ItemDef::new("mymod:helm").armor("head", "diamond").write());
        content(ctx, "item", "mymod:odd", &ItemDef::new("mymod:odd").tool("sword", "mymod:nope", 3.0, -2.4).write());
    });
    let v = yog_data::binary::decode(&crate::content_blob::blob(&h.read_locked())).unwrap();
    let items = v.get("items").unwrap().as_list().unwrap();
    let tool = items[0].get("tool").unwrap();
    assert_eq!(tool.get("kind").and_then(|k| k.as_str()), Some("pickaxe"));
    assert_eq!(tool.get("material").and_then(|m| m.get("durability")).and_then(|d| d.as_i64()), Some(1561));
    let armor = items[1].get("armor").unwrap();
    assert_eq!(armor.get("material").and_then(|m| m.get("defense")).and_then(|d| d.get("chest")).and_then(|c| c.as_i64()), Some(8));
    assert!(items[2].get("tool").is_none(), "an unknown material: a plain item");
}

#[test]
fn a_books_entries_about_absent_mods_are_left_out() {
    crate::YOG_MOD_IDS.write_locked().insert("othermod".into());
    let book = json(r#"{"id":"mymod:guide","name":"G","categories":[{"id":"mymod:c","name":"C"}],"entries":[
        {"id":"mymod:a","name":"A","category":"@category:mymod:c","pages":[]},
        {"id":"mymod:b","name":"B","category":"@category:mymod:c","pages":[],"requires":["othermod"]},
        {"id":"mymod:c","name":"C","category":"@category:mymod:c","pages":[],"requires":["pehkui"]}]}"#);
    let h = registering(|ctx| unsafe { content(ctx, "book", "mymod:guide", &book) });
    let h = h.read_locked();
    let renderers = h.book_renderers.locked();
    let ids: Vec<&str> = renderers["mymod:guide"].book().entries.iter().map(|e| e.id.as_str()).collect();
    assert_eq!(ids, ["mymod:a", "mymod:b"], "a Yog mod that's loaded counts; a missing one hides its entry");
}

#[test]
fn a_blocks_sound_group_arrives_whole() {
    let h = registering(|ctx| unsafe {
        content(ctx, "sound_group", "mymod:crystal", &yog_registry::SoundGroup::new("mymod:crystal").pitch(1.2).write());
        for (id, sound) in [("mymod:a", "mymod:crystal"), ("mymod:b", "amethyst"), ("mymod:c", "mymod:nope")] {
            content(ctx, "block", id, &yog_registry::BlockDef::new(id).sound(sound).write());
        }
    });
    let v = yog_data::binary::decode(&crate::content_blob::blob(&h.read_locked())).unwrap();
    let blocks = v.get("blocks").unwrap().as_list().unwrap();
    let sound = |i: usize, k: &str| blocks[i].get("sound").and_then(|s| s.get(k)).cloned();
    assert_eq!(sound(0, "break").as_ref().and_then(|x| x.as_str()), Some("mymod:crystal.break"));
    assert_eq!(sound(0, "pitch").and_then(|x| x.as_f64()).map(|p| (p * 10.0).round()), Some(12.0));
    assert_eq!(sound(1, "step").as_ref().and_then(|x| x.as_str()), Some("minecraft:block.amethyst_block.step"));
    assert!(blocks[2].get("sound").is_none(), "an unknown group is left to the default");
}

#[test]
fn a_block_of_a_game_kind_arrives_with_its_base_and_set() {
    let h = registering(|ctx| unsafe {
        let stairs = yog_registry::BlockDef::new("mymod:stairs").kind("stairs").base("mymod:planks");
        content(ctx, "block", "mymod:stairs", &stairs.write());
        let button = yog_registry::BlockDef::new("mymod:button").kind("button").set("dark_oak").press_ticks(20).mobs_only().no_occlusion();
        content(ctx, "block", "mymod:button", &button.write());
        content(ctx, "block", "mymod:plain", &yog_registry::BlockDef::new("mymod:plain").write());
    });
    let v = yog_data::binary::decode(&crate::content_blob::blob(&h.read_locked())).unwrap();
    let blocks = v.get("blocks").unwrap().as_list().unwrap();
    let str = |i: usize, k: &str| blocks[i].get(k).and_then(|x| x.as_str()).map(String::from);
    let int = |i: usize, k: &str| blocks[i].get(k).and_then(|x| x.as_i64());
    let yes = |i: usize, k: &str| blocks[i].get(k).and_then(|x| x.as_bool());
    assert_eq!((str(0, "kind"), str(0, "base"), str(0, "set")), (Some("stairs".into()), Some("mymod:planks".into()), None));
    assert_eq!((str(1, "set"), int(1, "press_ticks"), yes(1, "mobs_only"), yes(1, "no_occlusion")), (Some("dark_oak".into()), Some(20), Some(true), Some(true)));
    assert_eq!((str(2, "kind"), int(2, "press_ticks"), yes(2, "no_occlusion")), (Some(String::new()), Some(30), Some(false)), "a plain block: no kind");
}

/// For checking the Java reader by hand: `YOG_CONTENT_OUT=file cargo test -p yog-runtime write_a_content_blob`.
#[test]
fn write_a_content_blob() {
    let Some(out) = std::env::var_os("YOG_CONTENT_OUT") else { return };
    let mut h = RuntimeHandlers::default();
    h.items.push(yog_registry::ItemDef::new("m:apple").max_stack(16).name("Apple").food(yog_registry::FoodDef::new(4, 1.5).can_always_eat()));
    h.items.push(yog_registry::ItemDef::new("m:guide"));
    h.items.push(yog_registry::ItemDef::new("m:pick").tool("pickaxe", "minecraft:iron", 1.0, -2.8));
    h.items.push(yog_registry::ItemDef::new("m:helm").armor("head", "diamond"));
    h.books.insert("m:guide".into());
    let mut b = yog_registry::BlockDef::new("m:pipe");
    b.shape = Some([6.0, 6.0, 6.0, 10.0, 10.0, 10.0]);
    b.connects = true;
    b.connect_groups = vec!["m:pipe".into()];
    b.inventory_id = Some("m:box".into());
    b.sound = Some("metal".into());
    h.sound_groups.insert("m:crystal".into(), yog_registry::SoundGroup::new("m:crystal"));
    let mut c = yog_registry::BlockDef::new("m:crystal");
    c.sound = Some("m:crystal".into());
    h.blocks.push(c);
    h.blocks.push(b);
    let mut inv = yog_inventory::InventoryDef::new("m:box", 2);
    inv.title = "Box".into();
    h.inventories.push(inv);
    h.recipes.push(("m".into(), "pick".into(), r##"{"type":"shaped","pattern":["II"," S"],"key":{"I":"minecraft:iron_ingot","S":["minecraft:stick","#c:rods"]},"result":"m:pick","count":1}"##.into()));
    h.dimensions.insert("m:void".into(), r#"{"dimension_type":{"has_sky_light":false,"ambient_light":0.5}}"#.into());
    std::fs::write(out, crate::content_blob::blob(&h)).unwrap();
}

/// Content exchange, old (TSV and JSON strings) against new (Yog Data), on a
/// mod's real registrations: `YOG_BENCH_REG=hex_registrations.ydata
/// YOG_BENCH_OUT=dir cargo test --release -p yog-runtime content_exchange -- --ignored --nocapture`.
/// Writes what each way hands the host, for the Java side of the measure.
#[test]
#[ignore]
fn content_exchange() {
    use std::time::Instant;
    let (Some(reg), Some(out)) = (std::env::var_os("YOG_BENCH_REG"), std::env::var_os("YOG_BENCH_OUT")) else { return };
    let all = yog_data::binary::decode(&std::fs::read(reg).unwrap()).unwrap();
    let entries: Vec<(String, String, yog_data::Value)> = all
        .as_list()
        .unwrap()
        .iter()
        .map(|e| (e.get("kind").unwrap().as_str().unwrap().to_string(), e.get("id").unwrap().as_str().unwrap().to_string(), e.get("data").unwrap().clone()))
        .collect();
    let time = |what: &str, n: u32, f: &mut dyn FnMut()| {
        f();
        let t = Instant::now();
        for _ in 0..n {
            f();
        }
        println!("{what:<48} {:>9.1} µs", t.elapsed().as_secs_f64() * 1e6 / n as f64);
    };
    // Mod → runtime. New: the mod encodes, the runtime decodes and reads.
    let blobs: Vec<(String, String, Vec<u8>)> = entries.iter().map(|(k, i, v)| (k.clone(), i.clone(), yog_data::binary::encode(v))).collect();
    time("mod: encode every definition (books come built)", 200, &mut || {
        for (_, _, v) in entries.iter().filter(|(k, ..)| k != "book") {
            std::hint::black_box(yog_data::binary::encode(v));
        }
    });
    let mut handlers = None;
    time("runtime: register_content (decode + read)", 200, &mut || {
        handlers = Some(registering(|ctx| unsafe {
            for (k, i, b) in &blobs {
                blob(ctx, k, i, b);
            }
        }));
    });
    for kind in ["item", "block", "recipe", "book"] {
        time(&format!("  of which {kind}s"), 50, &mut || {
            std::hint::black_box(registering(|ctx| unsafe {
                for (k, i, b) in blobs.iter().filter(|(k, ..)| k == kind) {
                    blob(ctx, k, i, b);
                }
            }));
        });
    }
    let book = blobs.iter().find(|(k, ..)| k == "book").unwrap();
    let bv = yog_data::binary::decode(&book.2).unwrap();
    let parsed = yog_data::from_value::<yog_formats::book::Book>(&bv).unwrap().value;
    let old_json = parsed.to_json();
    time("  book, old: serde parse of its JSON", 50, &mut || { std::hint::black_box(serde_json::from_str::<yog_formats::book::Book>(&old_json).unwrap()); });
    time("  book, new: borrowed decode + read by its schema", 50, &mut || { std::hint::black_box(yog_data::read_bytes::<yog_formats::book::Book>(&book.2).unwrap()); });
    time("    of which the borrowed decode", 50, &mut || { std::hint::black_box(yog_data::binary::decode_ref(&book.2, &yog_data::Bump::new()).is_ok()); });
    time("  (owned decode, for comparison)", 50, &mut || { std::hint::black_box(yog_data::binary::decode(&book.2).unwrap()); });
    time("  book: BookRenderer::new (both)", 50, &mut || { std::hint::black_box(yog_book::BookRenderer::new(parsed.clone())); });
    let h = handlers.unwrap();
    let h = h.read_locked();
    // Runtime → host. Old: the TSV dialects and `ns\tname\tjson` lines
    // (the runtime kept each recipe's JSON as the mod sent it).
    let recipe_json: Vec<(String, String, String)> = h.recipes.iter().map(|(ns, n, v)| (ns.clone(), n.clone(), yog_data::json::to_json(v).to_string())).collect();
    let old = || {
        let items: Vec<String> = h.items.iter().map(|d| {
            let mut p = vec![d.id.clone(), format!("max_stack={}", d.max_stack)];
            if let Some(n) = &d.name { p.push(format!("name={n}")); }
            if let Some(t) = &d.tooltip { p.push(format!("tooltip={t}")); }
            if d.max_damage > 0 { p.push(format!("max_damage={}", d.max_damage)); }
            if d.fire_resistant { p.push("fire_resistant=1".into()); }
            if d.fuel_ticks > 0 { p.push(format!("fuel_ticks={}", d.fuel_ticks)); }
            if let Some(f) = &d.food { p.push(format!("food={}:{}:{}", f.nutrition, f.saturation, f.can_always_eat as u8)); }
            p.join("\t")
        }).collect();
        let blocks: Vec<String> = h.blocks.iter().map(|d| {
            let mut p = vec![d.id.clone(), format!("hardness={}", d.hardness), format!("resistance={}", d.resistance)];
            if let Some(n) = &d.name { p.push(format!("name={n}")); }
            if let Some(s) = d.shape { p.push(format!("shape={}:{}:{}:{}:{}:{}", s[0], s[1], s[2], s[3], s[4], s[5])); }
            if d.light_level > 0 { p.push(format!("light={}", d.light_level)); }
            if let Some(s) = &d.sound { p.push(format!("sound={s}")); }
            if d.requires_tool { p.push("requires_tool=1".into()); }
            if d.no_collision { p.push("no_collision=1".into()); }
            if d.slipperiness > 0.0 { p.push(format!("slipperiness={}", d.slipperiness)); }
            if d.connects { p.push("connects=1".into()); }
            if !d.connect_groups.is_empty() { p.push(format!("connect_groups={}", d.connect_groups.join(","))); }
            if let Some(i) = &d.inventory_id { p.push(format!("inventory_id={i}")); }
            if d.redstone_source { p.push("redstone=1".into()); }
            p.join("\t")
        }).collect();
        let recipes: Vec<String> = recipe_json.iter().map(|(ns, name, json)| format!("{ns}\t{name}\t{json}")).collect();
        [items.join("\n"), blocks.join("\n"), recipes.join("\n")]
    };
    let new = || crate::content_blob::blob(&h);
    time("host fetch, old: TSV + recipe JSON lines", 2000, &mut || { std::hint::black_box(old()); });
    time("host fetch, new: the blob, written", 2000, &mut || { std::hint::black_box(new()); });
    let out = std::path::PathBuf::from(out);
    let [items, blocks, recipes] = old();
    std::fs::write(out.join("old_items.tsv"), items).unwrap();
    std::fs::write(out.join("old_blocks.tsv"), blocks).unwrap();
    std::fs::write(out.join("old_recipes.txt"), recipes).unwrap();
    std::fs::write(out.join("new_content.ydata"), new()).unwrap();
}

#[test]
fn a_dimensions_terrain_is_generated_here_and_the_host_gets_the_rest() {
    let def = yog_data::text::parse_value(
        r#"{ id: "mymod:flat", dimension_type: { min_y: 0, height: 64 }, biome: "minecraft:plains",
            structures: false, features: false, mob_spawning: true,
            terrain: { base_height: 8, fill: "@block:minecraft:stone", layers: [{ block: "@block:minecraft:dirt", depth: 2 }] } }"#,
    )
    .unwrap();
    let h = registering(|ctx| unsafe { content(ctx, "dimension", "mymod:flat", &def) });
    let h = h.read_locked();
    let kept = &h.dimensions["mymod:flat"];
    assert!(kept.get("terrain").is_none(), "the host doesn't get the terrain");
    assert_eq!(kept.get("generator_type"), Some(&yog_data::Value::from("mymod:flat")));
    let g = &h.terrains["mymod:flat"];
    let mut column = Vec::new();
    g.generate(0, 0, 0, 64, |x, y, z, b| {
        if (x, z) == (0, 0) {
            column.push((y, b.to_string()));
        }
    });
    assert_eq!(column.len(), 9, "{column:?}");
    assert_eq!(column[8], (8, "minecraft:dirt".to_string()));
    assert_eq!(column[0], (0, "minecraft:stone".to_string()));
}
