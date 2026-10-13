# Design: `.ybook` guide books and the Book Designer

Status: design only. Nothing is implemented yet, and nothing was built while writing this.

Scope: Yog-Mod-Loader (yog-book, yog-cli, yog-abi, yog-runtime, yog-ui, yog-api, yog-registry, Java hosts),
Yog-IDLE (a new Book Designer), Substrate-Platform (generic pieces only), and HexMod-Yog (the first real book).

This implements decision **D3** of [`boilerplate-reduction.md`](boilerplate-reduction.md): *"the book is its own
content file with a WYSIWYG designer in Yog-IDLE (like the `.yui` screen designer) that shows the rendered book
while editing; the Rust builders are not the authoring path."* It also answers **Q8** of
[`materials-and-vanilla-ui.md`](materials-and-vanilla-ui.md): the book moves onto draw2d + materials as part of this
work, so that the IDE can render it.

The project has **no backward compatibility**. Everything in §12 is deleted, not deprecated.

---

## 0. Summary of decisions

| # | Decision | Why |
|---|---|---|
| B1 | **A book is one `.ybook` manifest plus one `.yentry` file per entry**: `content/books/<book>.ybook` and `content/books/<book>/<category path>/<entry>.yentry`. The book id is `<mod id>:<book>`. An entry's id is its path (`patterns/math`), and its category is its folder. | Hex Casting's book has 9 categories, 82 entries and 447 pages. In one file that is ~5k lines, where every edit and every merge touches the same file. Per-entry files give small diffs, parallel work without conflicts, and the same shape as Patchouli (so imported books and `$(l:…)` links keep their paths). The manifest holds everything about the *book* in one place (metadata, skin, item, macros, the category tree and the order of entries), so a reorder changes one file. |
| B2 | **JSON, like `.yblock`/`.yitem`/`.ymat`/`.yres`** (comments and trailing commas allowed, unknown keys are errors, a `$schema` for editors). | Pages are data, not a visual tree, so XML (`.yui`) buys nothing. The content compiler, the JSON editor with schema completion and the minimal-edit patcher are reused. |
| B3 | **Texts are `@Set.name` refs into `.yres`** (recommended, `@Book.page.media.1`), or literal neutral text (single-language books). Translations are the `.yres` culture files (`Book.ru_ru.yres`). There is no `i18n` flag. | This is how every other Yog format localizes. The existing `.yres` editor already shows cultures side by side and highlights missing translations. The designer creates the `.yres` entries itself, so authors never type a key. |
| B4 | **Markup is Patchouli's `$(…)` syntax**, with the same codes, built-in macros and per-book macros, plus a few Yog additions (§4). | Ported books (Hex: ~1000 strings in 13 languages) work verbatim. Modders already know it. It does not collide with `.yres` `{placeholder}` braces. In the designer, a formatting toolbar inserts the codes, so they are rarely typed by hand. |
| B5 | **Built-in page types** cover Patchouli's: `text`, `spotlight`, `crafting`, `smelting` (all cooking types), `stonecutting`, `smithing`, `image`, `entity`, `multiblock`, `link`, `relations`, `empty`. **Custom page types are `.yui` components** with `book_page="<ns>:<type>"`; the page's fields are the component's properties. | There is one extension mechanism for UI, and it already has what custom pages need: typed properties for the property grid, markup-only rendering without a build, compiled rendering through the designer host, and generated registration. Hex's pattern page becomes a Hex component. The loader has no Hex logic (the `Pattern` page variant is deleted). |
| B6 | **No Rust for a book.** `yog build` compiles the files to `yog/books/<book>.json` inside the `.yog`, and the runtime loads it like `yog/materials.json`. The book item is declared in the manifest. Typed handles (`books::thehexbook::patterns::MATH`) are generated for code that wants to open the book. | A book is data. Loading data from the `.yog` also allows reloading a book in a running game without the code ABI match that hot reload needs. |
| B7 | **yog-book becomes a pure layout and draw library on yog-ui + draw2d + materials.** Its raw-GL path (`BookGl`, embedded PNG decoding, the SVG and fontdue stubs) is deleted. The runtime links it. Mods no longer do. | Only draw2d calls can be recorded by the IDE's recording `YogGfxApi`. With that, the designer draws the book with **the same crate and the same code** as the game. |
| B8 | **The designer renders in Rust, not with a TypeScript port.** `src-tauri` links yog-book and draws through the existing recorder and rasterizer (Minecraft font and textures, vanilla pack). It returns the picture plus a hit map (pages, text runs, links, slots). | A TS port (as `layout.ts` is for `.yui`) would be a second implementation of text flow and Patchouli's page geometry, and would drift. The `.yui` designer only needs `layout.ts` for instant drag feedback, and books have no free-form dragging. |
| B9 | **Text flows.** A `text` page whose text does not fit continues on as many extra pages as needed, laid out at run time with the game's font in the current language. `"flow": false` gives Patchouli's clipping. | Translations are longer than English (Russian by about 20–30%). Patchouli forces authors to split pages by hand per language, which is what Hex's `page1..page10` keys are. |
| B10 | **Patchouli import** (`yog import patchouli`) converts a Patchouli book (book.json, categories, entries, templates, lang JSON / flattened JSON5) to `.ybook`/`.yentry`/`.yres`, and writes component stubs for custom page types. HexMod's book is migrated with it from the original Hex Casting sources. | Porting mods is a main use case for Yog. The Hex book is the real test: 447 pages and 6 custom page types. |

---

## 1. Current state (what this replaces)

**Data model** (`yog-book/src/lib.rs`, 970 lines). `Book`/`BookCategory`/`BookEntry`/`BookPage` are serde structs
with builder methods. `BookPage` has 12 variants, including the Hex-specific `Pattern` and the stubs `Svg` and
`CustomText`, whose features (`svg`, `fonts`) are empty and never enabled. `to_json` is hand-written, string by
string, for the mod → runtime boundary. `book_ui::build_book_ui` is an unused older UI.

**Rendering** (`renderer.rs`, 1690 lines). `BookRenderer` builds a yog-ui tree for text and buttons. It also has its
own GL pipeline (`BookGl`: shader, embedded `book_brown.png`/`crafting.png`, a sprite blitter), plus "overlays"
drawn through `ctx.draw2d()` (items, MC text, textures). The `book_texture`/`filler_texture` fields are ignored. The
entry view shows *one* content page on the left and the entry list on the right, so it is not Patchouli's
two-page spread. Imported books that pad with `empty` pages to put a pattern on the left and its description on the
right (Hex's `math` entry does this throughout) therefore come out wrong. Text is split on `\n` only: there is no
markup, word wrap only comes from yog-ui labels, and macros are stored and never applied. `Relations`, `Entity`,
`Image` and `Pattern` are placeholders.

**i18n.** `Book.i18n = true` means "every text is a key". `BookRenderer::set_translator` notices a language change by
re-translating the book name every frame (a "probe").

**Registration.** A mod builds the `Book` in Rust (`HexMod-Yog/src/book.rs`, 124 lines, 31 `page(Book::…_PAGEn_KEY)`
lines) and calls `registry.register_book(&book)`. That serializes it to JSON through ABI `register_book`, and the
runtime parses it back into a `BookRenderer`. The book item is coupled to the book **by id**: when Java creates the
items, an item whose id equals a registered book id becomes a `YogBookItem` (`YogHost.java`,
`NativeBridge.nativeBookJson`). That is why `thehexbook.yitem` must say `"register": "manual"` and be registered by
hand *before* `register_book`. `yog-registry` also has a `BookRecipe` (`yog:crafting_book`) and
`StartupGrant.book`/`AdvancementReward.book`, which hand out a `minecraft:written_book` with a `yog_book` NBT tag.
Nothing reads that tag.

**Leftovers.** `HexMod-Yog/assets/hexcasting/yog_book/thehexbook/**` is a partial copy of the Patchouli JSON. Nothing
reads it.

**IDE.** Yog-IDLE has no book support. Its relevant infrastructure:
- the `.yui` designer (`src/designer/*`: `ScreenDesigner`, `YuiSplitEditor`, `yuiText.ts` patching, Toolbox via
  `activeDesigner.ts`, Document Outline, PropertyGrid);
- the Rust render backend (`src-tauri/src/designer/`: `gfx.rs` recording `YogGfxApi`, `raster.rs`, `font.rs` with
  pixel-exact Minecraft text, `images.rs`, `components.rs` for markup-only components, and `custom.rs`/`host.rs`
  for compiled components drawn by the mod's own library);
- the `.yres` editor.

---

## 2. Files and layout

```
hexmod/
  content/
    books/
      thehexbook.ybook                 # the manifest      → book hexcasting:thehexbook
      thehexbook/
        basics/media.yentry            # entry basics/media in category basics
        basics/geodes.yentry
        patterns/math.yentry
        patterns/spells/basic.yentry   # category patterns/spells (a subcategory of patterns)
        …
  resources/
    Book.yres                          # neutral texts (en_us) of the book
    Book.ru_ru.yres                    # Russian
  src/ui/book/pattern_page.yui         # custom page type hexcasting:pattern (a <Component book_page=…>)
  textures/gui/book.png                # optional skin texture (Patchouli book.png layout)
```

- **Book id** = `<mod id>:<stem of the .ybook>`. Several books per mod are fine.
- **Entry id** = path under `<book>/` without the extension: `patterns/math`. The **category** is the folder:
  `patterns`. Nested folders are subcategories (`patterns/spells`, whose parent is `patterns`), as in Patchouli.
  This is the same id-is-the-path rule as F5 in `boilerplate-reduction.md`.
- Moving an entry to another category is a file move. The designer does it and rewrites links (§8.8).
- The folder `content/books/` is under `content/`, so the content compiler finds books with the same scan as
  `.yblock`/`.yitem`, and the Project Explorer's content node shows them. Project Explorer nests
  `thehexbook/` under `thehexbook.ybook`, as it nests `.rs` under `.yui`.

**Why not one file per book.** It would work for a 5-entry book. It fails at Hex's size: ~5k lines of JSON, every
designer save rewrites the whole file, two people editing different entries always conflict, and the split view's
source pane would show a huge document instead of the entry being edited. **Why not a file per page.** Pages are
small and are read in sequence. 447 files would make the tree and review worse, and reordering pages would become
a cross-file operation. **Why categories live in the manifest.** There are few of them, and their order, icons and
entry order are what a reorder changes. Keeping them in one place makes a drag-reorder a one-line diff.

---

## 3. Formats

### 3.1 `.ybook` (manifest)

```jsonc
// content/books/thehexbook.ybook
{
  "$schema": "https://yog.dev/schema/ybook-1.json",
  "name": "@Book.name",                          // nameplate + item name default
  "subtitle": "@Book.subtitle",                  // optional, under the nameplate (Patchouli "subtitle")
  "landing_text": "@Book.landing",
  "texts": "Book",                               // the .yres set the designer adds new texts to (default "Book")

  "skin": {                                      // optional; default = Yog's brown book (§7.3)
    "texture": "hexcasting:gui/patchi_book",     // a texture in Patchouli's book.png layout
    "filler": "hexcasting:gui/patchi_filler",    // the picture empty pages show
    "nameplate_color": "#00072B",
    "text_color": "#000000", "header_color": "#333333",
    "link_color": "#0000EE", "link_hover_color": "#8800EE",
    "font": "minecraft:default",                 // any yog-ui font id
    "parts": { "separator": "mat:hexcasting:gui/book_separator" }   // per-part material overrides (§7.3)
  },

  "item": {                                      // the book item; `false` = no item (opened from code only)
    "name": "@Items.thehexbook",                 // default: the book's name
    "tooltip": "@Items.thehexbook.desc",
    "model": { "parent": "generated", "textures": { "layer0": "hexcasting:item/thehexbook" } },
                                                 // default: <ns>:item/<book> if that texture exists, else minecraft:item/book
    "creative_tab": "hexcasting:hexcasting",
    "give_on_first_join": false
  },

  "show_progress": false,                        // unread markers and the completion counter (§6.7)
  "macros": {                                    // applied after Patchouli's built-in macros (§4.3)
    "$(thing)": "$(#8d6acc)",
    "$(action)": "$(#fc77be)",
    "_Media": "$(#74b3f2)Media/$"
  },

  "categories": [                                // display order
    {
      "id": "basics",                            // = folder under thehexbook/
      "name": "@Book.category.basics",
      "description": "@Book.category.basics.desc",
      "icon": "item:hexcasting:staff/oak",       // a paint ref (§3.6)
      "entries": ["media", "geodes", "couldnt_cast", "start_to_see"]   // display order
    },
    {
      "id": "patterns", "name": "@Book.category.patterns", "icon": "item:minecraft:bookshelf",
      "unlock": "hexcasting:root",
      "entries": ["readers_guide", "basics", "numbers", "math", "…"]
    },
    { "id": "patterns/spells", "name": "@Book.category.spells", "icon": "item:hexcasting:artifact",
      "entries": ["basic", "blockworks", "…"] }
  ]
}
```

Rules:
- Each `categories[].id` must be a folder under `<book>/`. A folder without an entry in `categories` is an error,
  with an IDE quick fix *Add category*.
- `entries` gives the order. Entry files that are not listed are appended in name order, and `yog content check`
  reports this as info. The designer always keeps the list complete.
- A subcategory is listed after its parent and is shown inside the parent's entry list, before the entries, as in
  Patchouli.

### 3.2 `.yentry`

```jsonc
// content/books/thehexbook/patterns/math.yentry
{
  "name": "@Book.entry.math",
  "icon": "item:minecraft:blaze_rod",
  "unlock": "hexcasting:root",                   // §3.5
  "read_by_default": true,
  "color": "#54398A",                            // optional title color in the entry list (Patchouli entry_color)
  "pages": [
    { "text": "@Book.page.math.numvec" },        // "type" defaults to "text"
    { "type": "empty" },
    { "type": "hexcasting:pattern", "anchor": "add",
      "op_id": "hexcasting:add", "input": "num|vec, num|vec", "output": "num|vec",
      "text": "@Book.page.math.add.1" },
    { "text": "@Book.page.math.add.2" },
    { "type": "crafting", "recipe": ["hexcasting:staff/oak", "hexcasting:staff/spruce"],
      "title": "@Book.page.staff.crafting.header", "text": "@Book.page.staff.crafting.desc" }
  ]
}
```

Fields common to every page:

| Field | Meaning |
|---|---|
| `type` | A built-in type (no namespace) or `<ns>:<type>` (a custom page, §3.4). Default `text`. |
| `anchor` | A name that links can target: `$(l:patterns/math#add)`. Unique in the entry. |
| `unlock` | The page is hidden until the condition holds (§3.5). |

Patchouli's entry `priority`/`sortnum` are replaced by the manifest's `entries` order. `secret` is a field of the
condition (§3.5).

### 3.3 Built-in page types

All text fields (`text`, `title`, `name`, `link_text`) are `@Set.name` refs or literals, with markup. Item fields take
an **item stack ref**: `"minecraft:diamond"`, `{ "id": "minecraft:diamond", "count": 3 }`, a tag `"#minecraft:logs"`,
or a list. Lists and tags cycle once per second, as in Patchouli.

| Type | Fields | Drawn as (Patchouli's geometry) |
|---|---|---|
| `text` | `text`, `title?`, `flow?` (default `true`) | Body text. The entry title header is on the entry's first page, and `title` is a header on later pages. Overflow continues on extra pages (B9). |
| `spotlight` | `item`, `title?`, `text?`, `link_recipe?` | Item in the spotlight frame, title (default: item name), text below. |
| `crafting` | `recipe` (id or list of ids), `recipe2?`, `title?`, `text?` | 3×3 grid + output, shapeless marker, one or two recipes per page. A list cycles (this covers Hex's `crafting_multi`). |
| `smelting` | `recipe`, `recipe2?`, `text?` | Input, station icon, output. The station follows the recipe type: furnace, blast furnace, smoker or campfire. |
| `stonecutting`, `smithing` | `recipe`, `recipe2?`, `text?` | As Patchouli. |
| `image` | `images` (texture ids, cycled by arrows), `title?`, `border?`, `text?` | 106×106 picture area. A texture is a Texture (materials D1), so `.png.mcmeta` animation works. |
| `entity` | `entity`, `scale?`, `offset?`, `rotate?`, `default_rotation?`, `name?`, `text?` | The living entity, rotating (§6.6). |
| `multiblock` | `name`, `multiblock` (`pattern` + `mapping`, Patchouli's format) or `structure` (a structure id), `enable_visualize?`, `text?` | An isometric preview of the structure (§6.6, first release). |
| `link` | `url`, `link_text`, `text?` | A button that opens the URL through vanilla's "open link?" confirmation. |
| `relations` | `entries`, `title?`, `text?` | A list of entry links. |
| `empty` | `filler?` (default `true`) | The skin's filler picture, or nothing. |

Deleted variants: `pattern` (Hex), `svg`, `custom_text`.

Each built-in type has a schema in yog-book (`page_schema(type) -> &[FieldSpec]`). It drives build validation and
the designer's property grid, so the two cannot disagree.

### 3.4 Custom page types: `.yui` components

A mod adds a page type by writing a `.yui` `<Component>` with one extra attribute:

```xml
<!-- HexMod-Yog/src/ui/book/pattern_page.yui -->
<Component name="PatternPage" book_page="hexcasting:pattern"
           description="@Strings.component.pattern_page" icon="item:hexcasting:scroll">
    <Property name="op_id"  type="text" doc="Action whose pattern is drawn"/>
    <Property name="input"  type="text" default=""/>
    <Property name="output" type="text" default=""/>
    <Property name="text"   type="book_text" doc="Description under the pattern"/>
    <Column w="100%" h="100%" gap="2">
        <BookHeader text="{x:Bind title()}"/>
        <HexPattern w="100%" h="64" signature="{x:Bind signature()}"/>
        <Label text="{x:Bind signature_line()}" align="center" visible="{x:Bind has_signature()}"/>
        <BookText text="{x:Bind text}"/>
    </Column>
</Component>
```

```rust
// pattern_page.rs (code-behind): op_id → the action's name and its pattern, from Hex's action table (boilerplate F1)
yog_api::ui_include!("pattern_page");
impl PatternPage {
    fn title(&self) -> String { crate::casting::action_by_id(&self.op_id).map(|a| a.name()).unwrap_or_default() }
    fn signature(&self) -> String { crate::casting::action_by_id(&self.op_id).map(|a| a.signature()).unwrap_or_default() }
    fn has_signature(&self) -> bool { !self.input.is_empty() || !self.output.is_empty() }
    fn signature_line(&self) -> String { format!("{} → {}", self.input, self.output) }
}
```

- **Fields = properties.** A page `{ "type": "hexcasting:pattern", "op_id": …, "text": … }` sets the component's
  properties by name. Page fields that are not properties are build errors. `anchor`, `unlock` and `type` are
  reserved.
- **New property type `book_text`**: a text that may be `@Set.name`, is translated by the runtime before the
  component sees it, and holds book markup. A plain `text` property is translated too, but its markup is not
  interpreted.
- **New elements** (yog-ui; drawn by the host through yog-book, §6.5):
  - `<BookText text=…/>`: book markup, wrapped to the element's width, with the book's colors, macros and
    clickable links;
  - `<BookHeader text=…/>`: the centred header plus the separator sprite, as the built-in pages draw it;
  - `<BookItem item=…/>`: an item stack ref with cycling and the vanilla tooltip on hover.
  They work only inside a `book_page` component. Elsewhere they are a build error.
- **Geometry.** The component is laid out into the page body (116×156 GUI px, or the body below the entry title on
  an entry's first page). `design_w`/`design_h` default to that size.
- **Markup-only components** (no code-behind) render in the designer without a build. This makes them Patchouli's
  JSON templates, in markup. **Compiled components** (Hex's pattern page, which looks up the pattern in mod code)
  render through the designer host, like compiled `.yui` instances today.
- **Registration is generated.** The component codegen also emits a `book_page` registration (the ctor registrar,
  as F8 does for screens). The runtime gets `register_book_page(type, …)` (§6.5). There is no hand-written Rust
  apart from the code-behind the component needs anyway.
- **Other mods' books.** A page type is global by id. An addon whose book `extends` Hex's book (§3.8) can use
  `hexcasting:pattern` when Hex is installed. The build validates against the dependency's `.yog` if it can find it,
  and otherwise warns.

Drawing primitives Hex needs and yog-ui lacks: a line/polygon primitive for the pattern strokes. This adds
**`draw2d_quad(x0,y0,…,x3,y3,color)`** to `YogGfxApi` (generic, recordable). With it, `HexPattern` is a yog-ui
custom widget in Hex. The existing `HexGrid` stays a GL `Surface` for the casting screen, which the designer cannot
record anyway.

### 3.5 Unlock conditions

```jsonc
"unlock": "hexcasting:enlightenment"                       // shorthand: an advancement
"unlock": { "advancement": "hexcasting:root", "secret": true }
"unlock": { "any": [ { "mod": "pehkui" }, { "mod": "gravitychanger" } ] }
"unlock": { "all": [ "hexcasting:root", { "not": { "mod": "create" } } ] }
"unlock": { "mc": ">=1.21" }                               // version-dependent content (§6.8)
```

| Condition | True when | Evaluated |
|---|---|---|
| `"<id>"` / `{ "advancement": id }` | the client player has the advancement | live, client side (new ABI query) |
| `{ "mod": id }` | a mod with that id is loaded (Yog or loader-native) | once, at load |
| `{ "mc": "<semver req>" }` | the running Minecraft version matches | once, at load |
| `{ "all": [] }`, `{ "any": [] }`, `{ "not": c }` | combinators | — |
| `{ "flag": "<ns>:<name>" }` | a flag the mod sets from code (`books::flags::X.set(true)`) | live |

`secret: true` (on any condition object) hides a locked entry or category completely. Otherwise it is listed
greyed out with a lock, as Patchouli does. Conditions apply to categories, entries and pages.

Patchouli's `flag` strings (`mod:pehkui`, `debug`) are converted to `mod`/`flag` conditions by the importer. Hex's
`hexcasting:any_interop` becomes an `any` over the interop mods, so it is data and needs no code.

### 3.6 Icons and pictures

Every "what to draw" field uses the **paint ref** grammar of `materials-and-vanilla-ui.md` §2.3:
- `item:<id>` is the item renderer (3D block items, glint);
- `<ns>:<path>` is a texture;
- `mat:<ns>:<path>` is a material.

Patchouli's "item id or `.png` path" becomes `item:hexcasting:staff/oak` or `minecraft:mob_effect/nausea`.
The importer converts it, and the designer's paint picker inserts full ids. Validation, renames and per-version
presence follow the same rules as every other format (§6.8).

### 3.7 The book item

The manifest's `item` is compiled like a `.yitem` with a generated id `<book id>` (`hexcasting:thehexbook`). It
uses the same model and name rules, and `ItemDef` gains `book: Option<String>`, so the host creates a
`YogBookItem` **because the def says so**, not because the ids are equal. The "manual registration before
`register_book`" ordering problem goes away, and so does `thehexbook.yitem`.

A crafting recipe for the book is an ordinary recipe that outputs the item. `give_on_first_join` produces a
`StartupGrant` of the item. `BookRecipe` and the `written_book` + `yog_book` NBT grants are deleted.

### 3.8 Addon books (`extends`)

Hex's ecosystem (Hexal, MoreIotas, …) adds entries to *thehexbook*. An addon writes
`content/books/thehexbook.ybook` with `"extends": "hexcasting:thehexbook"`. That manifest may contain only
`categories` (new ones, or existing ids with extra `entries`) and `macros`. Its entries live in its own
`thehexbook/` folder. The runtime merges the addon into the base book after both are loaded. Link targets across
the merge use the same paths. This is a late stage (§13, B9).

---

## 4. Text markup

### 4.1 Grammar

Patchouli's syntax, unchanged: `$(code)` switches state, and plain text is everything else.

| Code | Effect |
|---|---|
| `$(0)`…`$(f)` | Minecraft color by index |
| `$(#rgb)` / `$(#rrggbb)` | Any color |
| `$(l)` `$(o)` `$(n)` `$(m)` `$(k)` | Bold, italic, underline, strikethrough, obfuscated |
| `$()` `$(r)` `/$` | Reset to the page's default style |
| `$(br)` / `$(br2)` / `$(p)` | Line break / blank line / blank line |
| `$(li)` `$(li2)` `$(li3)` | List item at depth 1–3 (bullet, 8 px indent per level) |
| `$(l:<entry>[#<anchor>])` … `$(/l)` | Link to an entry of this book (`patterns/math#add`). `$(l:<ns>:<book>/<entry>)` links into another book. |
| `$(l:https://…)` … `$(/l)` | External link through vanilla's confirmation screen |
| `$(t:<text>)` … `$(/t)` | Hover tooltip |
| `$(k:<key mapping>)` | The key bound to a key mapping, e.g. `$(k:use)` → "Right Button" |
| `$(playername)` | The player's name |

Patchouli's built-in macros (`$(item)` → `$(#b0b)`, `$(thing)` → `$(#490)`, `$(bold)`, `$(italic)`/`$(italics)`,
`$(strike)`, `$(obf)`, `$(reset)`/`$(clear)`, `$(2br)`, `$(nocolor)`, `<br>`) are applied first, then the book's
`macros`, as plain string replacement with the longest key first. The order and the result are the same as
Patchouli's, so imported texts render identically.

**Yog additions** (same syntax, so nothing new to learn):

| Code | Effect |
|---|---|
| `$(i:<item stack ref>)` | Inline item icon, 8×8, with the item's tooltip on hover |
| `$(tr:<key>)` | Inline translated text (vanilla or `.yres` key), e.g. an item name that follows the language |
| `$$` | A literal `$` |

`$(c:/command)` (Patchouli's click-to-run-command) is supported (Q10). Clicking it shows a confirmation with the
exact command; on confirm the client sends it as a normal chat command, so the server's permission checks apply.
The tooltip shows the command.

### 4.2 From markup to the renderer

All of this is in `yog_book::markup` and `yog_book::flow`. The same code runs in the game and in the IDE.

1. **Expand** the macros on the *translated* string. Patchouli expands after translation too, which lets
   translators use macros.
2. **Parse** into blocks (paragraph, list item with depth, blank line) of spans. A span is
   `{ text, color, bold, italic, underline, strike, obfuscated, link?, tooltip?, inline_item? }`.
   Errors (an unclosed `$(l:`, an unknown code) do not stop rendering: the raw code is shown in red, and the build
   and IDE report it.
3. **Flow** the blocks into lines `width` px wide by words, measuring each run with the host's `text_width` on the
   run text with its `§l`/`§o` prefix (bold glyphs are 1 px wider in Minecraft's font). Line height is 9 and a
   blank line is 9, as in Patchouli. A word longer than the line breaks by characters.
4. **Pages.** With `flow`, lines that do not fit the page body go to a continuation page. Continuation pages are
   ordinary physical pages in the spread order. Links to an anchor resolve to the physical page.
5. **Draw** each run with `draw2d_text(text, x, y, color, shadow=false)`, using `§`-codes for bold, italic,
   underline, strikethrough and obfuscated (Minecraft's font draws them). The color is passed as the argument, so
   any `#rrggbb` works. Inline items use `draw2d_item` at 8 px.
6. **Hit map.** Link and tooltip spans give rectangles. Hover draws a link in `link_hover_color` and shows the
   tooltip (`draw2d_tooltip` from the materials design). A click navigates (§6.4).

Results are cached per (book, language, text). Parsing and flowing happen when the language or the screen scale
changes, not every frame.

---

## 5. Localization

- A text field is `@Set.name` (a `.yres` text, translated at run time like any other) or a literal. At build time
  a literal becomes an `en_us` lang entry under `<ns>.book.<book>.<entry path with / → .>.<page>.<field>`. The
  compiled book therefore always carries keys, and the runtime has one code path.
- Key names: the `.yres` default key of a non-`Items`/`Blocks` set is `<mod id>.<name>`. Imported names are the
  Patchouli key minus the `<mod id>.` prefix (`hexcasting.page.math.add.1` → `page.math.add.1` in `Book.yres`). The
  game keys stay exactly those of the original mod, so existing translations and resource packs keep working.
- **No placeholders.** Book texts never take `{args}`. The `.yres` compiler treats a text used from a book as
  placeholder-free (so `{` is literal there), and the designer warns if such a text is also used with arguments
  from code.
- The runtime asks the host for the language (`host_language()`, new ABI) instead of probing a translation each
  frame. On a change it re-translates, re-expands, re-flows and re-paginates. The view stays on the same entry and
  logical page.
- The designer edits translations in place (§8.6). The `.yres` editor stays the place for bulk translation work.

---

## 6. Runtime

### 6.1 yog-book, restructured

| Module | Contents |
|---|---|
| `model` | The compiled book (serde): `Book`, `Category`, `Entry`, `Page { type, anchor, unlock, fields: Map }`, `Skin`, `Condition`. It is internal: mods never build it. |
| `source` (feature `compile`) | `.ybook`/`.yentry` → `model`, with validation and diagnostics (file, line, column). It is used by yog-cli **and** by Yog-IDLE's backend, so the build and the designer compile identically. |
| `schema` | Field specs of the built-in page types (for validation, the designer property grid and the JSON schema). |
| `markup`, `flow` | §4 |
| `pages` | One function per built-in page type: page model + body rect → yog-ui widgets and draw commands, with Patchouli's coordinates (most of today's `build_page`, with the GL sprites replaced by materials). |
| `view` | Navigation state (landing → category → entry spread; history for *back*; search later), spreads, hit map, events. It replaces `state.rs` and the screen part of `renderer.rs`. |
| `draw` | Everything goes through `GfxContext::draw2d()`: materials for the skin, items, text and tooltips. There is no GL. |

Patchouli's layout is reproduced exactly:
- landing: nameplate, subtitle and landing text on the left, the category icon grid on the right;
- category: name and description on the left, the entry list on the right, paged;
- entry: **two content pages per spread**, the entry title header on the first page, arrows and a back button.

Because of this, books that align pages with `empty` look as they did in Patchouli.

### 6.2 Loading

- `yog build` writes `yog/books/<book>.json` (the compiled `model`, keys only) into the `.yog`, and the book texts
  into `assets/<ns>/lang/*.json` with the rest of the `.yres` texts.
- On mod load the runtime reads every `yog/books/*.json` from the archive (it already reads `yog.toml` from it),
  evaluates the load-time conditions (`mod`, `mc`), applies version renames (§6.8), and keeps one `BookView` per
  book. Addon books (`extends`) are merged after all mods are loaded.
- **Reloading data.** The control socket gets `{"cmd":"reload_books"}`: it re-reads `yog/books/*.json` and the
  lang files of the given `.yog` (the host reloads languages). A book is data, so this needs no code-ABI match,
  unlike `hot_reload`. `yog run`/`dev.sh --reload` sends it when only book/lang files changed.

### 6.3 Opening

- The book item opens its book (§3.7).
- From code: the generated handles (§7.2) call the new ABI `open_book(book, entry, anchor)` on the client.
- The host's screen for books stays the generic yog UI screen (`openUI(bookId, …)`). The runtime routes render,
  click, scroll and key events to the `BookView`, as `ui_jni.rs` does for `book_renderers` today.

### 6.4 Navigation and links

The view handles internal events. `link:<entry>#<anchor>` goes to the physical page holding the anchor and pushes
history. Other events are `back`, `home`, `cat:<id>`, `entry:<id>`, `next`, `prev`, `open_url:<url>` (vanilla
confirm-link screen, new host call) and `cycle` (the image page arrows). Locked targets show a "locked" toast and
do not navigate. Backspace and the mouse "back" button go back, as in Patchouli.

### 6.5 Custom pages (ABI)

```rust
// YogApi: registered by generated component code (§3.4)
pub register_book_page: unsafe extern "C" fn(ctx: *mut c_void, page_type: YogStr, ud: *mut c_void,
                                              render: YogBookPageRenderFn),
// render(ud, fields_json, x, y, w, h, mouse_x, mouse_y): the mod builds the component from the
// (already translated) fields, lays it out in the rect with its yog-ui, and draws through draw2d.

// YogGfxApi: what <BookText>/<BookHeader>/<BookItem> call while a custom page is drawn
pub book_text: unsafe extern "C" fn(text: YogStr, x: f32, y: f32, w: f32, measure_only: bool) -> f32, // → height
pub book_header: unsafe extern "C" fn(text: YogStr, x: f32, y: f32, w: f32),
pub book_item: unsafe extern "C" fn(stack_ref: YogStr, x: f32, y: f32, size: f32),
pub draw2d_quad: unsafe extern "C" fn(pts: *const [f32; 8], color: u32),
```

The runtime calls `book_*` during a custom page's render, so it knows which book and page is current. It applies
that book's macros, colors and link handling, and records link rects into the page's hit map, so links inside
custom pages are clickable. A custom type with no registered renderer (its mod is missing, or registration failed)
draws a placeholder box with the type id, and logs once.

### 6.6 Entities and multiblocks

- `entity` pages need `draw2d_entity(type, x, y, scale, yaw, pitch)`. The host creates a client-side entity
  instance once per type and renders it with `InventoryScreen.renderEntityInInventory`. This is a new ABI entry
  on all six hosts.
- `multiblock` needs block-state rendering into the GUI with a rotation: `draw2d_blocks(json, x, y, scale, yaw)`.
  Both ship in the first release (Q6), in stage B7.

### 6.7 Progress

With `show_progress`, entries not opened yet get Patchouli's "unread" sparkle, and the landing page shows
`n/m entries read`.

Storage is split by whether the data can affect the game (Q8):

- **Server, per player** — everything that can influence gameplay: read entries and any state conditions can
  depend on. It is saved with the world as player data (boilerplate D6 `PlayerData`, keyed `yog:books`), synced to
  the client on join and on change. Opening an entry sends `entry_read(book, entry)` to the server; the server
  validates that the entry is unlocked for that player. On a singleplayer world the integrated server stores it
  the same way.
- **Client, `<game dir>/yog/books/<book>.json`** — only what cannot affect the game: last opened entry and page,
  navigation history, bookmarks, UI preferences.

### 6.8 Versions and vanilla refs

- At build time, item, texture, recipe and entity ids are validated against the vanilla packs of every MC version
  the mod targets (materials §3.4): missing in some targeted versions is a warning, with a quick fix to add an
  `"mc"` condition; missing in all of them is an error.
- At load the runtime maps renamed ids through `vanilla/renames.toml` (`minecraft:scute` → `turtle_scute`), like
  typed handles do.
- **Recipes.**
  - Mod recipes come from `register_recipe_json` (as today), and from `data/<ns>/recipe(s)/*.json` in the `.yog`.
  - Vanilla recipes come from the client's recipe manager (new host query `recipe_json(id)`). It is complete on
    1.20.1/1.21.1. 1.21.2+ no longer syncs all recipes to the client, so a server→client fetch will be needed there,
    which is noted as a risk.
  - Recipe JSON in both formats (1.20 `"item"`/1.21 `"id"` results) is parsed by yog-book's existing
    `parse_recipe`, extended.

### 6.9 ABI summary (`ABI_MINOR` bump)

| Change | Purpose |
|---|---|
| − `register_book` | Books are data now |
| + `register_book_page`, `book_text`, `book_header`, `book_item` | Custom pages (§6.5) |
| + `draw2d_quad` | Polygons (Hex pattern strokes; generic) |
| + `open_book(book, entry, anchor)` | Typed handles |
| + `host_language()`, `client_has_advancement(id)`, `key_mapping_label(id)`, `open_url_confirm(url)`, `recipe_json(id)` | Client queries used by the book |
| + `draw2d_entity` (later: `draw2d_blocks`) | Entity / multiblock pages |
| `YogItemDef.book: YogStr` | The book item (§3.7) |
| − `nativeBookJson` (JNI), `YogStartupGrant.book` | Coupling by id, NBT books |

`host_mc_version`, `draw2d_material`, `draw2d_tooltip` and `draw2d_item_tooltip` come from the materials design
and are required by this one.

---

## 7. Build and codegen

### 7.1 `yog build`

`content.rs` gains `Kind::Book` and `Kind::Entry`. It compiles books with `yog_book::source` and:

1. resolves the category tree, entry order, conditions and paint refs, and validates page fields against the
   built-in schemas or the custom page components' `<Property>` declarations (read from `.yui`, as
   `components.rs` does in IDLE);
2. checks texts:
   - every `@Set.name` exists;
   - the markup parses;
   - link targets and anchors exist (including links inside `.yres` texts used by the book);
   - macros referenced as `$(name)` exist;
3. checks refs: items, textures, recipes and entities against the mod, its dependencies and the vanilla packs
   (§6.8);
4. writes `yog/books/<book>.json`, the literal texts to `en_us`, and the book item (model and lang) like a
   `.yitem`;
5. reports **page overflow per language** as an *info* when `flow` is off. This needs the vanilla font, so only
   with a vanilla pack; the designer shows the same check live.

Diagnostics carry the file, line and column of the `.ybook`/`.yentry`, or of the `.yres` entry holding the text.

### 7.2 Typed handles

`.yog-build/generated/books.g.rs`, included by `#[yog_api::main]` (boilerplate F8; until F8 lands, by the existing
`content!()`):

```rust
pub mod books {
    pub mod thehexbook {
        pub const BOOK: ::yog_api::BookId = ::yog_api::BookId::new("hexcasting:thehexbook");
        pub mod basics {
            pub const MEDIA: ::yog_api::EntryRef = ::yog_api::EntryRef::new(super::BOOK, "basics/media");
        }
        pub mod patterns {
            pub const MATH: ::yog_api::EntryRef = ::yog_api::EntryRef::new(super::BOOK, "patterns/math");
            pub mod math {   // anchors
                pub const ADD: ::yog_api::Anchor = super::MATH.anchor("add");
            }
            pub mod spells { /* subcategory */ }
        }
        pub mod flags { /* only if the book uses {"flag": …} */ }
    }
}
```

```rust
books::thehexbook::BOOK.open();                         // landing page
books::thehexbook::patterns::math::ADD.open();          // straight to the page with the "add" pattern
let item: ItemId = books::thehexbook::BOOK.item();      // the book item, e.g. for a startup grant in code
```

`BookId`/`EntryRef`/`Anchor` are `Copy` newtypes in yog-api, and `open()` calls `open_book`. Entry and anchor
names that are Rust keywords or start with a digit get `r#`/`_` prefixes, as in content codegen.

### 7.3 Skin

The default skin is Yog's brown book: the existing `yog-book/assets/book_brown.png` and `crafting.png`, moved to
`Yog-Mod-Loader/vanilla/overlay/common` as Yog-authored textures with `mat:yog:book/*` materials:

- `frame`, `nameplate`, `separator`;
- `arrow_next`/`arrow_prev`/`arrow_back` with hover states;
- `spotlight_frame`, `crafting_grid`, `smelting_strip`;
- `filler`, `entry_unread`, `lock`.

A skin `texture` in Patchouli's `book.png` layout replaces the texture of all of these, with the same regions.
`parts` overrides single parts with any paint ref. This is how Hex keeps its own `patchi_book.png` look with one
line.

---

## 8. Yog-IDLE: the Book Designer

### 8.1 Opening

| Provider | Files | Default |
|---|---|---|
| `yog.book` (Design + Source split, `YuiSplitEditor`'s shell) | `.ybook`, `.yentry` | ✓ |
| `yog.book.design` (design only) | `.ybook`, `.yentry` | |
| `yog.book.source` (JSON editor with the `ybook-1`/`yentry-1` schemas) | `.ybook`, `.yentry` | |

Opening the manifest or any entry file opens **the book**: one designer session per book, keyed by the manifest
path, with the entry selected. A second entry file of the same book focuses the existing tab and selects the
entry, so there are never two designers editing one book. The tab title is the book's name.

### 8.2 Layout

```
┌ toolbar: [Design|Preview]  language [en_us ▾] [+ Add language…]  MC [1.21.1 ▾]  zoom [2× ▾]  [⟲ in game] ┐
├───────────────┬──────────────────────────────────────────────────────────────┬──────────────────────┤
│ Book tree     │                 rendered spread (yog-book)                   │ Properties           │
│ ▸ Landing     │   ┌──────────────────────┬──────────────────────┐            │ (PropertyGrid of the │
│ ▾ Basics      │   │ Media                │  …pages 3–4…         │            │  selected book /     │
│   ▾ Media     │   │ ───────────          │                      │            │  category / entry /  │
│     ¶ 1 text  │   │ Media is a form of…  │                      │            │  page)               │
│     ¶ 2 text  │   └──────────────────────┴──────────────────────┘            │                      │
│   ▸ Geodes    ├──────────────────────────────────────────────────────────────┤                      │
│ ▸ Patterns    │ Text: [B][I][U][S][color ▾][macro ▾][link][item][key][li]    │                      │
│               │ Media is a form of $(media)mental energy$() external to a …  │                      │
├───────────────┴──────────────────────────────────────────────────────────────┴──────────────────────┤
│ Source: content/books/thehexbook/basics/media.yentry  (JSON, linked to the selection)                │
└───────────────────────────────────────────────────────────────────────────────────────────────────────┘
```

The tree is a Substrate `Tree` (it already supports drag and drop). Properties are the shell's Properties tool
window, and Toolbox and Document Outline are the shell's tool windows too (§8.4).

### 8.3 The canvas

- **Rendering.** A new command `book_designer_render { manifest, overlays: {path: text}, view, lang, mc, scale,
  preview, hover }` runs in `src-tauri/src/designer/book.rs`:
  - `overlays` are the unsaved buffers of open book, entry and `.yres` files;
  - `view` is `{ category | entry, spread }`;
  - it compiles with `yog_book::source`, builds a `BookView`, renders through the recording `YogGfxApi` (`gfx.rs`),
    and rasterizes with the Minecraft font and textures of the selected vanilla pack (`raster.rs`, `font.rs`);
  - it answers `{ image, hit: [{ kind: page|run|link|slot|button, rect, entry, page, field, span }],
    diagnostics }`.

  It is the code the game runs, on the same pixels. A 16 ms debounce follows typing, as the `.yui` designer does.
- **Custom pages** are drawn as `.yui` component instances are: markup-only ones by expanding the component
  (`components.rs`); compiled ones by the designer host (`host.rs`, `custom.rs`), which calls the component's
  generated `book_page` renderer. The host's recorder implements `book_text`/`book_header`/`book_item` by calling
  back into yog-book in the IDE process.
- **Design mode.** Clicking a page selects it in the tree, Properties and Source; clicking text selects the text
  field. Arrows and entry-list rows also navigate in Design mode, so the canvas can be browsed like the game.
  Continuation pages produced by `flow` are hatched in the corner (designer-only) and select their logical page.
- **Preview mode** behaves exactly like the game: links, back, item tooltips on hover, image cycling, locked
  entries. A **Conditions** toggle shows the book with everything unlocked, as a fresh player, or with picked
  advancements, mods and flags set.
- **Approximations** (where the IDE cannot draw what the game draws) are marked with a small badge:
  - `entity` pages show the spawn egg and the name;
  - `multiblock` shows a flat layer view;
  - code-registered recipes show the game's `[Recipe: id]` fallback, and the badge says why.
- **⟲ in game** sends `reload_books` to a running game (§6.2), for a final check in the real client.

### 8.4 Tree, Toolbox, Outline, drag and drop

- The **tree** is Landing, then categories (and subcategories), then entries, then pages. Icons are drawn from the
  paint refs; pages show their type icon and the first words of their text in the current language. Locked items
  show a lock and missing translations a red dot.
- **Drag and drop:**
  - pages within and between entries (between entries this is a cut/paste of JSON objects across two files);
  - entries within a category (rewrites `entries` in the manifest) and between categories (moves the file and
    rewrites links, §8.8);
  - categories (reorders `categories`; moving one into another nests the folder).
  The Substrate `Tree` gets generic insert-before/after drop positions (today a drop targets a node).
- The **Toolbox** lists the page types:
  - *Book*: text, spotlight, crafting, smelting, stonecutting, smithing, image, entity, multiblock, link,
    relations, empty;
  - *<mod> Pages*: the mod's `book_page` components, with their icon, description and category, as for `.yui`
    components.
  Dropping on a page boundary on the canvas or in the tree inserts a page there, and double-click appends to the
  selected entry. `activeDesigner.ts` becomes a small interface with two implementations (screen, book): items
  for the Toolbox, a tree for the Outline, `add(kind)`, select and focus.
- **Document Outline** shows the selected entry's pages, with anchors, for quick jumps in long entries (Hex's
  `patterns/math` has ~30 pages).
- **Commands:** New category, New entry (creates the file, the manifest line and its name text), Duplicate,
  Delete (with "remove its texts from `Book.yres`? (n texts, k translations)"), Copy/Paste pages as JSON, and
  Undo/Redo across all files of the book as one history.

### 8.5 Properties

The PropertyGrid shows the selected node:

- **Book:** name, subtitle, landing text, skin (paint pickers and color swatches), item (name, tooltip, model
  through the content editor's model picker, creative tab, first-join grant), show_progress, macros (a key/value
  grid with a live sample of each macro).
- **Category:** name, description, icon, unlock.
- **Entry:** name, icon, unlock (a condition editor: advancement picker from `data/**/advancements` + vanilla,
  mod picker, `mc` range, combinators as a small tree), read_by_default, color, and the anchors used (read-only).
- **Page:** the fields of its type from `yog_book::schema` (built-in) or the component's `<Property>` list
  (custom). Kinds:
  - item stack: the `item` picker from materials §10.6, with a list mode for cycling;
  - recipe: a searchable list of mod + dependency + vanilla recipes, each with a thumbnail rendered by yog-book;
  - texture: the paint picker;
  - entity: from the vanilla pack's entity list;
  - text fields show the current language's text with a "⟶ Book.page.math.add.1" key chip.

Changing a page's type keeps the fields both types share and moves the others into a `"// removed: …"` comment
block, so that nothing is silently lost (it can be undone).

### 8.6 Text editing

There is no `contenteditable` on top of a raster image. The selected text field opens in the **Text panel** under
the canvas, a CodeMirror editor with a `ybook-markup` language. Every keystroke re-renders the canvas (debounced),
so the effect is WYSIWYG while the source of truth stays the markup:

- Codes are highlighted. Color codes show a swatch, and macros show their expansion as an inlay hint. `$(l:…)`
  shows the target entry's name.
- The **toolbar** inserts or wraps: bold, italic, underline, strikethrough, color (palette = the book's color
  macros first, then the Minecraft colors, then custom), macro, link, inline item, key, list item, paragraph.
  Ctrl+B/I/U work.
- **Autocomplete:**
  - `$(l:` → entries (path + name in the current language), then `#` → that entry's anchors;
  - `$(` → codes and the book's macros;
  - `$(k:` → vanilla key mappings with their labels;
  - `$(i:` → items;
  - `$(tr:` → keys.
- **Broken links**, unknown macros and unclosed codes are squiggled, with quick fixes (pick an entry, create the
  anchor, close the code).
- **Clicking a word** in the canvas puts the cursor at that span (the hit map has span offsets). Selecting text in
  the panel highlights the runs on the canvas.
- **Saving** writes the text to the `.yres` file of the current language (neutral → `Book.yres`, `ru_ru` →
  `Book.ru_ru.yres`) through the same minimal-edit JSON patch as the `.yres` editor, or into the page's JSON for a
  literal.
- **A new text page** in a book whose texts are refs creates `@<texts set>.<entry path>.<n>` in the neutral `.yres`
  (`page.math.17`). *Extract to resource* turns a literal into a ref, and *Inline* does the opposite for
  single-language books.

### 8.7 Languages

- The **language dropdown** lists the neutral culture and every culture that has a `<texts set>.<locale>.yres`. The
  canvas, the tree labels and the Text panel follow it.
- **Missing translations:** a text without a translation renders in the neutral language with a hatched overlay
  (designer-only, Design mode only), and the tree shows red dots. The Error List gets
  `ru_ru: 37 texts missing in thehexbook` (one info per book, expandable per entry). *Next missing*
  (F8 / Shift+F8) walks them. An **untranslated-only** filter dims everything else.
- **Overflow:** with `flow` off, a page that does not fit in the current language is outlined red, with the number
  of excess lines. With `flow` on, pages that gained a continuation page in this language but not in the neutral
  one get a note, because they may break carefully aligned spreads. *Check all languages* runs both checks for
  every culture.
- **Add language…** picks a locale from the vanilla pack's language list and creates `Book.<locale>.yres` (and the
  matching files for the book's other sets, if asked), empty. Missing texts fall back to neutral, as `.yres` does.
- A **side-by-side** toggle shows the neutral spread and the current language's spread next to each other.

### 8.8 Refactors and navigation

- *Rename entry / Move entry* renames or moves the file, updates the manifest, and rewrites every
  `$(l:old…)` in the book's `.yres` texts, all cultures, plus `relations` pages and other books that link here.
  It shows a preview of the changed files first.
- *Rename anchor* does the same for `#anchor`.
- *Find links to here* lists the texts that link to the entry or anchor.
- *Go to text* (F12 on a page field) opens the `.yres` entry. From the `.yres` editor, *Find usages* of a book text
  opens the book at that page.
- Generated handles (`books.g.rs`) appear nested under the manifest in Project Explorer, read-only, as `.g.rs` do
  for `.yres`.

### 8.9 Source pane

The Source pane shows the JSON of the file that owns the selection: the manifest for the book or a category, the
entry file for an entry or page. Selection is linked both ways: the selected page's object is highlighted, and
moving the caret selects in the designer, as `YuiSplitEditor` does with `pathAt`/`spanAtPath`. Edits from the
designer are minimal text edits that keep comments, order and formatting. They go through a new generic
**`jsonEdit`** helper in Substrate (set/insert/remove/move at a JSON path, returning text edits, like
`jsonc-parser`'s `modify`), which the content and `.yres` editors can later reuse. While the JSON is invalid, the
designer keeps the last valid book and shows the error, as the `.yui` designer does.

### 8.10 New Item templates

- *Book (.ybook)* asks for a name. It creates the manifest, one category, one entry with a text page,
  `Book.yres` texts, and the item with the default model.
- *Book entry (.yentry)*, in a book folder.
- *Book page type (.yui)* creates a `<Component book_page=…>` skeleton with a `<BookHeader>` and a `<BookText>`.

The CLI equivalents are `yog book new <name>` and `yog book entry <book> <path>`, from the same template source
(yog-kit §9).

---

## 9. Patchouli import

```
yog import patchouli <patchouli_books/<book> dir | mod jar | mod source root>
      [--lang <dir with <locale>.json | .flatten.json5>] [--book <id>] [--set Book] [--into <mod root>]
```

It reads `data/<ns>/patchouli_books/<book>/book.json` and `assets/<ns>/patchouli_books/<book>/<lang>/{categories,
entries,templates}` (both 1.20 locations), plus the mod's lang files. It writes:

| Patchouli | Yog |
|---|---|
| `book.json` name, landing_text, subtitle, nameplate_color, `book_texture`/`filler_texture`, model, creative_tab, macros, show_progress, text/header/link colors | manifest fields; textures become texture ids (`hexcasting:textures/gui/patchi_book.png` → `hexcasting:gui/patchi_book`) and are copied into `textures/` if found |
| `i18n: true` texts (keys) | `@Book.<key minus "<ns>.">`, with every locale's lang value written to `Book[.<locale>].yres` |
| `i18n: false` texts (inline, per `<lang>/` folder) | generated names `<entry>.<n>`; `en_us` folder → neutral, other folders → cultures |
| categories (`sortnum`, `parent`, icon, description, advancement/flag/secret) | `categories[]` in `sortnum` order, folders, `unlock` |
| entries (`sortnum`, `priority`, `advancement`, `flag`, `secret`, `read_by_default`, `entry_color`, icon) | `.yentry` files, manifest `entries` order (priority first, then sortnum, then name) |
| `extra_recipe_mappings` | `item_mappings` (stage B9), reported until then |
| page `patchouli:<type>` | built-in type with renamed fields (`heading` → `title`, `recipe` …) |
| page `advancement`/`flag` | page `unlock` |
| `$(…)` texts | verbatim, including `$(c:…)` |
| template-based types (`<ns>:<template>`) | `src/ui/book/<template>_page.yui` stub: a `<Component book_page>` with one `<Property>` per `#var` the template uses, its `patchouli:text`/`header`/`image`/`entity` components translated to `<BookText>`/`<BookHeader>`/`<Image>`/`<Entity>` at their coordinates. Processors and `patchouli:custom` classes become `<!-- TODO -->` boxes with the Java class name. |
| code-registered types (Java `PageX` classes) | a stub component with properties from the union of the fields that the type's pages use, with types inferred (number, bool, list → text) |

`--lang` accepts flat lang JSON and the nested `*.flatten.json5` that Hex Casting and its addons use (the keys
are joined with `.`, and the `"": …` keys map to the parent). The importer prints a report: counts, unsupported
features, stubs written, and links whose target is missing.

---

## 10. HexMod-Yog migration

1. `yog import patchouli <ref>/HexMod/Common/src/main/resources --book thehexbook --lang …/lang` writes:
   - 1 manifest, 9 categories, 82 entries and 447 pages;
   - `Book.yres` plus 12 cultures from the original's 13 lang files;
   - 5 template stubs (`pattern`, `manual_pattern`, `manual_pattern_nosig`, `brainsweep`, `crafting_multi`).
2. Page types:
   - `crafting_multi` → built-in `crafting` with a recipe list; delete its stub;
   - `pattern`, `manual_pattern`, `manual_pattern_nosig` → **one** `PatternPage` component. Its properties are
     `op_id?`, `patterns?` (signatures with start directions, for the manual ones), `input`, `output`, `header?`
     and `text`, and the importer's type rename maps all three to `hexcasting:pattern`. The pattern lookup uses
     F1's action table;
   - `brainsweep` → `BrainsweepPage` (image + entity + recipe lookup), after the entity stage; until then it is
     markup-only, with an item icon instead of the entity.
3. The Russian text: today's `Book.ru_ru.yres` has HexMod-Yog's own terminology ("Рунный блокнот", "Мысли" for
   media) for 5 entries. The original has a complete `ru_ru` with different terms. Decided (Q4): import the original
   `ru_ru` and drop HexMod-Yog's terms.
4. Delete:
   - `src/book.rs`, and `registry.register_book` and `mod book` in `lib.rs`;
   - `content/items/thehexbook.yitem` and the manual registration in `src/item/mod.rs` (the item comes from the
     manifest);
   - the dead `assets/hexcasting/yog_book/**` copy;
   - the old `book.*` names in `Book.yres`.
5. Add Hex's book textures (`patchi_book.png`, `patchi_filler.png`, `brainsweep_recipe_nobg.png`) to
   `textures/gui/` (they come with the import), and `"skin": { "texture": "hexcasting:gui/patchi_book", … }`.
6. Recipes referenced by `crafting` pages that Hex-Yog does not register yet are reported by the build as warnings.
   They show `[Recipe: id]` until gameplay porting adds them.

---

## 11. Coordination with other designs

- **Materials** (`materials-and-vanilla-ui.md`): this needs S0 (vanilla pack, `host_mc_version`, versions) for
  validation and previews, S1 (texture ids) for icons and skins, and S4 (`draw2d_material`, `draw2d_tooltip`,
  `draw2d_item_tooltip`) for drawing. Until S4, yog-book can draw skin parts with `draw2d_image` regions; that is
  replaced in B8. The vanilla pack should also extract `data/minecraft/recipe(s)/**` (for designer recipe
  previews) and the entity id list.
- **Boilerplate** (`boilerplate-reduction.md`): F8 (`#[yog_api::main]` includes `books.g.rs`; self-registering
  components carry the `book_page` registration). F1 (Hex's action table) is what `PatternPage` reads. F10 is
  superseded by this document.
- **yog-kit / `.yui`**: `book_page` on `<Component>`, the `book_text` property type, and the elements
  `BookText`/`BookHeader`/`BookItem` (plus `Entity` with `draw2d_entity`) are grammar additions; IDLE `yui.ts`
  and `properties.ts` learn them.

---

## 12. Deletions (no compatibility)

| Removed | Replacement |
|---|---|
| `yog_book::{Book, BookCategory, BookEntry, BookPage, BookMacro, BookRegistry}` builders, `*_page()` helpers, `to_json`, `book_ui` | `.ybook`/`.yentry`; `yog_book::model` is internal |
| `BookPage::{Pattern, Svg, CustomText}`, `BookFont`/`BookFontRegistry`, the `svg`/`fonts`/`full` features, `svg.rs`, `font.rs` | custom page components; yog-ui fonts |
| `BookGl` (shader, PNG decoding, sprite blitter), `OverlayCmd`, `BookTheme` constants | draw2d + `mat:yog:book/*` + the skin |
| `Book.i18n`, `BookRenderer::set_translator` and the language probe | `@Set.name` refs, `host_language()` |
| `yog_api::book`, the `yog_book` re-exports in yog-api, yog-api's dependency on yog-book, `Registry::register_book` | data loading, `BookId`/`EntryRef` handles |
| ABI `register_book`, JNI `nativeBookJson`, Java item↔book id matching | `YogItemDef.book` |
| `yog_registry::BookRecipe`, `StartupGrant.book`, `AdvancementReward.book` (`written_book` + `yog_book` NBT) | normal recipes and grants of the real item |
| HexMod `src/book.rs`, `content/items/thehexbook.yitem`, `assets/hexcasting/yog_book/**` | §10 |

yog-book gets a major version bump. yog-api, yog-abi (minor), yog-registry, yog-ui and yog-cli are bumped as their
APIs change, and the crates are published when each stage lands.

---

## 13. Implementation stages

| Stage | Content | Depends on | Repos / crates |
|---|---|---|---|
| **B0** yog-book core | `model`, `schema`, `markup`, `flow`, `pages`, `view`, `draw` over draw2d; Patchouli-exact spreads; delete GL/SVG/fonts; unit tests: markup corpus from Hex's en_us + ru_ru, golden flow results with a fixed-width test font, spread layout against Patchouli coordinates | — (draws skin with `draw2d_image` until B8) | yog-book |
| **B1** Format + build | `source` (feature `compile`), `.ybook`/`.yentry` JSON schemas, `content.rs` Kind::Book/Entry, validation and diagnostics, `yog/books/*.json`, literal-text lang keys, book item → `ItemDef.book`, `books.g.rs` handles, `yog book new/entry` | B0 | yog-book, yog-cli, yog-registry, yog-api |
| **B2** Runtime + ABI | load `yog/books/*.json`, conditions (`mod`, `mc`, advancement), renames, `open_book`, `host_language`, `client_has_advancement`, `key_mapping_label`, `open_url_confirm`, `recipe_json`, `YogItemDef.book` in 6 hosts, `reload_books`; delete `register_book`, `nativeBookJson`, BookRecipe, NBT grants, yog-api's book module | B1; materials S0 for `host_mc_version` | yog-runtime, yog-abi, yog-api, Java ×6 |
| **B3** Designer v1 | `book_designer_render` (yog-book in src-tauri, recorder), book session and tab, tree with drag and drop (Substrate Tree insert positions), Properties per node and page type, Source pane with linked selection and `jsonEdit` (Substrate), Text panel with markup language, toolbar and autocomplete, language dropdown, Add language, missing-translation and overflow checks, refactors, templates | B1 (compiler), B0 (render); materials S0/S7a for the vanilla pack in IDLE | Yog-IDLE, Substrate (Tree drop positions, `jsonEdit`) |
| **B4** Custom pages | `<Component book_page>`, `book_text` property type, `BookText`/`BookHeader`/`BookItem`, generated `register_book_page`, ABI `register_book_page`/`book_*`/`draw2d_quad`, runtime dispatch, designer host support, Toolbox *<mod> Pages* | B2, B3; boilerplate F8 for self-registration (or an interim generated call in `content!()`) | yog-cli, yog-ui, yog-kit, yog-runtime, yog-abi, Yog-IDLE |
| **B5** Patchouli import | `yog import patchouli`, flattened JSON5, template → component stubs, report; IDLE *Import Patchouli book…* | B1, B4 (stubs target its grammar) | yog-cli, Yog-IDLE |
| **B6** HexMod book | import, `PatternPage` (+ `HexPattern` widget on `draw2d_quad`), skin, delete the old code and files, translation decision (Q4) | B4, B5; boilerplate F1 for op_id lookup | HexMod-Yog |
| **B7** Entities + multiblocks | `draw2d_entity` and `draw2d_blocks` (6 hosts), `entity` and `multiblock` pages (isometric preview with rotation), `<Entity>` element, Hex `BrainsweepPage` | B2 | yog-abi, yog-runtime, Java ×6, yog-ui, HexMod-Yog |
| **B8** Materials skin | `mat:yog:book/*` overlay materials, skin parts, item tooltips through `draw2d_item_tooltip` | materials S4 | vanilla/, yog-book |
| **B9** Extras | `extends` (addon books), progress/unread, item→entry mappings (Patchouli's shift-on-tooltip), `$(i:…)` inline icons, search | B2 | all |

Critical path to "Hex's book in the designer and in game": B0 → B1 → B2 → B4 → B5 → B7 → B6 (entity and
multiblock pages are part of the first release, Q6). B3 can start right after
B1 and runs in parallel with B2. Every stage ends with the crates involved bumped and published, and the three
mods updated in the same change. Building and running the game is done with the user's permission.

---

## 14. Risks and open questions

### Risks

- **R1 Flow vs. hand-aligned spreads.** Books that rely on `empty` pages to put a pattern on the left and its
  description on the right can lose that alignment in a language where a text page overflows into a continuation
  page. Mitigation: the designer's per-language note (§8.7), and imported text pages default to `flow: false`
  unless the import is asked to `--flow`, so imports stay exactly like the original.
- **R2 Pixel parity of text.** Flow measures with the host's `text_width`, and the designer with `font.rs`. Both
  must agree on bold widths and unifont glyphs. Mitigation: a shared test corpus (Hex's 13 languages) that compares
  `font.rs` widths against widths recorded from a real client once.
- **R3 Vanilla recipes on 1.21.2+** are not synced to the client (§6.8). This needs a server→client fetch through
  Yog's network channel once those versions are supported.
- **R4 Custom page cost.** A compiled custom page builds a yog-ui tree in the mod per frame. It is cached per
  (page, language, size), and rebuilt only on change, as `BookView` caches its own pages.
- **R5 Designer host dependency.** Compiled page types render only when the mod's designer library builds. When it
  does not, the designer shows the placeholder, as for `.yui` components.

### Open questions for the user

1. **Q1 Location — decided: `content/books/`.** Was: `content/books/` (proposed, the same scan and explorer node as blocks/items), or a top-level
   `books/` next to `content/`, `textures/`, `materials/`?
2. **Q2 Flow by default:** should text pages continue onto new pages automatically (proposed for new books), or
   should clipping with an overflow warning stay the default as in Patchouli?
3. **Q3 Markup — decided: Patchouli's `$(…)`.** Was: keep Patchouli's `$(…)` (proposed), or use a Markdown-like syntax (`**bold**`,
   `[text](patterns/math#add)`) with the importer converting? Markdown reads better in diffs, but breaks verbatim
   import and conflicts with nothing only if `.yres` braces stay out.
4. **Q4 Hex's Russian — decided: the original Hex Casting `ru_ru`; HexMod-Yog's own terms are dropped.** Was: take the original Hex Casting `ru_ru` (complete, 447 pages, the community's terms), or
   keep HexMod-Yog's own terminology ("Рунный блокнот", "Мысли") and translate the rest in that style?
5. **Q5 Default skin — decided: the Patchouli-like brown two-page book.** Was: the Patchouli-like brown two-page book (proposed; it is what Hex and every ported book
   expect), or a vanilla-looking skin built from `minecraft:gui/book` (vanilla's book screen is a single 146×180
   page, so it would be a different layout)?
6. **Q6 Entity and multiblock pages — decided: both in the first release.** Was: is `entity` (needed for Hex's brainsweep pages) wanted in the first
   release? Can `multiblock` wait for a mod that needs it?
7. **Q7 Code-registered recipes in the designer:** show the fallback (proposed), or have the designer host run the
   mod's `register` against a recording registry to capture them?
8. **Q8 Read progress — decided: on the server per player; the client keeps only state that cannot affect the game (§6.7).** Was: client-global per book (proposed), per world/server, or synced from the server like
   advancements?
9. **Q9 Addon books (`extends`) — decided: not in the first release.** Was: needed in the first release (only once Hex addons are ported), or later?
10. **Q10 `$(c:/command)` — decided: supported, with a confirmation (§4).** Was: is click-to-run-command wanted (with a confirmation), or should it stay unsupported?
