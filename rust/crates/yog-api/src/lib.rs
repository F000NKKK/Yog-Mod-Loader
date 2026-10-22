//! Yog API — the single crate mod authors depend on.
//!
//! A facade that re-exports every Yog domain plus the central [`Registry`] hub.
//! Add a new domain crate, re-export it here, and mods pick it up via
//! `yog_api::*`. Items are available both flat (`yog_api::Registry`) and
//! namespaced by domain (`yog_api::world::World`).

pub mod advancements;
pub mod body;
mod bus;
pub mod client;
mod draw;
pub mod entities;
pub mod java;
pub mod loot;
pub mod vanilla;
pub use bus::{EventCx, Flow, IntoFlow, emit};
pub use service::call;
mod interop;
mod lang;
mod registry;
mod scheduler;
mod service;

#[doc(hidden)]
pub use interop::{__yog_export_registry, __yog_serve, YogExportEntry};
pub use interop::{call_export, export_address};
pub use lang::{format_translation, tag_name, tr, tr_args, translation};
pub use registry::{Mod, ModInfo, Registry, UiHandled, close_ui, installed_mods, open_ui};
pub use yog_gfx::{
    GfxContext, core as gfx_core, draw2d as gfx_draw2d, gl as gfx_gl, image as gfx_image,
};

/// Stable C ABI — re-exported so mods don't need a direct `yog-abi` dependency.
pub use yog_abi::{ABI_VERSION, YogApi};
#[doc(hidden)]
pub use yog_abi::{YogBytes, YogReply, YogStatus};

/// Yog Data — what content definitions and book pages' `data`
/// (`Registry::on_book_page`) are; re-exported for mods.
pub use yog_data;
/// `#[derive(Schema)]`: a type read and written as Yog Data (stores, packets, frames).
pub use yog_data::Schema;
/// The names Yog shares — kinds, namespaces, lang keys — for a mod to use as the loader does.
pub use yog_ids;
/// The sync primitives Yog code uses (locks that outlive a panic) — a mod's too.
pub use yog_sync as sync;

pub use java::mod_loaded;

/// Inter-mod communication proc-macros.
pub use yog_interop::{bench, import, yog_export};

/// Re-exported so `#[yog_export]`-generated code can emit `#[::yog_api::ctor::ctor(unsafe)]`
/// without mods needing a direct `ctor` dependency. Constructors run when the
/// mod's `cdylib` is loaded — guaranteed to complete before the runtime calls
/// `yog_mod_register`, so the export registry is fully populated by then.
pub use ctor;

#[doc(hidden)]
pub use std::os::raw::c_void as __c_void;
#[doc(hidden)]
pub use yog_abi::{YogGfxApi as __YogGfxApi, YogStr as __YogStr};

/// Export a [`Mod`] as a dynamically loadable Yog mod.
///
/// Generates the two C-ABI entry points the runtime looks up:
/// - `yog_abi_version() -> u32`  — version check before loading
/// - `yog_mod_register(*const YogApi, *const c_char)` — registration entry point,
///   receives the mod's `id` from its manifest
///
/// Put this once at the crate root of a `cdylib` mod:
///
/// ```ignore
/// yog_api::export_mod!(MyMod);
/// ```
#[macro_export]
macro_rules! export_mod {
    ($mod_ty:ty) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn yog_abi_version() -> u32 {
            $crate::ABI_VERSION
        }

        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn yog_mod_register(
            api: *const $crate::YogApi,
            mod_id_ptr: *const ::std::os::raw::c_char,
        ) {
            // Parse mod_id from the C string passed by the runtime.
            let mod_id: &str = if mod_id_ptr.is_null() {
                "unknown"
            } else {
                // SAFETY: a non-null id is a NUL-terminated string from the runtime.
                match unsafe { ::std::ffi::CStr::from_ptr(mod_id_ptr) }.to_str() {
                    Ok(s) => s,
                    Err(_) => "unknown",
                }
            };
            // Store for interop use (yog_api::interop::current_mod_id()).
            $crate::__set_current_mod_id(mod_id);
            // Every mod is handed the same global `YogApi` table, so any
            // mod's own pointer works for reconstructing a `Registry` inside
            // an exported function later — even when called from a thread
            // that never ran `yog_mod_register` itself.
            $crate::__set_current_api_ptr(api as usize);

            // Catch panics so they never unwind across this `extern "C"` boundary
            // back into the runtime (which would be undefined behaviour).
            let outcome = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                // SAFETY: the runtime passes a valid YogApi pointer, verified via
                // yog_abi_version() and abi_version/size checks before this call.
                let mut registry = unsafe { $crate::Registry::from_raw(api) };
                // The world's folder known before the mod's own handlers run.
                $crate::storage::__know_world(&mut registry);
                <$mod_ty as $crate::Mod>::register(&mut registry);

                // Blocks and items of `content/**/*.yblock|*.yitem`
                // (`yog_api::content!()`, self-registered by `ctor`).
                let content: ::std::vec::Vec<fn(&mut $crate::Registry)> =
                    $crate::__yog_content_registrars().lock().clone();
                for register in content {
                    register(&mut registry);
                }

                // Every #[yog_export] function (registered by its `ctor`
                // when the library loaded) provides `mod_id:name`.
                for entry in $crate::__yog_export_registry().lock().iter() {
                    registry.provide_export(mod_id, entry);
                }

                // What it keeps, saved with the world (after the mod's own handlers).
                $crate::storage::__save_with_world(&mut registry);
            }));
            if outcome.is_err() {
                $crate::error!(
                    "mod {} panicked during register",
                    ::core::stringify!($mod_ty)
                );
            }
        }

        /// Yog-IDLE's screen designer: the version of the protocol below.
        #[unsafe(no_mangle)]
        pub extern "C" fn yog_ui_designer_abi() -> u32 {
            $crate::ui::component::designer::ABI
        }

        /// Yog-IDLE's screen designer: lists this mod's custom components
        /// and draws them (see `yog_ui::component::designer`). Never called
        /// in game — only by the designer host, which loads the mod's
        /// library built for the developer's machine.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn yog_ui_designer(
            request: $crate::YogBytes,
            gfx: *const $crate::__YogGfxApi,
            user_data: *mut $crate::__c_void,
            write: $crate::ui::component::designer::WriteFn,
        ) -> bool {
            // SAFETY: forwarded under the same contract.
            unsafe { $crate::ui::component::designer::call(request, gfx, user_data, write) }
        }
    };
}

/// Registers `.yui` components (types implementing
/// [`ui::component::Component`]) when the mod's library loads, so screens
/// can use `<Name …/>` and Yog-IDLE's designer host can draw them. The code
/// `yog build` generates from a `<Component>` `.yui` calls this — mods
/// don't write it themselves.
#[macro_export]
macro_rules! ui_component {
    ($($ty:ty),+ $(,)?) => {
        const _: () = {
            #[$crate::ctor::ctor(unsafe, crate_path = $crate::ctor)]
            fn __yog_register_ui_components() {
                $( $crate::ui::component::register::<$ty>(); )+
            }
        };
    };
}

/// Compiles in the mod's component `.yui` files that have no code-behind
/// `.rs` (those with one are pulled in by its `ui_include!`) — call it once,
/// in `src/lib.rs`:
///
/// ```ignore
/// yog_api::ui_components!();
/// ```
///
/// `yog build`, `yog check` and `yog ui gen` write the list to
/// `.yog-build/generated/components.g.rs` (a module per component).
#[macro_export]
macro_rules! ui_components {
    () => {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/generated/components.g.rs"
        ));
    };
}

/// Internal: set by `export_mod!` before calling `Mod::register`.
/// Used by `yog_api::interop::current_mod_id()` so `Interop::export` knows
/// which mod is calling.
#[doc(hidden)]
pub fn __set_current_mod_id(id: &str) {
    CURRENT_MOD_ID.with(|cell| cell.replace(Some(id.to_string())));
    let _ = MOD_ID.set(id.to_owned());
}

/// This mod's id (its manifest's), once the runtime has loaded it.
pub fn mod_id() -> &'static str {
    MOD_ID.get().map_or("", String::as_str)
}

/// This mod's id: each mod's library has its own copy of this crate.
static MOD_ID: yog_sync::OnceLock<String> = yog_sync::OnceLock::new();

/// Internal: the current mod's id, set during `yog_mod_register`.
#[doc(hidden)]
pub fn __current_mod_id() -> Option<String> {
    CURRENT_MOD_ID.with(|cell| cell.borrow().clone())
}

/// Internal: the global `YogApi` pointer, set by every mod's `yog_mod_register`
/// (it's the same singleton table for the whole process). Process-wide (not
/// thread-local) because `#[yog_export]`-generated wrappers read it whenever
/// an exported function is *called*, which may happen on any thread, long
/// after the loading thread's `yog_mod_register` returned.
#[doc(hidden)]
pub fn __set_current_api_ptr(ptr: usize) {
    API_PTR.store(ptr, yog_sync::atomic::Ordering::Release);
}

/// The `register` functions of `yog_api::content!()` — the content of
/// `content/**` the build generates — run by `export_mod!` after
/// `Mod::register`.
#[doc(hidden)]
pub fn __yog_content_registrars() -> &'static yog_sync::Mutex<Vec<fn(&mut Registry)>> {
    static REG: yog_sync::Mutex<Vec<fn(&mut Registry)>> = yog_sync::Mutex::new(Vec::new());
    &REG
}

#[doc(hidden)]
pub fn __current_api_ptr() -> usize {
    API_PTR.load(yog_sync::atomic::Ordering::Acquire)
}

static API_PTR: yog_sync::atomic::AtomicUsize = yog_sync::atomic::AtomicUsize::new(0);

std::thread_local! {
    static CURRENT_MOD_ID: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

pub use yog_book::BookRenderer;
pub use yog_command::CommandContext;
pub use yog_config::Config;
pub use yog_core::{
    BlockHit, BlockInfo, BlockPos, EntityInfo, Face, PlacedBlock, PlayerInfo, Point, Rotation,
    ScreenSlot, SlotStack, Stack, entity_flags, split_state,
};
pub use yog_dimensions::{GeneratedChunk, YogDimensionDef, YogDimensionTypeDef};
pub use yog_event::{Entity, Event, EventPhase, Message, Player, Service, World};
/// A packet the server or the client received (`on_packet` / `on_client_packet`).
pub use yog_event::{client::net::Packet as ClientPacketEvent, server::net::Packet as PacketEvent};
pub use yog_formats::book::{Book, BookCategory, BookEntry, BookPage};
pub use yog_inventory::{InventoryDef, SlotLayout};
pub use yog_network::Packet;
pub use yog_registry::{
    ArmorDefense, ArmorMaterial, ArmorPiece, ArmorSlot, BlockDef, BlockProperty, Cooking,
    EntityDef, EntityKind, Facing, FoodDef, ItemDef, Placement, Recipe, RecipeKind, SoundGroup,
    StartupGrant, ToolItem, ToolMaterial,
};
pub use yog_ui::{
    Align, Dock, FlexDir, FocusStyle, LayoutNode, Rect, Stretch, UiRoot, Units, widget,
};

/// Logging macros (`yog_api::info!`, `warn!`, `error!`).
pub use yog_logging::{error, info, warn};

/// Core types and handles.
pub mod core {
    pub use yog_core::*;
}

/// Events by plane — `event::player::session::Join`, `event::block::Break`, … —
/// subscribed with `Registry::on` / `before` / `after`.
pub mod event {
    pub use yog_event::*;
}

/// The bus's domains: each one's actions as functions
/// (`world::get_block(dim, pos)`, `entity::position(uuid)`,
/// `player::give_item(name, item, count)`) beside its events.
pub use yog_event::{
    block, bossbar, chat, entity, game, item, net, particle, player, scoreboard, sound, tag, world,
};

/// Content registration (custom items / blocks / food).
pub mod content {
    pub use yog_registry::*;
}

/// Networking (raw-byte packets over channels).
pub mod network {
    pub use yog_network::*;
}

/// Commands.
pub mod command {
    pub use yog_command::*;
}

/// Custom dimensions — dimension-type definitions and runtime chunk generation.
pub mod dimension {
    pub use yog_dimensions::*;
}

/// Where the mod keeps its data (typed stores by scope, values kept with the world).
pub mod storage;

/// Mod configuration (typed key/value files).
pub mod config {
    pub use yog_config::*;
}

/// Guide books: the `.ybook` format and its renderer.
pub mod book {
    pub use yog_book::*;
    pub use yog_formats::book::*;
}

/// UI framework — flexbox layout + widgets on top of yog-gfx.
pub mod ui {
    pub use yog_ui::*;
}

/// Pulls a `.yui` screen's generated code into the current module — the
/// code-behind's half of `src/<name>.yui`:
///
/// ```ignore
/// // src/mod_list.rs, next to src/mod_list.yui
/// yog_api::ui_include!("mod_list");
/// ```
///
/// `<name>` is the `.yui` file's path under `src/` without the extension
/// (`"screens/options"` for `src/screens/options.yui`). `yog build`,
/// `yog check` and `yog ui gen` write the code to
/// `.yog-build/generated/<name>.g.rs` (the build crate's own directory,
/// `CARGO_MANIFEST_DIR`), so it's never committed and never stale.
#[macro_export]
macro_rules! ui_include {
    ($name:literal) => {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/generated/",
            $name,
            ".g.rs"
        ));
    };
}

/// Pulls the mod's `.yres` resources in as module `res` — one type per
/// resource set, with an accessor per entry:
///
/// ```ignore
/// // src/lib.rs
/// yog_api::resources!();
///
/// use crate::res::{Icons, Strings};
/// let title = Strings::screen_title();        // the text in the player's language
/// let count = Strings::mods_count(58);        // "{count} mods" → "58 mods"
/// let logo = Icons::logo();                   // an image source, "mymod:images/logo"
/// ```
///
/// `yog build`, `yog check` and `yog res check` generate one
/// `.yog-build/generated/resources/<Set>.g.rs` per `resources/**/<Set>.yres`
/// plus `.yog-build/generated/resources.g.rs` including them all (see
/// Yog-IDLE's `docs/yres.md`); never committed, never stale.
#[macro_export]
macro_rules! resources {
    () => {
        pub mod res {
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/generated/resources.g.rs"
            ));
        }
    };
}

/// Pulls the mod's blocks and items — `content/**/*.yblock` and `*.yitem`,
/// made in Yog-IDLE's model editor — in as module `content`, and registers
/// them (every file without `"register": "manual"`) right after
/// `Mod::register`:
///
/// ```ignore
/// // src/lib.rs — that's all it takes
/// yog_api::content!();
///
/// // Ids and definitions, for code that needs them:
/// use crate::content::{Blocks, Items};
/// let id: &str = Blocks::RUBY_ORE;                 // "mymod:ruby_ore"
/// registry.register_item(Items::ruby().max_stack(16)); // a "manual" one, tweaked
/// ```
///
/// `yog build`, `yog check` and `yog content check` generate
/// `.yog-build/generated/yog-content.g.rs` — never committed, never stale.
/// The models, blockstates, loot tables, tags and lang entries go into the
/// `.yog` as assets. See Yog-IDLE's `docs/blocks-items.md`.
#[macro_export]
macro_rules! content {
    () => {
        pub mod content {
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/generated/yog-content.g.rs"
            ));
        }
    };
}

/// Inventory framework — real Container/Menu screens (BlockEntity-backed),
/// as opposed to `ui`'s HUD-drawn overlays. See `yog-inventory`'s DESIGN.md.
pub mod inventory {
    pub use yog_inventory::*;
}
