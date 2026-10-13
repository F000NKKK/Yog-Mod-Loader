# Design: Textures and Materials, the Vanilla Pack, and a vanilla-looking yog-ui

Status: design only. Nothing is implemented yet, and nothing was built while writing this.

Revision 2 records the user's answers to the open questions of revision 1: vanilla pack storage and reports, mandatory explicit namespaces, deletion of the flat look, scoped `.ytheme` files, vanilla tabs in Modlist, Minecraft-style block behaviours, `.ybook`, the run-configuration version, and a new project layout. It also states that HexMod-Yog reproduces the original Hex Casting UI rather than the vanilla theme.

Scope: Yog-Mod-Loader (yog-gfx, yog-ui, yog-kit, yog-cli, yog-abi, yog-runtime, yog-registry, Java hosts), Yog-IDLE, and the mods Yog-Modlist, HexMod-Yog and Yog-Pipes.

The project has **no backward compatibility**. Every old form listed in §11 is deleted, not deprecated.

---

## 0. Summary of decisions

| # | Decision | Why |
|---|---|---|
| D1 | **Texture** = a PNG plus an optional `.png.mcmeta` sidecar in vanilla's own format (animation, `texture.blur/clamp`, `gui.scaling`). Its id is `<ns>:<path>`, exactly as in Minecraft (`assets/<ns>/textures/<path>.png`). A mod's textures are PNGs anywhere under `content/` (§13), with id `<mod id>:<path under content/ without .png>`. The file is the declaration: there is no registry file to fill in. | Mods and vanilla then use one shape. The metadata format is the one Minecraft, resource packs and every tool already understand. The vanilla pack (D7) is just textures in that same shape. |
| D2 | **Material** = a texture or texture layers plus how to draw them: render layer, tint, emissive, UV transform, UI slicing and tiling, fills/frames/gradients, text hints, and interaction states. It is a `.ymat` file anywhere under `content/` (one JSON per material, like `.yblock`), or **inline** wherever a material ref is accepted in JSON formats. Its id is `<ns>:<path>`. | A material is structured data with states and layers, which doesn't fit `.yres`'s flat Name/Value/Type grid. One file per asset matches `.yblock`/`.yitem` and gives IDLE a natural document to open. |
| D3 | **A texture ref is a material ref.** Everywhere a material is accepted, a bare texture id means "that texture with default material settings". So a material file is only needed when a setting differs from the default. | There is one concept for authors to learn: "what to draw". |
| D4 | **`render_layer` defaults to `auto`.** The build inspects the alpha channel: no alpha → `solid`, only 0/255 alpha → `cutout`, partial alpha → `translucent`. | Most cutout bugs disappear without any authoring. |
| D5 | **Materials are resolved by the host, never by the mod.** Every `.yog` ships `yog/materials.json`. The runtime holds a `MaterialRegistry` containing the vanilla overlay for the running MC version plus every mod's materials. yog-ui passes `(material id, state bits)` over the ABI. | Vanilla GUI materials are version-dependent, so they must be resolved in the host. Using the same path for mod materials keeps one mechanism, and hot-reload and resource packs just work. |
| D6 | **The vanilla look is yog-ui's only built-in look.** The default control templates draw vanilla materials (`minecraft:gui/button`, …). The current flat "modern" defaults (`bg` rects, amber focus outline, gray scrollbar) are **deleted**, not kept as a theme. Flat colors stay available as brushes for authors who want them. | The user asked for vanilla-first. The "modern" look is not a theme today, just ad-hoc constants in `render.rs`, so there is nothing worth keeping. |
| D7 | **The Vanilla Pack:** for each MC version Yog supports, the vanilla content is exposed in Yog's own formats: textures (PNG+mcmeta), materials (`.ymat`), blocks/items (`.yblock`/`.yitem`), lang, GUI materials, id lists and data-generator reports. It is **generated locally on demand** by `yog vanilla sync` from the user's client jar, downloading from Mojang's servers automatically when there is none. It goes into a per-user cache and is **never committed or published**. By default the pack is extracted to disk. An IDLE Environment setting (`YOG_VANILLA_MODE=jar` for the CLI) switches to reading lazily from the jar, which is slower. Only Yog-authored data is committed in `Yog-Mod-Loader/vanilla/`: the extraction tool, GUI material overlays per version, and rename tables. | Mojang's assets must not be redistributed. Ids and region coordinates are facts that Yog authors itself, so committing them is fine. |
| D8 | **Typed handles everywhere.** Mod textures and materials → `crate::res::textures::…` and `crate::res::materials::…`. Vanilla → `crate::vanilla::{items, blocks, textures, materials, lang}`, generated from the vanilla packs of every MC version the mod targets. Ids that are absent or renamed in some targeted version get `Option`-returning accessors or a runtime rename. | This catches typos at compile time and lets one binary run on 1.20.1 and 1.21.1. |
| D10 | **Every reference carries an explicit namespace** (`minecraft:block/stone`, `hexcasting:block/slate`, `item:minecraft:diamond`). A bare id is an error in every format and in codegen, and the diagnostic suggests the fix. There is no short alias for the mod's own namespace: `mod:` is deleted. IDLE autocompletes full ids, and `yog migrate` rewrites old bare refs. | The user's rule: a reference is always explicit. A file's meaning then never depends on which mod it sits in or on what "default" a format picked. |
| D11 | **Themes are `.ytheme` assets with scoped cascading.** A theme can be attached to the whole mod, a screen, a component or any element subtree. The cascade is vanilla built-in → mod → screen → component → element subtree → named style → explicit attribute (§6.6). The built-in vanilla look is itself a `.ytheme` (`minecraft:vanilla`), and `yog:bare` is an undecorated base. | This is what the user asked for: Unity-style style sheets attachable at any level. Using one format for the built-in look and mod themes means there is one mechanism. |
| D12 | **Block behaviours mirror Minecraft's block classes.** `.yblock` `"behavior": "minecraft:transparent"` (glass), `minecraft:stained_glass`, `minecraft:leaves`, `minecraft:ice`, … brings that class's rules: same-block face culling, light passing, occlusion, suffocation, view blocking. Hosts map each behaviour to the real vanilla class per loader. Mod-defined behaviours come from mod code through a generic extension point (§4.6). A material only says how faces are drawn (layer, tint). | This is how Minecraft does it (`HalfTransparentBlock#skipRendering`, `LeavesBlock`, …). It is block logic, not appearance. |
| D13 | **New project layout (UE5/Unity-like):** `yog.toml` + `src/` (Rust) + `content/` (every asset, free-form folders, type by extension) + `pack/` (raw resource/data-pack passthrough) + hidden `.yog/` (build, cache, IDE state). `yog migrate`, one step of the project-format migration chain (§14), converts existing mods. | Assets are found by extension, not by a folder per type. Feature folders keep a block's model, textures and material together. Generated and intermediate files are out of sight. |
| D9 | **Fabric render layer fix:** `render_layer` reaches the host through `YogBlockDef`. Fabric registers it with `BlockRenderLayerMap`. Occlusion comes from the block's behaviour (D12), and a non-solid layer on a block whose behaviour is the *implicit* default `minecraft:solid` is a build error; writing `"behavior": "minecraft:solid"` explicitly accepts it (grass-block-style opaque cube with a cutout overlay, capability rule). Tint sources also reach the host, which registers block/item color providers. | Today `render_layer` only reaches the model JSON's `render_type`, so Fabric renders every block solid and full-cube cutout blocks x-ray their neighbours on every loader (§4.3). |
| D14 | **Project format versioning with automatic migration.** `yog.toml [project] format = N`. The CLI carries an ordered chain of migration steps N→N+1 and upgrades older projects automatically (CLI and IDLE share one implementation). The toolchain reads only the current format, so there are no compatibility code paths (§14). | Breaking changes like the ones in this design stay cheap for mod authors, and the no-backward-compat rule holds inside the toolchain. |

---

## 1. Current state (what this replaces)

### 1.1 Textures today

The same texture can be referenced in five ways that don't agree with each other:

| Where | Forms | Notes |
|---|---|---|
| `.yblock`/`.yitem` `model.textures` | `@Set.name` (`.yres` image), `path.png` (under `resources/`), `ns:path` / bare `path` (= `minecraft:`), `#slot` | Parsed by `yog-cli/src/content.rs:398-464` (`Ctx::texture`). A sibling `.png.mcmeta` is bundled only for the `path.png` form (content.rs:440-443). |
| `.yres` `"type": "image"` | a path relative to the `.yres` file | Bundled to `assets/<mod>/textures/<rel>.png` (yres.rs:1059-1072). No `.mcmeta`. Generated as `const fn name() -> &'static str`. No mod uses it today. |
| `.yui` `<Image source>` | `item:`, `block:`, `sprite:<yog name>`, `mod:<path>`, `<ns>:<path>`, `@Set.name`, plus `stretch`/`uv`/`tint` | Parsed by `yog_gfx::image::ImageSource` (yog-gfx/src/image.rs). `sprite:` is a 26-entry table (`SPRITES`) with 1.20.2+ atlas ids and 1.20.1 regions, plus a `border` used as a nine-slice inset. |
| HexMod hand-written | `assets/hexcasting/textures/**` | Copied as-is by `gather_assets` (main.rs:1892-1928). |
| IDLE paint save | `resources/{item,block}/<stem>[_<slot>].png` | `ContentEditor.tsx:509-530`, `content.rs:148-181`. |

Animation exists only when a `.png.mcmeta` sits next to a `path.png`. IDLE detects animation by an "height is a multiple of width" heuristic and previews only frame 0.

### 1.2 Render layers, tint, emissive

- `render_layer` (`solid|cutout|cutout_mipped|translucent`) is written **only** as `render_type` into the model JSON (content.rs:1013-1016, 1061-1063).
- It never reaches `BlockProps`, `yog_registry::BlockDef`, `YogBlockDef` or `nativeBlockDefs` (yog-runtime/src/lib.rs:5217-5265).
- No Java host contains `BlockRenderLayerMap`, `ItemBlockRenderTypes` or `noOcclusion`.

Consequences:
- **Fabric (1.20.1, 1.21.1):** every block renders solid. Cutout pixels show black.
- **Forge/NeoForge:** `render_type` works. However, every block is a full occluding block, so full-cube cutout blocks (HexMod's `*_edified_leaves`) cull their neighbours' faces and show the void through transparent pixels.
- **Tint:** a face's `tint` becomes `tintindex`, but no color provider is registered anywhere, so the tint does nothing.
- **Emissive:** does not exist.

### 1.3 UI look today

- `render.rs` draws flat `bg` rects.
- `Button` has no visuals of its own.
- Focus is a 1px amber outline.
- The scrollbar is a gray rect.
- `TextBox` is a black box with a 1px border, a hand-made EditBox imitation.
- `InvSlot` is a hand-made bevel.

There is no Theme, no brush type beyond `u32`, and no CheckBox, Slider, Tabs, Tooltip or Panel frame. Vanilla textures only appear when a mod writes `sprite:button` into an `<Image>` itself. Modlist and HexMod hardcode dark translucent ARGB palettes (Modlist `#E0101010`, `#FF333333`, gold `#FFD700`; HexMod `#AA0B0B12`, `#CC14141C`, `#FF2A2A36`).

Known yog-ui bug that the rewrite touches: `font_scale` affects measurement only and text is always drawn at 1× (render.rs:140/158/255; the InvSlot count is measured at 0.7 and drawn at 1×).

### 1.4 Vanilla assets in IDLE

- `Yog-IDLE/src-tauri/src/designer/assets.rs` finds client jars in place. It checks `$YOG_MC_CLIENT_JAR`, the Loom / ForgeGradle / NeoGradle caches, `~/.minecraft/versions`, and the loader repos' `.gradle` caches, and picks the newest version.
- It finds asset indexes and reads textures, models and fonts lazily from the zip.
- Nothing is extracted, and the version can't be chosen.
- `content.rs` and the designer both use it.

---

## 2. Concepts and references

### 2.1 Texture

A **texture** is `textures/<path>.png` in a mod project, with id `<mod id>:<path>`. In the vanilla pack it is `textures/<path>.png` with id `minecraft:<path>`.

Optional metadata goes in `textures/<path>.png.mcmeta`. This is vanilla's format, used verbatim, plus one optional `yog` object:

```jsonc
// textures/block/impetus/look.png.mcmeta
{
  "animation": { "frametime": 2, "interpolate": true, "frames": [0, 1, 2, { "index": 3, "time": 6 }] },
  "texture":   { "blur": false, "clamp": false }
}
```

```jsonc
// textures/gui/rune_button.png.mcmeta
{
  "gui": { "scaling": { "type": "nine_slice", "width": 200, "height": 20, "border": { "left": 4, "top": 3, "right": 4, "bottom": 3 } } }
}
```

| Key | Meaning | Who honors it |
|---|---|---|
| `animation.*` | Frames are stacked vertically. `frametime` is in ticks, `interpolate`, `frames`, and `width`/`height` for non-square frames. | Minecraft for atlas textures (blocks, items, 1.20.2+ GUI sprites). The Yog runtime for standalone GUI textures (it picks the frame and draws the sub-rect; §5.4). IDLE previews it everywhere. |
| `texture.blur/clamp` | As vanilla. | Minecraft. |
| `gui.scaling` | `stretch` (default), `tile` (`width`/`height`), `nine_slice` (`width`/`height`, `border` as a number or `{left,top,right,bottom}`). | Minecraft 1.20.2+ for atlas sprites under `gui/sprites/`. The Yog runtime everywhere else, including 1.20.1. |
| `yog.size` | `[w, h]`: natural size in GUI pixels when it differs from the pixel size (for 2× hi-res GUI art). | The Yog runtime and IDLE. |

**Atlas placement follows usage, not the folder.** Mod textures sit in free-form folders under `content/` (§13), so `yog build` cannot rely on vanilla's "`block/` and `item/` go to the block atlas" directory rule. Instead:
- Every texture used by a block or item model gets a `{"type": "single", "resource": "<ns>:<path>"}` entry in a generated `assets/minecraft/atlases/blocks.json`. Atlas source files exist since 1.19.3, so this works on 1.20.1 and 1.21.1.
- The runtime's pack builder (`YogPackProvider`) **merges** the `sources` arrays of every mod's atlas files into the single `.yog-pack`, instead of letting the last mod win.
- Textures used only by UI are standalone textures. The Yog runtime slices them itself, so they behave the same on 1.20.1 and 1.21.1.
- Generated output never adds to the GUI sprite atlas. `gui/sprites/**` exists only on 1.20.2+, and the overlay only uses vanilla's own sprites from it. A mod that wants its own atlas sprites (1.20.2+ only, e.g. so resource packs can override them) still ships `atlases/gui.json` and the PNGs raw under `pack/assets/` and references them as plain texture ids under `gui/sprites/`, which §5.4 draws with `blitSprite` (capability rule).
- In the vanilla pack, ids keep vanilla's folders (`minecraft:block/stone`, `minecraft:gui/sprites/widget/button`), because those are vanilla's real paths.

`yog build` validates:
- The PNG decodes.
- The animation frame count divides the height.
- `nine_slice` borders fit inside the image.
- Every `.png.mcmeta` has a `.png`.

It compiles each texture's metadata into `yog/textures.json` inside the `.yog`: `{ id: { w, h, frames, frametime, interpolate, scaling, size } }`. The runtime then knows sizes and slicing without decoding PNGs. This replaces the PNG-IHDR read in `NativeDraw.textureSize`.

### 2.2 Material

A **material** says what to draw and how. It has one schema for every use. Each consumer reads the subset that makes sense for it and rejects the rest at build time (§2.4).

```jsonc
// materials/gui/rune_button.ymat           → id hexcasting:gui/rune_button
{
  "$schema": "https://yog.dev/schema/ymat-1.json",
  "description": "Rune-carved button",        // shown in IDLE pickers

  // ── what to draw ─────────────────────────────────────────────
  "texture": "hexcasting:gui/rune_button",    // shorthand for a single texture layer
  // or "layers": [ … ] — see below; exactly one of texture/layers/fill

  // ── world (blocks / items) ──────────────────────────────────
  "render_layer": "auto",                     // auto | solid | cutout | cutout_mipped | translucent
  "tint": "foliage",                          // none | "#RRGGBB" | grass | foliage | water | dye | custom
  "emissive": 0,                              // 0..15: block light the faces render with
  "uv": { "rect": [0, 0, 16, 16], "rotation": 0 },  // a sub-rect of the texture in pixels + 0/90/180/270

  // ── UI ─────────────────────────────────────────────────────
  "slice": 3,                                 // nine-slice border override: n | [top, right, bottom, left]
  "mode": "nine_slice",                       // stretch | tile | nine_slice (default: from the texture's gui.scaling)
  "region": [0, 0, 200, 20],                  // the part of the texture this material uses, in pixels
  "size": [200, 20],                          // natural size override in GUI px
  "text": { "color": "#FFFFFF", "shadow": true },  // foreground hint for content drawn on top (§6.3)
  "padding": [4, 4, 4, 4],                    // default content padding hint for controls using it

  // ── interaction states (UI only) ────────────────────────────
  "states": {
    "hover":    { "texture": "hexcasting:gui/rune_button_hover" },
    "focused":  { "inherit": "hover" },
    "pressed":  { "texture": "hexcasting:gui/rune_button_pressed" },
    "disabled": { "texture": "hexcasting:gui/rune_button_disabled", "text": { "color": "#A0A0A0" } }
  }
}
```

**Layers.** A UI material can stack layers, drawn bottom to top. Each layer is one of:

| Layer | Fields | Used for |
|---|---|---|
| `{ "texture": id, "region", "mode", "slice", "tint", "uv", "size" }` | as above | sprites, nine-slice frames, tiled backgrounds |
| `{ "fill": "#AARRGGBB" }` | | flat fills (vanilla 1.20.1 EditBox inner black) |
| `{ "gradient": ["#top", "#bottom"] }` | vertical | the screen dim, the tooltip background |
| `{ "frame": "#AARRGGBB" \| { "top", "right", "bottom", "left" }, "width": 1, "inset": 0 }` | per-side colors | vanilla bevels (slot), EditBox border, list selection outline |
| `{ "frame_gradient": ["#top", "#bottom"], "width": 1, "inset": 1 }` | | the vanilla tooltip border (1.20.1/1.21.1) |
| `{ "material": id }` | | reuse another material as a layer |

Layers are what let one material id mean the right thing on every version. On 1.21.1 `minecraft:gui/text_field` is a nine-slice sprite. On 1.20.1 it is `fill` + `frame`, which is exactly what vanilla 1.20.1 `EditBox` does with `fill()` calls (§7.2).

**States** are `hover`, `pressed`, `focused`, `selected`, `checked`, `disabled` and `invalid` (a failed validation, bit 6 `INVALID`, mvvm-v2 Q10), plus combinations joined with `+` (`"checked+hover"`). They map 1:1 to `yog_ui::StateFlags`.
- **Resolution:** among the declared states whose flags are all set on the widget, the one with the **most flags** wins. A tie goes to the one declared later.
- A state object is a partial material that patches the base. `"inherit": "<state>"` copies another state's patch.
- `disabled` beats everything else, because the host never sets hover on disabled widgets anyway.

**Tint sources:**
- `"#RRGGBB"` is constant.
- `grass`/`foliage`/`water` are biome colors for blocks. For items they use vanilla's default colors (`GrassColor.getDefaultColor()` etc.).
- `dye` is the item's dyed color component (1.21 `DyedItemColor`; 1.20.1 the `display.color` NBT).
- `custom` calls the mod back: `on_block_color` / `on_item_color`, which is the later stage S3b.

UI tint is any `#AARRGGBB` multiply.

### 2.3 References (one grammar, every format)

Every place that takes "something to draw" takes a **paint ref**:

| Form | Meaning | Allowed in |
|---|---|---|
| `<ns>:<path>` | a texture with default material settings (D3). The namespace is **mandatory** (D10). | everywhere |
| `mat:<ns>:<path>` | a material (`content/<path>.ymat` of mod `<ns>`, or the vanilla pack's / overlay's). | everywhere |
| `{ … }` (JSON) | an inline material, same schema as `.ymat` | `.yblock`, `.yitem`, `.ymat` layers |
| `#slot` | another texture slot of the same model | `.yblock`/`.yitem` models |
| `#RRGGBB` / `#AARRGGBB` | a flat color brush | `.yui` brush attributes, `.ymat` fills |
| `item:<ns>:<id>` / `block:<ns>:<id>` | the item renderer (3D block items, enchant glint). This is not a texture. | `.yui` `Image`, `ItemSlot` |
| `@Set.name` | a `.yres` color (brush attributes only) | `.yui` |

**Namespaces are always explicit (D10, the user's rule).** This covers every asset reference in `.yblock`, `.yitem`, `.ymat`, `.ytheme`, `.yui`, `.ybook`, `.ysound`, `yog.toml`, code-generated ids and `yog_api` string APIs: `minecraft:block/stone`, `hexcasting:block/slate`, `item:minecraft:diamond`, `mat:minecraft:gui/button`.
- A bare `block/stone` or `item:diamond` is an **error**. The diagnostic points at the value and offers the candidates that exist: *"`block/stone` has no namespace — did you mean `minecraft:block/stone` (vanilla) or `hexcasting:block/stone` (this mod)?"*. IDLE shows the same as a quick fix.
- **No short alias for the mod's own namespace.** The old `mod:` prefix is deleted, and no `~:`/`self:` replaces it. A short alias is exactly the implicit reference the user ruled out, and it would read differently when a file is copied into another mod. To keep this cheap: IDLE's completion and pickers always insert the full id, starting with the project's own namespace; New Item templates are written with the project's id filled in; and renaming the mod id is a project-wide refactoring in IDLE (`yog migrate rename-namespace <old> <new>` in the CLI).
- **Not asset references, and so unchanged:** `#slot` (a model-local name), `@Set.name` (a `.yres` entry of this mod; the set name is the scope), `{x:Bind …}`, and translation keys after `@` (`@gui.done`). These are keys, not ids.
- Rust code uses typed handles (`res::…`, `vanilla::…`), which always carry the namespace. `yog_api` functions taking `&str` ids reject a bare id with `Err`/a logged error instead of assuming `minecraft:`. `yog_gfx::image::qualify` is deleted.
- The format migration (§14) rewrites the bare refs of existing files. Every bare id there meant `minecraft:` under the old rules, so the rewrite is mechanical.

**Deleted with no replacement form:**
- `sprite:<name>`: it becomes `mat:minecraft:gui/<name>` (§7).
- `mod:<path>` and every other implicit-namespace form.
- `path.png` texture paths in content files.
- `.yres` `image` entries and `@Set.name` as an image.
- `yog_gfx::image::{SPRITES, Sprite, LegacyRegion, suggest_alias}`: the overlay (§3.3) holds that data now.

### 2.4 Where each material property applies

| Property | Block face | Item layer / face | UI brush |
|---|---|---|---|
| `texture` (single) | ✓ required | ✓ required | ✓ |
| `layers` (>1), `fill`, `gradient`, `frame*` | ✗ error | ✗ error | ✓ |
| `render_layer` | ✓ (per block, §4.2) | ✓ (block items follow the block; sprites ignore it) | ignored (warning) |
| `tint` source | ✓ | ✓ (sources other than the constant become their default colors) | only `#AARRGGBB` |
| `emissive` | ✓ | ✓ | ignored |
| `uv.rect`, `uv.rotation` | ✓ (applied to face UVs at build) | ✓ | `region` instead |
| `slice`, `mode`, `size`, `text`, `padding`, `states` | ignored (warning) | ignored | ✓ |

---

## 3. The Vanilla Pack

### 3.1 What it is

For each supported MC version `V` (the union of `<loader>/versions/<V>.properties`, today `1.20.1` and `1.21.1`), a directory in Yog's formats holds everything vanilla offers that mods and the IDE refer to:

```
<cache>/yog/vanilla/1.21.1/
  pack.json                     {"minecraft":"1.21.1","jar_sha1":"…","asset_index":"17",
                                 "tool":"yog-vanilla 0.1.0","overlay_hash":"…","created":"…"}
  textures/**.png(.mcmeta)      extracted verbatim from assets/minecraft/textures/
  textures.json                 index: id → {w,h,frames,scaling,atlas}  (same schema as a mod's yog/textures.json)
  materials/**.ymat             1) GUI materials: copied from the committed overlay (§3.3)
                                2) block/item materials: generated, one per block texture with a
                                   non-default layer/tint (e.g. minecraft:block/oak_leaves →
                                   cutout_mipped + foliage), from vanilla's model render types
                                   and Yog's committed tint table
  content/blocks/**.yblock      generated with yog-cli's resource-pack importer (models, blockstates
  content/items/**.yitem         → Yog content), names as "@minecraft:block.minecraft.stone"
  lang/<locale>.json            verbatim from the jar (en_us) and asset objects (others)
  font/**, models/**, blockstates/**   verbatim, for previews and the importer
  ids.json                      {"items":[…],"blocks":[…],"lang_keys":N,"textures":N} — what codegen reads
  reports/                      Mojang data-generator output (registries, block states, tags, commands); part of v1
```

`<cache>` is the platform cache dir: `$XDG_CACHE_HOME` or `~/.cache` on Linux, `~/Library/Caches` on macOS, `%LOCALAPPDATA%` on Windows. `YOG_VANILLA_DIR` overrides it.

"In Yog's formats" means:
- Textures are already in Yog's texture format (D1).
- Materials are `.ymat`.
- Blocks and items are `.yblock`/`.yitem`, read-only, as produced by `yog import resourcepack --all`, which already exists (`content/import.rs`).
- Strings stay vanilla lang JSON. A `.yres` would only re-wrap 6000+ keys per locale with explicit `key` fields and buy nothing.

Block properties such as hardness are not in the assets. The data generator's reports are therefore part of v1 (the user's decision). They supply:
- `registries.json`: every registry's exact ids (items, blocks, entity types, sound events, particle types, …). This is the source for `ids.json` and the typed handles.
- `blocks.json`: the state properties of each block. These become the `states` of the vanilla `.yblock` files.
- The vanilla tags in `data/minecraft/tags/**`, used by pickers (`#minecraft:logs`) and validation.

If reports can't be produced (no JDK for that version), `sync` falls back to asset-derived ids (from `models/item/*.json`, `blockstates/*.json` and lang keys), marks the pack `"reports": false`, and IDLE shows a warning. Codegen still works, but registry-only ids (sound events, entity types without models) are missing.

### 3.2 How it is produced

New crate **`yog-vanilla`** in `Yog-Mod-Loader/rust/crates/yog-vanilla` (library). It is used by yog-cli and by Yog-IDLE's backend:

1. **Locate the jar.** Move the logic of `Yog-IDLE/src-tauri/src/designer/assets.rs` (`client_jars`, `asset_objects`, `version_key`) here unchanged, with one change: the version is chosen explicitly, not "newest". The lookup order stays: `$YOG_MC_CLIENT_JAR`, Loom/ForgeGradle/NeoGradle caches, launcher dirs, the loader repos' `.gradle` caches.
2. **Automatic download** (approved by the user; `--offline` disables it). When no local jar exists, read Mojang's `piston-meta` `version_manifest_v2.json` → the version JSON → client jar, server jar (for reports) and asset index → objects, verifying sha1 at every step. This is what every launcher does. The files land in the user's cache only (`<cache>/yog/mojang/<version>/`). IDLE shows the progress in the status bar and the Output pane.
3. **Extract** textures, mcmeta, models, blockstates, fonts and lang. This is the default **`extract`** mode. In **`jar`** mode (see below) only the generated Yog files and indexes are written, and pixel data, models and lang are read lazily from the jar and asset objects. Then run the importer to produce `content/**`, copy and validate the overlay (every texture an overlay material names must exist in this jar, otherwise `sync` fails and names the material), generate block/item materials, and write `textures.json`/`ids.json`/`pack.json`.
4. **Reports (v1, on by default; `--no-reports` skips them):** run `java -DbundlerMainClass=net.minecraft.data.Main -jar server.jar --reports --server --output <tmp>` with that version's JDK. The JDK is chosen as `build.sh` does, from `java_version` in `<loader>/versions/<V>.properties`, never the default system Java 25. Then copy `reports/` and the generated `data/minecraft/tags/**` into the pack. This runs once per version and per jar sha1. It is the only step that launches Java, and both IDLE and the CLI announce it before running it.

Staleness: `pack.json` records the jar sha1, the tool version, the overlay hash, the mode and whether reports are present. `yog build`/IDLE re-sync automatically when any of them changes. The commands are `yog vanilla sync [--mc V…] [--offline] [--no-reports] [--mode extract|jar]`, `yog vanilla status`, and `yog vanilla path V`.

**Storage mode.** This is an environment setting, not a project setting, because it concerns the machine and not the mod:

| | `extract` (default) | `jar` |
|---|---|---|
| Disk | ~60–90 MB per version (textures, models, lang for all locales) | indexes and generated Yog files only (~5 MB) |
| Speed | files read directly; Project Explorer browses real files | every texture/model/lang read decompresses from the jar or asset objects; first previews and pickers are noticeably slower |
| Set by | IDLE **Settings › Environment › Minecraft assets**: *Extract to cache (recommended)* / *Read from the game jar (saves disk space, slower)*. The second option carries the note: *"Previews, pickers and the designer will be slower: every texture is decompressed from the jar on first use."* | CLI: `YOG_VANILLA_MODE=jar`, or `[vanilla] mode = "jar"` in the user config `~/.config/yog/config.toml` |

Both modes go through the same `yog-vanilla` `Pack` API (`read(id)`, `list(kind)`), so tools never know which one is active. In `jar` mode the Project Explorer's *Minecraft* node is a virtual tree served by that API.

**Why generate locally instead of shipping:**
- Mojang's EULA and brand guidelines forbid redistributing their assets. Strings are their text too.
- Generating on the user's machine from their own jar is the same position as launchers and IDLE's current in-place reading.
- A pack cached on disk (rather than read lazily from the zip as today) gives the IDE browsable files, stable paths for the Project Explorer, and a single place the CLI, IDLE and the designer agree on. The lazy `jar` mode stays available for users short on disk.

### 3.3 What is committed: `Yog-Mod-Loader/vanilla/`

```
vanilla/
  versions.toml          # every supported MC version + its family
  overlay/
    common/materials/gui/*.ymat        # identical on all versions (panel, slot, tooltip, dim, list selection)
    1.20.1/materials/gui/*.ymat        # widgets.png / checkbox.png / slider.png / tab_button.png regions + procedural layers
    1.21.1/materials/gui/*.ymat        # gui/sprites/** atlas sprites
    tints.toml                         # vanilla block → tint source (grass_block: grass, oak_leaves: foliage, water: water, …)
  renames.toml           # id renames between supported versions
```

```toml
# vanilla/versions.toml
[versions."1.20.1"]
gui = "legacy"          # no GUI sprite atlas
overlay = ["common", "1.20.1"]
[versions."1.21.1"]
gui = "sprites"
overlay = ["common", "1.21.1"]
```

```toml
# vanilla/renames.toml — old id → new id, per kind, with the version that renamed it
[item]
"minecraft:scute" = { to = "minecraft:turtle_scute", since = "1.20.5" }
[block]
"minecraft:grass" = { to = "minecraft:short_grass", since = "1.20.3" }
[texture]
"minecraft:item/scute" = { to = "minecraft:item/turtle_scute", since = "1.20.5" }
```

A unit test in `yog-vanilla` checks that every version in `<loader>/versions/*.properties` has a `versions.toml` entry. Adding an MC version to the loader therefore forces writing its overlay. `yog vanilla sync --check-overlay V` validates the overlay against the jar, so overlay authoring is never guesswork.

The overlay is also **embedded in yog-runtime** (`include_str!` per version family). The game has the pixels; the runtime only needs the ids, regions and layers to resolve `mat:minecraft:gui/*`.

### 3.4 Version selection

- **`yog.toml`** gets `[mod] minecraft = ["1.20.1", "1.21.1"]`, the versions the mod supports. The default is every version the installed yog-cli knows (`versions.toml`). Run targets (`[run.fabric-1.21.1]`) must be in that list.
- **Codegen** (§3.5) reads the packs of every listed version.
- **IDLE** takes the Minecraft version from the **selected run configuration** (the existing run target selector, one per `[run.<loader>-<V>]`; the user's decision). No separate version selector exists. Previews (designer, content editor, material and theme editors), pickers and the Project Explorer's *Minecraft* node use that version's pack. Switching the run configuration to `fabric-1.20.1` re-renders every open preview with 1.20.1's GUI. The status bar shows the active version next to the run configuration. With no run configuration, IDLE uses the newest version in `[mod] minecraft`.
- **Runtime:** a new ABI query `host_mc_version() -> YogStr` (backed by `SharedConstants.getCurrentVersion().getName()` in each host) selects the overlay and the rename direction. Nothing exposes the version today.

### 3.5 Typed vanilla handles

`yog_api::vanilla!();` in `lib.rs` includes `.yog/build/generated/vanilla.g.rs` as `crate::vanilla`:

```rust
// .yog/build/generated/vanilla.g.rs  (generated from the 1.20.1 + 1.21.1 packs)
pub mod items {
    use ::yog_api::ItemId;
    /// `minecraft:diamond` — in every targeted version.
    pub const DIAMOND: ItemId = ItemId::new("minecraft:diamond");
    /// `minecraft:turtle_scute` (1.21.1) / `minecraft:scute` (1.20.1) — renamed in 1.20.5;
    /// the runtime maps it to the running version's id.
    pub const TURTLE_SCUTE: ItemId = ItemId::renamed("minecraft:turtle_scute");
    /// `minecraft:mace` — only in 1.21.1 of the targeted versions.
    pub fn mace() -> Option<ItemId> { ::yog_api::vanilla_since("1.21", ItemId::new("minecraft:mace")) }
}
pub mod blocks   { /* BlockId, same rules */ }
pub mod textures {
    pub mod block { pub const STONE: ::yog_api::Texture = ::yog_api::Texture::new("minecraft:block/stone"); }
    pub mod gui   { /* standalone gui textures common to all targets */ }
}
pub mod materials {
    pub mod gui {
        /// Vanilla button: 1.20.1 widgets.png regions, 1.21.1 `widget/button*` sprites.
        pub const BUTTON: ::yog_api::Material = ::yog_api::Material::new("minecraft:gui/button");
        pub const TEXT_FIELD: ::yog_api::Material = /* … */;
    }
}
pub mod lang {
    /// "Done" — `gui.done`.
    pub fn gui_done() -> String { ::yog_api::tr("gui.done") }
    pub const GUI_DONE_KEY: &str = "gui.done";
}
```

Rules:
- An id present in every targeted version gives a `const`.
- An id present in some versions gives a `fn … -> Option<…>`.
- A rename in `renames.toml` gives one constant under the **newest** name, flagged `renamed`. `ItemId`/`BlockId`/`Texture` handles carry the flag, and the runtime maps the id to the running version once, at registry lookup.
- Name collisions (`oak_planks` vs `oak_planks`) can't happen inside a kind. Rust keywords get an `r#` prefix.

Lang keys are many (~6k per version). They are generated as `const …_KEY` plus a `fn` each. If compile time suffers (§16, R4), lang moves behind `[vanilla] lang = true` in `yog.toml`.

In markup and JSON there are no handles. Plain, fully namespaced ids are already the cross-format reference: `item:minecraft:diamond`, `mat:minecraft:gui/button`, `minecraft:block/stone`, `@gui.done`. `yog build` validates them against the packs of every targeted version, with the same Option/rename rules (a warning for "missing in 1.20.1", an error for "missing everywhere").

The mod's own handles come from `resources!()`:

```rust
// .yog/build/generated/resources/textures.g.rs   (part of crate::res)
pub mod textures { pub mod block { pub const SLATE: ::yog_api::Texture = ::yog_api::Texture::new("hexcasting:block/slate"); } }
pub mod materials { pub mod gui { pub const RUNE_BUTTON: ::yog_api::Material = ::yog_api::Material::new("hexcasting:gui/rune_button"); } }
```

`Texture` and `Material` are `Copy` newtypes over `&'static str` (plus a flags byte), defined in **yog-gfx** and re-exported by yog-api and yog-ui. `impl From<Texture> for Material` is the implicit default material (D3). `impl From<Material> for Brush` and `impl From<u32> for Brush` are in yog-ui (§6.1).

---

## 4. Blocks and items

### 4.1 Format changes

```jsonc
// content/blocks/amethyst/citrine_edified_leaves.yblock
{
  "name": "@Blocks.citrine_edified_leaves",
  "behavior": "minecraft:leaves",                                     // §4.6: decay, distance states, no occlusion
  "block": { "hardness": 0.2, "sound": "grass", "tool": "hoe" },     // render_layer is gone from `block`
  "model": {
    "parent": "cube_all",
    "textures": {
      "all": { "texture": "hexcasting:block/citrine_edified_leaves", "render_layer": "cutout_mipped", "tint": "foliage" }
    }
  }
}
```

```jsonc
// content/blocks/slate/slate.yblock (elements model) — slots take any paint ref
"textures": { "0": "hexcasting:block/slate", "1": "mat:hexcasting:block/slate_glow" }
```

```jsonc
// content/items/staff/oak.yitem
"model": { "parent": "handheld", "textures": { "layer0": "hexcasting:item/staff/oak" } }
```

```jsonc
// content/items/pigments/dyed.yitem — tinted layer
"textures": { "layer0": "hexcasting:item/colorizer/base",
              "layer1": { "texture": "hexcasting:item/colorizer/overlay", "tint": "dye" } }
```

**Deleted:**
- `block.render_layer`.
- Face `tint` (an integer index). The tint index is now **derived**: the build numbers the distinct tint sources of a model 0..n and writes `tintindex` on the faces whose material has that source.
- `path.png` and `@Set.name` texture forms.

### 4.2 Compilation (yog-cli `content.rs`)

1. Resolve each slot to a material, in this order: inline → `mat:` file → texture default. Validate it against §2.4.
2. **Model JSON:**
   - `textures[slot]` = the material's texture id.
   - Faces using the slot get `tintindex` (derived), `uv` remapped through `uv.rect`/`uv.rotation`, and emissive (§4.4).
3. **Block layer:**
   - Per block, take the most transparent layer of all its materials: `translucent > cutout_mipped > cutout > solid`.
   - `auto` resolves per texture by its alpha (D4). The result is cached in `.yog/cache/texture-alpha.json` by PNG hash.
   - Mixed explicit layers give a warning ("face X is solid but the block renders cutout").
   - Write `render_type` into the model JSON (Forge/NeoForge) **and** `BlockProps.render_layer` (new) into the generated `BlockDef`.
4. **Tints:** `BlockProps.tints: [TintSource; n]` indexed by tintindex, and the same for items.
5. **Animated textures:** nothing to do. The `.mcmeta` travels with the texture (§5.1).

### 4.3 Registry/ABI/host (fixes the Fabric bug)

```rust
// yog-registry BlockDef (builder) — usable by code-registered blocks too (Yog-Pipes)
BlockDef::new("yog-pipes:glass_pipe").behavior("minecraft:transparent").render_layer(RenderLayer::Cutout).tint(0, TintSource::Constant(0x88CCFF))
ItemDef::new("hexcasting:dyed_pigment").tint(1, TintSource::Dye)
```

```rust
// yog-abi YogBlockDef / YogItemDef — appended fields (ABI_MINOR bump)
pub render_layer: u8,                 // 0 solid, 1 cutout, 2 cutout_mipped, 3 translucent
pub tint_count: u8,
pub tints: *const YogTintSource,      // { kind: u8 (0 const,1 grass,2 foliage,3 water,4 dye,5 custom), rgb: u32 }
pub emissive: u8,                     // max over faces, for the host-side fallback (§4.4)
pub behavior: *const c_char,          // namespaced behaviour id (§4.6), never null ("minecraft:solid")
pub behavior_args: *const c_char,     // JSON object of behaviour parameters ("{}" when none)
```

The runtime adds `render_layer`, `tints` and `emissive` to `nativeBlockDefs`/`nativeItemDefs` (yog-runtime/src/lib.rs:5217-5265).

Java hosts:

| Host | Render layer | Occlusion | Tint |
|---|---|---|---|
| Fabric 1.20.1/1.21.1 | `YogClient.onInitializeClient`: `BlockRenderLayerMap.INSTANCE.putBlock(block, RenderLayer.getCutout()…)` for every Yog block with layer ≠ solid. | From the block's **behaviour** (§4.6). The block class/properties of `minecraft:transparent`, `minecraft:leaves`, … set it, in `YogHost` block registration (1.21.1:554-590, 1.20.1:534-570). | `ColorProviderRegistry.BLOCK/ITEM.register(…)`: a source switch → `BiomeColors.getGrassColor/getFoliageColor/getWaterColor(world, pos)` with vanilla defaults when world/pos are null, constants, `DyedColorComponent` (1.21) / `display.color` (1.20.1), and `custom` → `nativeBlockColor(...)`. |
| NeoForge 1.20.1/1.21.1 | model `render_type` (already written) | from the behaviour (§4.6), `YogHost` (245-281) | `RegisterColorHandlersEvent.Block/Item` |
| Forge 1.20.1/1.21.1 | model `render_type` | from the behaviour (§4.6), (246-286) | `RegisterColorHandlersEvent.Block/Item` |

Occlusion, view blocking, suffocation, light passing and same-block face culling are **block logic**. They come from the block's behaviour (§4.6), never from the material. A material only decides how faces are drawn. A safety net remains: a block with the default `minecraft:solid` behaviour whose resolved layer is not `solid` makes the build fail with *"`ruby_glass` renders cutout but behaves as a solid block (neighbours' faces would be culled): set `behavior`, e.g. `minecraft:transparent`"*. Without that error, today's x-ray bug would come back. The error applies only when `behavior` is omitted: an explicit `"behavior": "minecraft:solid"` (or `BlockDef::behavior("minecraft:solid")`) declares that the block really occludes, which is what vanilla's grass block does with its `cutout_mipped` overlay (capability rule: a cutout-rendered full occluding cube stays possible).

### 4.4 Emissive

`emissive: n` makes faces render at block light ≥ n regardless of world light (glowing runes on HexMod slate/impetus).
- **Forge/NeoForge 1.20.1+:** element-level `neoforge_data`/`forge_data` `{ "block_light": n }` in the model JSON. The build splits elements per face material when needed.
- **Vanilla 1.21.x:** check during implementation whether element `light_emission` exists in 1.21.1's model format. If it does, write it too.
- **Fabric:** needs a Fabric Renderer API baked-model wrapper (`MaterialFinder.emissive(true)` / disable diffuse+AO) applied to quads with the marked tint/sprite. This is real work and is scheduled as **S3b**. Until it lands, Fabric ignores emissive and the build prints one info line.

### 4.5 Items rendering the same material in UI

A UI `Image source="hexcasting:item/staff/oak"` draws the texture. `item:hexcasting:staff/oak` draws the item through the item renderer, with tint, 3D and glint. Both keep working. The difference is now explicit in the ref grammar.

---

### 4.6 Block behaviours (Minecraft's block classes)

In Minecraft, how a block interacts with its neighbours and with light is decided by its **class**:
- `TransparentBlock` (1.20.3+; `AbstractGlassBlock`/`GlassBlock` on 1.20.1) skips faces against the same block (`HalfTransparentBlock#skipRendering`), passes skylight, has shade brightness 1, and is not view-blocking, suffocating or a redstone conductor.
- `StainedGlassBlock` adds a beacon beam color.
- `IceBlock` melts.
- `LeavesBlock` has `distance`/`persistent` states and decays.
- And so on.

Yog models exactly that. A `.yblock` names its vanilla-equivalent behaviour, **always namespaced** (D10):

```jsonc
"behavior": "minecraft:transparent"                                         // glass
"behavior": { "type": "minecraft:stained_glass", "color": "minecraft:purple" }
"behavior": "hexcasting:slate"                                              // mod-defined, see below
```

Omitted means `minecraft:solid` (plain `Block`).

**Built-in behaviours (v1):**

| Behaviour | 1.21.1 class | 1.20.1 class | Brings |
|---|---|---|---|
| `minecraft:solid` | `Block` | `Block` | full occlusion |
| `minecraft:half_transparent` | `HalfTransparentBlock` | `HalfTransparentBlock` | same-block face skip only |
| `minecraft:transparent` | `TransparentBlock` | `GlassBlock` (`AbstractGlassBlock`) | glass: face skip, `noOcclusion`, skylight passes, not view-blocking/suffocating/conductor |
| `minecraft:stained_glass` (`color`) | `StainedGlassBlock` | `StainedGlassBlock` | glass + beacon beam color |
| `minecraft:tinted_glass` | `TintedGlassBlock` | `TintedGlassBlock` | glass that blocks light |
| `minecraft:ice` | `IceBlock` | `IceBlock` | friction 0.98, melts to water near light |
| `minecraft:leaves` | `LeavesBlock` | `LeavesBlock` | `distance`/`persistent`/`waterlogged` states, decay, `noOcclusion`, not suffocating |

More behaviours are added the same way, each a row with its host classes: `minecraft:slab`, `stairs`, `fence`, `wall`, `pane`, `door`, `trapdoor`. They would remove the "no slabs/stairs/doors" limitation in blocks-items.md, but they are not part of this effort.

**State properties.** A behaviour that brings block-state properties (such as `leaves`' `distance`, `persistent` and `waterlogged`) contributes them to the generated blockstate JSON. The model is the same for every value unless `states` says otherwise. `yog content check` knows each behaviour's properties from the vanilla pack's `reports/blocks.json` of each targeted version.

**Hosts.** Each Java host has one thin subclass per behaviour, `YogTransparentBlock extends TransparentBlock` (1.20.1: `extends GlassBlock`), `YogLeavesBlock extends LeavesBlock`, …. They share a `YogBlockTraits` helper for what Yog adds on top: shape override (today `YogShapedBlock`), `connect` states, and inventory. `YogHost` picks the class from `YogBlockDef.behavior` (a new ABI field, the namespaced id, plus `behavior_args` as JSON for parameters like `color`). An unknown `minecraft:` behaviour is a build error, so the host never guesses.

**Mod-defined behaviours (the generic extension point).** The loader contains no mod logic. A mod registers a behaviour in code, and blocks reference it by id:

```rust
// HexMod: src/blocks/slate.rs
registry.block_behavior(BlockBehavior::new("hexcasting:slate")
    .extends("minecraft:half_transparent")          // the vanilla class the host instantiates
    .skip_rendering(SkipRule::SameBehavior)         // declarative, evaluated in Java (render thread)
    .state(BoolProp::new("energized"))
    .on_use(|cx| slate::use_slate(cx))              // Rust callbacks through the existing event ABI
    .on_neighbor_changed(|cx| slate::update(cx)));
```

- `extends` names a built-in behaviour; the host instantiates that behaviour's class. The callbacks (`on_use`, `on_neighbor_changed`, `random_tick`, `on_placed`, `on_removed`) go through one generic `YogBlockBehaviorVTable` in the ABI. This is the same pattern as item `on_use`.
- Rules evaluated per frame or per chunk mesh (`skip_rendering`, light passing, occlusion) are **declarative** (`SkipRule::{SameBlock, SameBehavior, Tag(…)}`) and evaluated in Java. A native call per face per chunk rebuild would be too slow.
- The `.yblock` validator checks at build time that the id exists, either built-in or declared in the mod's code. `yog content check` finds the declarations through the generated registration code (`content!()` already sees the crate).

A material and a behaviour are checked against each other, not merged: `minecraft:transparent` with a resolved `solid` layer gives a warning (vanilla glass is cutout), and a non-solid layer with the implicit `minecraft:solid` gives an error, an explicit one is accepted (§4.3).

## 5. Runtime and ABI for UI drawing

### 5.1 What a `.yog` carries

- `assets/<ns>/textures/<path>.png(.mcmeta)`: copied from `content/<path>.png(.mcmeta)`, so the id `<ns>:<path>` is also vanilla's resource location.
- `assets/minecraft/atlases/blocks.json`: `single` sources for the textures used by models (§2.1). The runtime merges these across mods.
- `yog/textures.json`: compiled texture metadata (§2.1).
- `yog/materials.json`: every material of the mod, **fully resolved**. Inline materials from `.yblock`/`.yui` are hoisted under synthetic ids `<ns>:__inline/<file>/<n>`. Refs are checked, states are flattened per combination, and layers are expanded to absolute texture regions.

### 5.2 MaterialRegistry (yog-runtime)

- On load: the overlay for `host_mc_version()` (embedded), then each mod's `yog/materials.json` and `yog/textures.json`. Hot reload replaces one mod's entries.
- `resolve(id, state) -> &[ResolvedLayer]`, cached per `(id, state)`.
- Unknown ids draw the magenta/black missing texture (what IDLE already does) and log once.

### 5.3 ABI (YogGfxApi, ABI_MINOR bump)

```rust
/// Draws material `id` (`mat:` prefix optional; a texture id = its default material) into
/// (x,y,w,h) for interaction `state` (StateFlags bits), multiplied by `tint` (0xAARRGGBB).
pub draw2d_material: unsafe extern "C" fn(id: YogStr, x: f32, y: f32, w: f32, h: f32, state: u8, tint: u32),
/// Natural size + content hints of material `id` in `state`; false if unknown.
pub material_info: unsafe extern "C" fn(id: YogStr, state: u8, out: *mut YogMaterialInfo) -> bool,
//  YogMaterialInfo { w: f32, h: f32, text_color: u32, text_shadow: u8, pad: [f32; 4], has_text: u8 }
/// Vanilla tooltip at (x,y): lines separated by '\n', § codes allowed. Drawn by the game
/// (GuiGraphics.renderTooltip / DrawContext.drawTooltip) → exact look on every version.
pub draw2d_tooltip: unsafe extern "C" fn(text: YogStr, x: f32, y: f32),
/// The item's vanilla tooltip (name, lore, advanced info) at (x,y).
pub draw2d_item_tooltip: unsafe extern "C" fn(id: YogStr, count: u32, x: f32, y: f32),
/// Draws `text` at (x,y) scaled by `scale` (fixes font_scale being ignored).
pub draw2d_text_scaled: unsafe extern "C" fn(text: YogStr, font: YogStr, x: f32, y: f32, scale: f32, color: u32, shadow: bool),
/// Running Minecraft version, e.g. "1.21.1".
pub host_mc_version: unsafe extern "C" fn() -> YogStr,
```

`draw2d_image`/`image_size` stay for `item:`/`block:` and plain textures. `Draw2D::image(id)` with a texture id now routes through `draw2d_material`. `sprite:` handling is removed from `yog-runtime/src/images.rs`.

### 5.4 Layer drawing in the runtime

The runtime does all the geometry in Rust and calls three existing Java primitives: `drawImage` (uv, ×64 sub-texel), `drawRect` and `drawGradientRect`, plus `drawGuiSprite` for atlas sprites.

| Layer | 1.21.1 (sprites) | 1.20.1 (legacy) |
|---|---|---|
| texture under `gui/sprites/` | `blitSprite`/`drawGuiTexture`. Vanilla applies the sprite's own `gui.scaling`, **including resource-pack overrides**. | n/a: the overlay never uses such ids on 1.20.1 (validated). |
| other texture, `stretch` | one `drawImage` | same |
| `nine_slice` | 9 `drawImage` (corners at natural size, edges/centre stretched or tiled per `gui.scaling`). This generalizes `draw_legacy` in images.rs:190-225. | same |
| `tile` | `drawImage` repeats, with the last row/column cut | same |
| animated standalone texture | frame = `(game_time / frametime) % frames`. `interpolate` draws two frames with alphas. | same |
| `fill`/`frame`/`gradient`/`frame_gradient` | `drawRect`/`drawGradientRect` | same |

Clipping stays as yog-ui does it (rect intersection plus UV adjustment) and is applied before the call. The material draw takes the clip rect: `draw2d_material` gains `clip: *const [f32; 4]` (null = none), so nine-slices are cut correctly in scroll views.

---

## 6. yog-ui: brushes, theme, controls

### 6.1 Brushes

```rust
// yog-ui
pub enum Brush { None, Color(u32), Material(Material) }
pub struct Style { pub bg: Brush, /* … */ pub color: TextColor, /* … */ }
pub enum TextColor { Inherit, Color(u32) }
```

- `bg="#AARRGGBB"`, `bg="@Set.color"`, `bg="mat:hexcasting:gui/rune_panel"` and `bg="hexcasting:gui/parchment"` all work, both in `.yui` and as `.bg(…)` in builders.
- `StylePatch.bg` becomes `Option<Brush>`, so `hover:bg="mat:…"` works. A material brush receives the widget's `StateFlags` automatically, so most state attributes become unnecessary.
- `Image source` takes a paint ref. `stretch`/`uv`/`tint` stay for plain textures. For materials, the material's own `mode`/`region` apply.
- The only brush attributes in markup are `bg` and `source`. One brush per widget keeps the model simple: decoration that needs more than one layer (a border plus a fill, say) is a material with layers. There are no separate `BorderBrush`/`BorderThickness` attributes.

### 6.2 The built-in vanilla theme

Each built-in control has a **default style** written in terms of vanilla material ids. That set of defaults is itself a theme, `minecraft:vanilla.ytheme` (§6.6), embedded in yog-ui and yog-cli. Codegen inlines it like any other theme. For builder code it is also compiled to Rust consts:

```rust
// yog-ui/src/theme.rs  — generated from minecraft:vanilla.ytheme (D6)
pub mod vanilla {
    use yog_gfx::Material as M;
    pub const BUTTON: M          = M::new("minecraft:gui/button");
    pub const TEXT_FIELD: M      = M::new("minecraft:gui/text_field");
    pub const CHECKBOX: M        = M::new("minecraft:gui/checkbox");
    pub const SLIDER: M          = M::new("minecraft:gui/slider");
    pub const SLIDER_HANDLE: M   = M::new("minecraft:gui/slider_handle");
    pub const SCROLLER: M        = M::new("minecraft:gui/scroller");
    pub const SCROLLER_TRACK: M  = M::new("minecraft:gui/scroller_background");
    pub const TAB: M             = M::new("minecraft:gui/tab");
    pub const PANEL: M           = M::new("minecraft:gui/panel");
    pub const INSET: M           = M::new("minecraft:gui/inset");
    pub const SLOT: M            = M::new("minecraft:gui/slot");
    pub const SLOT_HIGHLIGHT: M  = M::new("minecraft:gui/slot_highlight");
    pub const LIST_ENTRY: M      = M::new("minecraft:gui/list_entry");
    pub const LIST_BACKGROUND: M = M::new("minecraft:gui/list_background");
    pub const HEADER_SEPARATOR: M= M::new("minecraft:gui/header_separator");
    pub const FOOTER_SEPARATOR: M= M::new("minecraft:gui/footer_separator");
    pub const TOOLTIP: M         = M::new("minecraft:gui/tooltip");
    pub const DIM: M             = M::new("minecraft:gui/dim");
    pub const WHITE: u32 = 0xFFFFFFFF;  pub const GRAY: u32 = 0xFFA0A0A0;   // vanilla secondary / disabled text
    pub const PANEL_TEXT: u32 = 0xFF404040;                                  // container titles, no shadow
    pub const EDIT_TEXT: u32 = 0xFFE0E0E0;  pub const EDIT_DISABLED: u32 = 0xFF707070;
}
```

In `.yui` the same set is the built-in resource set **`Theme`** (already anticipated by `based_on="@Theme.Button"` in yog-kit.md §5):
- Styles: `@Theme.Button`, `@Theme.Title` (white, shadow, centered), `@Theme.PanelTitle` (#404040, no shadow), `@Theme.Secondary` (#A0A0A0), `@Theme.Error` (§c #FF5555), `@Theme.Success` (§a #55FF55).
- Materials: `@Theme.Panel`, `@Theme.Inset`, ….

A mod restyles a control for one screen with `<Style target="Button" bg="mat:hexcasting:gui/rune_button"/>` in `<Screen.Resources>`. Mod-wide, per-screen and per-subtree restyles are `.ytheme` files (§6.6).

### 6.3 Text color inheritance

`Style.color` defaults to `Inherit`. While rendering, each widget whose brush is a material with a `text` hint sets the inherited text color and shadow for its subtree.
- `minecraft:gui/panel` → #404040 without shadow, so a `Label` on a panel looks like a vanilla container title with no attributes.
- `minecraft:gui/button` → white with shadow, `disabled` → #A0A0A0.
- `minecraft:gui/text_field` → #E0E0E0.

The screen root's inherited color is white with shadow (vanilla screen text over a dimmed background). This replaces the `0xFF_CCCCAA` default and the frequent `shadow="false"` in today's mods.

### 6.4 Controls (vanilla mapping)

Sizes are vanilla's defaults. The material column shows what the overlay resolves to on each version; ids marked † must be confirmed by `yog vanilla sync --check-overlay` when the overlay is written.

| `.yui` element | Default style | Material on 1.21.1 | Material on 1.20.1 | Status |
|---|---|---|---|---|
| `Button` | h 20, text white + shadow, centered, ellipsis | `widget/button`, `_highlighted` (hover/focused), `_disabled`; nine-slice from the sprite's mcmeta | `gui/widgets.png` v=66/86/46, 200×20, slice 3 (today's `SPRITES` data) | restyled |
| `ToggleButton` | Button; `checked` = the hovered look (vanilla's "selected" convention for toggles); `checked+disabled` → disabled | same | same | restyled |
| `TextBox` | h 20, text #E0E0E0, placeholder #707070 | `widget/text_field`, `widget/text_field_highlighted` (focused) | `fill #FF000000` + `frame #FFA0A0A0` (focused `#FFFFFFFF`), as 1.20.1 `EditBox.renderWidget` | restyled |
| `CheckBox` **new** | 20×20 box + label at x+24, white + shadow (1.20.1: #E0E0E0) | `widget/checkbox`, `_selected`, `_highlighted`, `_selected_highlighted` | `gui/checkbox.png` (64×64): u 0/20 = unchecked/checked, v 0/20 = normal/focused | new |
| `Slider` **new** (`value` 0..1, `text`, two-way) | h 20 | track `widget/slider` (+`_highlighted`), handle `widget/slider_handle` (+`_highlighted`) 8×20 † | `gui/slider.png` regions † (1.20.1's `AbstractSliderButton` atlas) | new |
| `ScrollView`/`ListView` scrollbar | width 6 | `widget/scroller` knob on `widget/scroller_background` † | procedural: track `fill #FF000000`; knob `fill #FF808080` + `frame` bottom/right `#FFC0C0C0`-style highlight, as 1.20.1 `AbstractSelectionList` | restyled |
| `ListView` entries | `selected` → `minecraft:gui/list_entry` (selected): 1px outline #FFFFFFFF when the list is focused, #FF808080 otherwise, inner `#FF000000` | same (procedural in both) | same | restyled |
| `ListView background="list"` | `minecraft:gui/list_background` | `textures/gui/menu_list_background.png` tiled, plus `inworld_` variant when in-world † | `textures/gui/options_background.png` tiled, tint #FF404040 (the dirt) | new attribute |
| `Separator` **new** (`kind="header\|footer"`) | h 2 | `textures/gui/header_separator.png` / `footer_separator.png` † tiled | `frame` top #FF000000 (1.20.1 has none; a thin line) | new |
| `TabBar`/`Tab` **new** | tabs 130×24 max, text white | `widget/tab`, `_selected`, `_highlighted`, `_selected_highlighted` † | `gui/tab_button.png` regions † | new |
| `Panel` **new** (Column with `bg=@Theme.Panel`, padding 7) | inherited text #404040, no shadow | `textures/gui/demo_background.png` 248×166 nine-slice, border 4 | same texture | new |
| `Inset` (style) | sunken area for lists/previews in panels | `frame {top/left #FF373737, bottom/right #FFFFFFFF}` + fill #FF8B8B8B (the slot bevel at panel scale) | same | new |
| `InvSlot`/`ItemSlot` | 18×18 | `minecraft:gui/slot`: `fill #FF8B8B8B` + bevel frame (procedural on both); hover overlay `slot_highlight` `fill #80FFFFFF` over the item; count drawn at full scale like vanilla, white + shadow; hover shows `draw2d_item_tooltip` | same | restyled |
| `tooltip="…"` attribute **new** on any widget | after the frame, on hover | `draw2d_tooltip` (vanilla draws it: procedural bg #F0100010 + gradient frame #505000FF→#5028007F on 1.20.1/1.21.1, sprites on newer) | same | new |
| `Label` | inherits color/shadow | | | restyled |
| Screen `background` **new** attribute | `auto` (default) = vanilla `renderBackground` (in-world dim gradient #C0101010→#D0101010, plus blur on 1.21; title screen: panorama / dirt); `none` for overlay-like screens; `dim` = only the gradient | | | new |
| Screen `title` **new** attribute | drawn by the host the vanilla way: centered, y = 15 (lists) or panel-relative, white + shadow; also narrated | `Screen.getTitle()` | same | new |

Focus: vanilla draws focus only through the `_highlighted` sprite (buttons, fields) or a white outline (lists). The amber `FocusStyle::Outline` default is deleted. `focus_style` stays, with default `none`, because materials show focus. `focus_color: u32` is replaced by a `focus` brush (theme property or attribute; a color or a material) that `focus_style="outline"`/`"fill"` use, so a custom-colored focus outline or fill stays possible (capability rule).

`Slot`-like hover, `pressed` and tooltip timing need `move` events, which yog-kit already emits.

### 6.5 Deleted from yog-ui

`TRACK_COLOR`/`KNOB_COLOR`, `TEXT_BOX_BORDER`/`TEXT_BOX_FOCUS_BORDER`, the amber focus default, `Style.bg: u32`, `focus_color` (→ `focus` brush, §6.4), the hand-drawn slot rects in render.rs, and `image::whole_only` for sprites (sprites are gone).

Only the *built-in* flat look is deleted (Q4), not the ability to have one: every constant above is an ordinary theme value, so a mod can reproduce the old flat look as a `.ytheme` on `yog:bare` with color brushes (capability rule).

### 6.6 Themes (`.ytheme`)

A theme is a style sheet: control defaults plus named classes, attachable at any level of the UI. It is a `.ytheme` file anywhere under `content/`, with id `<mod>:<path under content/>`.

```jsonc
// content/ui/themes/slate.ytheme   → hexcasting:ui/themes/slate
{
  "based_on": "yog:bare",                  // default "minecraft:vanilla"; "yog:bare" = no decoration at all
  "resources": {                           // theme-local materials/colors, referenced as @name inside the theme
    "ink": "#FF2B1D40",
    "rune_panel": { "texture": "hexcasting:gui/scroll", "mode": "nine_slice", "slice": 6 }
  },
  "styles": {
    "Button":              { "bg": "mat:hexcasting:gui/rune_button", "color": "@ink", "shadow": false, "h": 18 },
    "Button:disabled":     { "color": "#FF6B5D80" },
    ".title":              { "color": "#FF3A2A55", "font_scale": 1.25 },
    "Label.hint":          { "color": "#FF8070A0" },
    "ListView":            { "background": "none", "bg": "@rune_panel" },
    "Panel":               { "bg": "@rune_panel", "padding": 6 }
  }
}
```

- **Selectors:** `Type`, `.class`, `Type.class`, each with an optional state suffix (`:hover`, `:pressed`, `:focused`, `:disabled`, `:checked`, `:selected`, `:invalid`). There are no descendant or child combinators: scope comes from where a theme is attached, not from selectors. That keeps resolution static and cheap.
- **Properties** are exactly the `.yui` attributes of the type (`bg`, `color`, `shadow`, `padding`, `h`, `font_scale`, `background`, `focus_style`, …), with the same value grammar: colors, `@` refs, paint refs with full namespaces (D10). A user component's own properties are allowed under its type name (`"IotaRow": { "accent": "#FF…" }`).
- `based_on` chains: `hexcasting:ui/themes/slate_dark` can be based on `hexcasting:ui/themes/slate`.

**Attach points:**

| Where | Syntax | Scope |
|---|---|---|
| Mod | `yog.toml` `[ui] theme = "hexcasting:ui/themes/slate"` | every screen of the mod |
| Screen | `<Screen theme="hexcasting:ui/themes/slate; hexcasting:ui/themes/big">` | that screen (a list; later entries win) |
| Component | `<Component theme="…">` at the component's root | every use of the component |
| Element subtree | `theme="…"` on any element | that element and its descendants |
| Class / inline | `class="title hint"`, `style="@Set.Danger"`, explicit attributes | one element |

**Cascade**, lowest to highest priority:
1. `minecraft:vanilla` (or whatever the base chain of the nearest theme ends in, e.g. `yog:bare`),
2. the mod theme,
3. the screen theme(s),
4. `<Screen.Resources>` `<Style target=…>`,
5. the component theme and component resources,
6. element-subtree themes, innermost last,
7. named style (`style=`) and classes (`class=`),
8. explicit attributes.

A closer scope always beats a farther one, whatever the selector. Within one scope the order is `Type` < `.class` < `Type.class`, then the state variants, and later list entries win. Merging is **per property**: a screen theme that sets only `Button.color` keeps the mod theme's `Button.bg`.

**Compilation.** Themes are resolved at codegen. The generated widget code contains the final per-element values, so there is no runtime cascade and no lookup cost. For builder code each theme also becomes a `Theme` static (`res::themes::ui::themes::SLATE`). `UiRoot::theme(res::themes::…)` and `.theme(…)` on any builder node apply it with the same cascade rules, evaluated once at build time of the tree.

**Hot reload.** A `.ytheme` edit regenerates the screens that use it. That is the same path as a `.yui` edit.

**IDLE:**
- **Theme editor** (`yog.theme` provider, plus a JSON code view):
  - left: a selector tree (types → classes → states) with add/remove;
  - centre: a live **gallery** of every control in every state, plus a chosen project screen rendered with the theme applied;
  - right: a PropertyGrid for the selected selector, with paint pickers.
- **Designer › Styles panel:** for the selected element it lists the cascade, showing each property's value and **source** (vanilla, mod theme, screen theme, …, attribute). Overridden values are struck through, and clicking a source opens it. *Extract to theme* moves the selected element's explicit attributes into a chosen theme as a `Type` or `.class` rule.
- Editing a theme re-renders every open designer that uses it.

---

## 7. Vanilla GUI materials (overlay examples)

### 7.1 Button

```jsonc
// vanilla/overlay/1.21.1/materials/gui/button.ymat
{
  "description": "Vanilla button",
  "texture": "minecraft:gui/sprites/widget/button",
  "text": { "color": "#FFFFFF", "shadow": true },
  "states": {
    "hover":    { "texture": "minecraft:gui/sprites/widget/button_highlighted" },
    "focused":  { "inherit": "hover" },
    "disabled": { "texture": "minecraft:gui/sprites/widget/button_disabled", "text": { "color": "#A0A0A0" } }
  }
}
```

```jsonc
// vanilla/overlay/1.20.1/materials/gui/button.ymat
{
  "description": "Vanilla button",
  "layers": [{ "texture": "minecraft:gui/widgets", "region": [0, 66, 200, 20], "mode": "nine_slice", "slice": 3 }],
  "text": { "color": "#FFFFFF", "shadow": true },
  "states": {
    "hover":    { "layers": [{ "texture": "minecraft:gui/widgets", "region": [0, 86, 200, 20], "mode": "nine_slice", "slice": 3 }] },
    "focused":  { "inherit": "hover" },
    "disabled": { "layers": [{ "texture": "minecraft:gui/widgets", "region": [0, 46, 200, 20], "mode": "nine_slice", "slice": 3 }],
                  "text": { "color": "#A0A0A0" } }
  }
}
```

1.20.1's vanilla `AbstractButton` draws the 200×20 region as two halves with a 20/4 px border on a tall atlas. The nine-slice with border 3 that Yog uses today matches visually at the usual heights. Exact parity is an implementation check.

### 7.2 Text field (1.20.1 procedural)

```jsonc
{
  "layers": [{ "fill": "#FF000000" }, { "frame": "#FFA0A0A0", "width": 1 }],
  "text": { "color": "#E0E0E0", "shadow": true },
  "padding": [0, 4, 0, 4],
  "states": {
    "focused":  { "layers": [{ "fill": "#FF000000" }, { "frame": "#FFFFFFFF", "width": 1 }] },
    "disabled": { "text": { "color": "#707070" } }
  }
}
```

### 7.3 Common materials

```jsonc
// vanilla/overlay/common/materials/gui/panel.ymat — the container frame used by demo/info screens
{ "texture": "minecraft:gui/demo_background", "mode": "nine_slice", "slice": 4,
  "text": { "color": "#404040", "shadow": false }, "padding": [7, 7, 7, 7] }

// vanilla/overlay/common/materials/gui/slot.ymat
{ "size": [18, 18],
  "layers": [{ "fill": "#FF8B8B8B" },
             { "frame": { "top": "#FF373737", "left": "#FF373737", "bottom": "#FFFFFFFF", "right": "#FFFFFFFF" } }] }

// vanilla/overlay/common/materials/gui/tooltip.ymat — for tooltip-looking custom panels
{ "layers": [{ "fill": "#F0100010", "inset": 1 },
             { "frame_gradient": ["#505000FF", "#5028007F"], "width": 1, "inset": 1 },
             { "frame": "#F0100010", "width": 1 }],
  "text": { "color": "#FFFFFF", "shadow": true }, "padding": [4, 4, 4, 4] }

// vanilla/overlay/common/materials/gui/dim.ymat
{ "layers": [{ "gradient": ["#C0101010", "#D0101010"] }] }
```

Every former `sprite:` name (hotbar, hearts, food, armor, air, xp bar, crosshair, creative scroller) becomes an overlay material `minecraft:gui/<name>` with the same 1.20.2+/1.20.1 data that `SPRITES` holds today, so HUD mods lose nothing.

---

## 8. `.yui` changes

```xml
<Screen id="mymod:settings" width="260" height="180" title="@Strings.settings.title" background="auto"
        theme="mymod:ui/settings" x:ViewModel="crate::ui::SettingsVm">
    <Screen.Resources>
        <Material x:Key="RunePanel" texture="hexcasting:gui/rune_panel" mode="nine_slice" slice="6"
                  text.color="#E0D0FF" text.shadow="false"/>
        <Style x:Key="Danger" target="Button" based_on="@Theme.Button" color="#FF5555"/>
    </Screen.Resources>
    <Panel w="100%" h="100%">
        <Label text="@Strings.settings.title" class="title"/>        <!-- #404040 via inheritance -->
        <CheckBox text="@Strings.show_badges" checked="{x:Bind show_badges, Mode=TwoWay}"/>
        <Slider w="150" value="{x:Bind volume, Mode=TwoWay}" text="{x:Bind volume_text()}"/>
        <TextBox w="150" text="{x:Bind name, Mode=TwoWay}" tooltip="@Strings.name.tip"/>
        <Image source="hexcasting:gui/logo"/>
        <Image source="mat:minecraft:gui/icon/heart"/>
        <Image source="item:minecraft:diamond"/>
        <Column bg="@RunePanel" padding="6"> … </Column>
        <Button text="@gui.done" command="{x:Bind cx.close()}" w="150"/>
    </Panel>
</Screen>
```

- `<Material x:Key>` in `<Screen.Resources>` takes the `.ymat` schema as attributes (dotted for nesting: `text.color`, `states.hover.texture`, or a child `<Material.States>` element). It compiles to `<ns>:__inline/<screen>/<key>` in `yog/materials.json`.
- Brush attributes are `bg` and `source`. `item`/`texture` bindings accept `Material`/`Texture` handles from code (`{x:Bind res::materials::gui::RUNE_BUTTON}`).
- New attributes on every element: `theme` (§6.6) and `class`. `<Screen theme>` takes a `;`-separated list.
- Bindable additions: `tooltip`, `value` (Slider), `checked` (CheckBox), `selected_tab` (TabBar).
- `yui.rs` `LEAVES`/`CONTAINERS` gain `CheckBox`, `Slider`, `Separator`, `TabBar`/`Tab`, `Panel`, and `kind_attrs` gains the new attributes. `sprite:`/`mod:` diagnostics change to "use `mat:minecraft:gui/<name>`" / "write the namespace".

---

## 9. Data flow per crate and host

```
content/**.png(.mcmeta) ──┐
content/**.ymat|.ytheme ──┤  yog-cli (yog build / yog check / yog res check)
content/**.yblock|.yitem ─┤    • texture scan + validation → yog/textures.json, assets/<ns>/textures/**
content/**.yui ───────────┤    • .ymat + inline materials → yog/materials.json (resolved, flattened)
yog.toml [mod] minecraft ─┘    • content → model JSON (texture ids, tintindex, uv, render_type, emissive)
                                         + BlockDef/ItemDef(render_layer, tints, emissive, behavior)
                               • .ytheme cascade resolved and inlined into screen code
vanilla packs (cache) ────────►• validation of every ref against each targeted version
   ▲  yog-vanilla               • generated/resources/{textures,materials}.g.rs, generated/vanilla.g.rs
   │                            • .yui → widget code with Brush::Material / theme defaults
client jar / piston-meta
                                        .yog
                                         │
yog-runtime ── loads overlay(host_mc_version) + every yog/materials.json → MaterialRegistry
   │           YogBlockDef/YogItemDef(render_layer, tints, emissive) → nativeBlockDefs
   │           draw2d_material/material_info/draw2d_tooltip/... → layer geometry (Rust)
   ▼
Java hosts ── NativeDraw primitives (drawImage, drawRect, drawGradientRect, drawGuiSprite, renderTooltip)
              all: block class per behaviour (YogTransparentBlock, YogLeavesBlock, …) + mod behaviour vtable
              Fabric: BlockRenderLayerMap + ColorProviderRegistry (+ FRAPI emissive in S3b)
              NeoForge/Forge: RegisterColorHandlersEvent (+ render_type, forge/neoforge_data)
              all: host_mc_version, screen title/background attributes

yog-gfx  ── Texture, Material (handles), material data model + compiled-JSON parser + slicing math
            (shared by yog-runtime, yog-cli, Yog-IDLE's designer recorder)
yog-ui   ── Brush, Theme + theme::vanilla, text inheritance, new controls, material-aware render
yog-kit  ── bindings for value/checked/selected_tab/tooltip; tooltip hover timing
yog-api  ── re-exports, resources!() includes textures/materials, vanilla!(), ItemId/BlockId rename mapping
```

The **material data model lives in yog-gfx** (`yog_gfx::material`), which is published and has no runtime dependencies. The runtime resolves with it, yog-cli validates and flattens with it, and IDLE's designer recorder draws with it. One implementation therefore covers the game and the preview, as `ImageSource` does today.

---

## 10. Yog-IDLE

Substrate stays generic: it gets only a reusable `AssetPicker` widget (a searchable thumbnail grid with tabs and a provider interface) and nothing Minecraft-specific.

1. **Vanilla pack service** (`src-tauri`): replace `designer/assets.rs`'s lazy jar reading with `yog-vanilla`.
   - `Resources::for_mod(root, version)` reads the mod first, then the pack of `version`.
   - First use of a version without a pack runs `yog vanilla sync` automatically (local jar, else download) with progress in Output. With `--offline`-equivalent settings and no jar, a bar explains what is missing.
   - **Settings › Environment › Minecraft assets:** *Extract to cache* (default) or *Read from the game jar (slower)*, written to `~/.config/yog/config.toml` so the CLI uses the same mode (§3.2). The page also shows the cache path and size, with *Clear*.
2. **Version = the selected run configuration** (§3.4). There is no separate version selector. The designer, content editor, material and theme editors and the pickers follow the run configuration, and the status bar shows its version.
3. **Project Explorer** presents the new layout (§13):
   - **Content**: the `content/` tree, with typed icons per extension (texture, material, theme, block, item, model, UI, resources, book, sound). Sidecars are nested under their file: `x.png.mcmeta` under `x.png`, `x.ysound` under its `.ogg` files. A toggle switches to a UE-style **content browser**: a thumbnail grid with type filters and search.
   - **Source**: `src/`.
   - **Pack**: `pack/`, raw files.
   - **Generated**: a virtual, read-only node over `.yog/build/generated` (handles, inlined screens), useful for "go to definition".
   - **External › Minecraft <V>**: a read-only virtual folder of the pack for the run configuration's version (textures, materials, content, lang). Files open in the normal editors read-only. *Copy to project* copies a texture, material or block into the mod with its id rewritten.
   - `.yog/` is hidden.
   - **Rename/move** of any content file rewrites every reference to its id across the project (`.yblock`, `.yitem`, `.ymat`, `.ytheme`, `.yui`, `.ybook`, Rust handles), using the same reference index as `yog check`.
4. **Texture editor** (PNG, a new `yog.texture` provider at priority 20 over the plain image view):
   - Pixel editing reuses `content/paint/*` (it is product-agnostic).
   - A **Metadata** panel writes `.png.mcmeta`: animation (a frame strip, frametime, interpolate, frame order with drag, play/pause preview), `gui.scaling` (with draggable nine-slice guides on the canvas), and `blur`/`clamp`.
5. **Material editor** (`.ymat`, `yog.material` provider plus a JSON code view, F7):
   - Left: PropertyGrid with sections World, UI, Layers and States. Layers form a reorderable list.
   - Centre: a live preview. In **UI mode** the material is drawn by the designer's Rust recorder at a chosen size, with state toggles (hover, pressed, focused, disabled, checked) and a resize handle to check nine-slice and tiling. In **World mode** a three.js cube (reusing `Viewport.material()`) is drawn with render layer, tint (a biome color picker), emissive and an animation preview.
   - Right: the texture with the region and slice guides, all draggable.
   - *Paint* opens the texture in the texture editor.
6. **Pickers:** new PropertyGrid descriptor kinds `"paint"`, `"material"`, `"texture"` and `"item"`. Each has a thumbnail, a datalist and a **…** button that opens `AssetPicker`, which has these tabs:
   - Textures (mod / other mods in the workspace / Minecraft V),
   - Materials (same sources plus *Theme*),
   - Items, Blocks,
   - Colors (`.yres` colors).
   They are used in the designer (`bg`, `source`, `item`), in the content editor's texture slots (which also offer *Make inline material…* / *Open material*), and in `.ymat` fields.
7. **Content editor:**
   - The Render Layer row moves from *Block* to each slot's material (inline or file), with an `auto` hint showing the detected layer.
   - The tint chip edits the slot material's `tint`.
   - Animated textures play in the viewport (a frame timer on the THREE texture offset).
   - Paint mode saves a new or copied texture **next to the `.yblock`/`.yitem` being edited**: `content/<same folder>/<stem>[_<slot>].png`. The slot gets the full id, e.g. `hexcasting:blocks/slate/slate`. `content::texture_target` is rewritten for ids only: own namespace → `<mod>/content/<path>.png`, everything else read-only and copied on the first stroke.
   - Painting a vanilla texture copies it from the pack.
8. **Designer:**
   - The recorder implements `draw2d_material`/`material_info`/`draw2d_tooltip`/`draw2d_text_scaled` with `yog_gfx::material` and the selected pack's overlay. Mod materials come from `yog/materials.json` produced by `yog check`, which already runs on save.
   - Tooltips show in Preview mode on hover. The Toolbox gains CheckBox, Slider, Separator, TabBar, Panel.
   - The HTML fallback (no pack) draws a material's first layer approximately.
   - Properties: `bg`/`source` use the paint picker. The color swatch row stays for color brushes.
9. **New Item templates:** *Material (.ymat)*, *Theme (.ytheme)*, *Texture (.png)* (16/32/64 px), *Sound (.ysound)* and *Book (.ybook)*, created in the selected `content/` folder, with the project's namespace filled in (D10). The project template (New Project) creates the §13 layout. The codegen regex in `OpenFilesContext.tsx:329` and the matcher in `src-tauri/src/yui.rs:17-24` include `.ymat`, `.ytheme` and `.png.mcmeta` (they run `yog res check`, which now also writes the texture, material and theme handles).
10. **`.yres` editor:** the `image` and `sound`/`file` types are removed from `RESOURCE_TYPES`. Older projects never reach the editor with them, because the format migration (§14) converts them on open.
11. **Theme editor and Styles panel:** §6.6.
12. **Migration banner:** §14.

---

## 11. Deletions (no compatibility)

| Removed | Replacement |
|---|---|
| `.yres` `"type": "image"`, its `texture` field, `@Set.name` as an image (yog.toml `icon` too) | a PNG under `content/` + a texture id (`icon = "yog-modlist:icon"`) |
| content `path.png`, `@Set.name` texture forms | texture ids |
| `block.render_layer`, face `tint` index | the material's `render_layer` / `tint` |
| `sprite:*`, `yog_gfx::image::{SPRITES, Sprite, LegacyRegion, suggest_alias}`, `sprite:` code in `yog-runtime/src/images.rs` and IDLE `designer/images.rs` | `mat:minecraft:gui/*`, overlay |
| `mod:<path>`, bare ids meaning `minecraft:` | full `<ns>:<path>` (D10) |
| `.yres` `sound`/`file` types | `.ogg` + `.ysound` under `content/`, raw files under `pack/` |
| `noOcclusion` derived from appearance, `YogShapedBlock` as the only block class | behaviours (§4.6), `YogBlockTraits` |
| `resources/`, top-level `assets/`/`data/`, `.yog-build/`, `.yog-idle/`, root `target/` | §13 layout |
| fixed control defaults in `render.rs` | `minecraft:vanilla.ytheme` + `.ytheme` cascade |
| yog-ui flat defaults, amber focus, `focus_color`, `Style.bg: u32` | theme, `Brush`, `focus` brush (a flat look is a `.ytheme` on `yog:bare`) |
| `NativeDraw.textureSize` (PNG IHDR read) | `yog/textures.json` + the vanilla index |
| IDLE `designer/assets.rs` jar-in-place reader | `yog-vanilla` |
| `java-common/.../ui/YogUILayout.java` (unused) | — |
| Modlist `Pill`, `StatBar` (unused) | — |

---

## 12. Per-mod migration

### 12.1 Yog-Modlist: a vanilla "pack selection"-style screen

The new `mod_list.yui` keeps the `x:Name`s used by its test: `search`, `filter_all`, `filter_platform`, `mod_list`, `detail_name`.

```
background="auto" title="@Strings.screen.title"    ← dimmed world / panorama, white title at y=15
anchor="fill"  (vanilla list screens use the whole window)
Grid rows="32,*,36" columns="*"
  row 0: Title (host) + mod_count Label (@Theme.Secondary) right
  row 1: Grid columns="*,8,*"
      left : TextBox search (h20) / TabBar filter_all|filter_yog|filter_platform (vanilla tabs, the
             user's decision) / ListView mod_list background="list"
             entries 36 px: Image (mod icon, 32×32, `icon` from manifest, fallback `item:minecraft:book`) +
             name (white) + version/source line (§7 gray, source color §a/§9)
      right: Panel-less ScrollView details background="list": name (white), version/id (#A0A0A0),
             source (§a #55FF55 Yog, §9 #5555FF Platform), authors, description (white, wrapped)
  row 2: Separator footer + Button done w150 h20 "@gui.done" centered
```

- `ModSource::color()` becomes vanilla formatting colors.
- `Pill.yui`, `stat_bar.yui` and `stat_bar.rs` are deleted.
- `Strings.yres` gains `screen.title` use; "Done" uses `@gui.done`, the vanilla key.
- The test (mod_list.rs:205-225) keeps passing. TestHost draws nothing, so only ids matter. The filters become `Tab`s; the test's `filter_*` names move to them.
- Modlist uses no theme of its own: it is the reference consumer of `minecraft:vanilla`.

### 12.2 HexMod-Yog: the original Hex Casting UI

**Rule (the user's): HexMod-Yog reproduces the original Hex Casting UI exactly.** It does not adopt the vanilla theme. Revision 1's plan (a vanilla `Panel`, dark §-colors for contrast) is withdrawn.

What this design guarantees, and nothing more:
- **No vanilla default leaks in.** HexMod attaches its own theme mod-wide: `yog.toml` `[ui] theme = "hexcasting:ui/hex"`, a `.ytheme` with `"based_on": "yog:bare"`. `yog:bare` has no backgrounds, no button sprites, no screen dimming (`background="none"`), and no text color hints. Every look comes from HexMod's own theme and materials.
- **The original art is expressible.** The original mod's textures are already in the repo (`assets/hexcasting/textures/gui/*`). They move into `content/` (below) and become materials: nine-slices, tiled backgrounds, state variants and layered frames are exactly what `.ymat` covers (§2.2). Anything the original draws procedurally (the pattern grid, stroke glow) stays in the `HexGrid` surface with its SDF shader.
- **Per-element exceptions** use subtree themes, classes and explicit attributes (§6.6), not vanilla controls with patched colors.

The exact reproduction spec (which screens, sizes, colors, fonts and animations, measured against the original mod) is **HexMod's own task**, not part of this document. This design must not block it. Missing capabilities found there become requests against §2.2 and §6.6.

**Content migration** (one format step, §14):
- `resources/block/**`, `resources/item/**` and `assets/hexcasting/textures/**` move into `content/`: each texture next to its single user (`content/blocks/slate/slate.png` beside `slate.yblock`), shared or GUI textures to `content/textures/<old path>`.
- All 48 `.yblock` and 70 `.yitem` files are rewritten from `"block/x.png"`/`"item/x.png"` to full ids (`"hexcasting:blocks/slate/slate"`).
- The three edified leaves get `"behavior": "minecraft:leaves"` and an inline material `{ render_layer: cutout_mipped, tint: none }` (or nothing, because `auto` detects cutout). `amethyst_sconce` gets `cutout` plus `minecraft:half_transparent` or a mod behaviour, decided while porting. Slates and other logic-carrying blocks use mod-defined behaviours (§4.6).
- The 34 `item/colorizer/*.png.mcmeta` animations move with their PNGs and keep working.
- `data/` (loot tables, tags) and non-texture `assets/` (OptiFine CTM, particles) go to `pack/data/` and `pack/assets/`.

**The book** becomes a `.ybook` with its own WYSIWYG designer (see `docs/design/ybook.md`). It uses materials for its art and the vanilla book look by default. The book look is decided there.

### 12.3 Yog-Pipes

No UI and no content files, so nothing to migrate. Its glass/transparent pipes (if any are added) use `.behavior("minecraft:transparent").render_layer(RenderLayer::Cutout)`. The layout migration (§13) still applies: `src/`, `.yog/`. The unused `ModelDef`/`FaceDef` (src/model.rs) gain a `material: Material` field when they are wired up. That is out of scope here, but noted so that the API takes `Material`, not texture strings.

### 12.4 Docs

The following docs are rewritten:
- Yog-IDLE `docs/yui.md` (Images, Sprites, new controls, Theme),
- `docs/yres.md` (no images),
- `docs/blocks-items.md` (materials; remove the Fabric limitation),
- new `docs/textures-materials.md`,
- new `docs/vanilla-pack.md`,
- new `docs/themes.md` (§6.6),
- new `docs/project-layout.md` (§13) and `docs/migration.md` (§14, with the per-format changelog),
- the Yog-Mod-Loader READMEs' feature list.

---

## 13. Mod project layout (D13)

### 13.1 Today

`src/` (Rust plus `.yui` and `*.designer.rs`), `resources/` (`.yres` plus PNGs), `content/` (`.yblock`/`.yitem`), top-level `assets/` and `data/` (copied verbatim by `gather_assets`, main.rs ~1892), `.yog-build/` (generated cargo project, main.rs:651), `.yog-idle/`, `target/`, `artifacts/`. The `.gitignore` template (main.rs:486) lists `.yog-build/`, `target/`, `artifacts/` and `*.designer.rs`. Asset kinds are split by folder for historical reasons, and textures sit far from the blocks that use them.

### 13.2 New layout

All names are lowercase.

```
yog.toml                [project] format = N (§14), [mod], [ui] theme, [run.*], …
yog.lock
README.md, LICENSE
src/                    Rust only. .yui code-behind view-models stay here and are referenced by path:
                        <Screen x:ViewModel="crate::ui::ModListVm">
content/                every asset; free-form/feature folders; the type is the extension
pack/assets/…           raw resource-pack passthrough (OptiFine CTM, particles, anything Yog has no format for)
pack/data/…             raw data-pack passthrough (loot tables, tags, recipes until .yloot/.ytag/.yrecipe exist)
artifacts/              built .yog files (visible, git-ignored)
.yog/                   hidden, git-ignored:
  build/                  generated cargo project (was .yog-build/)
  target/                 cargo target-dir (was ./target/)
  cache/                  texture alpha, vanilla refs index, incremental state
  idle/                   IDE state (was .yog-idle/)
  backup/                 pre-migration backups (§14)
```

**`content/` file types:**

| Extension | What | Id |
|---|---|---|
| `.png` (+ `.png.mcmeta`) | texture (D1) | path |
| `.ymat` | material (D2) | path |
| `.ytheme` | theme (§6.6) | path |
| `.yblock`, `.yitem` | block / item | file stem, `id` field overrides |
| `.ymodel` | shared model (parent for several blocks/items) | path |
| `.yui` | screen / component | `<Screen id>` / component name |
| `.yres` | strings, colors, numbers, bools only (`image`, `sound`, `file` types are deleted) | `@Set.name` |
| `.ybook` | guide book (`docs/design/ybook.md`) | path |
| `.ogg` + `.ysound` | sound event: variants, subtitle, category | path of the `.ysound` |
| `.yrecipe`, `.yloot`, `.ytag` | future | path |

"Path" means `<mod>:<path under content/ without the extension>`. Registry ids (blocks, items) use the file stem because they are global names that players see in commands; two files with the same stem are a build error that names both. Folder conventions and directory defaults (`_defaults.yblock`, `_defaults.yitem`) come from `docs/design/boilerplate-reduction.md` F5.

**What reads what:**
- yog-cli scans `content/**` once per build and dispatches by extension. The scan result is the reference index used by validation, the IDE rename refactoring (§10) and the migration's ref rewriting (§14).
- `src/` is the cargo crate root. `.yui` files under `content/` generate code into `.yog/build/generated/ui/`, and the screen binds to its view-model by Rust path. A `.yui` without `x:ViewModel` needs no Rust at all.
- `pack/` is copied verbatim into the `.yog`, after generated files. A collision with a generated file is a build error.
- yog-cli's paths (`src`, `.yog-build`, `artifacts`, `assets`/`data` in `gather_assets`, the `.gitignore` template) change accordingly. `CARGO_TARGET_DIR` points to `.yog/target`.

**Migration (one format step, §14):**
- `resources/*.yres` → `content/` (same relative path). `image` entries become PNG files plus full texture ids at each use. `sound` entries become `.ysound`, and `file` entries move to `pack/`.
- PNGs from `resources/` and `assets/<mod>/textures/` move **next to their single user** (`.yblock`/`.yitem`/`.ymat`), or to `content/textures/<old path>` when used by several files or by none. Every ref is rewritten.
- `src/**/*.yui` → `content/ui/**`. `x:ViewModel="X"` becomes `crate::<module of the old file>::X`. `*.designer.rs` files are deleted: designer code is generated into `.yog/build/generated/ui/` only.
- `assets/` and `data/` remaining after that → `pack/assets/`, `pack/data/`.
- `.yog-build/`, `.yog-idle/` and `target/` are deleted (regenerated in `.yog/`), and `.gitignore` is rewritten.

### 13.3 IDLE templates

New Project creates `yog.toml` (with `format`), `src/lib.rs`, `content/` with a sample `.yblock` and texture side by side, an empty `pack/`, and `.gitignore` with `.yog/` and `artifacts/`. New Item templates are listed in §10.

---

## 14. Project format versioning and automatic migration (D14)

**The format number.** `yog.toml` gets `[project] format = N`, an integer independent of the CLI's semver. It changes only when a project's files must change. The CLI knows its current format `C` and a table `format → minimum CLI version`.

**The chain.** The `yog-migrate` library crate (used by yog-cli and Yog-IDLE) holds an ordered list of steps `N → N+1`. Each step is a small transform over the project: renames, reference rewrites, folder moves, schema changes. It works on an in-memory file set and produces a change set (writes, moves, deletes) that can be previewed or applied. Steps use the same parsers as the build, at the version they had then: a step owns any old-schema parsing it needs, so the build never does.

**When it runs:**
- Every CLI command that reads a project (build, check, run, ui gen, …) and IDLE's project open compare `format` with `C`.
- `format < C`: the CLI migrates automatically and prints the summary. IDLE shows a banner, *"This project uses format 6. Migrate to 7?"*, with a diff preview (the change set as a tree of moves and per-file diffs) and *Migrate*. Nothing opens editable until the project is migrated.
- Safety: the working tree must be clean in git, otherwise the original files are copied to `.yog/backup/<timestamp>/` first. The change set is logged to `.yog/migrations.log` and printed.
- `format > C`: an error, *"This project needs yog-cli ≥ 0.9 (format 8); this is 0.8.3 (format 7)"*. Nothing is modified.
- `yog migrate --dry-run` prints the change set without applying it. `yog migrate rename-namespace <old> <new>` reuses the same ref-rewriting machinery but is not a format step.
- Steps are **idempotent**: running a step on an already-converted file set changes nothing, so an interrupted migration can be rerun.

**No compatibility code paths.** Every part of the toolchain other than `yog-migrate` reads only format `C`. Old formats exist only as migration inputs. This is how the no-backward-compat rule and painless upgrades coexist.

**Release process.**
- A change to any project file format ships with its migration step and a fixture: a small project in the old format and the expected result.
- CI keeps a fixture project for **every past format** and checks that each migrates to `C` and then passes `yog check`.
- `docs/migration.md` lists each format's changes for humans.

**The steps for this design.** One step per logically separate change, so each stays small and testable:

| Step | Change |
|---|---|
| 1→2 layout | §13.2 migration (folders, `.yres` split, `.yui` move, `.gitignore`) |
| 2→3 namespaces | bare ids → `minecraft:`, `mod:` → `<mod id>:` (D10) |
| 3→4 textures/materials | `path.png`/`@Set.name` textures → ids; `block.render_layer` and face `tint` → slot materials; `sprite:` → `mat:minecraft:gui/*` |
| 4→5 behaviours | blocks with a non-solid layer and no behaviour get `minecraft:transparent` or `minecraft:leaves` (by vanilla parent and name), with a note in the log to review them |

Today's projects have no `format` key and are treated as format 1. After this design, `C` = 5. Steps that land in the same release may still be separate steps: the chain runs them in order.

---

## 15. Implementation stages

| Stage | Content | Depends on | Crates / repos |
|---|---|---|---|
| **S0a** Format migration framework | `yog-migrate` crate (step trait, change set, dry-run, backup, git-clean check, log), `[project] format`, format→min-CLI table, the fixture CI job; IDLE banner + diff preview | — | yog-migrate, yog-cli, Yog-IDLE |
| **S0b** Project layout | §13: content scan by extension, `pack/`, `.yog/`, `CARGO_TARGET_DIR`, `.gitignore`, `.yres` without image/sound/file, `.ysound`; step 1→2; IDLE Explorer presentation and templates | S0a | yog-cli, yog-migrate, Yog-IDLE |
| **S0c** Namespaces | bare-id errors with suggestions in every format, delete `mod:` and `qualify`; step 2→3; IDLE completion inserts full ids, rename refactoring | S0a | yog-cli, yog-gfx, yog-migrate, Yog-IDLE |
| **S0** Vanilla pack | `yog-vanilla` crate (jar locate moved from IDLE, piston-meta download, extraction, importer run, index, `pack.json`), `vanilla/` dir with `versions.toml`, `renames.toml`, `tints.toml`, empty overlays; `yog vanilla sync/status/path`; `host_mc_version` ABI + hosts | — | yog-vanilla, yog-cli, yog-abi, yog-runtime, Java |
| **S1** Textures | PNGs from the `content/**` scan, ids, `.mcmeta` validation, atlas sources by usage (§2.1), `yog/textures.json`, `res::textures` handles, `Texture` type in yog-gfx; delete `path.png`, `@Set` textures | S0, S0b | yog-gfx, yog-cli, yog-api |
| **S2** Materials | `yog_gfx::material` (model, compiled JSON, state resolution, slicing math, tests), `.ymat` parser/compiler, inline materials, `yog/materials.json`, `res::materials` handles; step 3→4 | S1 | yog-gfx, yog-cli, yog-api |
| **S3** World materials | content compile via materials (tintindex derivation, uv, auto layer), `BlockDef`/`ItemDef` render_layer/tints/emissive, ABI fields, `nativeBlockDefs`; **Fabric `BlockRenderLayerMap`, color providers**; Forge/NeoForge emissive via `*_data` | S2 | yog-cli, yog-registry, yog-abi, yog-runtime, Java ×6 |
| **S3a** Block behaviours | §4.6: `behavior` in `.yblock`/`BlockDef`/`YogBlockDef`, built-in behaviour classes per host (×6), `YogBlockTraits`, the layer/behaviour check; mod-defined behaviours (`block_behavior`, vtable, declarative skip rules); step 4→5 | S3 (the layer check), can start with S0 | yog-registry, yog-abi, yog-runtime, yog-cli, yog-migrate, Java ×6 |
| **S3b** Fabric emissive | FRAPI baked-model wrapper | S3 | Java (fabric ×2) |
| **S4** UI materials in runtime | `MaterialRegistry`, `draw2d_material`/`material_info`/`draw2d_tooltip`/`draw2d_item_tooltip`/`draw2d_text_scaled`, layer geometry, animation frames; overlay for 1.20.1 + 1.21.1 written and checked with `--check-overlay`; delete `sprite:` | S2, S0 | yog-runtime, yog-abi, yog-gfx, Java ×6, vanilla/ |
| **S5** yog-ui vanilla | `Brush`, `minecraft:vanilla.ytheme` + `theme::vanilla`, text inheritance, restyled Button/ToggleButton/TextBox/ScrollView/ListView/slots, new CheckBox/Slider/Separator/TabBar/Panel/tooltip, screen `title`/`background`; `font_scale` fix; delete flat defaults; `.yui` grammar + `@Theme.*` + `<Material>` resources | S4 | yog-ui, yog-kit, yog-cli |
| **S5b** Themes | `.ytheme` parser, `yog:bare`, cascade resolution in codegen, `theme`/`class` attributes, `[ui] theme`, `Theme` statics + `UiRoot::theme`; IDLE theme editor, Styles panel, *Extract to theme* | S5 | yog-ui, yog-cli, Yog-IDLE |
| **S6** Vanilla handles | `vanilla!()`, `vanilla.g.rs` (items/blocks/textures/materials/lang), rename mapping in `ItemId`/`BlockId`, cross-version ref validation in all formats | S0, S2 | yog-cli, yog-api, yog-runtime |
| **S7** IDLE | (a) vanilla pack service + Environment setting + run-configuration version + Minecraft node (after S0); (b) texture editor + mcmeta (after S1); (c) material editor + pickers + `AssetPicker` in Substrate (after S2); (d) designer recorder for materials/new controls (after S4/S5); (e) content editor material slots, animation playback (after S3) | per item | Yog-IDLE, Substrate-Platform (AssetPicker only) |
| **S8** Mods | Modlist rewrite (§12.1); HexMod: migration via the format steps (after S3a), `hexcasting:ui/hex` theme on `yog:bare` + original-art materials so the UI port can reproduce the original (after S5b), `.ybook` (per ybook.md); Pipes: layout step only | S5b (UI), S3a (content) | mods |
| **S9** Docs + publish | docs (§12.4); crates.io: yog-gfx, yog-ui, yog-kit, yog-api, yog-registry, yog-cli, yog-runtime, new yog-vanilla | all | — |

Critical path: S0a → S0b → S1 → S2 → S4 → S5 → S5b → S8 (UI). S0a comes first because every later stage that changes files ships its migration step. The x-ray bug is fixed by S3 + S3a together. If it must be fixed before materials exist, the Java part of S3a (the built-in `minecraft:transparent`/`leaves` classes) plus S3's `BlockRenderLayerMap` is a small change, and a temporary migration step adds `behavior` to affected blocks.

ABI: S0, S3, S3a and S4 each append to `YogApi`/`YogGfxApi`/`YogBlockDef`, so they need ABI_MINOR bumps. Mods must be rebuilt (`--local-api` for hot reload).

---

## 16. Risks and open questions

### Risks

- **R1 Overlay accuracy.** 1.20.1 regions, slider/tab atlases and the exact 1.21.1 sprite ids (marked †) must match the jar. Mitigation: `--check-overlay` plus side-by-side screenshots against vanilla screens. Every future MC version needs an overlay before the loader supports it, which is enforced by a test.
- **R2 Version drift between 1.21.1 and newer.** 1.21.2+ moves tooltips and slot highlights to sprites and renames some GUI sprites. Material ids insulate mods. Only overlays change.
- **R3 Fabric emissive** needs FRAPI and a custom baked model, which interacts with Sodium/Indium. It is isolated in S3b.
- **R4 Generated vanilla handles size.** ~1.4k items, ~1k blocks, ~3k textures and ~6k lang keys per mod build could add compile time. Mitigation: consts only, one file, and lang optional behind a `yog.toml` flag if measured as slow.
- **R5 First-run UX.** Building a mod needs a vanilla pack for validation and handles. In CI without a jar, `yog build --no-vanilla` skips vanilla validation and generates handles only for ids actually referenced (from markup/JSON), which is weaker.
- **R6 `auto` render layer** can pick `translucent` for textures with a few semi-transparent anti-aliased pixels, which costs sorting. The build warns when <1% of pixels are partial and suggests `cutout`.
- **R7 Light panels and existing color palettes.** Mod text colors chosen for dark backgrounds become unreadable on `demo_background`. Text inheritance covers labels; explicit colors need review in mods that use the vanilla theme (Modlist). HexMod is not affected because it uses `yog:bare` (§12.2).
- **R8 Atlas merge collisions.** Several mods add `single` sources to `minecraft:blocks`; YogPackProvider merges them. Two mods can't collide on ids (namespaces differ), but a resource pack replacing `atlases/blocks.json` wholesale would drop Yog sources. YogPackProvider must append after packs, and this needs a test per loader.
- **R9 Migration of hand-edited projects.** Steps are mechanical, and unusual files (refs built by string concatenation in Rust, comments in JSON) can be missed. Mitigation: dry-run diff by default in IDLE, git-clean/backup requirement, and `yog check` right after migrating reports anything left.
- **R10 Theme resolution and dynamic content.** Themes are inlined at codegen, so elements created by builder code at runtime see only themes passed to the builder (`UiRoot::theme`). Mixing markup and builders in one subtree must pass the inherited theme down; the generated code does that, hand-written builders must too.

### Decided in revision 2 (former open questions)

| Q | Decision |
|---|---|
| Q1 Pack storage | Extract to the user cache by default; *read from jar* is an environment setting (IDLE Settings › Environment, `YOG_VANILLA_MODE`). Auto-download from piston-meta is allowed (`--offline` disables it). §3.2 |
| Q2 Reports | In v1, on by default (`--no-reports`), using the per-version JDK. §3.1, §3.2 |
| Q3 Default namespace | None: every reference is explicitly namespaced, no short alias. §2.3, D10 |
| Q4 Flat look | Deleted. D6 |
| Q5 Mod-wide restyle | `.ytheme` with scoped cascade (mod, screen, component, subtree). §6.6, D11 |
| Q6 Modlist filters | Vanilla tabs (`TabBar`). §12.1 |
| Q7 Glass-like culling | Block behaviour (`minecraft:transparent`, …), not material. §4.6, D12 |
| Q8 Book | `.ybook` with its own designer, in `docs/design/ybook.md`. §12.2 |
| Q9 IDLE preview version | The selected run configuration's version. §3.4 |
| Q10 Material location | Anywhere under `content/`, by extension. D2, §13 |

### Open questions

1. **Q11 Original Hex Casting UI reference.** Which original version (and which screenshots or source revision) is the reference for "exactly"? This belongs to HexMod's UI task, but the answer decides which art and behaviours the theme and materials must cover.
2. **Q12 `.ysound` vs `.yres`.** Proposed: sound events are `.ysound` files next to their `.ogg`s, and `.yres` keeps only strings/colors/numbers/bools. Alternative: sound events as a `sound` table inside `.yres`. The proposal matches "one file per asset" and lets IDLE nest it under the audio.
3. **Q13 `.yui` location.** Proposed: `.yui` lives in `content/` (it is an asset) and its view-model in `src/`, linked by Rust path. Alternative: `.yui` stays in `src/` beside its code-behind, as today. The first keeps `src/` Rust-only; the second keeps a screen and its logic in one folder.
4. **Q14 Registry ids from file stems.** Proposed: blocks and items take their id from the file stem (unique per mod), with an `id` override. Alternative: path-based ids like other assets (`hexcasting:blocks/slate/slate`), which are unique by construction but are long, player-visible names.
