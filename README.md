# Yog

> **The Gate and the Key** — write Minecraft mods in **Rust** instead of Java.

Yog is an open-source mod loader that exposes an ergonomic **Rust** API for
writing Minecraft mods (server-side and client-side), bridging into the Java
game through a thin **Fabric** host. Named after Yog-Sothoth, "the gate and the
key" — the gateway between the Java and Rust worlds.

Free and open source forever (`MIT OR Apache-2.0` for the mod API, `AGPL-3.0`
for the loader engine). If it's useful to you, support development via the
donation links below — there are no paid tiers.

## Status

**Active development.** The core bridge is proven end-to-end. A large event
surface, full world/entity control, commands, networking, custom items/blocks,
scheduling, and storage are already shipped. ABI is versioned and
forward-compatible.

## Scope & roadmap

- **Versions:** support only de-facto "LTS" modding versions (`.1` releases:
  1.20.1, 1.21.1, …). A new MC version is added only once the loader is stable
  across all current targets.
- **Loaders:** **Fabric** first → then **NeoForge** → then **Forge**.
- **Mappings:** **Yarn** (libre). We deliberately do **not** bundle Mojmaps —
  their license forbids redistribution.

### Supported Fabric platforms

| Minecraft | Yarn mappings | fabric-loader | fabric-api | Java | Status |
|-----------|--------------|--------------|------------|------|--------|
| **1.20.1** | 1.20.1+build.10 | ≥ 0.15.11 | 0.92.2+1.20.1 | 17 | ✅ tested |

Each platform has its own version-specific Mixin sources under
`fabric/platforms/<mc-version>/`. The active platform is selected by
`minecraft_version` in `fabric/gradle.properties`.

| Stage | What | ABI minor |
|------:|------|:---------:|
| ✅ 0 | Scaffold: Fabric host + Rust runtime | — |
| ✅ 1 | End-to-end bridge: events `Java → Rust` (block break, verified in-game) | 0 |
| ✅ 2 | Core event set; world get/set; player give/teleport; command registration | 1 |
| ✅ 3 | Dynamic mod loading; `.yog` packaging; self-contained jar; entity / effects / NBT | 2–3 |
| ✅ 4 | Cancellable events; networking; scoreboard; bossbar; scheduler; custom items/blocks | 4 |
| ✅ 5 | Entity spawn events; world entity count; `EntityPhase` unified API; entity NBT; particles | 5–6 |
| ✅ 6 | Player death/respawn, advancements, entity attribute get/set | 7 |
| ✅ 7 | Entity interact, item craft, explosion events | 8 |
| ✅ 8 | Item pickup, player move, container open/close, projectile hit; Config; typed packets | 9 |
| ✅ 9 | Client-side hooks: tick, HUD render, keyboard, screen open/close | 10 |
| ✅ 10 | Item NBT: held item + off-hand + full slot query/set | 11–12 |
| ✅ 11 | Low-level GPU pipeline: `YogGfxApi`, HUD + world rendering, `yog-gfx` crate | 13–14 |
| 🔲 12 | NeoForge host, then Forge host |  |

## API available now (ABI minor 14)

### Events

All event handlers share a single signature — one registration fires for both
phases:

```rust
registry.on_block_break(|event, phase, server| -> bool {
    match phase {
        EventPhase::Pre  => { /* return false to cancel */ true }
        EventPhase::Post => { /* observe-only */ true }
    }
});
```

| Registration | Event type | Cancellable (Pre) |
|---|---|:---:|
| `on_block_break` | `BlockBreakEvent` | ✅ |
| `on_chat` | `ChatEvent` | ✅ |
| `on_player_join` | `PlayerJoinEvent` | — |
| `on_player_leave` | `PlayerLeaveEvent` | — |
| `on_use_item` | `UseItemEvent` | — |
| `on_use_block` | `UseBlockEvent` | — |
| `on_attack_entity` | `AttackEntityEvent` | — |
| `on_entity_damage` | `EntityDamageEvent` | ✅ |
| `on_entity_death` | `EntityDeathEvent` | — |
| `on_entity_spawn` | `EntitySpawnEvent` | ✅ |
| `on_player_place_block` | `PlaceBlockEvent` | ✅ |
| `on_player_death` | `PlayerDeathEvent` | ✅ |
| `on_player_respawn` | `PlayerRespawnEvent` | — |
| `on_advancement` | `AdvancementEvent` | — |
| `on_entity_interact` | `EntityInteractEvent` | ✅ |
| `on_item_craft` | `CraftEvent` | — |
| `on_explosion` | `ExplosionEvent` | ✅ |
| `on_item_pickup` | `ItemPickupEvent` | ✅ |
| `on_player_move` | `PlayerMoveEvent` | — |
| `on_container_open` | `ContainerOpenEvent` | ✅ |
| `on_container_close` | `ContainerCloseEvent` | — |
| `on_projectile_hit` | `ProjectileHitEvent` | ✅ |
| `on_tick` | — | — |
| `on_server_started` | — | — |
| `on_server_stopping` | — | — |

#### Client-side events (render thread, no server context)

```rust
registry.on_client_tick(|_ev| { /* fires every client tick */ });

registry.on_key_press(|ev| -> bool {
    if ev.key_code == 69 && ev.action == 1 { // E pressed
        info!("E key pressed!");
        return false; // return false to suppress Minecraft handling
    }
    true
});

registry.on_screen_open(|ev| {
    info!("screen opened: {}", ev.screen_class); // e.g. "InventoryScreen"
});

registry.on_screen_close(|ev| {
    info!("screen closed: {}", ev.screen_class);
});
```

| Registration | Event type | Notes |
|---|---|---|
| `on_client_tick` | `ClientTickEvent` | Every client tick |
| `on_hud_render` | `GfxContext` | Every frame; full GPU access + 2D helpers — see Graphics below |
| `on_world_render` | `GfxContext` | After world geometry; `view_proj` + `camera_pos` filled |
| `on_key_press` | `KeyPressEvent` | Return `false` to suppress; `action`: 0=release, 1=press, 2=repeat |
| `on_screen_open` | `ScreenEvent` | GUI opened; `screen_class` is simple class name |
| `on_screen_close` | `ScreenEvent` | GUI closed |

### Graphics (ABI minor 14)

Mods get direct access to the OpenGL pipeline via `GfxContext` (from `yog-gfx`).
GPU resources (`u32` handles) are created once and stored between frames.

#### HUD overlay (2D)

```rust
use yog_api::{GfxContext, gfx_draw2d};

registry.on_hud_render(|ctx: &GfxContext| {
    let (w, h) = ctx.screen_size();
    let d = ctx.draw2d();
    d.rect(4.0, 4.0, 60.0, 14.0, 0x88_00_00_00);
    d.text("hello", 6.0, 5.0, 0xFF_FF_FF_FF, true);
    d.mc_texture("minecraft:textures/gui/icons.png",
        w as f32 / 2.0 - 9.0, h as f32 / 2.0 - 9.0,
        0.0, 0.0, 18.0, 18.0, 256.0, 256.0);
});
```

#### World geometry (3D, custom GLSL)

GPU resources live outside the closure and persist across frames:

```rust
use yog_api::{GfxContext, gfx_gl::{Buffer, VertexArray, ShaderProgram}};
use yog_api::gfx_core::{DrawMode, DataType};

struct MyRenderer {
    vbo: Option<Buffer>,
    vao: Option<VertexArray>,
    prog: Option<ShaderProgram>,
}

impl MyRenderer {
    fn init(&mut self, ctx: &GfxContext) {
        let vbo = ctx.create_buffer();
        // 3 × (xyz as f32) — a single triangle
        let verts: &[f32] = &[0.0, 0.0, 0.0,  1.0, 0.0, 0.0,  0.5, 1.0, 0.0];
        unsafe { vbo.upload(ctx, verts, false) };

        let vao = ctx.create_vao();
        vao.attrib(ctx, &vbo, 0, 3, DataType::F32, false, 12, 0);

        let prog = ctx.create_shader(VERT_GLSL, FRAG_GLSL).expect("shader compile failed");
        self.vbo = Some(vbo);
        self.vao = Some(vao);
        self.prog = Some(prog);
    }

    fn render(&mut self, ctx: &GfxContext) {
        if self.vbo.is_none() { self.init(ctx); }
        let prog = self.prog.as_ref().unwrap();
        let vao  = self.vao.as_ref().unwrap();
        // Shift triangle 8 blocks east of camera
        let cam = ctx.camera_pos();
        prog.uniform_mat4(ctx, "uViewProj", &ctx.view_proj());
        prog.uniform_3f(ctx, "uOffset", 8.0 - cam[0], -cam[1], -cam[2]);
        ctx.set_depth(true, false);
        ctx.draw_arrays(vao, prog, DrawMode::Triangles, 0, 3);
        ctx.set_depth(false, false);
    }
}

// In register():
let renderer = std::sync::Mutex::new(MyRenderer { vbo: None, vao: None, prog: None });
registry.on_world_render(move |ctx| {
    renderer.lock().unwrap().render(ctx);
});
```

`view_proj` is **camera-relative**: world position `P` maps to clip space as
`view_proj * (P - camera_pos)`.  This avoids floating-point precision loss for
far objects.

#### GfxContext API surface

```rust
// Frame info
ctx.screen_size() -> (i32, i32)
ctx.delta_tick()  -> f32
ctx.view_proj()   -> [f32; 16]   // col-major; zeroed in on_hud_render
ctx.camera_pos()  -> [f32; 3]    // zeroed in on_hud_render

// GPU resources
ctx.create_buffer() / delete_buffer(buf)
ctx.create_vao()    / delete_vao(vao)
ctx.create_shader(vert, frag) -> Result<ShaderProgram, ()>
ctx.delete_shader(prog)
ctx.create_texture_rgba(w, h, &[u8]) / delete_texture(tex)
ctx.texture_from_mc("minecraft:textures/…")  // borrows MC's texture; do NOT delete

// Draw
ctx.draw_arrays(vao, prog, DrawMode::Triangles, first, count)
ctx.draw_elements(vao, ebo, prog, DrawMode::Triangles, count, u32_idx)

// State
ctx.set_blend(enabled, src_factor, dst_factor)   // blend::SRC_ALPHA etc.
ctx.set_depth(test, write)
ctx.set_scissor(x, y, w, h)  // physical pixels
ctx.clear_scissor()
ctx.set_viewport(x, y, w, h)

// Uniforms (via ShaderProgram)
prog.uniform_1i / 1f / 2f / 3f / 4f / mat4(ctx, "name", value)

// 2D helpers (HUD only)
ctx.draw2d().text(text, x, y, color, shadow)
ctx.draw2d().rect(x1, y1, x2, y2, color)
ctx.draw2d().gradient(x1, y1, x2, y2, top_color, bottom_color)
ctx.draw2d().mc_texture(id, x, y, u0, v0, w, h, tex_w, tex_h)
```

### World

```rust
let world = World::new(srv, "minecraft:overworld");
world.get_block(pos)          // -> Option<String>
world.set_block(pos, "minecraft:stone")
world.get_time()              // -> Option<i64>
world.set_time(6000)
world.is_raining()
world.set_weather(true, 6000)
world.entity_count("minecraft:zombie")  // -> i32
```

### Player

```rust
let player = Player::new(srv, "Steve");
player.give("minecraft:diamond", 4)
player.teleport(x, y, z)
player.teleport_to_dim("minecraft:the_nether", x, y, z)
player.kick("Goodbye")
player.set_gamemode("creative")
player.send_title("Title", "Subtitle", 10, 70, 20)
player.send_actionbar("message")
player.inventory()            // -> Vec<(slot, item_id, count)>
player.set_slot(36, "minecraft:stone", 1)
player.scoreboard_get("kills")
player.scoreboard_set("kills", 10)
```

### Item NBT (ABI minor 11–12)

```rust
// Main hand
srv.get_held_item_nbt("Steve")          // -> Option<String>  (SNBT)
srv.set_held_item_nbt("Steve", "{Enchantments: [{id: \"minecraft:sharpness\", lvl: 5}]}")

// Off hand
srv.get_offhand_item_nbt("Steve")       // -> Option<String>
srv.set_offhand_item_nbt("Steve", "{display: {Name: '{\"text\":\"Shield++\"}'}}")

// Arbitrary inventory slot
srv.get_slot_item("Steve", 0)           // -> Option<(item_id, count, snbt)>
srv.set_slot_item("Steve", 0, "minecraft:diamond_sword", 1, "{Damage: 0}")
srv.set_slot_item("Steve", 9, "minecraft:air", 0, "")  // clear slot
```

### Entity (universal by UUID)

```rust
let entity = Entity::new(srv, uuid);
entity.teleport(x, y, z)
entity.teleport_to_dim("minecraft:the_nether", x, y, z)
entity.position()             // -> Option<(f64, f64, f64)>
entity.health() / set_health(20.0)
entity.kill()
entity.velocity() / set_velocity(vx, vy, vz) / add_velocity(vx, vy, vz)
entity.add_effect("minecraft:speed", 200, 1, true)
entity.get_nbt()              // -> Option<String>  (SNBT)
entity.set_nbt("{CustomName: 'Bob'}")
entity.attribute_get("minecraft:generic.max_health")  // -> Option<f64>
entity.attribute_set("minecraft:generic.max_health", 40.0)
```

### Server actions (via `&dyn Server` / `srv`)

```rust
srv.broadcast("Hello, world!");
srv.spawn_entity("minecraft:zombie", "minecraft:overworld", x, y, z)
srv.spawn_particles("minecraft:overworld", x, y, z, "minecraft:flame", 20, 0.5, 0.5, 0.5, 0.1)
srv.play_sound("minecraft:overworld", x, y, z, "minecraft:entity.player.levelup", 1.0, 1.0)
srv.drop_loot("minecraft:entities/zombie", "minecraft:overworld", x, y, z)
srv.has_item_tag("minecraft:oak_planks", "minecraft:planks")
srv.get_block_nbt("minecraft:overworld", pos)
srv.set_block_nbt("minecraft:overworld", pos, "{...}")
srv.game_dir()
srv.online_players()          // -> Vec<String>
```

### Networking (raw bytes)

```rust
registry.on_packet("mymod:channel", |e, srv| { /* server received */ });
srv.send_to_player("Steve", "mymod:channel", &bytes);
```

### Commands

```rust
registry.on_command("hello", |ctx, srv| {
    Some(format!("hi, {}!", ctx.source))
});
registry.on_typed_command("tp", "float float float", |ctx, srv| {
    let (x, y, z) = (ctx.arg_float(0), ctx.arg_float(1), ctx.arg_float(2));
    srv.teleport(&ctx.source, x, y, z);
    None
});
```

### Custom content

```rust
registry.register_item(ItemDef {
    id: "mymod:ruby".into(),
    max_stack: 64,
    name: Some("Ruby".into()),
    tooltip: Some("Shiny.".into()),
    ..Default::default()
});
registry.add_shaped_recipe(ShapedRecipe { /* ... */ });
```

### Scheduler

```rust
registry.schedule_once(200, |srv| srv.broadcast("2 seconds later"));
registry.schedule_repeating(1200, |srv| srv.broadcast("every minute"));
```

### Storage

Scoped, typed, auto-flushing key-value store.  Writes are atomic
(temp + rename); unflushed mutations are persisted on `Drop`.

```rust
// Global store — one file for the whole server
let mut store = Storage::open(&srv.game_dir(), "mymod");
store.set("motd", "Hello!");
store.set("spawn_x", 0i64);
store.set("spawn_y", 64.0f64);

// Per-player store — one file per UUID (survives restarts)
let mut ps = Storage::open_player(&srv.game_dir(), "mymod", &player_uuid);
ps.set("coins", 100i64);
ps.set("flags", vec![0xAB_u8, 0xCD]);   // raw bytes for custom serialization
let coins  = ps.get_int("coins").unwrap_or(0);
let online = ps.get_bool("first_login_done").unwrap_or(false);

// Per-dimension / per-chunk / per-entity scopes
let mut ws  = Storage::open_world(&srv.game_dir(), "mymod", "minecraft:overworld");
let mut cs  = Storage::open_chunk(&srv.game_dir(), "mymod", "minecraft:overworld", 2, -5);
let mut es  = Storage::open_entity(&srv.game_dir(), "mymod", &entity_uuid);

// Explicit flush (otherwise auto-flushed on drop)
ps.flush().ok();
```

File layout: `<game_dir>/yog-data/<mod_id>/{global,player/<uuid>,world/<dim>,
entity/<uuid>,chunk/<dim>_<cx>_<cz>}.kv`  
Format: `key\ttype\tvalue` plain text, sorted, human-readable.

### Config

```rust
let game_dir = srv.game_dir().unwrap_or_default();
let mut cfg = Config::load(&game_dir, "mymod");
// Reads <game_dir>/yog-config/mymod.cfg  (created on first save)
cfg.set("max_players", 20);
cfg.save_defaults().ok();   // only writes if file doesn't exist yet

let max = cfg.get_int_or("max_players", 20);
let pvp = cfg.get_bool_or("pvp_enabled", true);
let msg = cfg.get_or("welcome_message", "Welcome!");
```

### Typed networking

```rust
use yog_api::{packet, Packet};

// Declare a typed packet — encode/decode is automatic
packet! {
    pub struct SyncCoinsPacket {
        player: String,
        coins:  i64,
    }
}

// Send
let pkt = SyncCoinsPacket { player: "Steve".into(), coins: 100 };
srv.send_to_player("Steve", "mymod:coins", &pkt.encode());

// Receive
registry.on_typed_packet::<SyncCoinsPacket, _>("mymod:coins", |pkt, srv| {
    info!("{} has {} coins", pkt.player, pkt.coins);
});
```

See `example-mod/src/` for full working usage.

## Architecture

```
   Rust mod  (cdylib, depends on yog-api, exported via export_mod!)   →  .yog
        │  dlopen + C-ABI (yog_mod_register / YogApi / YogServer tables)
   yog-runtime  (cdylib: JNI bridge + dispatch + mod loader)   ← embedded in jar
        │  JNI  (Java_dev_yog_NativeBridge_*)
   Fabric host  (dev.yog: NativeBridge, YogHost) + version-specific Mixins
        │  Yarn mappings (not Mojmap)
   Minecraft (active: 1.20.1)
```

- The Java side is thin: it extracts the embedded runtime native, subscribes to
  **Fabric API events**, and forwards them across JNI. All mod logic is Rust.
- The ABI is **versioned** (`ABI_MAJOR.ABI_MINOR`). Mods are forward-compatible:
  a mod built against minor N loads fine on runtime minor M ≥ N.
- All event fn pointers carry a `phase: u8` (0 = Pre, 1 = Post) so one
  registration covers both sides of an action without duplication.
- The runtime native is **bundled inside the loader jar** (`resources/natives/
  <os>-<arch>/`) and extracted at startup — players never handle a loose
  `.so`/`.dll`. The jar carries every platform's native at once.
- **Mods are dynamically loaded** from `<game dir>/yog-mods/`: a mod is a cdylib
  (or a `.yog` archive holding per-platform natives), `dlopen`'d via a small
  C-ABI guarded by `ABI_VERSION`.

## Layout

```
yog/
├── build.sh                     # task runner (build / run / test / publish)
├── rust/                        # Rust workspace
│   └── crates/
│       ├── yog-abi/             # stable C ABI types (YogApi, YogServer)   [MIT/Apache]
│       ├── yog-core/            # core types + Server trait                [MIT/Apache]
│       ├── yog-event/           # event types + EventPhase enum            [MIT/Apache]
│       ├── yog-world/           # World wrapper (get/set block, time, …)   [MIT/Apache]
│       ├── yog-entity/          # Entity wrapper (teleport, health, NBT)   [MIT/Apache]
│       ├── yog-player/          # Player wrapper (inventory, kick, …)      [MIT/Apache]
│       ├── yog-registry/        # custom items/blocks/recipes               [MIT/Apache]
│       ├── yog-command/         # command types + arg parsing              [MIT/Apache]
│       ├── yog-network/         # typed + raw packet helpers               [MIT/Apache]
│       ├── yog-storage/         # persistent key-value storage             [MIT/Apache]
│       ├── yog-config/          # mod configuration (typed key/value files) [MIT/Apache]
│       ├── yog-logging/         # logging macros                           [MIT/Apache]
│       ├── yog-gfx/             # GPU pipeline facade (GfxContext, gl, draw2d) [MIT/Apache]
│       ├── yog-api/             # FACADE + Registry hub + export_mod!      [MIT/Apache]
│       └── yog-runtime/         # cdylib: JNI bridge + dispatch + loader   [AGPL]
├── example-mod/                 # standalone example mod (.yog output)
└── fabric/                      # Fabric host mod (Java)                   [AGPL]
    ├── build.gradle             # adds platforms/<mc-version>/ to sourceSets
    ├── gradle.properties        # active MC version + Yarn / loader / fabric-api pins
    ├── src/main/
    │   ├── java/dev/yog/        # version-agnostic host: NativeBridge, YogHost, …
    │   └── resources/           # embedded native libs (natives/<os>-<arch>/)
    └── platforms/
        └── 1.20.1/              # version-specific Mixin sources + resources
            └── src/main/
                ├── java/dev/yog/mixin/   # all Mixin classes for 1.20.1
                └── resources/            # fabric.mod.json, yog.mixins.json
```

## Build & run (needs JDK 17, Rust, network)

`build.sh` is a dotnet-style task runner (auto-picks JDK 17 for Gradle 8.8):

```bash
./build.sh build               # compile rust + fabric, build the example .yog
./build.sh run fabric          # build + run the Fabric dev server
./build.sh run fabric --client # build + run the Fabric dev CLIENT
./build.sh test                # cargo test
./build.sh publish fabric      # release build -> artifacts/fabric/
./build.sh clean
./build.sh --help
```

`build` cross-compiles for **every supported platform**
(linux/windows/macos × x86_64/aarch64) using
[`cargo-zigbuild`](https://github.com/rust-cross/cargo-zigbuild):

```bash
cargo install cargo-zigbuild
rustup target add aarch64-unknown-linux-gnu x86_64-pc-windows-gnu \
                  x86_64-apple-darwin aarch64-apple-darwin
```

Without them only your local platform is bundled (the rest are skipped with a note).

**First run:**
```bash
./build.sh run fabric
# -> set eula=true in fabric/run/eula.txt, then run again
```

Break a block / chat / run `/hello`. The Rust mod reacts:
```
[yog] runtime initialised — the gate is open.
[yog] loaded 1 mod(s) from .../yog-mods
[yog] [example-mod] server started — Yog is awake.
[yog] [example-mod] Steve broke minecraft:stone at (10, 64, -3)
```

## Writing a mod

### 1. Create a project

```bash
yog new my-mod     # creates my-mod/ with yog.toml + src/lib.rs
cd my-mod
```

`yog.toml` is the project manifest (instead of `Cargo.toml`):

```toml
[mod]
id          = "my-mod"
name        = "My Mod"
version     = "0.1.0"
description = "Does something cool."
authors     = ["You"]
license     = "MIT"
```

### 2. Write the mod

`src/lib.rs` (the only required source file):

```rust
use yog_api::{info, EventPhase, Mod, Registry};

struct MyMod;

impl Mod for MyMod {
    fn register(r: &mut Registry) {
        // Single handler fires for Pre AND Post.
        r.on_block_break(|e, phase, _srv| {
            if phase == EventPhase::Pre && e.block_id == "minecraft:bedrock" {
                info!("Nice try, {}.", e.player_name);
                return false; // cancel
            }
            true
        });

        r.on_command("hello", |ctx, srv| {
            Some(format!("Hi, {}!", ctx.source))
        });
    }
}

yog_api::export_mod!(MyMod);
```

### 3. Build

```bash
yog build          # -> artifacts/my-mod.yog
```

Cross-compiles for every supported platform (linux/windows/macos ×
x86_64/aarch64) in one shot. Install dependencies first:

```bash
yog setup          # checks cargo-zigbuild, zig, and rustup cross-compile targets
```

### 4. Install & test

Drop `artifacts/my-mod.yog` into `<game dir>/yog-mods/` and start the server.
Players also install mods this way — no extra tools needed.

A `.yog` archive is a zip containing per-platform natives under
`natives/<os>-<arch>/` plus a `yog.toml` manifest. The Yog runtime
selects the right native at startup.

### Project layout

```
my-mod/
├── yog.toml           # mod metadata (id, name, version, …)
├── src/
│   └── lib.rs         # entry point: impl Mod + export_mod!(MyMod)
│   └── …              # other source files as needed
└── artifacts/
    └── my-mod.yog     # built package — share this file with players
```

Assets (textures, sounds, data packs) live in `assets/` and `data/` and are
bundled into the `.yog` automatically:

```
my-mod/
├── assets/<namespace>/textures/item/my_item.png
├── data/<namespace>/recipes/my_recipe.json
```

## License

Split to keep the loader free while keeping the mod ecosystem open
(see [`LICENSING.md`](LICENSING.md)):

- **Loader engine** — `yog-runtime` + the Fabric host → **AGPL-3.0-only**.
- **`yog-api` + domain crates** → **MIT OR Apache-2.0**, so mods use any license.

## Support

This project is free. Donation links will live here _(TBD)_.
