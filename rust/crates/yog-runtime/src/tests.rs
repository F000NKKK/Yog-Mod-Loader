//! The runtime's registration and dispatch, without a game: `api_*`
//! functions write into a local `RuntimeHandlers`, callbacks are this
//! test's own `extern "C"` functions, the "mod" is the test process.

use std::ffi::c_void;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, RwLock};

use yog_abi::{YogItemDef, YogServer, YogStartupGrantDef, YogStr};
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
fn items_are_copied_out_of_the_c_struct() {
    let h = registering(|ctx| unsafe {
        let def = YogItemDef {
            id: YogStr::from_str("mymod:ruby"),
            max_stack: 16,
            name: YogStr::from_str(""),
            tooltip: YogStr::from_str("Shiny"),
            max_damage: 0,
            fire_resistant: true,
            fuel_ticks: 0,
            food_nutrition: 4,
            food_saturation: 0.3,
            food_always_eat: false,
        };
        api_register_item(ctx, &def);
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
fn startup_grants_split_their_items() {
    let h = registering(|ctx| unsafe {
        let g = YogStartupGrantDef {
            id: YogStr::from_str("mymod:starter"),
            items: YogStr::from_str("mymod:staff|minecraft:book"),
            book: YogStr::from_str(""),
            command: YogStr::from_str("say hi"),
        };
        api_register_startup_grant(ctx, &g);
    });
    let h = h.read().unwrap();
    let g = &h.startup_grants[0];
    assert_eq!(g.items, ["mymod:staff", "minecraft:book"]);
    assert_eq!(g.book, None);
    assert_eq!(g.command.as_deref(), Some("say hi"));
}

#[test]
fn books_get_the_recipes_registered_before_and_after_them() {
    let recipe = r#"{"type":"shapeless","ingredients":["minecraft:stick"],"result":"minecraft:torch"}"#;
    let book = r#"{"id":"mymod:guide","name":"Guide","categories":[],"entries":[]}"#;
    let h = registering(|ctx| unsafe {
        api_register_recipe_json(ctx, YogStr::from_str("mymod"), YogStr::from_str("a"), YogStr::from_str(recipe));
        api_register_book(ctx, YogStr::from_str("mymod:guide"), YogStr::from_str(book));
        api_register_recipe_json(ctx, YogStr::from_str("mymod"), YogStr::from_str("b"), YogStr::from_str(recipe));
        api_register_book(ctx, YogStr::from_str("mymod:broken"), YogStr::from_str("{ not json"));
    });
    let h = h.read().unwrap();
    assert_eq!(h.recipes.len(), 2);
    assert!(h.book_renderers.lock().unwrap().contains_key("mymod:guide"));
    assert!(h.books.contains_key("mymod:broken"), "its JSON is kept for the host");
    assert!(!h.book_renderers.lock().unwrap().contains_key("mymod:broken"), "but it has no renderer");
}

#[test]
fn scheduled_tasks_run_when_due() {
    let fired = Box::new(AtomicUsize::new(0));
    let ud = &*fired as *const AtomicUsize as *mut c_void;
    let h = registering(|ctx| unsafe {
        api_schedule_once(ctx, 0, ud, count_task);
        api_schedule_once(ctx, 3, ud, count_task);
        api_schedule_repeating(ctx, 2, ud, count_task);
        api_schedule_repeating(ctx, 0, ud, count_task);
    });
    let h = h.read().unwrap();
    let mut per_tick = Vec::new();
    for _ in 0..6 {
        let due = h.scheduler.lock().unwrap().due();
        per_tick.push(due.len());
    }
    // tick 1: once(0), every tick; 2: every 2, every tick; 3: once(3), every tick; …
    assert_eq!(per_tick, [2, 2, 2, 2, 1, 2]);
    assert!(h.scheduler.lock().unwrap().once_tasks.is_empty(), "once tasks are gone once run");
}

type Ping = unsafe extern "C" fn(*mut c_void) -> bool;

unsafe extern "C" fn ping_yes(ud: *mut c_void) -> bool {
    unsafe { (*(ud as *const AtomicUsize)).fetch_add(1, Ordering::SeqCst) };
    true
}

unsafe extern "C" fn ping_no(ud: *mut c_void) -> bool {
    unsafe { (*(ud as *const AtomicUsize)).fetch_add(1, Ordering::SeqCst) };
    false
}

#[test]
fn pre_phase_handlers_stop_at_the_first_no_and_skip_retired_mods() {
    let calls = Box::new(AtomicUsize::new(0));
    let ud = &*calls as *const AtomicUsize as *mut c_void;
    let live = module();
    let retired = module();
    retired.retire(|| {});
    let entries: Vec<Entry<Ping>> = vec![
        (retired.clone(), ud, ping_no as Ping),
        (live.clone(), ud, ping_yes as Ping),
        (live.clone(), ud, ping_no as Ping),
        (live.clone(), ud, ping_yes as Ping),
    ];
    assert!(!allowed(&entries, |ud, f| unsafe { f(ud) }), "a handler said no");
    assert_eq!(calls.load(Ordering::SeqCst), 2, "the retired one is skipped, the last never runs");
    assert!(allowed(&entries, |_, _| -> bool { panic!("a mod panicked") }), "a panic doesn't cancel the action");
}

#[test]
fn tsv_values_cannot_break_lines() {
    assert_eq!(tsv_clean("a\tb\nc\rd"), "a b c d");
}

#[test]
fn the_hosts_get_content_as_one_yog_data_blob() {
    let h = registering(|ctx| unsafe {
        let def = YogItemDef {
            id: YogStr::from_str("mymod:apple"),
            max_stack: 16,
            name: YogStr::from_str("Apple"),
            tooltip: YogStr::from_str(""),
            max_damage: 0,
            fire_resistant: false,
            fuel_ticks: 200,
            food_nutrition: 4,
            food_saturation: 1.5,
            food_always_eat: true,
        };
        api_register_item(ctx, &def);
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
