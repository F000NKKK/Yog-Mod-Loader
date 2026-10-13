# Design: No manual rendering in mods

Status: design only, nothing implemented. Scope: `yog-ui`, `yog-gfx`, `yog-api`, `yog-abi`, `yog-book`,
`yog-kit`, `yog-cli` (`.yui`/`.ymat`/content codegen and validation), `yog-runtime`, the Java hosts, Yog-IDLE's
designer recorder, and the mods HexMod-Yog, Yog-Modlist and Yog-Pipes.

The user's request: *"Completely eliminate manual graphics via OpenGL in all mods. yog-api and yog-cli must be
sufficient for everything needed, without manual rendering and manual hacks."*

This builds on, and in places overrides, decisions recorded elsewhere:

- [materials-and-vanilla-ui.md](materials-and-vanilla-ui.md): `.ymat` materials, `.ytheme`, brushes,
  `draw2d_material`, `draw2d_text_scaled`. This document **extends the `.ymat` schema** with stroke, shape and
  marker layers, parameters and path effects (§5). Its §12.2 sentence "the pattern grid, stroke glow stays in the
  `HexGrid` surface with its SDF shader" is **withdrawn**.
- [mvvm-v2.md](mvvm-v2.md): `#[view_model]`, `#[on(E)]`, `#[recv]`, `open_on=`/`close_when=`, conditional
  styles. Its §12.8 (`SurfaceView`, `source="{x:Bind grid}"`) and decision **Q16** ("VM-owned surfaces are the
  normal way") are **superseded**: there are no surfaces (R2). Its "animations are out of scope" (§7) is replaced
  by §7 here.
- [ybook.md](ybook.md): B5 (custom page types are `.yui` components; the loader's `Pattern` page is deleted) and
  B7 (yog-book draws only through yog-ui/draw2d/materials) stay. Its §3.4/§6.5 plan for a Hex-side `HexPattern`
  widget on a new `draw2d_quad` is **superseded** by the shape elements and `draw2d_triangles` (§4, §11).
- [boilerplate-reduction.md](boilerplate-reduction.md): F12's `surface::shared::<T>()` helper is dropped
  (nothing to share).

---

## 0. Summary of decisions

| # | Decision | Why |
|---|---|---|
| R1 | **A mod contains no drawing code.** No GL handles, no shaders, no vertex buffers, no `draw2d_*` calls, no tessellation, no pixel-coordinate math for drawing, no per-frame paint callbacks. Everything a mod shows is declared: `.yui` markup bound to view-model data, `.ymat` materials, `.ytheme` styles, and content files (`.yitem`, `.yblock`, `.yparticle`, `.yoverlay`). | The user's rule. It is also what makes every screen previewable in IDLE (only declared drawing can be recorded) and what lets one mod binary look right on every MC version and loader. |
| R2 | **`Surface` is deleted, not kept as an escape hatch.** `yog_ui::Surface`, the id registry, `<Surface>`, mvvm-v2's planned `SurfaceView`, `Registry::on_hud_render`/`on_world_render`/`on_ui_render`, and the `yog_api::{gfx_gl, gfx_core, gfx_draw2d, gfx_image, GfxContext}` re-exports go. When a mod needs something that cannot be declared, the answer is a new **generic** primitive in the loader, never a hole in the API. | An escape hatch is where the hand-written GL came from (§1). No-backward-compat rule: delete outright. |
| R3 | **Vector shapes are `.yui` elements**: `Line`, `Polyline`, `Polygon`, `Path`, `Ellipse`, `Marker`. Their geometry binds to VM data (`points="{x:Bind wip()}"`); their look is a material or plain stroke/fill attributes. | WPF/WinUI `Shape`s in Yog's markup. A pattern stroke is a polyline, a grid dot is a marker. |
| R4 | **Materials gain vector layers**: `stroke`, `fill` for shapes, `markers`, plus **parameters** (`@stroke`, `@seed`, …), a closed **color/number expression set** (`mix`, `lighten`, `darken`, `alpha`, `min`, `max`, arithmetic), and generic **path effects** (`jitter`, `separate_revisits`, `trim_end`). | Hex's "glow" is two strokes (wide + lightened core) with a tail→head gradient; its "zappy" line is a noise displacement path effect. Both are style, so they belong in a material, not in Rust. Skia's `PathEffect` and SVG markers are the precedents. |
| R5 | **`<Lattice>` is a coordinate-system container** (hex pointy/flat, square). Children are positioned in lattice coordinates, the cell size can be fixed, area-based (`cell="area:512"`) or fit-to-content; pointer events report the nearest lattice point and the fractional position in cell units; `<Lattice.Dots>` draws lattice points with a proximity parameter. `yog_api::geom::{hex, square}` provides the matching pure math (neighbours, directions, nearest point). | Removes `coord_to_px`/`px_to_coord`/`hex_size`/`range_around` from mod code. An axial hex lattice is generic geometry (strategy games, tile pickers), not Hex Casting logic. |
| R6 | **Tessellation and noise live in yog-ui** (linked into each mod but loader code), and reach the host through **one new recordable ABI primitive, `draw2d_triangles`**. The GL half of `YogGfxApi` is deleted. | Keeps the ABI tiny and every frame recordable by IDLE's `YogGfxApi` recorder. The algorithm in HexMod's `render_lib.rs` (round joins and caps, per-vertex colors) becomes the generic stroker. |
| R7 | **Time and animation are markup**: `<Animate>` (oscillate/loop/once, period in ticks or seconds, easing), an `opacity` attribute and a `transform` attribute (scale, rotate, translate) on every element, and an implicit frame clock for animated materials (`jitter.speed`). | Hex's ravenmind pulse, its 1.5× ravenmind box and the moving zaps need these. Nothing about them is Hex-specific. |
| R8 | **Pointer, wheel and key input are element events bound to commands with event arguments** (`pointer_pressed="{x:Bind press($e.coord)}"`), with capture between press and release. | Replaces `Surface::pointer`/`scroll`. The state machine stays VM logic; only the plumbing disappears. |
| R9 | **Particles are content.** `.yparticle` declares a particle type (textures, lifetime, size/alpha curves, physics, full-bright, color from spawn). `particles::emit(kind, Emitter, color)` spawns them from server code; the host registers real particle types on every loader and version. | Replaces HexMod's `minecraft:dust` string building with version sniffing (`DUST_SYNTAX`), and gives Hex its own conjure particle as the original has. |
| R10 | **Item and block appearance driven by data is content.** `.yitem` gets `bar` (durability-style bar from item data), `variants` (model/texture chosen by item data), and `tooltip` components (a `.yui` component rendered inside the vanilla tooltip). `.yblock` gets `decals`: a `.yui` component drawn on a block face, bound to block-entity data, rendered to a cached texture. | Covers Hex's media bars, filled/empty cyphers and foci, pattern tooltips on scrolls/slates, and patterns on slates and akashic bookshelves, all without a renderer in the mod. |
| R11 | **World overlays and HUD layers are content plus data**: `.yoverlay` (a 3D wireframe/billboard shape with spin, bob, see-through and a color parameter) shown with `overlay::show(kind, pos, params)`; `<Hud>` roots in `.yui` for crosshair/corner HUD panels bound to a VM. | Hex's sentinel and scrying-lens readouts. Both are generic: waypoints, markers, info HUDs. |
| R12 | **The loader stays generic.** Nothing in this document knows about patterns, iotas, media or pigments. Hex's constants (colors, zappy parameters, sizes) live in HexMod's `.ymat`/`.ytheme`/content files. | The user's rule (loader-generic-only). |
| R13 | **`yog check` enforces it**: a mod whose generated `Cargo.toml` would depend on `yog-gfx`/`yog-abi` directly, or whose code uses `extern "C"` draw tables, is an error. | Once the APIs are gone, the only way back to manual drawing is a direct dependency on internals. |

---

## 1. Audit: where mods and mod-facing crates draw by hand

Measured on the sources as of this document (HexMod-Yog HEAD `ca7677d` plus its working tree, which another
agent is changing; `lib.rs:34` still refers to the deleted `ui::hex_grid`). Line counts exclude `#[cfg(test)]`.

### 1.1 HexMod-Yog

| Where | What it draws / does | Why the current API forced it |
|---|---|---|
| `src/ui/render_lib.rs:13-15, 284-340` | `Mesh::draw`: a GLSL 330 position-color shader (`VERT`/`FRAG`, 304-321), a VAO/VBO, `set_blend`, `set_depth`, `draw_arrays(Triangles)`; GL objects cached in `static GL: Mutex<Option<Gl>>` (340) | yog-ui can draw only axis-aligned rects, gradients, text and images. There is no triangle, line or polygon primitive. |
| `src/ui/render_lib.rs:210-282` | `Mesh`: hand-built `x,y,r,g,b,a` vertex arrays, `tri`, `fan`, `quad`, `offset` | Same: no geometry type in the API. |
| `src/ui/render_lib.rs:344-441` | `draw_line_seq`: thick polyline tessellation with miter-clamped joins, round join fans, round caps, per-vertex tail→head color | No stroked-path primitive. |
| `src/ui/render_lib.rs:443-453` | `draw_spot`: hexagonal dot as a triangle fan | No shape/marker primitive. |
| `src/ui/render_lib.rs:468-503` | `draw_pattern_from_points`: two strokes (5 px color, 2 px "screen" lightened core), nodes darkened ×0.9 (`dodge`) | No layered stroke style; colors computed per call. |
| `src/ui/render_lib.rs:505-581` | `make_zappy`: noise-displaced subdivision animated by time, with "readability offset" at revisited points | No path effects, no animation clock. |
| `src/ui/render_lib.rs:95-141, 583-609` | `coord_to_px`, `px_to_coord`, `range_around`, `to_lines`, `find_center`, `get_centered_pattern`: hex↔pixel layout and fit-to-box | No coordinate-system container, no fit-content layout for drawings. |
| `src/ui/render_lib.rs:28-93, 158-208` | Own `Vec2`, ARGB channel helpers, `screen`/`dodge` color math, `ticks()` clock from `Instant` | No color expressions, no shared geometry types, no UI clock. |
| `src/ui/noise.rs:1-149` | Bit-exact port of Minecraft's `SimplexNoise` + `java.util.Random` to drive the zaps | The noise is only needed because the mod tessellates the zaps itself. |
| `src/ui/pattern_render.rs:1-50` | `mesh_pattern`/`draw_pattern`: a pattern centred in a box (tooltip/book look: pale outline, dark core, grid dots, start dot); its own docs say "nothing calls it yet (Yog has no item-tooltip hook)" | No shape elements, no tooltip component hook. |
| `src/ui/spellcasting.yui:10` | The whole casting screen is one `<Surface source="hexcasting:spellcasting_grid">` | Only a `Surface` can draw arbitrary geometry or take raw pointer events. |
| `src/ui/spellcasting.rs:291-361` | `Grid::mesh`: guide dots fading around the cursor, resolved patterns, the stroke being drawn | As above. |
| `src/ui/spellcasting.rs:362-413` | Stack/ravenmind boxes as two translucent quads (`draw_box`), ravenmind placement math, 1.5× scale "not supported" comment (540) | No inset fill layers, no `transform`. |
| `src/ui/spellcasting.rs:440-469, 601-614` | `describe`/`fit`/`draw_desc`: hand text measurement, word-break, grey `...` drawn as a second text call | `Label` can't trim to one line with a styled ellipsis. |
| `src/ui/spellcasting.rs:509-543` | `GridSurface::paint`: locks the session, runs the "staff left the hand" tick check inside paint, builds and draws the mesh, draws text, computes the pulsing ravenmind alpha from `sin(ticks)` | No per-frame VM hook other than paint, no animation. |
| `src/ui/spellcasting.rs:473-578, 634-681` | `static SESSION: Mutex<Session>`, `GridSurface` registration through `Once`, lock-and-forward `pointer`/`scroll` | `Surface` takes `&self` and is global, so state needs a static. mvvm-v2 §12.8 planned to fix the lock but kept the manual drawing. |
| `src/ui/spellcasting.rs:44-50` | Layout constants in pixels (`LHS_IOTAS_ALLOCATION`, `BOX_COLOR`, `ELLIPSIS_COLOR`) | Style constants in Rust because there is no markup for this screen. |
| `src/protocol.rs:51-60`, `src/iota/mod.rs:86-98` | Color tables per resolution kind and per iota type | Style data in Rust, used only by the drawing code. (Not drawing itself; moves to the theme, §13.1.) |
| `src/casting/env.rs:141-195` | `Spray::spawn` cone/sphere emitter math; `spawn_dust` building `minecraft:dust{color:[…]}` vs `minecraft:dust r g b s` strings and sniffing which syntax works into `static DUST_SYNTAX: AtomicU8` | No custom particle types and no version-independent colored particle API. A manual hack around both. |
| *history:* `src/ui/grid_gl.rs` (160 lines, deleted in `a4f63ea`) | A second GLSL program: SDF circles and capsules with anti-aliasing, `uMode` uniform, `static GL: Mutex` | Same cause. Each rewrite of the screen produced a new shader. |

Total hand-written rendering Rust in HexMod today: **~1,090 lines** (`render_lib.rs` 610, `noise.rs` 149,
`pattern_render.rs` 50, drawing in `spellcasting.rs` ~240, particle hack ~50), plus ~130 lines of surface/session
glue.

### 1.2 Yog-Modlist

No GL. Two "manual hacks" by layout:

| Where | What | Why |
|---|---|---|
| `src/stat_bar.yui:9-11`, `src/stat_bar.rs:22-24` | A progress bar as a `Row` with a `Spacer` whose width is a computed percentage | No `ProgressBar` and no fill-fraction material. (The materials doc already deletes `StatBar`; §13.3 adds the generic control.) |
| `src/pill.yui:7-11` | A 1 px frame drawn as an outer `Row` with padding 1 around an inner `Row` | No frame brush in today's yog-ui. Covered by materials' `frame` layer. |
| `src/mod_list.rs:33` | Badge colors per source as `u32` constants | Style in Rust; moves to the theme. |

### 1.3 Yog-Pipes

No drawing code (`src/` has no UI or gfx use). Nothing to remove. Its future visible pipes go through `.yblock`
materials (materials doc §12.3).

### 1.4 Mod-facing crates (the API that invites manual drawing)

| Where | What | Fate |
|---|---|---|
| `yog-api/src/lib.rs:18-20` | Re-exports `gfx_core`, `gfx_draw2d`, `gfx_gl`, `gfx_image`, `GfxContext` to every mod | Deleted (R2). |
| `yog-gfx/src/lib.rs:1-289`, `gl/mod.rs` | Public buffers, VAOs, GLSL programs, textures, draw calls, blend/depth/scissor/viewport; doc example teaches mods to write shaders | GL half deleted; `draw2d` becomes loader-internal (§11). |
| `yog-gfx/src/draw2d.rs` | `Draw2D` text/rect/gradient/item/image calls | Internal to yog-ui/yog-book; not re-exported. |
| `yog-ui/src/surface.rs:47-97` | `Surface` trait (`paint(&self, gfx, rect)`, `pointer`, `scroll`) + global `static SURFACES: Mutex` registry | Deleted. |
| `yog-cli/src/yui.rs:253, 298, 545-566, 729` | `<Surface source=…>` element and diagnostics | Deleted; `<Surface>` becomes a build error pointing at §4/§6. |
| `yog-kit/tests/surface.rs` | Surface pointer capture tests | Rewritten against element pointer events (§9). |
| `yog-api/src/registry.rs:2268-2310` | `on_ui_render` ("clicks are forwarded as `click:X:Y` … hit-test on your own stored layout"), `on_hud_render`, `on_world_render` with a raw `GfxContext` | Deleted. HUD → `<Hud>` (§10.5); world → `.yoverlay`/decals (§10.3-10.4). |
| `yog-book/src/renderer.rs:37-385` | `BookGl`: its own GLSL program, vertex format, PNG decoding, texture uploads, quads | Already deleted by ybook B7; restated here because it is the last in-tree user of the GL ABI. |
| `yog-book` `BookPage::Pattern` (`lib.rs:97`, `renderer.rs:1672`) | A Hex page type in the loader (draws only labels) | Already deleted by ybook B5. |
| `Yog-IDLE/src-tauri/src/designer/gfx.rs:146-204` | The recorder's GL half is inert: anything drawn through GL is invisible in the designer | Becomes moot: there is no GL half (§11). |

### 1.5 Root causes

Every item above comes from one of these gaps:

1. No geometry primitive below "rectangle": no triangles, lines, polygons, strokes or markers.
2. No style for strokes: width, joins, caps, gradients along a path, layered strokes.
3. No path effects and no clock, so animated or procedural strokes need custom tessellation.
4. No coordinate-system container, so mods convert between their own coordinates and pixels.
5. No pointer events with arguments on ordinary elements, so input meant `Surface`.
6. No `opacity`/`transform`/animation in markup.
7. Text can't trim to a line with a styled ellipsis.
8. No content formats for particles, data-driven item looks, block-face drawings, world markers or HUD panels.
9. An open escape hatch (`Surface` + raw `GfxContext`) that made writing GL the path of least resistance.

---

## 2. What the original Hex Casting renders

The original sources are not in the workspace. This inventory is from the HexMod-Yog port's own references
(`render_lib.rs`, `pattern_render.rs` and `spellcasting.rs` cite `RenderLib.kt`, `GuiSpellcasting`,
`PatternTooltipComponent`, `PatternColors`, `StrokeSettings`) and from knowledge of Hex Casting 0.11.x. Items
marked † must be checked against the original source by the porting work (port-faithfully rule) before their
content files are written; the primitives they need do not depend on the details.

| Feature | Original | Technique in the original | Yog replacement |
|---|---|---|---|
| Pattern strokes ("glow") | `RenderLib.drawPatternFromPoints`, `drawLineSeq` | Two triangle strips with round joins/caps: width 5 in the pattern color, width 2 in the "screen" color (each channel halfway to white); tail→head color gradient when Ctrl is held | `Polyline` + stroke material (§5.3) |
| Zappy motion | `RenderLib.makeZappy`, `NOISE = SimplexNoise(9001)` | 10 noise-jittered sub-points per segment, scrolled by `ClientTickCounter`, irregularity 0.2 (success) / 0.9 (mishap) / 0.1 (being drawn); revisited points pulled 0.2 into their segments | `jitter` + `separate_revisits` path effects (§5.2) |
| Node spots | `RenderLib.drawSpot` | Small hexagon fans, color ×0.9 | `markers` layer |
| Grid dots | `GuiSpellcasting.render` | Dots within 3 cells of the cursor, size and alpha fading with distance, skipping used points; hex size = √(w·h/512) | `<Lattice cell="area:512">` + `<Lattice.Dots>` (§6) |
| Staff casting GUI | `GuiSpellcasting` | Full-screen, no background, game not paused; stack list top-left in a translucent double box (70% width), ravenmind top-right scaled 1.5× with a pulsing alpha, iota text cut with a grey "..." | `.yui` screen (§13.1) |
| Pattern tooltip on scrolls/slates | `PatternTooltipComponent` | 128×128 pattern drawn over the scroll/slate paper texture, pale outline + dark core, grid dots, start dot | `.yitem` `tooltip` component (§10.2) |
| Item looks from data | item property overrides | Filled/empty cyphers, trinkets, artifacts (`has_patterns`), focus/spellbook overlays per stored iota, sealed state; phial fill level† | `.yitem` `variants` (§10.2) |
| Media bar | `ItemMediaHolder.getBarWidth/getBarColor` | Vanilla item bar, color lerped by fill† | `.yitem` `bar` |
| Patterns in the world | slate block-entity renderer, wall scroll entity renderer, akashic bookshelf† ; 0.11 bakes them into textures (`PatternTextureManager`) | Pattern drawn on a block face or paper, re-baked when the data changes | `.yblock` `decals` (§10.3) |
| Spell circle | circle components, impetus | Blockstate `energized` switching textures/glow as the circle runs; slates light up in the caster's pigment† | blockstate-driven materials (materials doc §4) + decal `emissive_when` |
| Conjure particles | `ConjureParticle`, `MsgCastParticleS2C`, `ParticleSpray` | Custom translucent full-bright particle, colored per particle by the pigment (time + direction), spawned in cones/spheres | `.yparticle` + `particles::emit` (§10.1) |
| Sentinel | `HexAdditionalRenderers` | Rotating wireframe shape at a world position in the pigment color, drawn with line render type, visible through walls† | `.yoverlay` (§10.4) |
| Scrying lens | `ScryingLensOverlays` | HUD panel next to the crosshair with icon + text rows for the looked-at block | `<Hud anchor="crosshair">` (§10.5) |
| Book pattern pages | Patchouli `hexcasting:pattern`/`manual_pattern` | Same look as the tooltip, centred in the page | `PatternPage` component with `Lattice` + `Polyline` (§13.2) |

---

## 3. Principles

1. **Declared, not drawn.** A mod states *what* is on screen (shapes, text, images), *where* (layout, lattice),
   *how it looks* (materials, themes) and *what changes it* (bindings, animations). The loader draws.
2. **Logic stays Rust, and stays pure.** The pattern state machine, which dots are used, what a stroke's points
   are, how a pigment colors a particle: this is game logic and stays in the mod. It returns data; it never takes
   a graphics context.
3. **One recordable path.** Everything ends in `draw2d_*` calls, now including `draw2d_triangles`. IDLE records
   them, so every screen, book page, tooltip and decal previews in the designer with design-time data.
4. **Generic or nothing.** A primitive goes into the loader only if a non-Hex mod could use it unchanged. Hex's
   numbers live in Hex's files.
5. **Deterministic tessellation.** The stroker and the noise are pure functions in yog-ui with golden-image tests
   in IDLE's rasterizer, so the in-game and designer pictures match.

---

## 4. Vector shapes in `.yui`

### 4.1 Elements

| Element | Geometry attributes | Notes |
|---|---|---|
| `Line` | `x1 y1 x2 y2` | |
| `Polyline` | `points` | Open path. |
| `Polygon` | `points` | Closed path; `fill` applies. |
| `Path` | `data` (SVG path syntax: `M L H V C Q A Z`) | Curves flattened by the tessellator at a tolerance of ¼ GUI px. |
| `Ellipse` | `cx cy rx ry` (or `w`/`h` box) | |
| `Marker` | `x y`, `shape` (`circle`, `square`, `polygon:N`, `star:N`), `radius`, `rotation` | A single dot. |

All are leaves. They draw in their parent's content box: in a `Canvas` (absolute positioning, already a yog-ui
container), in a `Lattice` (§6), or in any panel, where the shape's own `w`/`h` box is its coordinate space.
Shapes take the common attributes (`visible`, `opacity`, `transform`, `class`, `style`, `theme`, events).

### 4.2 Geometry bindings

`points` accepts a literal (`"0,0 10,0 15,8"`) or a binding returning any `IntoIterator<Item = P>` where
`P: Into<yog_api::geom::Point>`. `Point` is `{ x: f32, y: f32 }`; `geom::LatticePoint` (§6) converts. Codegen
stores the collected `Vec<Point>` in a `Slot`, so an unchanged point list costs a comparison, not a re-tessellation.
yog-ui caches the triangle list per shape keyed by (points, resolved material, size, time bucket for animated
effects).

### 4.3 Look

Either attributes for the common case or a material:

```xml
<Polyline points="{x:Bind path()}" stroke="#FF55FF55" stroke_width="2" join="round" cap="round"/>
<Polygon points="0,0 20,0 10,15" fill="#80FFFFFF" stroke="@Colors.edge" stroke_width="1"/>
<Polyline points="{x:Bind pts()}" material="mat:hexcasting:ui/pattern" param.seed="{x:Bind seed}"/>
```

- `stroke`, `stroke_end` (gradient along the path), `stroke_width`, `join` (`round|miter|bevel`), `cap`
  (`round|butt|square`), `fill`, `antialias` (default `false`, §15 Q5).
- `material="mat:…"` replaces all of these with the material's layers (§5). The plain attributes then become
  **material parameters**: `stroke="…"` sets `@stroke`, `stroke_end` sets `@stroke_end`, `param.<name>="…"` sets
  any other declared parameter. Themes and conditional styles can set them like any attribute
  (`.ytheme` class `Evaluated { stroke: #C87385DE; stroke_end: #C8FECBE6; param.irregularity: 0.2 }`).

### 4.4 Many shapes

Repetition uses the existing items host, so a list of patterns is a template, not a loop in Rust:

```xml
<ItemsControl items="{x:Bind patterns}">
    <DataTemplate>
        <Polyline points="{x:Bind points()}" material="mat:hexcasting:ui/pattern" class="{x:Bind kind}"
                  param.seed="{x:Bind $index}"/>
    </DataTemplate>
</ItemsControl>
```

New binding names: `$index` (the row index inside a template) and `$e` (event arguments, §9). `class="{x:Bind
kind}"` accepts any value implementing `yog_ui::ClassName` (derived for fieldless enums: variant name → class).

---

## 5. Stroke materials (`.ymat` extension)

The materials doc's schema (§2.2 there) is extended. Existing UI layers (`texture`, `fill`, `gradient`, `frame`,
`frame_gradient`, `material`) keep their meaning for boxes. The new layers apply to shapes; a shape material may
not use box layers and vice versa (build error).

### 5.1 Parameters and expressions

```jsonc
"params": { "seed": 0, "irregularity": 0.2, "gradient": 0 }   // declared with defaults; types inferred
```

Implicit parameters: `@stroke`, `@stroke_end`, `@fill` (from attributes), `@t` (0..1 along the path, per vertex),
`@cell` (the lattice cell size in GUI px, §6), `@size` (the shape's box, `min(w, h)`), `@proximity` (lattice dots,
§6.3), `@time` (client ticks with partial tick).

Any numeric or color field may be an expression over a **closed** set, checked at `yog build`:

| Form | Meaning |
|---|---|
| `#AARRGGBB`, numbers, `@name` | literals, parameters |
| `+ - * /`, `min(a,b)`, `max(a,b)`, `clamp(x,a,b)` | arithmetic |
| `mix(c1, c2, t)` | per-channel linear blend (alpha too) |
| `lighten(c, k)` | RGB moved `k` toward white, alpha kept (Hex's `screen` is `lighten(c, 0.5)`) |
| `darken(c, k)` | RGB × (1 − k), alpha kept (Hex's `dodge` is `darken(c, 0.1)`) |
| `alpha(c, a)` | replace alpha (0..1) |

No conditionals, no user functions: a boolean is a 0/1 number used as a factor (`@t * @gradient`).

### 5.2 Path effects

Applied in order to the shape's points before any layer draws; layers can opt to draw on the source points
instead (`"on": "source"`).

**`separate_revisits`** `{ "amount": 0.2 }`: a vertex equal to an earlier vertex of the same path is pulled
`amount` of the way into its adjacent segments and the path is split there, so strokes crossing an earlier point
stay readable. (Exactly `makeZappy`'s dup-index "daisy chain" handling, keyed on geometry only.)

**`jitter`**: noise displacement along each segment.

```jsonc
{ "jitter": {
    "hops": 10,                         // sub-points per segment
    "amplitude": 2.5,                   // × hop length
    "speed": 0.1,                       // noise scroll per tick (0 = static; >0 marks the element animated)
    "irregularity": "@irregularity",    // extra per-hop perturbation
    "seed": "@seed",
    "noise": "minecraft:simplex:9001",  // built-in noise sources: minecraft:simplex:<seed>, value:<seed>
    "angle_scale": 3,                   // θ = angle_scale · noise · 2π
    "lanes": [1337, 69420],             // noise y-coordinates for angle and radius
    "envelope_ramp": 8,                 // radius × min(1, ramp · (0.5 − |0.5 − p|))
    "last_segment": 1.0                 // fraction of the last segment's hops emitted (open ends)
} }
```

For segment *i* and hop *j* (progress `p = j / (hops + 1)`, `z = @time · speed`): perturb
`= noise(i, j, sin z) · irregularity`; `θ = angle_scale · noise(i + p + perturb − z, lanes[0], seed) · 2π`;
`r = noise(i + p − z, lanes[1], seed) · amplitude · hop_len · envelope(p)`. This is a complete, generic
specification of `makeZappy`, so Hex's lines shake exactly as the original's (port-faithfully), while nothing
in it mentions patterns. The noise implementation is HexMod's `noise.rs`, moved into yog-ui as the
`minecraft:simplex` source (it is Minecraft's algorithm, not Hex's).

**`trim_end`** `{ "length": 0.2 }`: shortens the path's last segment by a fraction (generic "don't draw into the
cursor").

### 5.3 Shape layers

| Layer | Fields |
|---|---|
| `stroke` | `width`, `color` (expression; may use `@t`), `join`, `cap`, `cap_steps` (round cap/join resolution; default 10 = Hex's 18°), `t` (`index` = vertex index / count, as Hex; or `length`), `on` |
| `fill` (shape) | `color`; even-odd or non-zero `rule` |
| `markers` | `at` (`vertices`, `start`, `end`, `vertices_but_last`), `shape`, `radius`, `rotation`, `color`, `on` (`source` = the points before effects) |

Layers draw bottom to top. `antialias: true` on a stroke adds a one-pixel alpha fringe in the tessellator (no
shader).

### 5.4 Hex's pattern materials (in HexMod, not the loader)

```jsonc
// HexMod-Yog/content/ui/pattern.ymat → hexcasting:ui/pattern
{
  "params": { "seed": 0, "irregularity": 0.2, "gradient": 0 },
  "path_effects": [
    { "separate_revisits": { "amount": 0.2 } },
    { "jitter": { "hops": 10, "amplitude": 2.5, "speed": 0.1, "irregularity": "@irregularity", "seed": "@seed",
                  "noise": "minecraft:simplex:9001", "angle_scale": 3, "lanes": [1337, 69420],
                  "envelope_ramp": 8, "last_segment": 1.0 } }
  ],
  "layers": [
    { "stroke": { "width": 5, "color": "mix(@stroke, @stroke_end, @t * @gradient)", "join": "round", "cap": "round" } },
    { "stroke": { "width": 2, "color": "mix(lighten(@stroke, 0.5), lighten(@stroke_end, 0.5), @t * @gradient)",
                  "join": "round", "cap": "round" } },
    { "markers": { "at": "vertices", "on": "source", "shape": "polygon:6", "radius": 2, "color": "darken(@stroke_end, 0.1)" } }
  ]
}
```

```jsonc
// hexcasting:ui/pattern_wip: the stroke being drawn (no spot under the cursor)
{ "based_on": "mat:hexcasting:ui/pattern",
  "layers": { "2": { "markers": { "at": "vertices_but_last" } } } }   // patch layer 2
```

```jsonc
// hexcasting:ui/grid_dot: GuiSpellcasting's guide dots
{ "layers": [ { "markers": { "shape": "polygon:6", "radius": "2 * @proximity",
                             "color": "mix(#0066CCB2, #FF7FFFE5, @proximity)" } } ] }
```

```jsonc
// hexcasting:ui/iota_box: drawBox, two translucent rectangles, one inset
{ "layers": [ { "fill": "#50303030" }, { "fill": "#50303030", "inset": 2.5 } ] }
```

(`inset` on `fill` is a small addition to the materials doc's box layers.)

---

## 6. `<Lattice>`: a coordinate system

### 6.1 Attributes

```xml
<Lattice kind="hex" w="100%" h="100%" cell="area:512" origin="center"> … </Lattice>
```

| Attribute | Values |
|---|---|
| `kind` | `hex` (pointy-top axial, Hex Casting's), `hex_flat`, `square` |
| `cell` | a length (`18`), `area:N` (cell size so that N cells fill the box: `√(w·h / N)`, Hex's `512`), or `fit` |
| `fit` | with `cell="fit"`: `contain` scales the children's bounding box into the element; `fit_scale` (`0.667` = Hex's "a third each side of the centre"), `max_cell` (a length expression: `max(16, 25%)`) |
| `origin` | `center`, `top_left`, or `x,y` |

Children (`Polyline`, `Marker`, `Lattice.Dots`, anything with `x`/`y`) use lattice coordinates: `LatticePoint
{ q: f32, r: f32 }` (axial for hex, column/row for square). Fractional coordinates are allowed, so a stroke can end
at the pointer.

### 6.2 Pointer

Pointer events on a `Lattice` carry, besides pixel positions, `$e.coord` (nearest lattice point, integer) and
`$e.pos` (fractional `LatticePoint`). Distances in `geom::hex` are in cell units, so mod logic like Hex's snap
("move at least one cell size from the anchor, then pick the direction by angle") is written without pixels:

```rust
// yog_api::geom::hex (pure math, no drawing)
pub fn distance(a: LatticePoint, b: LatticePoint) -> f32;          // cell units
pub fn nearest(p: LatticePoint) -> Axial;
pub fn direction(from: Axial, to: LatticePoint) -> HexDirection;   // 6-way by angle
pub fn neighbors(a: Axial) -> [Axial; 6];
pub fn range(center: Axial, radius: i32) -> impl Iterator<Item = Axial>;
```

HexMod's `HexCoord`/`HexDir` convert to and from `Axial`/`HexDirection` (or are replaced by them; a mod decision).

### 6.3 `<Lattice.Dots>`

```xml
<Lattice.Dots around="pointer" radius="3" falloff="1 4" exclude="{x:Bind used_spots}"
              material="mat:hexcasting:ui/grid_dot"/>
```

Draws one marker per lattice point within `radius` of `around` (`pointer`, `center`, or a bound point), skipping
`exclude` (any `Contains<Axial>`). `@proximity` is `clamp(1 − (d − falloff₀) / (falloff₁ − falloff₀), 0, 1)` with `d`
in cells: `falloff="1 4"` is Hex's "full brightness within one cell, gone at four".

---

## 7. Time and animation

- **Clock.** yog-ui has one client clock, `@time` in ticks with the partial tick (`ClientTickCounter`'s total).
  Elements with an animated material (`jitter.speed > 0`) or an `<Animate>` repaint every frame; others don't.
- **`opacity`** (0..1) on any element multiplies the alpha of everything it and its children draw (text color,
  fills, triangles, image tint). Items drawn through the item renderer ignore it (documented).
- **`transform`** on any element: `scale(s)`, `scale(sx, sy)`, `rotate(deg)`, `translate(x, y)`, combined
  left-to-right, around `transform_origin` (default top-left). It is a render transform: layout is unaffected
  (WPF `RenderTransform`). Text scaling uses `draw2d_text_scaled` (materials doc §5.3); rotated text is not
  supported (build warning).
- **`<Animate>`** as a child of the element it animates:

```xml
<Label …>
    <Animate property="opacity" from="0.588" to="1" period="31.4t" easing="sine" repeat="oscillate"/>
</Label>
```

  `property` is any numeric or color attribute (`opacity`, `transform` components as `scale`/`rotate`, `param.*`,
  colors); `period`/`duration` in ticks (`t`) or seconds (`s`); `repeat` = `once | loop | oscillate`; `easing` =
  `linear | sine | quad | cubic` (in/out variants); `begin="{x:Bind cond}"` restarts it when `cond` becomes true.
  This is all mvvm-v2's "out of scope" animation needs here, and it is enough for Hex: the ravenmind pulse is
  `lerp((sin(0.2·t) + 1) / 2, 150, 255) / 255`, i.e. an oscillation between 0.588 and 1 with period 2π / 0.2 ≈
  31.4 ticks.

---

## 8. Text additions

- `<Label max_lines="1" trim="ellipsis" trim_text="..." trim_color="#FFAAAAAA">`: cut at the last space that fits
  (or the last character), then draw `trim_text` in `trim_color` (alpha follows the label). This is exactly
  Hex's `getDisplayWithMaxWidth` + grey "...", and removes `describe`/`fit`/`draw_desc`.
- `scale` on `Label` (via `draw2d_text_scaled`), and `transform` (§7).

---

## 9. Input on elements

Every element may declare `pointer_pressed`, `pointer_moved` (while pressed: drag), `pointer_hover`,
`pointer_released`, `pointer_left`, `scrolled`. Each takes a command binding; `$e` holds `x`, `y` (local GUI px),
`button`, `dy` (wheel), and inside a `Lattice` also `coord`/`pos`. A press captures the pointer to the element
until release, as `Surface` did. A handler returning `bool` (or `Flow`) decides whether the event is consumed;
`()` means consumed.

Modifier keys are a built-in binding source: `{x:Bind $input.ctrl}`, `$input.shift`, `$input.alt` (read on the
render thread each pass). Hex's "gradient while Ctrl is held" is `param.gradient="{x:Bind $input.ctrl}"`.

---

## 10. Beyond screens

### 10.1 Particles: `.yparticle`

```jsonc
// HexMod-Yog/content/particles/conjure.yparticle → hexcasting:conjure
{
  "textures": ["hexcasting:particle/cloud"],     // sprite set; animated by age or random per particle
  "render": "translucent",                        // opaque | translucent | additive
  "full_bright": true,
  "lifetime": [12, 24],                           // ticks, random in range †
  "size":  { "from": 0.15, "to": 0.0, "easing": "linear" },   // †
  "alpha": { "from": 1.0,  "to": 0.0 },                        // †
  "physics": { "gravity": 0.0, "drag": 0.96, "collide": false },  // †
  "color": "spawn"                                // the color passed at spawn; or a constant
}
```

(† values copied from `ConjureParticle` while porting.) The build registers it; each host registers a real
particle type and a provider for it on every loader and version.

```rust
// yog_api::particles (server side; sends one packet per batch to clients in range)
pub fn emit(level: &Level, kind: Particle, emitter: Emitter, color: impl FnMut(Vec3) -> u32);
pub struct Emitter { pub pos: Vec3, pub vel: Vec3, pub spread: f64, pub fuzz: f64, pub count: u32 }
// spread: max angle from vel (cone); fuzz: radius of the start sphere. `color` gets each particle's unit
// direction, so per-particle colors (Hex's pigments) are mod logic, computed once per particle.
```

Hex's `Spray` becomes `Emitter` data; `Spray::spawn`, `random_on_sphere`, `spawn_dust` and `DUST_SYNTAX`
(`env.rs:141-195`) are deleted. Vanilla particles go through the same call with `vanilla::particles::DUST` and a
color, so no mod formats particle command strings again.

### 10.2 Items: `bar`, `variants`, `tooltip`

```jsonc
// HexMod-Yog/content/items/battery.yitem (excerpt)
"bar": {
  "value": "data:hexcasting:media / data:hexcasting:start_media",
  "color": "mix(#FF…, #FF…, @value)",            // † MediaHelper's colors
  "visible": "data:hexcasting:media != null"
},
"variants": [
  { "when": "data:hexcasting:patterns != null", "texture": "hexcasting:item/cypher_filled" }
],
"tooltip": { "component": "hexcasting:PatternTooltip", "when": "data:hexcasting:pattern != null",
             "props": { "pattern": "data:hexcasting:pattern" } }
```

- `data:<key>` reads the item's custom data (1.20.1 NBT tag, 1.21 `custom_data` component), mapped by the host.
  The same expression set as §5.1, plus `!= null` and comparisons (closed set, build-checked).
- `variants` compile to vanilla model overrides/predicates (1.20.1) or item model conditions (1.21.x); the host
  registers the generic predicate functions. First match wins; the base model is the fallback.
- `tooltip` renders a `.yui` `<Component>` inside the vanilla tooltip (a `ClientTooltipComponent` in the host),
  with the item data as properties. `PatternTooltip` is a markup-only HexMod component: the scroll paper image and
  a `Lattice` + `Polyline` with the book material (§13.2). No Rust.

### 10.3 Blocks: `decals`

```jsonc
// HexMod-Yog/content/blocks/slate/slate.yblock (excerpt)
"decals": [
  { "face": "facing", "component": "hexcasting:SlatePattern", "props": { "pattern": "be:pattern" },
    "resolution": 64, "inset": 0.002, "emissive_when": "state:energized" }
]
```

The runtime renders the component with yog-ui into an offscreen texture when its props change (cache keyed by
the props' hash, shared between identical blocks), and the host's generic block-entity renderer draws it as a
textured quad on the face (`facing` resolves from the blockstate). This is what Hex 0.11's `PatternTextureManager`
does by hand, and it keeps a spell circle of hundreds of slates cheap. Wall scrolls (entities) get the same
`decals` key in a later `.yentity`. Depends on block-entity data being readable by key (`be:`); see §16.

### 10.4 World overlays: `.yoverlay`

```jsonc
// HexMod-Yog/content/overlays/sentinel.yoverlay → hexcasting:sentinel
{
  "shape": { "lines": [[0, 0.5, 0], [0.5, 0, 0], …] },   // † the original's vertex list
  "width": 2, "color": "@color", "see_through": true,
  "spin": { "axis": "y", "period": "200t" }, "bob": { "amplitude": 0.05, "period": "80t" }
}
```

```rust
let h = overlay::show(player, res::overlays::SENTINEL, pos, params! { color: pigment.color(t, Vec3::ZERO) });
overlay::hide(player, h);
```

Shapes: `lines` (3D polylines), `billboard` (a material facing the camera), `box`. Shown per player or per level;
the host draws them in its world-render pass with the line/translucent render types.

### 10.5 HUD layers: `<Hud>`

```xml
<Hud id="hexcasting:lens" anchor="crosshair" offset="8,0" x:ViewModel="LensVm" visible="{x:Bind has_lines()}">
    <ItemsControl items="{x:Bind lines}">
        <DataTemplate><Row gap="2"><Image source="{x:Bind icon}" w="16" h="16"/><Label text="{x:Bind text}"/></Row></DataTemplate>
    </ItemsControl>
</Hud>
```

A `<Hud>` root is a screen without input, drawn in the HUD pass, anchored to `crosshair`, a corner or an edge.
Its VM is fed by client events (`#[on(LookAt)]`), so what a lens shows is mod logic, and how is markup.

---

## 11. ABI and runtime

**Added to `YogGfxApi`** (recordable):

```rust
/// Triangles in GUI px. `texture` empty = untextured; otherwise a texture/material id whose UVs are in `u, v`.
pub draw2d_triangles: unsafe extern "C" fn(verts: *const YogVertex2D, count: u32, texture: YogStr),
#[repr(C)] pub struct YogVertex2D { pub x: f32, pub y: f32, pub u: f32, pub v: f32, pub color: u32 }
```

Hosts implement it with the GUI vertex consumer (`VertexFormat.Mode.TRIANGLES`, position-color or
position-tex-color) per version; IDLE's recorder gets a `DrawOp::Triangles` and its rasterizer fills them. This
supersedes ybook's `draw2d_quad`.

**Added to `YogApi`**: `particles_emit` (kind, emitter, colors), `overlay_show`/`overlay_hide`; item/block defs
carry `bar`/`variants`/`tooltip`/`decals` from the compiled content (no per-call ABI: the runtime reads
`yog/particles.json`, `yog/overlays.json` and the extended block/item JSON from the `.yog`, like materials).

**Runtime-internal**: decal render-to-texture (the runtime drives yog-ui into an FBO through the host), the
`<Hud>` pass, tooltip components.

**Deleted from the ABI** (an `ABI_VERSION` bump, not a minor): the GL half of `YogGfxApi` (`buf_*`, `vao_*`,
`prog_*`, `tex_create/delete/bind`, `draw_arrays`, `draw_elements`, `set_blend`, `set_depth`, `set_viewport`,
`view_proj`, `camera_pos`, `player_pos`), and `on_hud_render`, `on_world_render`, `on_ui_render` from `YogApi`.
yog-book's `BookGl` (ybook B7) was the last in-tree user.

**Crates**: yog-gfx keeps `GfxContext` + `draw2d` for yog-ui and yog-book only; yog-api stops re-exporting it.
yog-ui gains `shape`, `tessellate` (the generalized `drawLineSeq`, fills via ear clipping, markers), `effects`
(path effects), `noise`, `lattice`, `anim`. `yog_api::geom` (points, lattice math) is new and pure.

---

## 12. Deletions (no compatibility)

| Removed | Replacement |
|---|---|
| `yog_ui::Surface`, `surface::{register, unregister, get}`, `WidgetKind::Surface`, `<Surface>` | shapes (§4), `Lattice` (§6), element events (§9) |
| mvvm-v2 §12.8 `SurfaceView`, `source="{x:Bind field}"`, `output=` (never implemented) | same |
| `Registry::on_hud_render`, `on_world_render`, `on_ui_render` | `<Hud>`, `.yoverlay`, decals; screens are `.yui` |
| `yog_api::{gfx_gl, gfx_core, gfx_draw2d, gfx_image, GfxContext}` | none for mods |
| GL half of `YogGfxApi`; `view_proj`/`camera_pos`/`player_pos` | `draw2d_triangles`; overlays get positions from the host |
| ybook §3.4 `HexPattern` widget, §6.5 `draw2d_quad` | `Lattice` + `Polyline` + material; `draw2d_triangles` |
| boilerplate F12 `surface::shared::<T>()` | nothing to share |
| HexMod `render_lib.rs`, `noise.rs`, `pattern_render.rs`, `GridSurface`, `SESSION`, `spawn_dust`, `DUST_SYNTAX` | §13 |
| Yog-Modlist `StatBar` (already deleted by the materials doc), `Pill`'s nested-row frame | `ProgressBar` control; `frame` material layer |

`yog migrate` has nothing to rewrite mechanically here: code that drew by hand has to be redone in markup. The
format step that removes `<Surface>` emits a diagnostic per occurrence linking this document.

---

## 13. Before and after

### 13.1 HexMod: the staff's casting screen

**Before** (`src/ui/spellcasting.yui` + the drawing in `spellcasting.rs` and `render_lib.rs`, abridged):

```xml
<Screen id="hexcasting:spellcasting" width="427" height="240" anchor="fill" pause="false" x:ViewModel="SpellcastingVm">
    <Surface w="100%" h="100%" source="hexcasting:spellcasting_grid" />
</Screen>
```

```rust
impl Surface for GridSurface {
    fn paint(&self, gfx: &GfxContext, rect: Rect) {
        let mut s = session();                                   // static Mutex
        … staff-in-hand check inside paint …
        let mut mesh = grid.mesh(client::control_down());        // 70 lines: dots, patterns, wip stroke
        let d2 = gfx.draw2d();
        let width_of = |t: &str| d2.text_width(t, "");
        mesh.append(grid.boxes(&width_of));                      // draw_box: two quads
        mesh.offset(rect.x, rect.y);
        mesh.draw(gfx);                                          // GLSL program, VAO, VBO, blend, draw_arrays
        for desc in grid.stack_descs(&width_of) { draw_desc(&d2, &width_of, desc, …); y += 10.0; }
        if let Some(desc) = grid.ravenmind_desc(&width_of) {
            let opacity = lerp(((ticks() * 0.2).sin() as f32 + 1.0) / 2.0, 150.0, 255.0) as u32;
            // "The original draws this 1.5× larger; Draw2D has no text scale."
            draw_desc(&d2, &width_of, desc, …, (desc.color & 0x00FF_FFFF) | opacity << 24);
        }
    }
    fn pointer(&self, ev: Pointer, size: (f32, f32)) -> bool { … lock, px → hex, forward … }
    fn scroll(&self, dy: f32, _: (f32, f32)) -> bool { … }
}
// + render_lib.rs: Mesh, GLSL, draw_line_seq, draw_spot, make_zappy, coord_to_px, px_to_coord, …
// + noise.rs: SimplexNoise port
```

**After**, `src/ui/spellcasting.yui`:

```xml
<Screen id="hexcasting:spellcasting" anchor="fill" pause="false" background="none" modal="true"
        theme="hexcasting:ui/casting" x:ViewModel="SpellcastingVm"
        open_on="protocol::OPEN_GUI" close_when="{x:Bind !holding_staff()}"
        scrolled="{x:Bind scroll($e.dy)}">
    <Lattice kind="hex" w="100%" h="100%" cell="area:512" origin="center"
             pointer_pressed="{x:Bind press($e.coord)}"
             pointer_moved="{x:Bind drag($e.pos)}"
             pointer_hover="{x:Bind hover($e.pos)}"
             pointer_released="{x:Bind release()}">
        <Lattice.Dots around="pointer" radius="3" falloff="1 4" exclude="{x:Bind used_spots}"
                      material="mat:hexcasting:ui/grid_dot"/>
        <ItemsControl items="{x:Bind patterns}">
            <DataTemplate>
                <Polyline points="{x:Bind points()}" material="mat:hexcasting:ui/pattern" class="{x:Bind kind}"
                          param.seed="{x:Bind $index}" param.gradient="{x:Bind $input.ctrl}"/>
            </DataTemplate>
        </ItemsControl>
        <Polyline points="{x:Bind wip()}" material="mat:hexcasting:ui/pattern_wip" class="Drawing"
                  param.seed="{x:Bind patterns.len()}" param.gradient="{x:Bind $input.ctrl}"/>
    </Lattice>

    <Column x="10" y="10" w="70%" padding="7 5 3 5" bg="mat:hexcasting:ui/iota_box" visible="{x:Bind !stack.is_empty()}">
        <ItemsControl items="{x:Bind stack.iter().rev()}">
            <DataTemplate>
                <Label text="{x:Bind display()}" class="{x:Bind kind()}" h="10" shadow="true"
                       max_lines="1" trim="ellipsis" trim_color="#FFAAAAAA"/>
            </DataTemplate>
        </ItemsControl>
    </Column>

    <Column anchor="top_right" margin="0 10 10 0" w="15%" h="15" transform="scale(1.5)" transform_origin="top_right"
            padding="5" bg="mat:hexcasting:ui/iota_box" visible="{x:Bind ravenmind.is_some()}">
        <Label text="{x:Bind ravenmind?.display()}" class="{x:Bind ravenmind?.kind()}" max_lines="1"
               trim="ellipsis" trim_color="#FFAAAAAA" shadow="true">
            <Animate property="opacity" from="0.588" to="1" period="31.4t" easing="sine" repeat="oscillate"/>
        </Label>
    </Column>
</Screen>
```

`content/ui/casting.ytheme` (style data moved out of Rust: `protocol.rs:51-60`, `iota/mod.rs:86-98`):

```jsonc
{ "based_on": "yog:bare", "classes": {
  "Unresolved": { "stroke": "#C87F7F7F", "stroke_end": "#C8CCCCCC", "param.irregularity": 0.9 },
  "Evaluated":  { "stroke": "#C87385DE", "stroke_end": "#C8FECBE6", "param.irregularity": 0.2 },
  "Escaped":    { "stroke": "#C8DDCC73", "stroke_end": "#C8FFFAE5", "param.irregularity": 0.2 },
  "Undone":     { "stroke": "#C8B26B6B", "stroke_end": "#C8CCA88E", "param.irregularity": 0.2 },
  "Errored":    { "stroke": "#C8DE6262", "stroke_end": "#C8FFC7A0", "param.irregularity": 0.9 },
  "Invalid":    { "stroke": "#C8B26B6B", "stroke_end": "#C8CCA88E", "param.irregularity": 0.9 },
  "Drawing":    { "stroke": "#FF64C8FF", "stroke_end": "#FFFECBE6", "param.irregularity": 0.1 },
  "Double":     { "color": "#FF55FF55" }, "Vec3": { "color": "#FFFF3030" }, …
} }
```

The VM (mvvm-v2 shape) keeps only logic. The drawing-related methods are data accessors:

```rust
#[view_model]
#[derive(Default)]
pub struct SpellcastingVm {
    hand: Hand,
    patterns: ObsVec<ResolvedPattern>,
    stack: Vec<Iota>,
    ravenmind: Option<Iota>,
    draw: DrawState,
    used_spots: HashSet<Axial>,
    pointer: LatticePoint,
    scroll_acc: f64,
    ambiance: SoundHandle,
}

#[view_model]
impl SpellcastingVm {
    // #[on(Opened<OpenGui>)] start, #[on(Closed)] stop, #[on(Back)] back, #[recv] result, holding_staff:
    // unchanged from mvvm-v2 §15.2.

    fn press(&mut self, at: Axial) { /* draw_start: same rules, no pixels */ }
    fn drag(&mut self, at: LatticePoint) {
        self.pointer = at;
        /* draw_move: `geom::hex::distance(anchor, at) < 1.0` → return; `direction(anchor, at)`; append/undo rules */
    }
    fn hover(&mut self, at: LatticePoint) { self.pointer = at; }
    fn release(&mut self) { /* draw_end → protocol::NEW_PATTERN.send(...) */ }
    fn scroll(&mut self, dy: f32) { /* notch accumulation → protocol::SCROLL.send(...) */ }

    /// The stroke being drawn, ending at the pointer (data, not drawing).
    fn wip(&self) -> Vec<LatticePoint> {
        match &self.draw {
            DrawState::BetweenPatterns => Vec::new(),
            DrawState::JustStarted(s) => vec![(*s).into(), self.pointer],
            DrawState::Drawing { start, wip, .. } => wip.positions_from(*start).map(Into::into).chain([self.pointer]).collect(),
        }
    }
}

impl ResolvedPattern { pub fn points(&self) -> Vec<LatticePoint> { self.pattern.positions_from(self.origin).map(Into::into).collect() } }
```

### 13.2 The book pattern page

**Before**: the loader's `BookPage::Pattern` draws only the op id and "input → output" labels
(`yog-book/src/renderer.rs:1672-1684`), so the pattern is not drawn at all. HexMod's `pattern_render.rs`
(`mesh_pattern`, 50 lines) plus `render_lib.rs`'s tessellation, centring and GL (~430 lines) exist for it but
nothing can call them. ybook §3.4 planned a Hex-side `<HexPattern>` widget calling `draw2d_quad` on top of that.

**After**, `HexMod-Yog/src/ui/book/pattern_page.yui` (from ybook §3.4, the drawing line changed):

```xml
<Component name="PatternPage" book_page="hexcasting:pattern" …>
    <Property name="op_id" type="text"/> <Property name="input" type="text" default=""/>
    <Property name="output" type="text" default=""/> <Property name="text" type="book_text"/>
    <Column w="100%" h="100%" gap="2">
        <BookHeader text="{x:Bind title()}"/>
        <Lattice kind="hex" w="100%" h="64" cell="fit" fit="contain" fit_scale="0.667" max_cell="max(16, 25%)">
            <Polyline points="{x:Bind points()}" material="mat:hexcasting:ui/book_pattern"/>
        </Lattice>
        <Label text="{x:Bind signature_line()}" align="center" visible="{x:Bind has_signature()}"/>
        <BookText text="{x:Bind text}"/>
    </Column>
</Component>
```

```jsonc
// hexcasting:ui/book_pattern (PatternColors + StrokeSettings.fromStroke(0.8/16) of the box)
{ "layers": [
  { "markers": { "at": "vertices", "shape": "polygon:6", "radius": "1.6 * max(0.08 * @cell, 1)", "color": "#80D2C8C8" } },
  { "stroke":  { "width": "0.1 * @size",  "color": "#FFD2C8C8", "join": "round", "cap": "round" } },
  { "stroke":  { "width": "0.05 * @size", "color": "#FF554D54", "join": "round", "cap": "round" } },
  { "markers": { "at": "start", "shape": "polygon:6", "radius": "2 * max(0.08 * @cell, 1)", "color": "#FF5B7BD7" } }
] }
```

The code-behind keeps ybook's lookups (`title`, `signature_line`, …) and gains `points()`, a one-line call to
the pattern's `positions()`. The same `Lattice` + material renders the scroll/slate tooltip (§10.2) and the slate
decal (§10.3).

### 13.3 Line counts

Hand-written Rust only (non-test). "After" for logic is an estimate with the current formatting.

| Code | Before | After | Notes |
|---|---:|---:|---|
| HexMod `ui/render_lib.rs` | 610 | 0 | stroker/markers → yog-ui; hex math → `yog_api::geom::hex`; `positions_from` → `math` |
| HexMod `ui/noise.rs` | 149 | 0 | → yog-ui `minecraft:simplex` |
| HexMod `ui/pattern_render.rs` | 50 | 0 | → `book_pattern.ymat` |
| HexMod `spellcasting.rs` drawing (`mesh`, `boxes`, `draw_box`, `describe`/`fit`/`draw_desc`, `paint`, px helpers, style consts) | ~240 | 0 | → `.yui`, `.ymat`, `.ytheme` |
| HexMod `spellcasting.rs` surface/session glue | ~130 | 0 | mvvm-v2 + §9 |
| HexMod `spellcasting.rs` grid logic + VM | ~310 | ~270 | pixel math gone; rules unchanged |
| HexMod `casting/env.rs` particle emitter + dust hack | ~50 | ~3 | `particles::emit` |
| HexMod color tables (`protocol.rs`, `iota/mod.rs`) | 22 | 0 | → `casting.ytheme` |
| **HexMod rendering Rust total** | **~1,250** | **0** | |
| HexMod markup/content added | — | ~55 `.yui`, ~50 `.ymat`, ~25 `.ytheme` | declarative, previewable in IDLE |
| Yog-Modlist `stat_bar.rs` + layout hacks | 25 (+24 `.yui`) | 0 | `ProgressBar`, `frame` |

---

## 14. Staged plan

Each stage ships on its own: crates bumped (breaking where noted), published, the mods updated in the same change,
`dev.sh` run only with the user's permission.

| Stage | Content | Depends on |
|---|---|---|
| **N0** Geometry core | `yog_api::geom`; yog-ui shapes (`Line`, `Polyline`, `Polygon`, `Path`, `Ellipse`, `Marker`) with attribute styling; the stroker (generalized `drawLineSeq`), fills, markers; `draw2d_triangles` in the ABI (minor), runtime, Java hosts (all loaders × versions), IDLE recorder + rasterizer; `opacity`, `transform`; yui codegen + IDLE `yui.ts` for the new elements; golden-image tests | — |
| **N1** Stroke materials | `.ymat` `params`, expressions, `stroke`/`fill`/`markers` layers, `path_effects` (`separate_revisits`, `jitter`, `trim_end`), `minecraft:simplex` noise in yog-ui; `material=`/`param.*` on shapes; theme classes setting params; build validation | materials doc S1–S4 (material pipeline, `materials.json`) |
| **N2** Lattice, input, animation | `<Lattice>`, `Lattice.Dots`, `cell`/`fit` modes, lattice pointer args; element pointer/wheel events with `$e`, `$input`, `$index`; `<Animate>` + clock; `Label` trim/scale | mvvm-v2 M-stages for commands with arguments |
| **N3** HexMod casting screen + deletions | Migrate §13.1; delete `render_lib.rs`, `noise.rs`, `pattern_render.rs`, `GridSurface`. Then **delete** `Surface`, `<Surface>`, `on_*_render`, gfx re-exports, the GL ABI half (`ABI_VERSION` bump), mvvm-v2 §12.8 from its plan; `yog check` lint (R13) | N0–N2, mvvm-v2 M9b (open_on/close_when) |
| **N4** Book and tooltips | `PatternPage` per §13.2 (with ybook's stages); `.yitem` `tooltip` components; `PatternTooltip` | ybook custom pages, N3 |
| **N5** Particles | `.yparticle`, `particles::emit`, host particle registration per loader/version; HexMod's conjure particle and pigment colors; delete `spawn_dust`/`DUST_SYNTAX` | — (can run in parallel with N1–N4) |
| **N6** Item data looks | `.yitem` `bar`, `variants` (+ `data:` refs mapped per version) | — |
| **N7** World | `.yblock` `decals` (render-to-texture, BE renderer), `.yoverlay`, `<Hud>`; HexMod slates, akashic shelves, sentinel, lens | block-entity data access, N4 |

---

## 15. Open questions (with recommendations)

1. **Q1 Keep a hidden `Surface` escape hatch?** *Recommendation: no.* Every manual-GL file in the workspace came
   through it. A gap found later is a loader feature request (generic primitive), handled like N0–N7.
2. **Q2 Zappy fidelity: exact or similar?** *Recommendation: exact.* The `jitter` parameters (§5.2) describe
   `makeZappy` completely with Minecraft's simplex noise, so the port matches the original (port-faithfully) and the
   effect is still generic.
3. **Q3 Mod-registered path effects (pure Rust functions point list → point list)?** *Recommendation: no.* It
   reopens per-mod geometry code and makes the designer depend on a compiled mod for every preview. If a second
   real effect is needed, add it to the loader.
4. **Q4 Where to tessellate: yog-ui (mod side) or the runtime?** *Recommendation: yog-ui.* One ABI function, and
   the IDLE recorder gets final triangles. The cost is code size in each mod, which is small.
5. **Q5 Anti-aliasing by default?** *Recommendation: off* (the original draws aliased triangles); `antialias:
   true` per stroke for mods that want it, implemented as an alpha fringe without shaders.
6. **Q6 Decals: render-to-texture or direct geometry each frame?** *Recommendation: render-to-texture, cached by
   props hash* (the original 0.11 approach). Direct geometry costs thousands of triangles per spell circle per frame.
7. **Q7 Particle colors: server callback or declarative color sources?** *Recommendation: callback* (`FnMut(dir)
   -> u32` per particle) now; a declarative `cycle` color source only if a second mod needs it.
8. **Q8 Lattice kinds.** *Recommendation:* `hex` (pointy), `hex_flat`, `square` in N2; triangle lattices when
   asked for.
9. **Q9 Animation scope.** *Recommendation:* `<Animate>` as in §7 only. State transitions (animate on property
   change) and keyframes later, when a screen needs them.
10. **Q10 Expression language size in `.ymat`/`.yitem`.** *Recommendation:* the closed set of §5.1 plus `data:`
    refs, comparisons and `!= null` for item content. No conditionals or user functions.
11. **Q11 Style tables (pattern and iota colors) in the theme?** *Recommendation: yes*, as §13.1 shows: they
    are pure style, and the theme lets resource packs and IDLE edit them. The VM exposes the enum; `class=` maps it.
12. **Q12 Lint severity for direct `yog-gfx`/`yog-abi` dependencies.** *Recommendation: error.* With the APIs
    removed, a direct dependency can only mean manual drawing against internals.
13. **Q13 `world_render` for debugging tools (e.g. Yog's own debugger overlays)?** *Recommendation:* those are
    loader code and may use runtime internals; nothing is exposed to mods.

---

## 16. Coordination with other designs

- **materials-and-vanilla-ui.md**: gains §5 here as an extension of its `.ymat` schema (new layer kinds, `params`,
  expressions, `path_effects`, `inset` on `fill`, `based_on` + layer patching for materials). Its §12.2 HexGrid/SDF
  sentence is withdrawn. Blockstate-driven materials (circle `energized`) are its §4 territory.
- **mvvm-v2.md**: §12.8 and Q16 superseded; V14's "custom-drawn surfaces" now reads "no custom drawing". New
  binding names `$e`, `$input`, `$index`, `ClassName` for `class=`, and element events (§9) go into its binding
  grammar and codegen (§5.10 split modules). §7's "animations out of scope" is replaced by §7 here.
- **ybook.md**: §3.4's `<HexPattern>`/`draw2d_quad` superseded by §13.2; B5/B7 unchanged. IDLE's book designer
  renders pattern pages with no special support, because they are ordinary components.
- **boilerplate-reduction.md**: F12's `surface::shared` dropped; F1's action table is what `PatternPage` reads.
- **Block-entity data**: decals (§10.3) need block-entity fields readable by key from content (`be:<key>`); this
  belongs to the block-entity work in yog-registry and is a prerequisite of N7.
- **Substrate-Platform**: untouched. The recorder and rasterizer changes are in Yog-IDLE (`src-tauri/src/designer`).
