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
| ✅ 0 | Scaffold: Fabric host + Rust runtime + example mod |
| ▶️ 1 | End-to-end bridge: events `Java → Rust` (run & verify locally) |
| ✅ 2a | Event set: block break, chat, player join/leave, server lifecycle |
| ▶️ 2b | Rust→Minecraft actions via `Server` handle (broadcast ✅; commands + world `get`/`set` next) |
| 3 | Dynamic mod loading (`.so`/`.dll`) via a stable C-ABI plugin interface |
| 4 | Client-side hooks (rendering / UI) — the real differentiator |
| 5 | NeoForge host, then Forge host |

### Events available now

`on_block_break`, `on_chat`, `on_player_join`, `on_player_leave`,
`on_server_started`, `on_server_stopping` — see `yog-example-mod` for usage.

## Architecture

```
   Rust mod  (yog-example-mod, depends on yog-api)
        │  registers handlers
   yog-api  (events + Registry — pure Rust, no JVM)
        │
   yog-runtime  (cdylib: JNI entry points + dispatch)   ← libyog_runtime.so
        │  JNI  (Java_dev_yog_NativeBridge_*)
   Fabric host  (dev.yog: NativeBridge, YogHost) + Fabric API events
        │  Yarn mappings (not Mojmap)
   Minecraft 1.20.1
```

The Java side is intentionally thin: it loads the native library, subscribes to
**Fabric API events**, and forwards them across JNI. All mod logic lives in Rust.
(Fabric API events are more stable across versions than raw Mixins; Mixins
return in stage 4 for deeper hooks like client rendering that Fabric API does
not cover.)

## Layout

```
yog/
├── build.sh                     # build Rust runtime + stage the native lib
├── rust/                        # Rust workspace
│   └── crates/
│       ├── yog-api/             # public API for mod authors (events, Registry)
│       ├── yog-runtime/         # cdylib: JNI entry points + dispatch
│       └── yog-example-mod/     # sample mod using the API
└── fabric/                      # Fabric host mod (Java)
    ├── build.gradle
    ├── gradle.properties        # MC / Yarn / loader / fabric-api versions
    └── src/main/
        ├── java/dev/yog/        # NativeBridge, YogHost
        └── resources/           # fabric.mod.json
```

## Build & run (local — needs JDK 17, Rust, and network access)

Use the `build.sh` helper (it auto-picks a JDK 17 for the Gradle parts — Gradle
8.8 can't run on Java 23+):

```bash
./build.sh            # = ./build.sh rust : build Rust runtime + stage native lib
./build.sh fabric     # build the Fabric host mod
./build.sh run        # run the Fabric dev server (also builds rust first)
./build.sh neoforge   # (roadmap, not implemented yet)
```

1. **Run the dev server:**
   ```bash
   ./build.sh run
   ```
   First run creates `fabric/run/eula.txt` — set `eula=true` and run again.
   (If your JDK 17 is elsewhere: `YOG_JAVA17_HOME=/path/to/jdk17 ./build.sh run`.)
2. Break a block / chat / join. You should see the Rust mod react in the console:
   ```
   [yog] runtime initialised — the gate is open.
   [example-mod] server started — Yog is awake.
   [example-mod] Steve joined (069a79f4-…)
   [example-mod] Steve broke minecraft:stone at (10, 64, -3)
   ```

> ⚠️ **Confirm versions/mappings.** The pinned numbers in `gradle.properties`
> and the Fabric/Yarn accessor names in `YogHost.java` (e.g. `getUuidAsString`,
> `getContent`) are for 1.20.1 — check against the exact Yarn build
> (see <https://fabricmc.net/develop>) if compilation complains.

## Naming

Sibling to the **Nyarla** project — both Lovecraftian. Future sub-components may
take themed names (e.g. a mod-manifest format → *Necronomicon*).

## License

Copyleft to keep the loader free, but split so the mod ecosystem stays open
(see [`LICENSING.md`](LICENSING.md)):

- **Loader engine** — `yog-runtime` + the Fabric host → **AGPL-3.0-only**. Nobody
  can ship a closed-source fork of the loader itself.
- **`yog-api` + example mod** → **MIT OR Apache-2.0**, so mods can use any license.

## Support

This project is free. If you'd like to support development, donation links will
live here _(TBD)_.
