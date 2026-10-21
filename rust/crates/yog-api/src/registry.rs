//! Registration hub and mod entry-point trait.
//!
//! [`Registry`] wraps the [`YogApi`] C table passed to `yog_mod_register` and
//! provides an ergonomic Rust API over it.  All closures registered here are
//! boxed on the heap and their raw pointers handed to the runtime; the runtime
//! then drives them via C function-pointer calls.  Closures are intentionally
//! leaked (never freed) — they live for the entire process lifetime, which is
//! correct for a game server.

use yog_abi::YogApi;
use yog_command::CommandContext;
use yog_dimensions::{GeneratedChunk, YogDimensionDef};
use yog_event::client::net::Packet as ClientPacketEvent;
use yog_event::server::net::Packet as PacketEvent;
use yog_formats::book::Book;
use yog_gfx::GfxContext;
use yog_network::Packet;
use yog_registry::{
    ArmorMaterial, BlockDef, ItemDef, Recipe, SoundGroup, StartupGrant, ToolMaterial,
};

// ── Trampoline helpers ────────────────────────────────────────────────────────
//
// A panic must never unwind out of a trampoline: the runtime calls them
// through `extern "C"`, so it would abort the whole game. `guard` stops it
// at the boundary — the panic message is already in the log — and the
// event goes on as if the mod hadn't handled it.

pub(crate) fn guard<R>(fallback: R, call: impl FnOnce() -> R) -> R {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(call)).unwrap_or_else(|_| {
        crate::error!("a mod's callback panicked (see above); carrying on without it");
        fallback
    })
}
//
// Each trampoline is a `unsafe extern "C" fn` that:
//   1. Casts `ud` back to the original boxed closure.
//   2. Converts ABI C structs to Rust event types.
//   4. Calls the closure.
//
// Closures are Box::into_raw'd in Registry methods — they are INTENTIONALLY
// leaked and live for the process lifetime.

// ── ABI minor 10 — client-side trampolines ────────────────────────────────────

// ── Registry ─────────────────────────────────────────────────────────────────

/// Wraps the [`YogApi`] table and provides an ergonomic registration API.
///
/// Obtained inside `yog_mod_register` via `export_mod!`.  Closures registered
/// here are boxed and leaked — they live as long as the process (which is the
/// correct lifetime for a server mod).
/// Pointer to the runtime's `YogApi` table (a process-lifetime static in the
/// runtime), captured when the mod registers. Lets free functions like
/// [`installed_mods`] work outside of `register()` — e.g. in UI handlers.
static GLOBAL_API: yog_sync::atomic::AtomicPtr<YogApi> =
    yog_sync::atomic::AtomicPtr::new(std::ptr::null_mut());

/// The runtime's `YogApi` table, null before this mod registered.
pub(crate) fn global_api() -> *mut YogApi {
    GLOBAL_API.load(yog_sync::atomic::Ordering::Acquire)
}

pub use yog_event::game::ModInfo;

/// All installed mods known to the loader: .yog mods plus, where the host
/// exposes them, platform (Java) mods. Callable at any time after this mod's
/// `register()` ran — including client-side UI handlers. Note that during
/// `register()` mods that load after this one are not in the list yet; query
/// lazily (e.g. on first render) for a complete view.
pub fn installed_mods() -> Vec<ModInfo> {
    crate::call(&yog_event::game::Mods {}).unwrap_or_default()
}

/// Open the Yog UI registered as `ui_id` (client side; no-op on dedicated
/// servers). `modal` blocks game input, `pause` pauses singleplayer. Callable
/// from any handler after registration — e.g. an `on_client_packet` handler.
pub fn open_ui(ui_id: &str, modal: bool, pause: bool) {
    crate::call(&yog_event::ui::Open {
        ui_id: ui_id.to_owned(),
        modal,
        pause,
    });
}

/// Close the Yog UI screen showing `ui_id` — e.g. from that screen's own
/// "Done" button. Does nothing when another screen is open. Deferred onto the
/// render thread, so the handler that calls it finishes first. Client side;
/// a no-op on dedicated servers.
pub fn close_ui(ui_id: &str) {
    crate::call(&yog_event::ui::Close {
        ui_id: ui_id.to_owned(),
    });
}

/// What a [`Registry::register_ui`] handler returns: `()` (never marks
/// events as used) or `bool` (`true` = the handler used this `key:`/`char:`
/// event — the host then doesn't, e.g. Esc doesn't close the screen).
pub trait UiHandled {
    fn handled(self) -> bool;
}

impl UiHandled for () {
    fn handled(self) -> bool {
        false
    }
}

impl UiHandled for bool {
    fn handled(self) -> bool {
        self
    }
}

pub struct Registry {
    api: *const YogApi,
}

// SAFETY: `api` is a static provided by the runtime, valid for process lifetime.
unsafe impl Send for Registry {}
unsafe impl Sync for Registry {}

/// A record of this mod's (`yog_api::info!`…): the `log/record` event, which
/// the runtime writes to the game's log.
fn log_to_host(level: yog_logging::Level, message: &str) {
    crate::emit(&yog_event::log::Record {
        level: level as u32,
        message: message.to_owned(),
    });
}

impl Registry {
    /// Build from the pointer passed by the runtime. Only called by `export_mod!`.
    ///
    /// # Safety
    ///
    /// `api` must point to the runtime's `YogApi` table, valid for the rest of
    /// the process.
    pub unsafe fn from_raw(api: *const YogApi) -> Self {
        GLOBAL_API.store(api as *mut YogApi, yog_sync::atomic::Ordering::Release);
        // The mod's records go to the game's log, through the runtime.
        yog_logging::set_sink(log_to_host);
        // The actions' functions and handles (`player::give_item`, `Player`) reach the bus through it.
        yog_event::set_caller(crate::service::call_at);
        // Atoms number the same in every mod: the runtime's table.
        if !yog_data::atom::install(crate::service::ATOMS) {
            yog_logging::warn!(
                "atoms were made before the mod was registered: their numbers are this mod's own"
            );
        }
        Self { api }
    }

    /// The runtime's table.
    pub(crate) fn api(&self) -> &YogApi {
        unsafe { &*self.api }
    }

    /// Hands the runtime content of `kind` named `id`, its definition a Yog
    /// Data blob (`content/register`, one service for every kind).
    fn register_blob(&mut self, kind: &str, id: &str, data: &[u8]) {
        let register = yog_event::content::Register {
            kind: kind.to_owned(),
            id: id.to_owned(),
            data: data.to_vec(),
        };
        if self.call(&register) != Some(true) {
            crate::error!("{kind} {id}: the runtime didn't take it");
        }
    }
    // ── commands ─────────────────────────────────────────────────────────────

    pub fn on_command<F>(&mut self, name: impl AsRef<str>, handler: F)
    where
        F: Fn(&CommandContext) -> Option<String> + Send + Sync + 'static,
    {
        self.command(name.as_ref(), None, handler);
    }

    /// Register a command with Brigadier-typed arguments.
    ///
    /// `schema` is a space-separated list of argument types:
    /// `int`, `float`, `word`, `string` (greedy, must be last), `player`, `blockpos`.
    ///
    /// In the handler use `ctx.arg_int(0)`, `ctx.arg_blockpos(1)`, etc.
    pub fn on_typed_command<F>(
        &mut self,
        name: impl AsRef<str>,
        schema: impl AsRef<str>,
        handler: F,
    ) where
        F: Fn(&CommandContext) -> Option<String> + Send + Sync + 'static,
    {
        self.command(name.as_ref(), Some(schema.as_ref()), handler);
    }

    /// Declares command `name` with `schema` to the host, and answers its
    /// runs that fit it — each way it takes arguments ranked as
    /// [`yog_command::Fit::ORDER`] says, the first answer winning.
    fn command<F>(&mut self, name: &str, schema: Option<&str>, handler: F)
    where
        F: Fn(&CommandContext) -> Option<String> + Send + Sync + 'static,
    {
        let declare = yog_event::command::CommandSpec {
            name: name.to_owned(),
            schema: schema.map(str::to_owned),
        };
        if self.call(&declare) != Some(true) {
            crate::error!("command {name}: the host wasn't told of it");
        }
        let handler = yog_sync::Arc::new(handler);
        for &fit in yog_command::Fit::ways(schema) {
            let (handler, schema) = (yog_sync::Arc::clone(&handler), declare.schema.clone());
            self.change_key_at::<yog_event::server::command::Run>(
                name,
                fit.rank() as i32,
                move |run, _| {
                    if run.reply.is_some() {
                        return;
                    }
                    let Some(args) = fit.take(schema.as_deref(), &run.args) else {
                        return;
                    };
                    let ctx = CommandContext {
                        name: run.name.clone(),
                        args,
                        source: run.source.clone(),
                        uuid: run.uuid.clone(),
                        dimension: run.dimension.clone(),
                    };
                    run.reply = handler(&ctx);
                },
            );
        }
    }

    // ── networking ───────────────────────────────────────────────────────────

    /// Runs `handler` on every packet a client sends on `channel` (server side).
    pub fn on_packet<F>(&mut self, channel: impl AsRef<str>, handler: F)
    where
        F: Fn(&PacketEvent) + Send + Sync + 'static,
    {
        self.on_key::<PacketEvent, _>(channel.as_ref(), move |p, _| handler(p));
    }

    /// Runs `handler` on every packet the server sends on `channel` (client side).
    pub fn on_client_packet<F>(&mut self, channel: impl AsRef<str>, handler: F)
    where
        F: Fn(&ClientPacketEvent) + Send + Sync + 'static,
    {
        self.on_key::<ClientPacketEvent, _>(channel.as_ref(), move |p, _| handler(p));
    }

    /// Register a typed-packet handler.
    ///
    /// The payload is decoded from raw bytes using `P`'s [`Packet`] impl.
    /// Malformed payloads are silently dropped.
    pub fn on_typed_packet<P, F>(&mut self, channel: impl AsRef<str>, handler: F)
    where
        P: Packet + Send + Sync + 'static,
        F: Fn(&P) + Send + Sync + 'static,
    {
        self.on_packet(channel, move |ev| {
            if let Some(pkt) = P::decode(&ev.payload) {
                handler(&pkt);
            }
        });
    }

    // ── content ──────────────────────────────────────────────────────────────

    /// Hands the runtime content of `kind` named `id`: its definition.
    fn content(&mut self, kind: &str, id: &str, def: &yog_data::Value) {
        self.register_blob(kind, id, &yog_data::binary::encode(def))
    }

    /// JSON text as Yog Data (recipes and books are written as JSON by their crates).
    fn json(text: &str) -> yog_data::Value {
        yog_data::json::from_json_text(text).unwrap_or_default()
    }

    /// Register a recipe (Yog's format — see [`Recipe`]; the host writes
    /// its Minecraft version's JSON).
    pub fn add_recipe(&mut self, recipe: Recipe) {
        let (ns, name) = recipe.ns_name();
        let id = format!("{ns}:{name}");
        self.content(yog_ids::kind::RECIPE, &id, &recipe.to_data());
    }

    /// Register a recipe given as Yog's recipe JSON — how `yog build`
    /// registers the mod's own kinds (`{"type": "mymod:infusion", "slots":
    /// …}`), which stay the mod's data: for its machines and its books.
    pub fn add_recipe_json(&mut self, id: &str, json: &str) {
        let id = yog_ids::qualify(id);
        self.content(yog_ids::kind::RECIPE, &id, &Self::json(json));
    }

    pub fn register_item(&mut self, def: ItemDef) {
        use yog_data::Schema;
        self.content(yog_ids::kind::ITEM, &def.id.clone(), &def.write());
    }

    pub fn register_block(&mut self, def: BlockDef) {
        use yog_data::Schema;
        self.content(yog_ids::kind::BLOCK, &def.id.clone(), &def.write());
    }

    /// Registers an entity type (what a `.yentity` makes). With a `model`,
    /// the loader draws it — its body model with the clip of its state
    /// (`clips`: `idle`, `walk`, or entity data `yog:state`).
    pub fn register_entity(&mut self, def: yog_registry::EntityDef) {
        use yog_data::Schema;
        if crate::entities::drawn(&def) {
            self.on_body(crate::entities::draw);
        }
        self.content(yog_ids::kind::ENTITY, &def.id.clone(), &def.write());
    }

    /// A compiled `.yentity` (the generated code calls this).
    pub fn register_entity_data(&mut self, id: &str, data: &[u8]) {
        match yog_data::read_bytes::<yog_registry::EntityDef>(data) {
            Ok(def) => self.register_entity(def),
            Err(e) => crate::error!("entity {id}: {e:?}"),
        }
    }

    /// Add placed feature `feature` (the mod's datapack's, like an ore's
    /// `mymod:mythril_ore`) to the biomes of tag `biomes` at generation
    /// `step` (`underground_ores`). An ore `.ydef` does this for you.
    pub fn add_biome_feature(&mut self, feature: &str, biomes: &str, step: &str) {
        use yog_data::Schema;
        let f = yog_registry::BiomeFeature {
            feature: feature.into(),
            biomes: biomes.into(),
            step: step.into(),
        };
        self.content(yog_ids::kind::BIOME_FEATURE, feature, &f.write());
    }

    /// Register a tool material — what tool items naming it
    /// (`ItemDef::tool`) are made of. The game's are built in.
    pub fn register_tool_material(&mut self, material: ToolMaterial) {
        use yog_data::Schema;
        self.content(
            yog_ids::kind::TOOL_MATERIAL,
            &material.id.clone(),
            &material.write(),
        );
    }

    /// Register an armor material — what armor items naming it
    /// (`ItemDef::armor`) are made of. The game's are built in.
    pub fn register_armor_material(&mut self, material: ArmorMaterial) {
        use yog_data::Schema;
        self.content(
            yog_ids::kind::ARMOR_MATERIAL,
            &material.id.clone(),
            &material.write(),
        );
    }

    /// Register a sound group — what blocks naming it (`BlockDef::sound`)
    /// sound like. The game's groups are built in.
    pub fn register_sound_group(&mut self, group: SoundGroup) {
        use yog_data::Schema;
        self.content(
            yog_ids::kind::SOUND_GROUP,
            &group.id.clone(),
            &group.write(),
        );
    }

    /// Register a real Container/Menu inventory screen — see
    /// `yog_inventory::InventoryDef`. Attach it to a block via
    /// `BlockDef::inventory(id)`.
    pub fn register_inventory(&mut self, def: yog_inventory::InventoryDef) {
        use yog_data::Schema;
        self.content(yog_ids::kind::INVENTORY, &def.id.clone(), &def.write());
    }

    // ── startup grants ───────────────────────────────────────────────────────

    /// Register a startup grant: items/books to give once when a player first joins.
    pub fn register_startup_grant(&mut self, grant: StartupGrant) {
        use yog_data::Schema;
        self.content(
            yog_ids::kind::STARTUP_GRANT,
            &grant.id.clone(),
            &grant.write(),
        );
    }

    /// Register a book.
    pub fn register_book(&mut self, book: &Book) {
        use yog_data::Schema;
        self.content(yog_ids::kind::BOOK, &book.id, &book.write());
    }

    /// Register a book given as Yog Data, binary — what `yog build` makes
    /// of a `content/**/*.ybook`; the bytes go to the runtime as they are.
    pub fn register_book_data(&mut self, id: &str, data: &[u8]) {
        self.register_blob(yog_ids::kind::BOOK, id, data)
    }

    /// Register a font given as a compiled `.yfont` Yog Data blob.
    pub fn register_font_data(&mut self, id: &str, data: &[u8]) {
        self.register_blob(yog_ids::kind::FONT, id, data)
    }

    /// Register (or patch) a dimension's *type* (sky, lighting, physics, ...).
    ///
    /// `def.id` need not be a new custom id — pass a vanilla id
    /// (`"minecraft:overworld"`, `"minecraft:the_nether"`,
    /// `"minecraft:the_end"`) to patch that dimension's type/generator
    /// instead of creating a new one (e.g. to replace the overworld's
    /// terrain with your own generator type).
    ///
    /// Must be called from `register()` — the same mod-init window as
    /// blocks/items — so the host can declare/patch the dimension before
    /// Minecraft's registries freeze. See the `yog-dimensions` crate docs;
    /// set `def.generator_type` (via [`YogDimensionDef::generator_type`]) to
    /// pick a generator type registered with
    /// [`Registry::register_chunk_generator`].
    pub fn register_dimension(&mut self, def: &YogDimensionDef) {
        let json = match def.to_json() {
            Ok(json) => json,
            Err(e) => {
                yog_logging::error!("dimension {} not registered: {e}", def.id);
                return;
            }
        };
        let value = Self::json(&json);
        self.content(yog_ids::kind::DIMENSION, &def.id.clone(), &value);
    }

    /// Register a dimension given as Yog Data, binary — what `yog build`
    /// makes of a `.ydef` of kind `dimension` (its `terrain` generated by
    /// the runtime itself); the bytes go to the runtime as they are.
    pub fn register_dimension_data(&mut self, id: &str, data: &[u8]) {
        self.register_blob(yog_ids::kind::DIMENSION, id, data)
    }

    /// Register a named, reusable chunk-generator type under
    /// `generator_type_id` (e.g. `"mymod:custom_terrain"`) — analogous to
    /// how Biomes O'Plenty registers selectable world-type presets. Any
    /// dimension (custom or vanilla) can opt into this generator via
    /// [`YogDimensionDef::generator_type`]; the platform host may also list
    /// registered generator type ids as options on the world-creation screen.
    ///
    /// `generator` is called once per chunk column that needs generating in
    /// any dimension using this type — write whatever terrain logic you want
    /// in the [`GeneratedChunk`] (combine as many noise layers as you like;
    /// this crate doesn't prescribe a generation style). Its terrain, if
    /// declared, is in the column already; another mod's handler for the
    /// same type (ores, say) sees what this one placed.
    pub fn register_chunk_generator<F>(&mut self, generator_type_id: &str, generator: F)
    where
        F: Fn(&mut GeneratedChunk) + Send + Sync + 'static,
    {
        self.change_key_at::<GeneratedChunk>(
            generator_type_id,
            yog_abi::priority::DEFAULT,
            move |chunk, _| generator(chunk),
        );
    }

    /// Register a menu entry — the host renders a button on vanilla screens
    /// (TitleScreen on Fabric, ModListScreen on Forge/NeoForge) that opens `ui_id`.
    /// `label` is the human-readable button text (e.g. "Yog Mods").
    /// `ui_id` is the UI to open when clicked (e.g. "example:menu").
    /// See [`installed_mods`]. During `register()` the list only contains
    /// mods loaded before this one.
    pub fn installed_mods(&self) -> Vec<ModInfo> {
        installed_mods()
    }

    /// The raw `YogApi` table.
    pub fn raw_api(&self) -> *const yog_abi::YogApi {
        self.api
    }

    pub fn register_menu_entry(&mut self, label: &str, ui_id: &str) {
        if self.call(&yog_event::menu::MenuEntry {
            label: label.to_owned(),
            ui_id: ui_id.to_owned(),
        }) != Some(true)
        {
            crate::error!("menu entry {ui_id}: the host wasn't told of it");
        }
    }

    /// Register a UI tree with an event callback.
    /// `ui_id` is the unique identifier (e.g. "mymod:menu").
    /// `handler` receives `(ui_id, event)` for the screen's input: `click:X:Y`,
    /// `drag:X:Y`, `release:X:Y`, `scroll:DY`, `key:KEY:MODS` (GLFW key code
    /// and modifier bits, on press and repeat) and `char:CODEPOINT` (a typed
    /// character). It returns `()` or a `bool`: `true` marks a `key:`/`char:`
    /// event as used, so the host skips its default handling of the key
    /// (Esc closing the screen, ...) — see [`UiHandled`].
    pub fn register_ui<F, R>(&mut self, ui_id: &str, handler: F)
    where
        F: Fn(&str, &str) -> R + Send + Sync + 'static,
        R: UiHandled,
    {
        self.before_key::<yog_event::client::ui::Input, _>(ui_id, move |input, _| {
            !handler(&input.ui_id, &input.event).handled()
        });
    }

    // ── client-side hooks (ABI minor 10) ─────────────────────────────────────

    /// Register a render callback for a specific UI screen (`ui_id`).
    ///
    /// Called from `YogUIScreen.render()` — i.e. AFTER `renderBackground()` darkens
    /// the screen — so your UI draws on top of the dimmed world view.
    /// Use this instead of `on_hud_render` for book/inventory screens.
    ///
    /// Clicks are forwarded as `"click:X:Y"` via the `register_ui` handler so you
    /// can do hit-testing on your own stored layout.
    pub fn on_ui_render<F>(&mut self, ui_id: &str, handler: F)
    where
        F: Fn(&GfxContext) + Send + Sync + 'static,
    {
        self.on_draw::<yog_event::client::render::Ui>(Some(ui_id), handler);
    }

    /// Draw the pictures of book pages of type `drawn` whose `drawer` is
    /// `drawer` (`"mymod:diagram"`): `handler` gets the page's `data` and
    /// the picture's box `[x, y, w, h]` in screen pixels, every frame the
    /// page is open.
    ///
    /// ```text
    /// { type: "drawn", drawer: "mymod:diagram", title: "Wiring", height: 64,
    ///   caption: "{u|in} → {u|out}", text: "…", data: { nodes: 3 } }
    /// ```
    pub fn on_book_page<F>(&mut self, drawer: &str, handler: F)
    where
        F: Fn(&GfxContext, &yog_data::Value, [f32; 4]) + Send + Sync + 'static,
    {
        self.on_key::<yog_event::client::render::Page, _>(drawer, move |page, _| {
            if let Some(gfx) = crate::draw::frame_gfx() {
                handler(&gfx, &page.data, page.rect);
            }
        });
    }

    /// A picture under the tooltips of `item` (as Hex Casting's scrolls show
    /// their pattern): `size(item, snbt)` says whether a stack — its custom
    /// data as SNBT — gets one and its size in GUI pixels; `draw` draws it
    /// in the box `[x, y, w, h]` (screen pixels) while the tooltip shows.
    pub fn on_item_tooltip<S, D>(&mut self, item: &str, size: S, draw: D)
    where
        S: Fn(&str, &str) -> Option<(f32, f32)> + Send + Sync + 'static,
        D: Fn(&GfxContext, &str, &str, [f32; 4]) + Send + Sync + 'static,
    {
        self.provide_key::<yog_event::item::Picture>(item, move |p, _| {
            size(&p.item, &p.snbt).map(|(w, h)| [w, h])
        });
        self.on_key::<yog_event::client::render::Tooltip, _>(item, move |t, _| {
            if let Some(gfx) = crate::draw::frame_gfx() {
                draw(&gfx, &t.item, &t.snbt, t.rect);
            }
        });
    }

    /// A stack's own name and tooltip, from its custom data (SNBT): `text(item,
    /// snbt)` gives `Some((name, lines))` — `name` `None` keeps the item's;
    /// `lines` stand instead of the item's description — or `None` for the
    /// item's own. A written blueprint names its design:
    ///
    /// ```ignore
    /// registry.on_item_text(Items::BLUEPRINT, |_, snbt| {
    ///     let bp = Blueprint::from_nbt(snbt)?;
    ///     Some((Some(bp.name.clone()), vec![format!("{} ports", bp.ports)]))
    /// });
    /// ```
    pub fn on_item_text<F>(&mut self, item: &str, text: F)
    where
        F: Fn(&str, &str) -> Option<(Option<String>, Vec<String>)> + Send + Sync + 'static,
    {
        self.provide_key::<yog_event::item::Text>(item, move |t, _| {
            text(&t.item, &t.snbt).map(|(name, lines)| yog_event::item::Shown { name, lines })
        });
    }

    /// A compiled `.ymodel` (the generated code calls this): found later by
    /// [`crate::body::model`].
    pub fn register_body_model(&mut self, id: &str, data: &[u8]) {
        if let Err(e) = yog_body::register_model(id, data) {
            crate::error!("{e}");
        }
    }

    /// A compiled `.yanim` (the generated code calls this): its clips found
    /// later by [`crate::body::clip`].
    pub fn register_body_anims(&mut self, id: &str, data: &[u8]) {
        if let Err(e) = yog_body::register_anims(id, data) {
            crate::error!("{e}");
        }
    }

    /// Register a handler asked each frame about the body of every tracked
    /// entity ([`crate::body::track`]): it answers with [`crate::body::Ops`]
    /// — pose vanilla parts, turn the body, draw meshes (rigs, morphs),
    /// disguise, hide, capture. Every mod's ops add up.
    pub fn on_body<F>(&mut self, handler: F)
    where
        F: Fn(&yog_body::Frame, &mut yog_body::Ops) + Send + Sync + 'static,
    {
        self.change::<yog_event::client::render::Body>(move |e, _| {
            let Some(frame) = yog_body::Frame::decode(&e.frame) else {
                return;
            };
            let mut ops = yog_body::Ops::default();
            handler(&frame, &mut ops);
            e.ops.extend_from_slice(&ops.0);
        });
    }

    /// Change the loot of some tables as `modifier` says.
    pub fn add_loot_modifier(&mut self, modifier: crate::loot::LootModifier) {
        self.change::<yog_event::world::loot::Roll>(move |loot, _| {
            crate::loot::apply(&modifier, loot)
        });
    }

    /// Register an attribute every player has — what effects change and
    /// [`yog_event::entity::attribute_value`] reads.
    pub fn register_attribute(&mut self, def: yog_registry::AttributeDef) {
        use yog_data::Schema;
        self.content(yog_ids::kind::ATTRIBUTE, &def.id.clone(), &def.write());
    }

    /// Register a mob effect.
    pub fn register_effect(&mut self, def: yog_registry::EffectDef) {
        use yog_data::Schema;
        self.content(yog_ids::kind::EFFECT, &def.id.clone(), &def.write());
    }

    /// Register a potion and its brewing.
    pub fn register_potion(&mut self, def: yog_registry::PotionDef) {
        use yog_data::Schema;
        self.content(yog_ids::kind::POTION, &def.id.clone(), &def.write());
    }

    /// A compiled `.ydef` the game registers (`attribute`, `effect`,
    /// `potion`) — the generated code calls this.
    pub fn register_definition_data(&mut self, kind: &str, id: &str, data: &[u8]) {
        self.register_blob(kind, id, data)
    }

    /// A compiled `.ydef` loot modifier (the generated code calls this).
    pub fn add_loot_modifier_data(&mut self, id: &str, data: &[u8]) {
        match yog_data::read_bytes::<crate::loot::LootModifier>(data) {
            Ok(m) => self.add_loot_modifier(m),
            Err(e) => crate::error!("loot modifier {id}: {e:?}"),
        }
    }

    /// Register loot function `name` (`mymod:pattern_scroll`): what loot
    /// modifiers naming it (`function`) run on each stack they add — set
    /// its custom data, say.
    pub fn loot_function<F>(&mut self, name: &str, function: F)
    where
        F: Fn(&mut crate::loot::LootStack, &crate::loot::Loot) + Send + Sync + 'static,
    {
        let mut f = crate::loot::functions().write();
        f.insert(name.to_string(), yog_sync::Arc::new(function));
    }

    /// Register a handler that moves, turns or zooms the camera each frame;
    /// it changes the [`crate::body::CameraOut`] the handlers before it made.
    pub fn on_camera<F>(&mut self, handler: F)
    where
        F: Fn(&yog_body::CameraFrame, &mut yog_body::CameraOut) + Send + Sync + 'static,
    {
        self.change::<yog_event::client::render::Camera>(move |e, _| handler(&e.frame, &mut e.out));
    }

    /// Register a handler that may replace the local player's travel step:
    /// `Some(velocity)` (blocks per tick, drag applied) moves the player by
    /// it instead of vanilla's walking, swimming, falling or gliding.
    pub fn on_travel<F>(&mut self, handler: F)
    where
        F: Fn(&yog_body::TravelFrame) -> Option<yog_body::Vec3> + Send + Sync + 'static,
    {
        self.change::<yog_event::client::game::Travel>(move |e, _| {
            if e.velocity.is_none() {
                e.velocity = handler(&e.frame);
            }
        });
    }

    /// Register a handler called every frame when the HUD is rendered.
    ///
    /// `gfx` provides full GPU pipeline access plus 2D convenience draw calls.
    /// `view_proj` and `camera_pos` are zero in HUD context; use `gfx.draw2d()` for HUD elements.
    pub fn on_hud_render<F>(&mut self, handler: F)
    where
        F: Fn(&GfxContext) + Send + Sync + 'static,
    {
        self.on_draw::<yog_event::client::render::Hud>(None, handler);
    }

    /// Register a handler called every frame at the end of world rendering.
    ///
    /// `gfx.view_proj()` is the combined projection × view matrix (camera-relative).
    /// `gfx.camera_pos()` is the camera world position.
    /// To render at world position P, translate by `P - camera_pos` before drawing.
    pub fn on_world_render<F>(&mut self, handler: F)
    where
        F: Fn(&GfxContext) + Send + Sync + 'static,
    {
        self.on_draw::<yog_event::client::render::World>(None, handler);
    }

    /// A key binding the player sees in the game's controls (under this
    /// mod's name) and can rebind there; the game keeps the key it's bound
    /// to. `id` names it (`mymod:xray`), `name` is its label's translation
    /// key (a `.yres` text's `…_KEY`), `default_key` a GLFW key code
    /// ([`crate::client::keys`]; `None`: unbound). `handler` runs on the
    /// client thread at each press.
    ///
    /// ```ignore
    /// registry.key_binding("mymod:xray", Strings::KEY_XRAY_KEY, Some(keys::X), || toggle_xray());
    /// ```
    pub fn key_binding<F>(&mut self, id: &str, name: &str, default_key: Option<i32>, handler: F)
    where
        F: Fn() + Send + Sync + 'static,
    {
        let declare = yog_event::key_binding::KeyBindingSpec {
            id: id.to_owned(),
            name: name.to_owned(),
            default_key: default_key.unwrap_or(crate::client::keys::UNBOUND),
        };
        if self.call(&declare) != Some(true) {
            crate::error!("key binding {id}: the host wasn't told of it");
        }
        self.on_key::<yog_event::client::input::Binding, _>(id, move |_, _| handler());
    }
}

// ── Mod trait ─────────────────────────────────────────────────────────────────

/// Implemented by every Yog mod. Called once at startup to register handlers.
pub trait Mod {
    fn register(registry: &mut Registry);
}
