//! Yog image sources — the names every Yog API that shows a picture takes
//! instead of raw Minecraft resource paths, so mods don't depend on the game
//! version, the loader or where Mojang keeps a texture this release.
//!
//! | Source | Draws |
//! |---|---|
//! | `item:<id>` | an item's icon through the item renderer (3D block items, modded items), e.g. `item:apple`, `item:mymod:wand` |
//! | `block:<id>` | a block's item form, e.g. `block:crafting_table` |
//! | `sprite:<name>` | a vanilla GUI sprite by Yog's stable name (see [`SPRITES`]), e.g. `sprite:button`, `sprite:icon/heart` |
//! | `mod:<path>` | the mod's own image `assets/<mod id>/textures/<path>.png` (`.yui` files; `yog build` fills in the mod id) |
//! | `<ns>:<path>` | `assets/<ns>/textures/<path>.png` — another mod's (or your own) image, e.g. `mymod:gui/logo` |
//! | `<ns>:<path>.svg` | `assets/<ns>/textures/<path>.svg` — an SVG image, sharp at any size |
//! | `<ns>:textures/<path>.png` | a raw resource id — what `mod:`/`<ns>:<path>` resolve to; `.yui` files reject it in favour of the alias ([`suggest_alias`]) |
//!
//! Ids without a namespace are `minecraft:`. The host resolves a source when
//! it draws it ([`Draw2D::image`](crate::draw2d::Draw2D::image)) — sprites
//! map to the GUI atlas on Minecraft 1.20.2+ and to `textures/gui/*.png`
//! regions on 1.20.1 — so the same mod binary draws right on every version.
//! This module is the shared definition (the runtime and Yog-IDLE's designer
//! resolve with it); mods normally only pass strings.

use std::sync::LazyLock;

use yog_graphics::{Rect, Vec2};

/// A parsed image source — see the [module docs](self).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageSource {
    /// `item:<id>` — a namespaced item id.
    Item(String),
    /// `block:<id>` — a namespaced block id (drawn as its item).
    Block(String),
    /// `sprite:<name>` — a [`SPRITES`] name, or a raw GUI-atlas sprite id
    /// when it has a namespace (`sprite:minecraft:widget/button`, 1.20.2+).
    Sprite(String),
    /// `mod:<path>` — the calling mod's own texture; needs the mod id
    /// (see [`ImageSource::with_mod_id`]).
    ModPath(String),
    /// A texture resource id, `<ns>:textures/<path>.png`.
    Texture(String),
}

/// Side of the square 1.20.1 GUI textures most legacy regions live in.
const LEGACY_TEX_SIZE: f32 = 256.0;

/// `<ns>` + `<path>` → the texture resource id: `gui/logo` →
/// `<ns>:textures/gui/logo.png`, `gui/logo.svg` →
/// `<ns>:textures/gui/logo.svg` (an SVG image); paths already under
/// `textures/` (or ending in `.png`) are kept as they are.
pub fn texture_id(ns: &str, path: &str) -> String {
    use yog_ids::res::{PNG, SVG, TEXTURES};
    let path = path.trim_start_matches(yog_ids::sep::PATH);
    if path.ends_with(SVG) {
        let path = path.strip_prefix(TEXTURES).unwrap_or(path);
        return yog_ids::join(ns, &format!("{TEXTURES}{path}"));
    }
    if path.starts_with(TEXTURES) || path.ends_with(PNG) {
        let png = if path.ends_with(PNG) { "" } else { PNG };
        yog_ids::join(ns, &format!("{path}{png}"))
    } else {
        yog_ids::join(ns, &format!("{TEXTURES}{path}{PNG}"))
    }
}

/// A texture named either way — `ns:textures/x.png` or `ns:x` (`x` alone is
/// the game's) — as the short source `ns:x`.
pub fn short_source(texture: &str) -> String {
    let (ns, path) = yog_ids::split(texture.trim());
    let path = path.strip_prefix(yog_ids::res::TEXTURES).unwrap_or(path);
    yog_ids::join(ns, path.strip_suffix(yog_ids::res::PNG).unwrap_or(path))
}

/// The schemes an image source starts with (`item:apple`).
pub use yog_ids::image_scheme as scheme;

/// The source as written: `item:minecraft:apple`, `sprite:button`; a
/// texture as its resource id.
impl std::fmt::Display for ImageSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ImageSource::Item(id) => write!(f, "{}:{id}", scheme::ITEM),
            ImageSource::Block(id) => write!(f, "{}:{id}", scheme::BLOCK),
            ImageSource::Sprite(name) => write!(f, "{}:{name}", scheme::SPRITE),
            ImageSource::ModPath(path) => write!(f, "{}:{path}", scheme::MOD),
            ImageSource::Texture(id) => f.write_str(id),
        }
    }
}

impl ImageSource {
    /// Parses a source string; `None` for an empty or malformed one.
    pub fn parse(source: &str) -> Option<ImageSource> {
        let s = source.trim();
        let (scheme, rest) = s.split_once(yog_ids::sep::NAMESPACE)?;
        if rest.is_empty() {
            return None;
        }
        Some(match scheme {
            scheme::ITEM => ImageSource::Item(yog_ids::qualify(rest.trim())),
            scheme::BLOCK => ImageSource::Block(yog_ids::qualify(rest.trim())),
            scheme::SPRITE => ImageSource::Sprite(rest.to_string()),
            scheme::MOD => ImageSource::ModPath(rest.to_string()),
            ns if !ns.is_empty() => ImageSource::Texture(texture_id(ns, rest)),
            _ => return None,
        })
    }

    /// Resolves `mod:<path>` against `mod_id` (others are unchanged).
    pub fn with_mod_id(self, mod_id: &str) -> ImageSource {
        match self {
            ImageSource::ModPath(path) => ImageSource::Texture(texture_id(mod_id, &path)),
            other => other,
        }
    }
}

/// `<ns>:textures/<path>.png` → the Yog source a `.yui` file or a mod should
/// use instead: `item:` for item textures, `block:` for block textures, a
/// `sprite:` with the same image, or the short `<ns>:<path>` form. `None`
/// when `id` isn't a raw texture id.
pub fn suggest_alias(id: &str) -> Option<String> {
    let (ns, path) = id.trim().split_once(yog_ids::sep::NAMESPACE)?;
    let path = path.strip_prefix(yog_ids::res::TEXTURES)?;
    let stem = path.strip_suffix(yog_ids::res::PNG).unwrap_or(path);
    // `minecraft:` is the default namespace of every alias.
    let short = |id: &str| -> String {
        if ns == yog_ids::ns::MINECRAFT {
            id.to_string()
        } else {
            yog_ids::join(ns, id)
        }
    };
    if let Some(item) = stem.strip_prefix(yog_ids::res::ITEM_DIR) {
        return Some(ImageSource::Item(short(item)).to_string());
    }
    if let Some(block) = stem.strip_prefix(yog_ids::res::BLOCK_DIR) {
        return Some(ImageSource::Block(short(block)).to_string());
    }
    if ns == yog_ids::ns::MINECRAFT
        && let Some(atlas_id) = stem.strip_prefix(yog_ids::res::GUI_SPRITES)
        && let Some(sprite) = sprite_by_atlas(atlas_id)
    {
        return Some(ImageSource::Sprite(sprite.name.to_string()).to_string());
    }
    Some(yog_ids::join(ns, stem))
}

/// A region of a texture on Minecraft 1.20.1 (before the GUI sprite atlas).
#[derive(Debug, Clone, PartialEq)]
pub struct LegacyRegion {
    /// Texture resource id.
    pub texture: String,
    /// The region, in the texture's pixels.
    pub region: Rect,
    /// The whole texture's size.
    pub texture_size: Vec2,
}

/// One entry of Yog's GUI sprite table ([`SPRITES`]).
#[derive(Debug, Clone, PartialEq)]
pub struct Sprite {
    /// The stable Yog name (`sprite:<name>`).
    pub name: String,
    /// What it is, for editors' pickers.
    pub description: String,
    /// Natural size in GUI pixels (1.20.2+; 1.20.1 uses the legacy region's).
    pub size: Vec2,
    /// GUI-atlas sprite id on Minecraft 1.20.2+ (`minecraft:<id>`, i.e.
    /// `textures/gui/sprites/<id>.png`).
    pub atlas: Option<String>,
    /// Where it lives on 1.20.1.
    pub legacy: Option<LegacyRegion>,
    /// Yog's own picture of it (`yog:textures/gui/…`), for a version that has neither.
    pub own: Option<LegacyRegion>,
    /// Nine-slice border in pixels when stretched (0 = scale the whole image).
    pub border: f32,
}

impl Sprite {
    /// Natural size on a host with (`true`) or without the GUI sprite atlas.
    pub fn size(&self, atlas: bool) -> Vec2 {
        match (atlas, &self.legacy) {
            (false, Some(l)) => Vec2::new(l.region.w, l.region.h),
            _ => self.size,
        }
    }

    /// Drawn by Yog itself (no texture on any version) — the inventory slot.
    pub fn is_procedural(&self) -> bool {
        self.atlas.is_none() && self.legacy.is_none() && self.own.is_none()
    }

    /// Where it may come from, in the order a host tries them: the GUI
    /// atlas's sprite (`minecraft:<id>`), the game's texture region, Yog's
    /// own picture. The host draws it from the first its game has.
    pub fn sources(&self) -> Vec<SpriteSource<'_>> {
        let mut out = Vec::new();
        if let Some(a) = &self.atlas {
            out.push(SpriteSource::Atlas(yog_ids::qualify(a)));
        }
        out.extend(
            self.legacy
                .iter()
                .chain(&self.own)
                .map(SpriteSource::Region),
        );
        out
    }
}

/// One place a sprite may come from (see [`Sprite::sources`]).
#[derive(Debug, Clone, PartialEq)]
pub enum SpriteSource<'a> {
    /// A GUI-atlas sprite, by full id (`minecraft:widget/button`).
    Atlas(String),
    /// A region of a texture.
    Region(&'a LegacyRegion),
}

/// Yog's own assets (`yog:textures/gui/…`, `yog:models/item/…`), shipped in this crate's `res/`
/// (a resource pack's layout: the hosts take the folder as resources).
const OWN: &[(&str, &[u8])] = &[
    (
        "textures/gui/panel.png",
        include_bytes!("../res/assets/yog/textures/gui/panel.png"),
    ),
    (
        "models/item/spawn_egg.json",
        include_bytes!("../res/assets/yog/models/item/spawn_egg.json"),
    ),
    (
        "textures/item/spawn_egg.png",
        include_bytes!("../res/assets/yog/textures/item/spawn_egg.png"),
    ),
    (
        "textures/item/spawn_egg_overlay.png",
        include_bytes!("../res/assets/yog/textures/item/spawn_egg_overlay.png"),
    ),
];

/// The bytes of Yog's own asset `path` (`textures/gui/panel.png`, `models/item/spawn_egg.json`), if it has one.
pub fn own_asset(path: &str) -> Option<&'static [u8]> {
    OWN.iter().find(|(p, _)| *p == path).map(|(_, b)| *b)
}

/// Looks up a [`SPRITES`] entry by its Yog name.
pub fn sprite(name: &str) -> Option<&'static Sprite> {
    SPRITES.iter().find(|s| s.name == name.trim())
}

/// Looks up a [`SPRITES`] entry by its 1.20.2+ atlas id (`widget/button`).
pub fn sprite_by_atlas(atlas: &str) -> Option<&'static Sprite> {
    SPRITES.iter().find(|s| s.atlas.as_deref() == Some(atlas))
}

/// The texture of GUI-atlas sprite `atlas` of namespace `ns`:
/// `minecraft:textures/gui/sprites/<atlas>.png`.
pub fn atlas_texture(ns: &str, atlas: &str) -> String {
    yog_ids::join(
        ns,
        &yog_ids::texture_file(&format!("{}{atlas}", yog_ids::res::GUI_SPRITES)),
    )
}

/// A sprite as `assets/sprites.ydata` writes it.
#[derive(yog_data::Schema)]
struct SpriteDef {
    name: String,
    description: String,
    size: [f32; 2],
    atlas: Option<String>,
    legacy: Option<LegacyDef>,
    own: Option<LegacyDef>,
    #[data(default)]
    border: f32,
}

#[derive(yog_data::Schema)]
struct LegacyDef {
    texture: String,
    region: [f32; 4],
    /// The texture's side, when it isn't [`LEGACY_TEX_SIZE`].
    texture_size: Option<f32>,
}

fn region(l: LegacyDef) -> LegacyRegion {
    let [u, v, w, h] = l.region;
    let side = l.texture_size.unwrap_or(LEGACY_TEX_SIZE);
    LegacyRegion {
        texture: l.texture,
        region: Rect::new(u, v, w, h),
        texture_size: Vec2::new(side, side),
    }
}

/// Yog's GUI sprite names (`assets/sprites.ydata`).
pub static SPRITES: LazyLock<Vec<Sprite>> = LazyLock::new(|| {
    yog_data::from_text::<Vec<SpriteDef>>(include_str!("../assets/sprites.ydata"))
        .unwrap_or_else(|e| panic!("yog-gfx's sprites: {e}"))
        .value
        .into_iter()
        .map(|d| Sprite {
            name: d.name,
            description: d.description,
            size: Vec2::new(d.size[0], d.size[1]),
            atlas: d.atlas,
            legacy: d.legacy.map(region),
            own: d.own.map(region),
            border: d.border,
        })
        .collect()
});

/// How an image fills the box it's given — WPF's `Stretch`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum Stretch {
    /// Natural size, centered; cut to the box when larger.
    None = 0,
    /// The whole box (aspect ratio not kept).
    Fill = 1,
    /// As large as fits, aspect ratio kept, centered.
    #[default]
    Uniform = 2,
    /// Covers the box, aspect ratio kept, centered; the overflow is cut.
    UniformToFill = 3,
}

impl Stretch {
    /// Every stretch, with its name in `.yui` (`stretch="Fill"`).
    pub const ALL: [(Stretch, &'static str); 4] = [
        (Stretch::None, "None"),
        (Stretch::Fill, "Fill"),
        (Stretch::Uniform, "Uniform"),
        (Stretch::UniformToFill, "UniformToFill"),
    ];

    /// Its name in `.yui`.
    pub fn name(self) -> &'static str {
        Self::ALL
            .iter()
            .find(|(s, _)| *s == self)
            .map_or("", |(_, n)| n)
    }

    /// A stretch by its name, case and `_` ignored (`fill`, `uniform_to_fill`).
    pub fn parse(name: &str) -> Option<Stretch> {
        let wanted = name.trim().replace('_', "");
        Self::ALL
            .iter()
            .find(|(_, n)| n.eq_ignore_ascii_case(&wanted))
            .map(|(s, _)| *s)
    }
}

/// Where an image of size `natural` goes in the box `bx` under `stretch`:
/// `(dest, src)` — the destination rect and the part of the image shown, as
/// a rect in fractions of the image (less than the whole for
/// `None`/`UniformToFill` images bigger than the box).
pub fn fit(stretch: Stretch, natural: Vec2, bx: Rect) -> (Rect, Rect) {
    let whole = Rect::new(0.0, 0.0, 1.0, 1.0);
    if natural.x <= 0.0 || natural.y <= 0.0 || bx.w <= 0.0 || bx.h <= 0.0 {
        return (Rect::new(bx.x, bx.y, 0.0, 0.0), whole);
    }
    let scale = match stretch {
        Stretch::Fill => return (bx, whole),
        Stretch::None => 1.0,
        Stretch::Uniform => (bx.w / natural.x).min(bx.h / natural.y),
        Stretch::UniformToFill => (bx.w / natural.x).max(bx.h / natural.y),
    };
    let d = natural * scale;
    // Per axis: centered when it fits, else cut evenly on both sides.
    let axis = |pos: f32, size: f32, d: f32| -> (f32, f32, f32, f32) {
        if d <= size {
            ((pos + (size - d) / 2.0).floor(), d, 0.0, 1.0)
        } else {
            let cut = (d - size) / 2.0 / d;
            (pos, size, cut, 1.0 - 2.0 * cut)
        }
    };
    let (dx, dw, u, uw) = axis(bx.x, bx.w, d.x);
    let (dy, dh, v, vh) = axis(bx.y, bx.h, d.y);
    (Rect::new(dx, dy, dw, dh), Rect::new(u, v, uw, vh))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sources() {
        assert_eq!(
            ImageSource::parse("item:apple"),
            Some(ImageSource::Item("minecraft:apple".into()))
        );
        assert_eq!(
            ImageSource::parse("item:mymod:wand"),
            Some(ImageSource::Item("mymod:wand".into()))
        );
        assert_eq!(
            ImageSource::parse("block:stone"),
            Some(ImageSource::Block("minecraft:stone".into()))
        );
        assert_eq!(
            ImageSource::parse("sprite:button"),
            Some(ImageSource::Sprite("button".into()))
        );
        assert_eq!(
            ImageSource::parse("mymod:gui/logo"),
            Some(ImageSource::Texture("mymod:textures/gui/logo.png".into()))
        );
        assert_eq!(
            ImageSource::parse("minecraft:textures/item/apple.png"),
            Some(ImageSource::Texture(
                "minecraft:textures/item/apple.png".into()
            ))
        );
        assert_eq!(
            ImageSource::parse("mod:gui/logo").map(|s| s.with_mod_id("mymod")),
            Some(ImageSource::Texture("mymod:textures/gui/logo.png".into()))
        );
        assert_eq!(
            ImageSource::parse("mymod:gui/logo.svg"),
            Some(ImageSource::Texture("mymod:textures/gui/logo.svg".into()))
        );
        assert_eq!(
            ImageSource::parse("mymod:textures/gui/logo.svg"),
            Some(ImageSource::Texture("mymod:textures/gui/logo.svg".into()))
        );
        assert_eq!(ImageSource::parse(""), None);
        assert_eq!(ImageSource::parse("apple"), None);
        assert_eq!(ImageSource::parse("item:"), None);
    }

    #[test]
    fn suggests_aliases_for_raw_paths() {
        assert_eq!(
            suggest_alias("minecraft:textures/item/apple.png").as_deref(),
            Some("item:apple")
        );
        assert_eq!(
            suggest_alias("mymod:textures/block/ore.png").as_deref(),
            Some("block:mymod:ore")
        );
        assert_eq!(
            suggest_alias("minecraft:textures/gui/widgets.png").as_deref(),
            Some("minecraft:gui/widgets")
        );
        assert_eq!(
            suggest_alias("minecraft:textures/gui/sprites/widget/button.png").as_deref(),
            Some("sprite:button")
        );
        assert_eq!(
            suggest_alias("mymod:textures/gui/logo.png").as_deref(),
            Some("mymod:gui/logo")
        );
        assert_eq!(suggest_alias("item:apple"), None);
    }

    #[test]
    fn sprite_names_are_unique() {
        for (i, s) in SPRITES.iter().enumerate() {
            assert!(
                SPRITES[i + 1..].iter().all(|o| o.name != s.name),
                "{}",
                s.name
            );
        }
        assert!(sprite("slot").unwrap().is_procedural());
        assert_eq!(
            sprite("hotbar_selection").unwrap().size(false),
            Vec2::new(24.0, 24.0)
        );
        assert_eq!(
            sprite("hotbar_selection").unwrap().size(true),
            Vec2::new(24.0, 23.0)
        );
    }

    #[test]
    fn fits_like_wpf() {
        let icon = Vec2::new(16.0, 16.0);
        let bx = Rect::new(0.0, 0.0, 156.0, 29.0);
        let whole = Rect::new(0.0, 0.0, 1.0, 1.0);
        // 16×16 into 156×29.
        assert_eq!(
            fit(Stretch::Uniform, icon, bx),
            (Rect::new(63.0, 0.0, 29.0, 29.0), whole)
        );
        assert_eq!(
            fit(Stretch::Fill, icon, Rect::new(1.0, 2.0, 156.0, 29.0)).0,
            Rect::new(1.0, 2.0, 156.0, 29.0)
        );
        assert_eq!(
            fit(Stretch::None, icon, bx).0,
            Rect::new(70.0, 6.0, 16.0, 16.0)
        );
        let (d, s) = fit(Stretch::UniformToFill, icon, bx);
        assert_eq!(d, bx);
        assert!((s.y - (156.0 - 29.0) / 2.0 / 156.0).abs() < 1e-5, "{s:?}");
        // None, bigger than the box: cut to it.
        let (d, s) = fit(
            Stretch::None,
            Vec2::new(40.0, 10.0),
            Rect::new(0.0, 0.0, 20.0, 10.0),
        );
        assert_eq!(d, Rect::new(0.0, 0.0, 20.0, 10.0));
        assert_eq!(s, Rect::new(0.25, 0.0, 0.5, 1.0));
    }
}
