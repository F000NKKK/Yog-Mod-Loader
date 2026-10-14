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

/// The namespace of ids that don't name one.
const DEFAULT_NS: &str = "minecraft";
/// Side of the square 1.20.1 GUI textures most legacy regions live in.
const LEGACY_TEX_SIZE: f32 = 256.0;

/// `apple` → `minecraft:apple`; namespaced ids are kept.
pub fn qualify(id: &str) -> String {
    let id = id.trim();
    if id.contains(':') {
        id.to_string()
    } else {
        format!("{DEFAULT_NS}:{id}")
    }
}

/// `<ns>` + `<path>` → the texture resource id: `gui/logo` →
/// `<ns>:textures/gui/logo.png`, `gui/logo.svg` →
/// `<ns>:textures/gui/logo.svg` (an SVG image); paths already under
/// `textures/` (or ending in `.png`) are kept as they are.
pub fn texture_id(ns: &str, path: &str) -> String {
    let path = path.trim_start_matches('/');
    if path.ends_with(".svg") {
        let path = path.strip_prefix("textures/").unwrap_or(path);
        return format!("{ns}:textures/{path}");
    }
    if path.starts_with("textures/") || path.ends_with(".png") {
        let png = if path.ends_with(".png") { "" } else { ".png" };
        format!("{ns}:{path}{png}")
    } else {
        format!("{ns}:textures/{path}.png")
    }
}

impl ImageSource {
    /// Parses a source string; `None` for an empty or malformed one.
    pub fn parse(source: &str) -> Option<ImageSource> {
        let s = source.trim();
        let (scheme, rest) = s.split_once(':')?;
        if rest.is_empty() {
            return None;
        }
        Some(match scheme {
            "item" => ImageSource::Item(qualify(rest)),
            "block" => ImageSource::Block(qualify(rest)),
            "sprite" => ImageSource::Sprite(rest.to_string()),
            "mod" => ImageSource::ModPath(rest.to_string()),
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
    let (ns, path) = id.trim().split_once(':')?;
    let path = path.strip_prefix("textures/")?;
    let stem = path.strip_suffix(".png").unwrap_or(path);
    // `minecraft:` is the default namespace of every alias.
    let short = |id: &str| -> String {
        if ns == DEFAULT_NS {
            id.to_string()
        } else {
            format!("{ns}:{id}")
        }
    };
    if let Some(item) = stem.strip_prefix("item/") {
        return Some(format!("item:{}", short(item)));
    }
    if let Some(block) = stem.strip_prefix("block/") {
        return Some(format!("block:{}", short(block)));
    }
    if ns == DEFAULT_NS
        && let Some(atlas_id) = stem.strip_prefix("gui/sprites/")
        && let Some(sprite) = SPRITES.iter().find(|s| s.atlas == Some(atlas_id))
    {
        return Some(format!("sprite:{}", sprite.name));
    }
    Some(format!("{ns}:{stem}"))
}

/// A region of a texture on Minecraft 1.20.1 (before the GUI sprite atlas).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LegacyRegion {
    /// Texture resource id.
    pub texture: &'static str,
    pub u: f32,
    pub v: f32,
    pub w: f32,
    pub h: f32,
    /// Full texture size.
    pub tex_w: f32,
    pub tex_h: f32,
}

/// One entry of Yog's GUI sprite table.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sprite {
    /// The stable Yog name (`sprite:<name>`).
    pub name: &'static str,
    /// What it is, for editors' pickers.
    pub description: &'static str,
    /// Natural size in GUI pixels (1.20.2+; 1.20.1 uses the legacy region's).
    pub w: f32,
    pub h: f32,
    /// GUI-atlas sprite id on Minecraft 1.20.2+ (`minecraft:<id>`, i.e.
    /// `textures/gui/sprites/<id>.png`).
    pub atlas: Option<&'static str>,
    /// Where it lives on 1.20.1.
    pub legacy: Option<LegacyRegion>,
    /// Nine-slice border in pixels when stretched (0 = scale the whole image).
    pub border: f32,
}

impl Sprite {
    /// Natural size on a host with (`true`) or without the GUI sprite atlas.
    pub fn size(&self, atlas: bool) -> (f32, f32) {
        match (atlas, self.legacy) {
            (false, Some(l)) => (l.w, l.h),
            _ => (self.w, self.h),
        }
    }

    /// Drawn by Yog itself (no texture on any version) — the inventory slot.
    pub fn is_procedural(&self) -> bool {
        self.atlas.is_none() && self.legacy.is_none()
    }
}

/// Looks up a [`SPRITES`] entry by its Yog name.
pub fn sprite(name: &str) -> Option<&'static Sprite> {
    SPRITES.iter().find(|s| s.name == name.trim())
}

const WIDGETS: &str = "minecraft:textures/gui/widgets.png";
const ICONS: &str = "minecraft:textures/gui/icons.png";
const TABS: &str = "minecraft:textures/gui/container/creative_inventory/tabs.png";
const CHECKBOX: &str = "minecraft:textures/gui/checkbox.png";
/// The demo screen's window — vanilla's plain grey window frame, the same
/// texture on every version.
const WINDOW: &str = "minecraft:textures/gui/demo_background.png";

/// A region of a [`LEGACY_TEX_SIZE`]-square texture.
const fn region(texture: &'static str, u: f32, v: f32, w: f32, h: f32) -> Option<LegacyRegion> {
    region_of(texture, LEGACY_TEX_SIZE, u, v, w, h)
}

const fn region_of(
    texture: &'static str,
    size: f32,
    u: f32,
    v: f32,
    w: f32,
    h: f32,
) -> Option<LegacyRegion> {
    Some(LegacyRegion {
        texture,
        u,
        v,
        w,
        h,
        tex_w: size,
        tex_h: size,
    })
}

#[allow(clippy::too_many_arguments)]
const fn sprite_def(
    name: &'static str,
    description: &'static str,
    w: f32,
    h: f32,
    atlas: &'static str,
    legacy: Option<LegacyRegion>,
    border: f32,
) -> Sprite {
    Sprite {
        name,
        description,
        w,
        h,
        atlas: Some(atlas),
        legacy,
        border,
    }
}

/// Yog's GUI sprite names. Adding one: pick a stable name, its 1.20.2+
/// atlas id (`assets/minecraft/textures/gui/sprites/<id>.png` in the client
/// jar) and its 1.20.1 region (`textures/gui/*.png`, in pixels).
pub const SPRITES: &[Sprite] = &[
    sprite_def(
        "button",
        "Button background",
        200.0,
        20.0,
        "widget/button",
        region(WIDGETS, 0.0, 66.0, 200.0, 20.0),
        3.0,
    ),
    sprite_def(
        "button_hover",
        "Button background, hovered/focused",
        200.0,
        20.0,
        "widget/button_highlighted",
        region(WIDGETS, 0.0, 86.0, 200.0, 20.0),
        3.0,
    ),
    sprite_def(
        "button_disabled",
        "Button background, disabled",
        200.0,
        20.0,
        "widget/button_disabled",
        region(WIDGETS, 0.0, 46.0, 200.0, 20.0),
        3.0,
    ),
    Sprite {
        name: "panel",
        description: "Window background (vanilla's grey frame)",
        w: 248.0,
        h: 166.0,
        atlas: None,
        legacy: region(WINDOW, 0.0, 0.0, 248.0, 166.0),
        border: 4.0,
    },
    Sprite {
        name: "slot",
        description: "Inventory slot (18×18)",
        w: 18.0,
        h: 18.0,
        atlas: None,
        legacy: None,
        border: 1.0,
    },
    sprite_def(
        "scrollbar",
        "Scrollbar knob (creative inventory)",
        12.0,
        15.0,
        "container/creative_inventory/scroller",
        region(TABS, 232.0, 0.0, 12.0, 15.0),
        0.0,
    ),
    sprite_def(
        "scrollbar_disabled",
        "Scrollbar knob, disabled",
        12.0,
        15.0,
        "container/creative_inventory/scroller_disabled",
        region(TABS, 244.0, 0.0, 12.0, 15.0),
        0.0,
    ),
    sprite_def(
        "checkbox",
        "Checkbox, unchecked",
        20.0,
        20.0,
        "widget/checkbox",
        region_of(CHECKBOX, 64.0, 0.0, 0.0, 20.0, 20.0),
        0.0,
    ),
    sprite_def(
        "checkbox_checked",
        "Checkbox, checked",
        20.0,
        20.0,
        "widget/checkbox_selected",
        region_of(CHECKBOX, 64.0, 0.0, 20.0, 20.0, 20.0),
        0.0,
    ),
    sprite_def(
        "hotbar",
        "Hotbar",
        182.0,
        22.0,
        "hud/hotbar",
        region(WIDGETS, 0.0, 0.0, 182.0, 22.0),
        0.0,
    ),
    sprite_def(
        "hotbar_selection",
        "Hotbar selection frame",
        24.0,
        23.0,
        "hud/hotbar_selection",
        region(WIDGETS, 0.0, 22.0, 24.0, 24.0),
        0.0,
    ),
    sprite_def(
        "crosshair",
        "Crosshair",
        15.0,
        15.0,
        "hud/crosshair",
        region(ICONS, 0.0, 0.0, 15.0, 15.0),
        0.0,
    ),
    sprite_def(
        "xp_bar",
        "Experience bar, empty",
        182.0,
        5.0,
        "hud/experience_bar_background",
        region(ICONS, 0.0, 64.0, 182.0, 5.0),
        0.0,
    ),
    sprite_def(
        "xp_bar_progress",
        "Experience bar, full",
        182.0,
        5.0,
        "hud/experience_bar_progress",
        region(ICONS, 0.0, 69.0, 182.0, 5.0),
        0.0,
    ),
    sprite_def(
        "icon/heart",
        "Heart",
        9.0,
        9.0,
        "hud/heart/full",
        region(ICONS, 52.0, 0.0, 9.0, 9.0),
        0.0,
    ),
    sprite_def(
        "icon/heart_half",
        "Half heart",
        9.0,
        9.0,
        "hud/heart/half",
        region(ICONS, 61.0, 0.0, 9.0, 9.0),
        0.0,
    ),
    sprite_def(
        "icon/heart_empty",
        "Heart container (empty)",
        9.0,
        9.0,
        "hud/heart/container",
        region(ICONS, 16.0, 0.0, 9.0, 9.0),
        0.0,
    ),
    sprite_def(
        "icon/food",
        "Hunger shank",
        9.0,
        9.0,
        "hud/food_full",
        region(ICONS, 52.0, 27.0, 9.0, 9.0),
        0.0,
    ),
    sprite_def(
        "icon/food_half",
        "Half hunger shank",
        9.0,
        9.0,
        "hud/food_half",
        region(ICONS, 61.0, 27.0, 9.0, 9.0),
        0.0,
    ),
    sprite_def(
        "icon/food_empty",
        "Hunger shank, empty",
        9.0,
        9.0,
        "hud/food_empty",
        region(ICONS, 16.0, 27.0, 9.0, 9.0),
        0.0,
    ),
    sprite_def(
        "icon/armor",
        "Armor",
        9.0,
        9.0,
        "hud/armor_full",
        region(ICONS, 34.0, 9.0, 9.0, 9.0),
        0.0,
    ),
    sprite_def(
        "icon/armor_half",
        "Half armor",
        9.0,
        9.0,
        "hud/armor_half",
        region(ICONS, 25.0, 9.0, 9.0, 9.0),
        0.0,
    ),
    sprite_def(
        "icon/armor_empty",
        "Armor, empty",
        9.0,
        9.0,
        "hud/armor_empty",
        region(ICONS, 16.0, 9.0, 9.0, 9.0),
        0.0,
    ),
    sprite_def(
        "icon/air",
        "Air bubble",
        9.0,
        9.0,
        "hud/air",
        region(ICONS, 16.0, 18.0, 9.0, 9.0),
        0.0,
    ),
    sprite_def(
        "icon/air_bursting",
        "Air bubble, bursting",
        9.0,
        9.0,
        "hud/air_bursting",
        region(ICONS, 25.0, 18.0, 9.0, 9.0),
        0.0,
    ),
];

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

/// Where a `nat_w × nat_h` image goes in the box `(x, y, w, h)` under
/// `stretch`: `(dest, src_fraction)` — the destination rect and the part of
/// the image shown, as fractions `(u0, v0, u1, v1)` of the image (less than
/// the whole for `None`/`UniformToFill` images bigger than the box).
pub fn fit(
    stretch: Stretch,
    nat_w: f32,
    nat_h: f32,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
) -> ([f32; 4], [f32; 4]) {
    if nat_w <= 0.0 || nat_h <= 0.0 || w <= 0.0 || h <= 0.0 {
        return ([x, y, 0.0, 0.0], [0.0, 0.0, 1.0, 1.0]);
    }
    let scale = match stretch {
        Stretch::Fill => return ([x, y, w, h], [0.0, 0.0, 1.0, 1.0]),
        Stretch::None => 1.0,
        Stretch::Uniform => (w / nat_w).min(h / nat_h),
        Stretch::UniformToFill => (w / nat_w).max(h / nat_h),
    };
    let (dw, dh) = (nat_w * scale, nat_h * scale);
    // Per axis: centered when it fits, else cut evenly on both sides.
    let axis = |pos: f32, size: f32, d: f32| -> (f32, f32, f32, f32) {
        if d <= size {
            ((pos + (size - d) / 2.0).floor(), d, 0.0, 1.0)
        } else {
            let cut = (d - size) / 2.0 / d;
            (pos, size, cut, 1.0 - cut)
        }
    };
    let (dx, dw, u0, u1) = axis(x, w, dw);
    let (dy, dh, v0, v1) = axis(y, h, dh);
    ([dx, dy, dw, dh], [u0, v0, u1, v1])
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
            (24.0, 24.0)
        );
        assert_eq!(sprite("hotbar_selection").unwrap().size(true), (24.0, 23.0));
    }

    #[test]
    fn fits_like_wpf() {
        // 16×16 into 156×29.
        let (d, s) = fit(Stretch::Uniform, 16.0, 16.0, 0.0, 0.0, 156.0, 29.0);
        assert_eq!(d, [63.0, 0.0, 29.0, 29.0]);
        assert_eq!(s, [0.0, 0.0, 1.0, 1.0]);
        assert_eq!(
            fit(Stretch::Fill, 16.0, 16.0, 1.0, 2.0, 156.0, 29.0).0,
            [1.0, 2.0, 156.0, 29.0]
        );
        assert_eq!(
            fit(Stretch::None, 16.0, 16.0, 0.0, 0.0, 156.0, 29.0).0,
            [70.0, 6.0, 16.0, 16.0]
        );
        let (d, s) = fit(Stretch::UniformToFill, 16.0, 16.0, 0.0, 0.0, 156.0, 29.0);
        assert_eq!(d, [0.0, 0.0, 156.0, 29.0]);
        assert!((s[1] - (156.0 - 29.0) / 2.0 / 156.0).abs() < 1e-5, "{s:?}");
        // None, bigger than the box: cut to it.
        let (d, s) = fit(Stretch::None, 40.0, 10.0, 0.0, 0.0, 20.0, 10.0);
        assert_eq!(d, [0.0, 0.0, 20.0, 10.0]);
        assert_eq!(s, [0.25, 0.0, 0.75, 1.0]);
    }
}
