//! The runtime's registration and dispatch, without a game: `api_*`
//! functions write into a local `RuntimeHandlers`, callbacks are this
//! test's own `extern "C"` functions, the "mod" is the test process.

use std::ffi::c_void;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, RwLock};

use yog_abi::{YogServer, YogStr};
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
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let handlers = Box::new(RwLock::new(RuntimeHandlers::default()));
    *CURRENT_REGISTRATION.lock().unwrap() = Some(module());
    f(&*handlers as *const RwLock<RuntimeHandlers> as *mut c_void);
    *CURRENT_REGISTRATION.lock().unwrap() = None;
    handlers
}

/// `register_content` as a mod calls it: the definition's blob.
unsafe fn content(ctx: *mut c_void, kind: &str, id: &str, def: &yog_data::Value) {
    let blob = yog_data::binary::encode(def);
    unsafe { api_register_content(ctx, YogStr::from_str(kind), YogStr::from_str(id), blob.as_ptr(), blob.len()) }
}

fn json(text: &str) -> yog_data::Value {
    yog_data::json::from_json_text(text).unwrap()
}

unsafe extern "C" fn no_command(
    _: *mut c_void,
    _: *const YogServer,
    _: *const yog_abi::YogCommandEvent,
    _: *mut u8,
    _: u32,
    _: *mut u32,
) {
}

unsafe extern "C" fn count_task(ud: *mut c_void, _: *const YogServer) {
    unsafe { (*(ud as *const AtomicUsize)).fetch_add(1, Ordering::SeqCst) };
}

#[test]
fn commands_keep_every_registration_under_one_name() {
    let h = registering(|ctx| unsafe {
        api_register_command(ctx, YogStr::from_str("hex"), std::ptr::null_mut(), no_command);
        api_register_typed_command(ctx, YogStr::from_str("hex"), YogStr::from_str("int"), std::ptr::null_mut(), no_command);
    });
    let h = h.read().unwrap();
    let entries = &h.commands["hex"];
    assert_eq!(entries.len(), 2, "a second registration doesn't replace the first");
    assert_eq!(entries[0].1, None);
    assert_eq!(entries[1].1.as_deref(), Some("int"));
}

#[test]
fn items_arrive_as_their_definition() {
    let h = registering(|ctx| unsafe {
        let mut def = yog_registry::ItemDef::new("mymod:ruby").max_stack(16).name("").tooltip("Shiny").food(yog_registry::FoodDef::new(4, 0.3));
        def.fire_resistant = true;
        content(ctx, "item", "mymod:ruby", &def.write());
    });
    let h = h.read().unwrap();
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
    let h = h.read().unwrap();
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
        api_register_content(ctx, YogStr::from_str("book"), YogStr::from_str("mymod:junk"), b"junk".as_ptr(), 4);
        content(ctx, "spell", "mymod:fireball", &book);
    });
    let h = h.read().unwrap();
    assert_eq!(h.recipes.len(), 2);
    assert_eq!(h.recipes[1].1, "b");
    assert!(h.book_renderers.lock().unwrap().contains_key("mymod:guide"));
    assert!(h.books.contains_key("mymod:broken"), "its JSON is kept for the host");
    assert!(!h.book_renderers.lock().unwrap().contains_key("mymod:broken"), "but it has no renderer");
    assert!(!h.books.contains_key("mymod:junk"));
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
    let h = h.read().unwrap();
    assert_eq!(h.blocks[0].shape, Some([0.0, 0.0, 0.0, 16.0, 8.0, 16.0]));
    assert_eq!(h.blocks[0].inventory_id.as_deref(), Some("mymod:box"));
    assert_eq!((h.inventories[0].slot_count, h.inventories[0].background_size), (3, Some((176.0, 120.0))));
}

#[test]
fn tsv_values_cannot_break_lines() {
    assert_eq!(tsv_clean("a\tb\nc\rd"), "a b c d");
}

#[test]
fn the_hosts_get_content_as_one_yog_data_blob() {
    let h = registering(|ctx| unsafe {
        let def = yog_registry::ItemDef::new("mymod:apple").max_stack(16).name("Apple").tooltip("").fuel(200).food(yog_registry::FoodDef::new(4, 1.5).can_always_eat());
        content(ctx, "item", "mymod:apple", &def.write());
    });
    let bytes = yog_data::binary::encode(&crate::content_blob::snapshot(&h.read().unwrap()));
    let v = yog_data::binary::decode(&bytes).unwrap();
    let item = &v.get("items").unwrap().as_list().unwrap()[0];
    assert_eq!(item.get("id").and_then(|x| x.as_str()), Some("mymod:apple"));
    assert_eq!(item.get("max_stack").and_then(|x| x.as_i64()), Some(16));
    assert!(item.get("tooltip").is_none(), "an empty tooltip is no tooltip");
    let food = item.get("food").unwrap();
    assert_eq!((food.get("nutrition").and_then(|x| x.as_i64()), food.get("always_edible").and_then(|x| x.as_bool())), (Some(4), Some(true)));
    assert_eq!(v.get("blocks").unwrap().as_list().unwrap().len(), 0);
}

/// For checking the Java reader by hand: `YOG_CONTENT_OUT=file cargo test -p yog-runtime write_a_content_blob`.
#[test]
fn write_a_content_blob() {
    let Some(out) = std::env::var_os("YOG_CONTENT_OUT") else { return };
    let mut h = RuntimeHandlers::default();
    h.items.push(yog_registry::ItemDef::new("m:apple").max_stack(16).name("Apple").food(yog_registry::FoodDef::new(4, 1.5).can_always_eat()));
    let mut b = yog_registry::BlockDef::new("m:pipe");
    b.shape = Some([6.0, 6.0, 6.0, 10.0, 10.0, 10.0]);
    b.connects = true;
    b.connect_groups = vec!["m:pipe".into()];
    b.inventory_id = Some("m:box".into());
    b.sound = Some("metal".into());
    h.blocks.push(b);
    let mut inv = yog_inventory::InventoryDef::new("m:box", 2);
    inv.title = "Box".into();
    h.inventories.push(inv);
    h.recipes.push(("m".into(), "pick".into(), r##"{"type":"shaped","pattern":["II"," S"],"key":{"I":"minecraft:iron_ingot","S":["minecraft:stick","#c:rods"]},"result":"m:pick","count":1}"##.into()));
    h.dimensions.insert("m:void".into(), r#"{"dimension_type":{"has_sky_light":false,"ambient_light":0.5}}"#.into());
    std::fs::write(out, yog_data::binary::encode(&crate::content_blob::snapshot(&h))).unwrap();
}
