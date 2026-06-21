# Yog

> **The Gate and the Key** — write Minecraft mods in **Rust** instead of Java.

Yog is an open-source mod loader that exposes an ergonomic **Rust** API for
writing Minecraft mods (server *and*, later, client), bridging into the Java
game through a thin **Fabric** host. Named after Yog-Sothoth, "the gate and the
key" — the gateway between the Java and Rust worlds.

Free and open source forever (`MIT OR Apache-2.0`). If it's useful to you, you
can support development via the donation links below — there are no paid tiers.

## Status

🚧 **MVP / proof-of-bridge.** Goal of this stage: prove an end-to-end
`Java → Rust` event dispatch on a single server event (**block break**) on
**Minecraft 1.20.1**.

## Scope & roadmap

- **Versions:** start at **1.20.1**; support only de-facto "LTS" modding
  versions (`.1` releases: 1.20.1, 1.21.1, …). A new MC version is added only
  once the loader is stable across all current targets.
- **Loaders:** **Fabric** first → then **NeoForge** → then **Forge**.
- **Mappings:** **Yarn** (libre). We deliberately do **not** bundle Mojmaps —
  their license forbids redistribution.

| Stage | What |
|------:|------|
| ✅ 0 | Scaffold: Fabric host + Rust runtime |
| ✅ 1 | End-to-end bridge: events `Java → Rust` (verified in-game) |
| ✅ 2a | Event set: block break, chat, player join/leave, server lifecycle |
| ✅ 2b | Rust→Minecraft: broadcast, world `get`/`set` block, command registration |
| ✅ 3 | Dynamic mod loading via C-ABI; `.yog` mod packaging; self-contained jar |
| 4 | Client-side hooks (rendering / UI) — the real differentiator |
| 5 | NeoForge host, then Forge host |

### API available now

Events: `on_block_break`, `on_chat`, `on_player_join`, `on_player_leave`,
`on_server_started`, `on_server_stopping`. World: `World::get_block` /
`set_block`. Commands: `on_command`. Actions on the [`Server`] handle:
`broadcast`. See `example-mod` for usage.

## Architecture

```
   Rust mod  (cdylib, depends on yog-api, exported via export_mod!)   →  .yog
        │  dlopen + C-ABI (yog_mod_register)
   yog-runtime  (cdylib: JNI bridge + dispatch + mod loader)   ← embedded in jar
        │  JNI  (Java_dev_yog_NativeBridge_*)
   Fabric host  (dev.yog: NativeBridge, YogHost) + Fabric API events
        │  Yarn mappings (not Mojmap)
   Minecraft 1.20.1
```

- The Java side is thin: it extracts the embedded runtime native, subscribes to
  **Fabric API events**, and forwards them across JNI. All mod logic is Rust.
- The runtime native is **bundled inside the loader jar** (`resources/natives/
  <os>-<arch>/`) and extracted at startup — players never handle a loose
  `.so`/`.dll`. The jar can carry every platform's native at once.
- **Mods are dynamically loaded** from `<game dir>/yog-mods/`: a mod is a cdylib
  (or a `.yog` archive holding per-platform natives), `dlopen`'d via a small
  C-ABI guarded by [`ABI_VERSION`](rust/crates/yog-api/src/lib.rs).

## Layout

```
yog/
├── build.sh                     # dotnet-style task runner
├── rust/                        # Rust workspace (multi-crate + facade)
│   └── crates/
│       ├── yog-core/            # core types + handles (BlockPos, Server)   [MIT/Apache]
│       ├── yog-event/           # event types                              [MIT/Apache]
│       ├── yog-world/           # world access: get/set block              [MIT/Apache]
│       ├── yog-command/         # command types                            [MIT/Apache]
│       ├── yog-logging/         # logging macros (infra)                   [MIT/Apache]
│       ├── yog-api/             # FACADE + Registry hub + export_mod!       [MIT/Apache]
│       ├── yog-cli/             # `yog build` → .yog packaging             [MIT/Apache]
│       └── yog-runtime/         # cdylib: JNI bridge + dispatch + loader    [AGPL]
├── example-mod/                 # a standalone mod, built on its own to .yog
└── fabric/                      # Fabric host mod (Java)                    [AGPL]
    ├── build.gradle
    ├── gradle.properties        # MC / Yarn / loader / fabric-api versions
    └── src/main/
        ├── java/dev/yog/        # NativeBridge, YogHost
        └── resources/           # fabric.mod.json (+ embedded natives)
```

## Build & run (local — needs JDK 17, Rust, and network access)

`build.sh` is a dotnet-style task runner (it auto-picks a JDK 17 for the Gradle
parts — Gradle 8.8 can't run on Java 23+):

```bash
./build.sh build               # compile rust + fabric, build the example .yog
./build.sh run fabric          # build + run the Fabric dev server
./build.sh run fabric --client # build + run the Fabric dev CLIENT (test in-game)
./build.sh test                # cargo test
./build.sh publish fabric      # release build -> artifacts/fabric/ (+ artifacts/native/)
./build.sh clean               # remove build outputs and artifacts
./build.sh build -c Debug      # Debug configuration
./build.sh --help
```

`build` cross-compiles the runtime for **every supported platform**
(linux/windows/macos × x86_64/aarch64) and embeds them all into the jar, and
builds the example mod into a multi-platform `.yog`. This uses
[`cargo-zigbuild`](https://github.com/rust-cross/cargo-zigbuild) + `zig` plus the
rustup targets; install them for full coverage:

```bash
cargo install cargo-zigbuild   # and have `zig` on PATH
rustup target add aarch64-unknown-linux-gnu x86_64-pc-windows-gnu \
                  x86_64-apple-darwin aarch64-apple-darwin
```

Without them, only the platforms whose toolchain you have are bundled (the build
skips the rest with a note).

1. **Run the dev server:**
   ```bash
   ./build.sh run fabric
   ```
   First run creates `fabric/run/eula.txt` — set `eula=true` and run again.
   (If your JDK 17 is elsewhere: `YOG_JAVA17_HOME=/path/to/jdk17 ./build.sh run fabric`.)
2. Break a block / chat / join / run `/yog hi`. The Rust mod reacts in the
   console (and in chat):
   ```
   [yog] [INFO] runtime initialised — the gate is open.
   [yog] [INFO] loaded 1 mod(s) from .../yog-mods
   [yog] [INFO] [example-mod] server started — Yog is awake.
   [yog] [INFO] [example-mod] Steve broke minecraft:stone at (10, 64, -3)
   ```

> ⚠️ **Confirm versions/mappings.** The pinned numbers in `gradle.properties`
> and the Fabric/Yarn accessor names in `YogHost.java` are for 1.20.1 — check
> against the exact Yarn build (see <https://fabricmc.net/develop>) if
> compilation complains.

## Writing & building a mod

A mod is a `cdylib` crate that depends on `yog-api`, implements `Mod`, and
exports itself:

```rust
use yog_api::{info, Mod, Registry};

struct MyMod;
impl Mod for MyMod {
    fn register(r: &mut Registry) {
        r.on_chat(|e, _srv| info!("{}: {}", e.player_name, e.message));
        r.on_command("hello", |ctx, _srv| Some(format!("hi {}", ctx.source)));
    }
}
yog_api::export_mod!(MyMod);
```

Build it into a distributable `.yog` with the Yog CLI, then drop it in your
`yog-mods/` folder:

```bash
yog build           # -> artifacts/<name>.yog
```

A `.yog` is just a zip: per-platform natives under `natives/<os>-<arch>/` plus a
`yog.toml` manifest. Players install a mod by dropping the `.yog` into
`<game dir>/yog-mods/` — no loose `.so`/`.dll`, nothing to think about.

## License

Copyleft to keep the loader free, but split so the mod ecosystem stays open
(see [`LICENSING.md`](LICENSING.md)):

- **Loader engine** — `yog-runtime` + the Fabric host → **AGPL-3.0-only**. Nobody
  can ship a closed-source fork of the loader itself.
- **`yog-api` + example mod** → **MIT OR Apache-2.0**, so mods can use any license.

## Support

This project is free. If you'd like to support development, donation links will
live here _(TBD)_.
