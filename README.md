# Yog

> **The Gate and the Key** — write Minecraft mods in **Rust** instead of Java.

Yog is an open-source mod loader that exposes an ergonomic **Rust** API for
writing Minecraft mods (server-side first, client later), bridging into the Java
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

- **Versions:** start at **1.20.1**; support only de-facto "LTS" modding
  versions (`.1` releases: 1.20.1, 1.21.1, …). A new MC version is added only
  once the loader is stable across all current targets.
- **Loaders:** **Fabric** first → then **NeoForge** → then **Forge**.
- **Mappings:** **Yarn** (libre). We deliberately do **not** bundle Mojmaps —
  their license forbids redistribution.

| Stage | What | ABI minor |
|------:|------|:---------:|
| ✅ 0 | Scaffold: Fabric host + Rust runtime | — |
| ✅ 1 | End-to-end bridge: events `Java → Rust` (block break, verified in-game) | 0 |
| ✅ 2 | Core event set; world get/set; player give/teleport; command registration | 1 |
| ✅ 3 | Dynamic mod loading; `.yog` packaging; self-contained jar; entity / effects / NBT | 2–3 |
| ✅ 4 | Cancellable events; networking; scoreboard; bossbar; scheduler; custom items/blocks | 4 |
| ✅ 5 | Entity spawn events; world entity count; `EntityPhase` unified API; entity NBT; particles | 5–6 |
| ✅ 6 | Player death/respawn, advancements, entity attribute get/set | 7 |
| 🔲 7 | Client-side hooks (rendering / UI) |  |
| 🔲 8 | NeoForge host, then Forge host |  |

## API available now (ABI minor 7)

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
| `on_tick` | — | — |
| `on_server_started` | — | — |
| `on_server_stopping` | — | — |

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

```rust
let store = Storage::open(srv, "mymod");
store.set("key", "value");
store.get("key")    // -> Option<String>
```

See `example-mod/src/` for full working usage.

## Architecture

```
   Rust mod  (cdylib, depends on yog-api, exported via export_mod!)   →  .yog
        │  dlopen + C-ABI (yog_mod_register / YogApi / YogServer tables)
   yog-runtime  (cdylib: JNI bridge + dispatch + mod loader)   ← embedded in jar
        │  JNI  (Java_dev_yog_NativeBridge_*)
   Fabric host  (dev.yog: NativeBridge, YogHost) + Fabric API events
        │  Yarn mappings (not Mojmap)
   Minecraft 1.20.1
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
│       ├── yog-network/         # packet event type                        [MIT/Apache]
│       ├── yog-storage/         # persistent key-value storage             [MIT/Apache]
│       ├── yog-logging/         # logging macros                           [MIT/Apache]
│       ├── yog-api/             # FACADE + Registry hub + export_mod!      [MIT/Apache]
│       └── yog-runtime/         # cdylib: JNI bridge + dispatch + loader   [AGPL]
├── example-mod/                 # standalone example mod (.yog output)
└── fabric/                      # Fabric host mod (Java)                   [AGPL]
    ├── build.gradle
    ├── gradle.properties        # MC / Yarn / loader / fabric-api versions
    └── src/main/
        ├── java/dev/yog/        # NativeBridge, YogHost
        └── resources/           # fabric.mod.json (+ embedded natives)
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

A mod is a `cdylib` crate depending on `yog-api`:

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

Build with the Yog CLI, drop into `yog-mods/`:

```bash
yog build     # -> artifacts/<name>.yog
```

A `.yog` is a zip: per-platform natives under `natives/<os>-<arch>/` plus a
`yog.toml` manifest. Players install a mod by dropping the `.yog` into
`<game dir>/yog-mods/` — no loose `.so`/`.dll`.

## License

Split to keep the loader free while keeping the mod ecosystem open
(see [`LICENSING.md`](LICENSING.md)):

- **Loader engine** — `yog-runtime` + the Fabric host → **AGPL-3.0-only**.
- **`yog-api` + domain crates** → **MIT OR Apache-2.0**, so mods use any license.

## Support

This project is free. Donation links will live here _(TBD)_.
