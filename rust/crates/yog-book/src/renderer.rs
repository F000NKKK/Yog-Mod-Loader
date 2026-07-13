//! Book renderer — ties together yog-ui layout, yog-gfx GPU pipeline,
//! SVG icon rasterization, and custom font rendering.

use std::collections::HashMap;
use yog_gfx::{
    core::{blend, DataType, DrawMode},
    gl, GfxContext,
};
use yog_ui::{widget, Align, FlexDir, UiRoot};

use crate::font::{BookFontRegistry, FontAtlas};
use crate::state::BookViewState;
use crate::svg;
use crate::theme::BookTheme;
use crate::{Book, BookPage};

// Patchouli-compatible book texture layout (512×256 sprite sheet).
// UV coordinates are in pixels on a 512×256 sheet.
// The full open-book background occupies 272×180 at UV (0,0).
pub const BOOK_TEX_W: f32 = 512.0;
pub const BOOK_TEX_H: f32 = 256.0;
// Open-book dimensions in texture-space
pub const BK_W: f32 = 272.0;
pub const BK_H: f32 = 180.0;
// Per-page dimensions and offsets (in book-local coords)
pub const PAGE_W: f32 = 116.0;
pub const PAGE_H: f32 = 156.0;
pub const TOP_PAD: f32 = 18.0; // vertical space above page text area
pub const LEFT_X: f32 = 15.0; // left page X inside book
pub const RIGHT_X: f32 = 141.0; // right page X inside book
                                // Separator strip UV origin on the sprite sheet
pub const SEP_U: f32 = 140.0;
pub const SEP_V: f32 = 180.0;
pub const SEP_W: f32 = 110.0;
pub const SEP_H: f32 = 3.0;

// ── Vertex for the custom 2D shader (pos + uv + color) ───────────────────────

#[repr(C)]
#[derive(Copy, Clone)]
struct Vert {
    x: f32,
    y: f32,
    u: f32,
    v: f32,
    r: f32,
    g: f32,
    b: f32,
    a: f32,
}

fn rgba_f(c: u32) -> (f32, f32, f32, f32) {
    let a = ((c >> 24) & 0xFF) as f32 / 255.0;
    let r = ((c >> 16) & 0xFF) as f32 / 255.0;
    let g = ((c >> 8) & 0xFF) as f32 / 255.0;
    let b = (c & 0xFF) as f32 / 255.0;
    (r, g, b, a)
}

fn quad(
    verts: &mut Vec<Vert>,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    u0: f32,
    v0: f32,
    u1: f32,
    v1: f32,
    color: u32,
) {
    let (r, g, b, a) = rgba_f(color);
    let p = [
        Vert {
            x,
            y,
            u: u0,
            v: v0,
            r,
            g,
            b,
            a,
        },
        Vert {
            x: x + w,
            y,
            u: u1,
            v: v0,
            r,
            g,
            b,
            a,
        },
        Vert {
            x,
            y: y + h,
            u: u0,
            v: v1,
            r,
            g,
            b,
            a,
        },
        Vert {
            x: x + w,
            y: y + h,
            u: u1,
            v: v1,
            r,
            g,
            b,
            a,
        },
    ];
    // two triangles: 0,1,2  1,3,2
    verts.extend_from_slice(&[p[0], p[1], p[2], p[1], p[3], p[2]]);
}

// ── GLSL shaders ─────────────────────────────────────────────────────────────

const VERT: &str = r#"
#version 330 core
layout(location = 0) in vec2 aPos;
layout(location = 1) in vec2 aUv;
layout(location = 2) in vec4 aCol;
out vec2 fUv;
out vec4 fCol;
uniform vec2 uScreen;
void main() {
    fUv  = aUv;
    fCol = aCol;
    vec2 ndc = aPos / uScreen * 2.0 - vec2(1.0);
    gl_Position = vec4(ndc.x, -ndc.y, 0.0, 1.0);
}
"#;

const FRAG: &str = r#"
#version 330 core
in vec2 fUv;
in vec4 fCol;
out vec4 outColor;
uniform sampler2D uTex;
uniform int uMode;  // 0 = solid color, 1 = RGBA texture, 2 = alpha-only (font)
void main() {
    if (uMode == 0) {
        outColor = fCol;
    } else if (uMode == 1) {
        outColor = texture(uTex, fUv) * fCol;
    } else {
        float alpha = texture(uTex, fUv).a;
        outColor = vec4(fCol.rgb, fCol.a * alpha);
    }
}
"#;

// ── Embedded textures ─────────────────────────────────────────────────────────

static BOOK_PNG: &[u8] = include_bytes!("../assets/book_brown.png");
static CRAFTING_PNG: &[u8] = include_bytes!("../assets/crafting.png");

fn decode_png(data: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
    use png::Decoder;
    let decoder = Decoder::new(std::io::Cursor::new(data));
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0u8; reader.output_buffer_size().map(|s| s as usize).unwrap_or(0)];
    let info = reader.next_frame(&mut buf).ok()?;
    let rgba = match info.color_type {
        png::ColorType::Rgba => buf[..info.buffer_size()].to_vec(),
        png::ColorType::Rgb => {
            let rgb = &buf[..info.buffer_size()];
            let mut out = Vec::with_capacity(rgb.len() / 3 * 4);
            for px in rgb.chunks(3) {
                out.extend_from_slice(px);
                out.push(255);
            }
            out
        }
        _ => return None,
    };
    Some((rgba, info.width, info.height))
}

// ── GL resource cache ─────────────────────────────────────────────────────────

struct BookGl {
    prog: gl::ShaderProgram,
    vao: gl::VertexArray,
    vbo: gl::Buffer,
    book_tex: Option<(gl::Texture, u32, u32)>,
    crafting_tex: Option<(gl::Texture, u32, u32)>,
    svg_tex: HashMap<u64, (gl::Texture, u32, u32)>,
    font_atlas: HashMap<u64, (gl::Texture, FontAtlas)>,
}

impl BookGl {
    fn init(ctx: &GfxContext) -> Option<Self> {
        let prog = ctx.create_shader(VERT, FRAG).ok()?;
        let vbo = ctx.create_buffer();
        let vao = ctx.create_vao();

        const STRIDE: u32 = 32; // 8 × f32
        vao.attrib(ctx, &vbo, 0, 2, DataType::F32, false, STRIDE, 0); // pos
        vao.attrib(ctx, &vbo, 1, 2, DataType::F32, false, STRIDE, 8); // uv
        vao.attrib(ctx, &vbo, 2, 4, DataType::F32, false, STRIDE, 16); // col

        let load = |data: &[u8]| {
            decode_png(data).map(|(rgba, w, h)| {
                let tex = ctx.create_texture_rgba(w, h, &rgba, true);
                (tex, w, h)
            })
        };
        let book_tex = load(BOOK_PNG);
        let crafting_tex = load(CRAFTING_PNG);

        Some(BookGl {
            prog,
            vao,
            vbo,
            book_tex,
            crafting_tex,
            svg_tex: HashMap::new(),
            font_atlas: HashMap::new(),
        })
    }

    fn svg_tex(
        &mut self,
        ctx: &GfxContext,
        hash: u64,
        data: &str,
        w: u32,
        h: u32,
    ) -> Option<&(gl::Texture, u32, u32)> {
        if !self.svg_tex.contains_key(&hash) {
            let pixels = svg::rasterize(data, w, h)?;
            let tex = ctx.create_texture_rgba(w, h, &pixels, true);
            self.svg_tex.insert(hash, (tex, w, h));
        }
        self.svg_tex.get(&hash)
    }

    fn flush(&self, ctx: &GfxContext, verts: &[Vert]) {
        if verts.is_empty() {
            return;
        }
        unsafe {
            self.vbo.upload(ctx, verts, true);
        }
        ctx.draw_arrays(
            &self.vao,
            &self.prog,
            DrawMode::Triangles,
            0,
            verts.len() as u32,
        );
    }
}

// ── Draw helpers ──────────────────────────────────────────────────────────────

impl BookGl {
    fn begin_frame(&self, ctx: &GfxContext, sw: f32, sh: f32) {
        ctx.set_blend(true, blend::SRC_ALPHA, blend::ONE_MINUS_SRC_ALPHA);
        ctx.set_depth(false, false);
        self.prog.uniform_2f(ctx, "uScreen", sw, sh);
        self.prog.uniform_1i(ctx, "uTex", 0);
    }

    /// Blit a subrect of a sprite sheet at an arbitrary screen position.
    fn draw_book_sprite(&self, ctx: &GfxContext, spr: &BgSprite) {
        let sheet = match spr.sheet {
            SpriteSheet::Book => &self.book_tex,
            SpriteSheet::Crafting => &self.crafting_tex,
        };
        let Some((tex, tw, th)) = sheet else { return };
        let u0 = spr.u / *tw as f32;
        let v0 = spr.v / *th as f32;
        let u1 = (spr.u + spr.uw) / *tw as f32;
        let v1 = (spr.v + spr.uh) / *th as f32;
        self.prog.uniform_1i(ctx, "uMode", 1);
        ctx.bind_texture(0, tex);
        let mut v = Vec::with_capacity(6);
        quad(
            &mut v,
            spr.x,
            spr.y,
            spr.w,
            spr.h,
            u0,
            v0,
            u1,
            v1,
            0xFF_FFFFFF,
        );
        self.flush(ctx, &v);
    }

    /// Blit the open-book background from the embedded book texture.
    fn draw_book_bg(&self, ctx: &GfxContext, bx: f32, by: f32, bw: f32, bh: f32) {
        let Some((tex, tw, th)) = &self.book_tex else {
            return;
        };
        let su = BK_W / *tw as f32;
        let sv = BK_H / *th as f32;
        self.prog.uniform_1i(ctx, "uMode", 1);
        ctx.bind_texture(0, tex);
        let mut v = Vec::with_capacity(6);
        quad(&mut v, bx, by, bw, bh, 0.0, 0.0, su, sv, 0xFF_FFFFFF);
        self.flush(ctx, &v);
    }

    fn draw_svg(&mut self, ctx: &GfxContext, data: &str, x: f32, y: f32, w: f32, h: f32) {
        let hash = svg::svg_hash(data);
        let tw = w as u32;
        let th = h as u32;
        if let Some(&(tex, _, _)) = self.svg_tex(ctx, hash, data, tw, th) {
            self.prog.uniform_1i(ctx, "uMode", 1);
            ctx.bind_texture(0, &tex);
            let mut v = Vec::with_capacity(6);
            quad(&mut v, x, y, w, h, 0.0, 0.0, 1.0, 1.0, 0xFF_FFFFFF);
            self.flush(ctx, &v);
        }
    }

    fn draw_text_custom(
        &mut self,
        ctx: &GfxContext,
        ttf: &[u8],
        size_px: f32,
        text: &str,
        mut x: f32,
        y: f32,
        color: u32,
    ) {
        let hash = font_hash(ttf);
        if !self.font_atlas.contains_key(&hash) {
            if let Some(atlas) = FontAtlas::build(ttf, size_px) {
                let tex = ctx.create_texture_rgba(
                    atlas.atlas_size,
                    atlas.atlas_size,
                    &atlas.pixels,
                    true,
                );
                self.font_atlas.insert(hash, (tex, atlas));
            }
        }
        // Get raw pointers so the borrow of self.font_atlas ends before we call
        // self.prog / self.flush, which also borrow fields of self.
        let ptrs = self
            .font_atlas
            .get(&hash)
            .map(|(t, a)| (t as *const gl::Texture, a as *const FontAtlas));
        if let Some((tex_ptr, atlas_ptr)) = ptrs {
            // SAFETY: we hold &mut self; font_atlas is not mutated below.
            let tex = unsafe { &*tex_ptr };
            let atlas = unsafe { &*atlas_ptr };
            self.prog.uniform_1i(ctx, "uMode", 2);
            ctx.bind_texture(0, tex);
            let baseline = y + size_px;
            let mut verts = Vec::new();
            for ch in text.chars() {
                if let Some(g) = atlas.glyphs.get(&ch) {
                    if g.width > 0 {
                        let gx = x + g.xoff;
                        let gy = baseline - g.yoff - g.height as f32;
                        quad(
                            &mut verts,
                            gx,
                            gy,
                            g.width as f32,
                            g.height as f32,
                            g.u0,
                            g.v0,
                            g.u1,
                            g.v1,
                            color,
                        );
                    }
                    x += g.advance;
                }
            }
            self.flush(ctx, &verts);
        }
    }
}

fn font_hash(data: &[u8]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    data.hash(&mut h);
    h.finish()
}

// ── Background sprite (texture blit, drawn before yog-ui) ────────────────────

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum SpriteSheet {
    Book,
    Crafting,
}

/// A subrect blit from one of the embedded sprite sheets, drawn before yog-ui.
#[derive(Clone)]
pub(crate) struct BgSprite {
    pub sheet: SpriteSheet,
    /// Source rect in pixels on the sheet.
    pub u: f32,
    pub v: f32,
    pub uw: f32,
    pub uh: f32,
    /// Destination on screen.
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

// ── Pending overlay draw commands ─────────────────────────────────────────────

#[derive(Clone)]
pub(crate) enum OverlayCmd {
    Svg {
        data: String,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
    },
    Text {
        text: String,
        font: crate::font::BookFont,
        x: f32,
        y: f32,
        color: u32,
    },
    /// MC default-font text rendered via draw2d (e.g. nameplate title/subtitle).
    McText {
        text: String,
        x: f32,
        y: f32,
        color: u32,
    },
    /// Item icon rendered through our GL pipeline from an MC-managed texture.
    McItem {
        item_id: String,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
    },
    /// Flat texture icon blitted as-is (16×16), for icon strings that name a
    /// texture resource directly rather than an item — matches Patchouli's
    /// `BookIcon.from`: any icon string ending in `.png` is a raw texture,
    /// everything else is parsed as an item stack.
    McTexture {
        path: String,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
    },
}

/// Push a book-author-supplied icon (category/entry icon field) as the right
/// overlay kind. Matches Patchouli's `BookIcon.from(String)`: `.png` → raw
/// texture blit, anything else → item stack render.
fn push_icon(overlays: &mut Vec<OverlayCmd>, icon: &str, x: f32, y: f32, w: f32, h: f32) {
    if icon.ends_with(".png") {
        overlays.push(OverlayCmd::McTexture {
            path: icon.to_string(),
            x,
            y,
            w,
            h,
        });
    } else {
        overlays.push(OverlayCmd::McItem {
            item_id: icon.to_string(),
            x,
            y,
            w,
            h,
        });
    }
}

// ── Recipe visuals (parsed from registered recipe JSON) ───────────────────────

#[derive(Clone)]
pub(crate) enum RecipeVis {
    Shaped {
        grid: Vec<Option<String>>,
        width: usize,
        output: String,
        count: u32,
    },
    Shapeless {
        ingredients: Vec<String>,
        output: String,
        count: u32,
    },
    Smelting {
        input: String,
        output: String,
    },
}

fn parse_recipe(json: &str) -> Option<RecipeVis> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    match v.get("type")?.as_str()? {
        "minecraft:crafting_shaped" => {
            let pattern: Vec<&str> = v
                .get("pattern")?
                .as_array()?
                .iter()
                .filter_map(|p| p.as_str())
                .collect();
            let width = pattern.iter().map(|r| r.chars().count()).max()?.max(1);
            let key = v.get("key")?.as_object()?;
            let mut grid = Vec::new();
            for row in &pattern {
                let mut chars: Vec<char> = row.chars().collect();
                chars.resize(width, ' ');
                for ch in chars {
                    grid.push(
                        key.get(&ch.to_string())
                            .and_then(|k| k.get("item"))
                            .and_then(|i| i.as_str())
                            .map(str::to_owned),
                    );
                }
            }
            let result = v.get("result")?;
            Some(RecipeVis::Shaped {
                grid,
                width,
                output: result.get("item")?.as_str()?.to_owned(),
                count: result.get("count").and_then(|c| c.as_u64()).unwrap_or(1) as u32,
            })
        }
        "minecraft:crafting_shapeless" => {
            let ingredients = v
                .get("ingredients")?
                .as_array()?
                .iter()
                .filter_map(|i| i.get("item").and_then(|x| x.as_str()).map(str::to_owned))
                .collect();
            let result = v.get("result")?;
            Some(RecipeVis::Shapeless {
                ingredients,
                output: result.get("item")?.as_str()?.to_owned(),
                count: result.get("count").and_then(|c| c.as_u64()).unwrap_or(1) as u32,
            })
        }
        "minecraft:smelting"
        | "minecraft:blasting"
        | "minecraft:smoking"
        | "minecraft:campfire_cooking" => Some(RecipeVis::Smelting {
            input: v.get("ingredient")?.get("item")?.as_str()?.to_owned(),
            output: v.get("result")?.as_str()?.to_owned(),
        }),
        _ => None,
    }
}

/// Normalize icon ids like "yog:item/ruby" / "yog:block/x" to registry item
/// ids ("yog:ruby") accepted by the MC item renderer.
fn normalize_item_id(id: &str) -> String {
    let (ns, path) = id.split_once(':').unwrap_or(("minecraft", id));
    let name = path
        .trim_start_matches("item/")
        .trim_start_matches("block/");
    format!("{ns}:{name}")
}

/// "yog:ruby_block" → "Ruby Block" (fallback display name from an item id).
fn pretty_item_name(id: &str) -> String {
    let name = id.rsplit(':').next().unwrap_or(id);
    name.split('_')
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

// ── BookRenderer ──────────────────────────────────────────────────────────────

pub struct BookRenderer {
    pub book: Book,
    pub state: BookViewState,
    pub theme: BookTheme,
    fonts: BookFontRegistry,
    recipes: HashMap<String, RecipeVis>,
    gl: Option<BookGl>,
    pub ui: Option<UiRoot>,
    bg_sprites: Vec<BgSprite>,
    overlays: Vec<OverlayCmd>,
    dirty: bool,
    last_sw: f32,
    last_sh: f32,
}

impl BookRenderer {
    pub fn new(book: Book) -> Self {
        let theme = BookTheme::default().with_nameplate(&book.nameplate_color);
        Self {
            book,
            state: BookViewState::default(),
            theme,
            fonts: BookFontRegistry::default(),
            recipes: HashMap::new(),
            gl: None,
            ui: None,
            bg_sprites: Vec::new(),
            overlays: Vec::new(),
            dirty: true,
            last_sw: 0.0,
            last_sh: 0.0,
        }
    }

    pub fn handle_event(&mut self, ev: &str) {
        if self.state.handle(ev, &self.book) {
            self.dirty = true;
        }
    }

    /// Register a custom TTF/OTF font for use in `BookPage::CustomText`.
    pub fn register_font(&mut self, id: impl Into<String>, ttf: Vec<u8>) {
        self.fonts.register(id, ttf);
    }

    /// Register a recipe (vanilla recipe JSON) so Crafting/Smelting pages can
    /// render it visually. Unknown recipe types are ignored.
    pub fn add_recipe(&mut self, id: impl Into<String>, json: &str) {
        if let Some(vis) = parse_recipe(json) {
            self.recipes.insert(id.into(), vis);
            self.dirty = true;
        }
    }

    pub fn render(&mut self, ctx: &GfxContext, sw: f32, sh: f32) {
        // Lazy GL init.
        if self.gl.is_none() {
            self.gl = BookGl::init(ctx);
        }

        // Render at Patchouli-native size (272×180 GUI pixels). Scale DOWN only
        // if the screen is smaller; never scale up (MC font is designed for 1×).
        let scale = ((sw - 16.0) / BK_W)
            .min((sh - 16.0) / BK_H)
            .min(1.0)
            .max(0.4);
        let bw = BK_W * scale;
        let bh = BK_H * scale;
        let bx = (sw - bw) / 2.0;
        let by = (sh - bh) / 2.0;

        // Rebuild widget tree if dirty or screen resized.
        if self.dirty || sw != self.last_sw || sh != self.last_sh {
            let (root, bg_sprites, overlays) = build_ui(
                &self.book,
                &self.state,
                &self.theme,
                &self.recipes,
                sw,
                sh,
                bx,
                by,
                bw,
                bh,
            );
            self.ui = Some(root);
            self.bg_sprites = bg_sprites;
            self.overlays = overlays;
            self.dirty = false;
            self.last_sw = sw;
            self.last_sh = sh;
        }

        // 1. Draw book background texture first.
        if let Some(gl) = &mut self.gl {
            gl.begin_frame(ctx, sw, sh);
            gl.draw_book_bg(ctx, bx, by, bw, bh);
        }

        // 1b. Background sprites (nameplate banner, separator lines) from book texture.
        if let Some(gl) = &mut self.gl {
            for spr in &self.bg_sprites {
                gl.draw_book_sprite(ctx, spr);
            }
        }

        // 2. yog-ui: text, buttons, icons (transparent BG — texture is already there).
        if let Some(ui) = &mut self.ui {
            if ui.needs_layout {
                ui.layout(sw, sh);
            }
            ui.render(ctx);
        }

        // 3. Custom GL overlays (SVG icons, custom font text) — raw GL first.
        if let Some(gl) = &mut self.gl {
            gl.begin_frame(ctx, sw, sh);
            for ov in self.overlays.clone() {
                match ov {
                    OverlayCmd::Svg { data, x, y, w, h } => gl.draw_svg(ctx, &data, x, y, w, h),
                    OverlayCmd::Text {
                        text,
                        font,
                        x,
                        y,
                        color,
                    } => {
                        if let Some(ttf) = self.fonts.get(&font.font_id) {
                            gl.draw_text_custom(ctx, ttf, font.size_px, &text, x, y, color);
                        }
                    }
                    OverlayCmd::McText { .. }
                    | OverlayCmd::McItem { .. }
                    | OverlayCmd::McTexture { .. } => {}
                }
            }
        }

        // 4. MC-pipeline overlays (item stacks, MC-font text) — after all raw
        //    GL, because item rendering resyncs and mutates MC's GL state.
        for ov in self.overlays.clone() {
            match ov {
                OverlayCmd::McItem {
                    item_id,
                    x,
                    y,
                    w,
                    h,
                } => {
                    ctx.draw2d()
                        .item(&normalize_item_id(&item_id), x, y, w.min(h));
                }
                OverlayCmd::McText { text, x, y, color } => {
                    ctx.draw2d().text(&text, x, y, color, false);
                }
                OverlayCmd::McTexture { path, x, y, w, h } => {
                    // Standalone 16×16 icon texture, not an atlas region —
                    // blit it whole (u0=v0=0, tw/th = the icon's own size).
                    ctx.draw2d().mc_texture(&path, x, y, 0.0, 0.0, w, h, w, h);
                }
                _ => {}
            }
        }
    }
}

// ── UI builder ────────────────────────────────────────────────────────────────

/// Book label: MC font, no drop-shadow (Patchouli draws book text shadowless).
fn lbl(text: impl Into<String>) -> widget::Widget {
    widget::label(text).shadow(false)
}
/// Book button: no drop-shadow.
fn btn(text: impl Into<String>) -> widget::Widget {
    widget::button(text).shadow(false)
}

/// Build the yog-ui widget tree + bg sprites + overlay commands for the current book state.
/// `bx/by/bw/bh` are the screen-space book rect (same values used to blit the bg texture).
fn build_ui(
    book: &Book,
    state: &BookViewState,
    theme: &BookTheme,
    recipes: &HashMap<String, RecipeVis>,
    sw: f32,
    sh: f32,
    bx: f32,
    by: f32,
    bw: f32,
    bh: f32,
) -> (UiRoot, Vec<BgSprite>, Vec<OverlayCmd>) {
    let _ = (sw, sh);
    let mut bg_sprites: Vec<BgSprite> = Vec::new();
    let mut overlays: Vec<OverlayCmd> = Vec::new();

    // Scale from Patchouli's fixed BK_W×BK_H coordinate space → actual bw×bh.
    let sx = bw / BK_W;
    let sy = bh / BK_H;

    // In book-local: left page starts at x=LEFT_X, right at RIGHT_X, both y=TOP_PAD.
    let pw = PAGE_W * sx;
    let ph = PAGE_H * sy;
    let spine_gap = (RIGHT_X - LEFT_X - PAGE_W) * sx; // ≈10px scaled

    // Content x/y in screen space (for SVG overlay positioning).
    let lx = bx + LEFT_X * sx;
    let rx = bx + RIGHT_X * sx;
    let py = by + TOP_PAD * sy;

    // Separator x offset within a page: centered = PAGE_W/2 - SEP_W/2 = 58-55 = 3.
    let sep_cx = (PAGE_W / 2.0 - SEP_W / 2.0) * sx; // = 3*sx

    if state.at_home {
        // Nameplate banner: book-local (-8, 12) from LEFT_PAGE_X, size 140×31.
        bg_sprites.push(BgSprite {
            sheet: SpriteSheet::Book,
            u: 0.0,
            v: 180.0,
            uw: 140.0,
            uh: 31.0,
            x: bx + (LEFT_X - 8.0) * sx,
            y: by + 12.0 * sy,
            w: 140.0 * sx,
            h: 31.0 * sy,
        });
        // Separator below "Categories" header on right page (y=12 page-local), centered.
        bg_sprites.push(BgSprite {
            sheet: SpriteSheet::Book,
            u: SEP_U,
            v: SEP_V,
            uw: SEP_W,
            uh: SEP_H,
            x: rx + sep_cx,
            y: by + (TOP_PAD + 12.0) * sy,
            w: SEP_W * sx,
            h: (SEP_H * sy).max(1.0),
        });
    } else {
        // Entry view: separator on right page below category name (y=12 page-local).
        bg_sprites.push(BgSprite {
            sheet: SpriteSheet::Book,
            u: SEP_U,
            v: SEP_V,
            uw: SEP_W,
            uh: SEP_H,
            x: rx + sep_cx,
            y: by + (TOP_PAD + 12.0) * sy,
            w: SEP_W * sx,
            h: (SEP_H * sy).max(1.0),
        });
    }

    let (left_page, right_page) = if state.at_home {
        let l = build_landing_left(book, theme, pw, ph, lx, py, sx, sy, bx, by, &mut overlays);
        let r = build_categories_right(book, state, theme, pw, ph, rx, py, sx, sy, &mut overlays);
        (l, r)
    } else {
        let l = build_entry_left(
            book,
            state,
            theme,
            recipes,
            pw,
            ph,
            lx,
            py,
            sx,
            sy,
            sep_cx,
            &mut bg_sprites,
            &mut overlays,
        );
        let r = build_entries_right(book, state, theme, pw, ph, rx, py, sx, sy, &mut overlays);
        (l, r)
    };

    // Root row spans the full book width.
    // padding-left = LEFT_X*sx offsets both pages from the left cover edge.
    // padding-top  = TOP_PAD*sy offsets from the header banner.
    // spine_gap spacer sits between the two pages.
    // gap must be 0: analytic overlay positions assume pages sit exactly at
    // LEFT_X / RIGHT_X — any flex gap would shift widgets (and their focus
    // highlights) away from the absolutely-positioned icons.
    let root_widget = widget::panel(FlexDir::Row)
        .w(bw)
        .h(bh)
        .gap(0.0)
        .padding(TOP_PAD * sy, 0.0, 0.0, LEFT_X * sx)
        .child(left_page)
        .child(widget::spacer().w(spine_gap)) // spine gap
        .child(right_page);

    // Outer shell: positions the book on screen via a full-screen transparent Row.
    // We use a Column + Row arrangement to apply top/left offsets without margin support.
    let outer = widget::panel(FlexDir::Column)
        .w(bw)
        .h(bh)
        .child(root_widget);

    // Screen-level container: positions book at (bx, by).
    // bx offset via padding, by offset via padding-top.
    // Note: layout computes from (0,0), so we pass bx/by into the UiRoot manually
    // by wrapping in a full-screen panel with the right padding.
    let screen_root = widget::panel(FlexDir::Row)
        .padding(by, 0.0, 0.0, bx)
        .child(outer);

    let ui = UiRoot::new(&book.id, screen_root);
    (ui, bg_sprites, overlays)
}

// ── Left page: landing (home view) ───────────────────────────────────────────

fn build_landing_left(
    book: &Book,
    theme: &BookTheme,
    page_w: f32,
    page_h: f32,
    _lx: f32,
    _py: f32,
    sx: f32,
    sy: f32,
    bx: f32,
    by: f32,
    overlays: &mut Vec<OverlayCmd>,
) -> widget::Widget {
    // Patchouli: title at book-local (13, 16), subtitle at (24, 24).
    // These overlap the nameplate sprite which is above/within the page top.
    overlays.push(OverlayCmd::McText {
        text: book.name.clone(),
        x: bx + 13.0 * sx,
        y: by + 16.0 * sy,
        color: theme.nameplate,
    });
    if let Some(author) = &book.author {
        overlays.push(OverlayCmd::McText {
            text: format!("by {}", author),
            x: bx + 24.0 * sx,
            y: by + 24.0 * sy,
            color: theme.nameplate,
        });
    }

    // Widget content: landing text + entry count.
    // Landing text starts at page-local y=25 (= book-local y=43).
    // We push it down with top padding = 25*sy.
    let mut col = widget::panel(FlexDir::Column)
        .w(page_w)
        .h(page_h)
        .padding(25.0 * sy, 6.0, 4.0, 4.0)
        .gap(0.0);

    for para in book.landing_text.split('\n') {
        if para.is_empty() {
            col = col.child(widget::spacer().h(4.0 * sy));
        } else {
            col = col.child(lbl(para).color(theme.text));
        }
    }

    let total = book.entries.len();
    col = col.child(widget::spacer().flex(1.0));
    col = col.child(
        lbl(format!("{} entries", total))
            .color(theme.nav)
            .h(9.0 * sy)
            .align(Align::Center),
    );
    col
}

// ── Right page: category list (home view) ────────────────────────────────────

fn build_categories_right(
    book: &Book,
    state: &BookViewState,
    theme: &BookTheme,
    page_w: f32,
    page_h: f32,
    rx: f32,
    py: f32,
    sx: f32,
    sy: f32,
    overlays: &mut Vec<OverlayCmd>,
) -> widget::Widget {
    // Patchouli right-page layout (landing):
    //   "Categories" header at page-local y=0, centered
    //   Separator at page-local y=12 (drawn as bg sprite, not here)
    //   4-column 24×24 icon grid starting at page-local (10, 25)
    let cell_w = 24.0 * sx;
    let cell_h = 24.0 * sy;
    let grid_pad_left = 10.0 * sx;
    let header_h = 9.0 * sy;

    let mut col = widget::panel(FlexDir::Column)
        .w(page_w)
        .h(page_h)
        .padding(0.0, 4.0, 4.0, 0.0)
        .gap(0.0);

    col = col.child(
        lbl("Categories")
            .color(theme.divider)
            .h(header_h)
            .align(Align::Center)
            .no_wrap(),
    );
    // Gap covering the separator region up to grid start (page-local y=25).
    col = col.child(widget::spacer().h(25.0 * sy - header_h));

    // 4-column icon grid; icons drawn as GL overlays at exact Patchouli positions.
    let cats = &book.categories;
    let icon_s = 16.0 * sx.min(sy);
    let mut row_i = 0usize;
    loop {
        let row_start = row_i * 4;
        if row_start >= cats.len() {
            break;
        }
        let mut row =
            widget::panel(FlexDir::Row)
                .h(cell_h)
                .gap(0.0)
                .padding(0.0, 0.0, 0.0, grid_pad_left);
        for col_i in 0..4 {
            let idx = row_start + col_i;
            if idx >= cats.len() {
                break;
            }
            let cat = &cats[idx];
            let selected = !state.at_home && idx == state.cat;
            let bg = if selected { theme.nav_selected_bg } else { 0 };

            if let Some(icon_id) = &cat.icon {
                push_icon(
                    overlays,
                    icon_id,
                    rx + (10.0 + col_i as f32 * 24.0 + 4.0) * sx,
                    py + (25.0 + row_i as f32 * 24.0 + 4.0) * sy,
                    icon_s,
                    icon_s,
                );
            }
            let cell = widget::panel(FlexDir::Column)
                .w(cell_w)
                .h(cell_h)
                .bg(bg)
                .on_click(format!("cat:{}", idx))
                .id(format!("book_cat_{}", idx));
            row = row.child(cell);
        }
        col = col.child(row);
        row_i += 1;
    }
    col
}

// ── Left page: entry content (entry view) ────────────────────────────────────

fn build_entry_left(
    book: &Book,
    state: &BookViewState,
    theme: &BookTheme,
    recipes: &HashMap<String, RecipeVis>,
    page_w: f32,
    page_h: f32,
    ox: f32,
    oy: f32,
    sx: f32,
    sy: f32,
    sep_cx: f32,
    bg_sprites: &mut Vec<BgSprite>,
    overlays: &mut Vec<OverlayCmd>,
) -> widget::Widget {
    let entry = state.current_entry(book);
    let page = entry.and_then(|e| e.pages.get(state.page));
    let page_count = state.page_count(book);
    let title_text = entry.map(|e| e.name.as_str()).unwrap_or("");

    // Patchouli PageText page 0:
    //   title centered at page-local (PAGE_W/2, 0)
    //   separator at page-local (0, 12)  → centered ≡ sep_cx offset
    //   text body at page-local (0, 22)
    let title_h = 9.0 * sy;
    let sep_h_px = (SEP_H * sy).max(1.0);
    let sep_y = oy + 12.0 * sy;
    let body_oy = oy + 22.0 * sy;

    bg_sprites.push(BgSprite {
        sheet: SpriteSheet::Book,
        u: SEP_U,
        v: SEP_V,
        uw: SEP_W,
        uh: SEP_H,
        x: ox + sep_cx,
        y: sep_y,
        w: SEP_W * sx,
        h: sep_h_px,
    });

    let page_label = format!("{}/{}", state.page + 1, page_count);
    let nav = widget::panel(FlexDir::Row)
        .h(12.0 * sy)
        .gap(4.0)
        .padding(0.0, 2.0, 0.0, 2.0)
        .child(
            btn("◀")
                .w(12.0 * sx)
                .h(12.0 * sy)
                .color(theme.nav)
                .on_click("prev_page")
                .id("prev_page"),
        )
        .child(
            lbl(&page_label)
                .color(theme.nav)
                .flex(1.0)
                .align(Align::Center),
        )
        .child(
            btn("⌂")
                .w(12.0 * sx)
                .h(12.0 * sy)
                .color(theme.nav)
                .on_click("home")
                .id("book_home"),
        )
        .child(
            btn("▶")
                .w(12.0 * sx)
                .h(12.0 * sy)
                .color(theme.nav)
                .on_click("next_page")
                .id("next_page"),
        );

    let page_body = build_page(
        page, state.page, theme, recipes, bg_sprites, overlays, ox, body_oy, sx, sy,
    );

    widget::panel(FlexDir::Column)
        .w(page_w)
        .h(page_h)
        .padding(0.0, 6.0, 4.0, 4.0)
        .gap(0.0)
        .child(
            lbl(title_text)
                .color(theme.title)
                .h(title_h)
                .align(Align::Center)
                .no_wrap(),
        )
        .child(widget::spacer().h((12.0 - 9.0) * sy)) // gap: title end → sep start
        .child(widget::spacer().h(sep_h_px)) // height occupied by sep sprite
        .child(widget::spacer().h((22.0 - 12.0 - SEP_H) * sy)) // gap: sep end → body
        .child(page_body)
        .child(nav)
}

// ── Right page: entry list for selected category (entry view) ─────────────────

fn build_entries_right(
    book: &Book,
    state: &BookViewState,
    theme: &BookTheme,
    page_w: f32,
    page_h: f32,
    rx: f32,
    py: f32,
    sx: f32,
    sy: f32,
    overlays: &mut Vec<OverlayCmd>,
) -> widget::Widget {
    let entries = state.entries_visible(book);
    let cat_name = book
        .categories
        .get(state.cat)
        .map(|c| c.name.as_str())
        .unwrap_or("Entries");
    let spread_count = state.list_spread_count(book);

    // Patchouli GuiBookEntryList layout (right page):
    //   category name at page-local y=0, centered
    //   separator at page-local y=12 (drawn as bg sprite in build_ui)
    //   entries start at page-local y=20, each h=11
    let header_h = 9.0 * sy;
    let row_h = 11.0 * sy;
    let icon_size = 8.0 * sx.min(sy);

    let mut col = widget::panel(FlexDir::Column)
        .w(page_w)
        .h(page_h)
        .padding(0.0, 4.0, 4.0, 0.0)
        .gap(0.0);

    col = col.child(
        lbl(cat_name)
            .color(theme.divider)
            .h(header_h)
            .align(Align::Center)
            .no_wrap(),
    );
    // sep region: y=9..15 (sep h=3), then gap to entries start at y=20
    col = col.child(widget::spacer().h((12.0 - 9.0) * sy)); // gap to sep
    col = col.child(widget::spacer().h((SEP_H * sy).max(1.0))); // sep height
    col = col.child(widget::spacer().h((20.0 - 12.0 - SEP_H) * sy)); // gap to entries

    let abs_start = state.list_spread_start();
    for (i, entry) in entries.iter().enumerate() {
        let abs_i = abs_start + i;
        let selected = abs_i == state.entry;
        let bg = if selected { theme.nav_selected_bg } else { 0 };
        let color = if selected {
            theme.nav_selected
        } else {
            theme.nav
        };

        // Icon at page-local (1, 20 + i*11 + 1), 8×8 (Patchouli renders entry
        // icons at 0.5× scale).
        if let Some(icon_id) = &entry.icon {
            push_icon(
                overlays,
                icon_id,
                rx + 1.0 * sx,
                py + (20.0 + i as f32 * 11.0 + 1.0) * sy,
                icon_size,
                icon_size,
            );
        }

        let mut row = widget::panel(FlexDir::Row)
            .h(row_h)
            .gap(2.0)
            .bg(bg)
            .on_click(format!("entry:{}", abs_i))
            .id(format!("book_entry_{}", abs_i))
            .padding(1.0, 2.0, 1.0, 2.0);

        row = row.child(widget::spacer().w(icon_size + 2.0));
        row = row.child(lbl(&entry.name).color(color).flex(1.0).no_wrap());
        col = col.child(row);
    }

    if spread_count > 1 {
        col = col.child(widget::spacer().flex(1.0));
        let spread_label = format!("{}/{}", state.list_spread + 1, spread_count);
        col = col.child(
            widget::panel(FlexDir::Row)
                .h(12.0 * sy)
                .gap(2.0)
                .child(
                    btn("◀")
                        .w(12.0 * sx)
                        .h(12.0 * sy)
                        .color(theme.nav)
                        .on_click("prev_list")
                        .id("prev_list"),
                )
                .child(
                    lbl(&spread_label)
                        .color(theme.nav)
                        .flex(1.0)
                        .align(Align::Center),
                )
                .child(
                    btn("▶")
                        .w(12.0 * sx)
                        .h(12.0 * sy)
                        .color(theme.nav)
                        .on_click("next_list")
                        .id("next_list"),
                ),
        );
    }
    col
}

/// Draw a Patchouli-style crafting recipe: title, 100×62 grid background
/// (UV 0,0 on the crafting sheet), 3×N ingredient icons, output item.
/// `rx0`/`ry` are body-local recipe origin; `ox`/`oy` the body screen origin.
#[allow(clippy::too_many_arguments)]
fn draw_crafting_grid(
    theme: &BookTheme,
    bg_sprites: &mut Vec<BgSprite>,
    overlays: &mut Vec<OverlayCmd>,
    col: &mut widget::Widget,
    grid: &[Option<String>],
    width: usize,
    output: &str,
    count: u32,
    shapeless: bool,
    ox: f32,
    oy: f32,
    rx0: f32,
    ry: f32,
    sx: f32,
    sy: f32,
) {
    let mut c = std::mem::replace(col, widget::panel(FlexDir::Column));
    c = c.child(
        lbl(pretty_item_name(output))
            .color(theme.title)
            .h(10.0 * sy)
            .align(Align::Center),
    );

    // Grid background: blit at (rx0-2, ry-2) size 100×62, UV(0,0).
    bg_sprites.push(BgSprite {
        sheet: SpriteSheet::Crafting,
        u: 0.0,
        v: 0.0,
        uw: 100.0,
        uh: 62.0,
        x: ox + (rx0 - 2.0) * sx,
        y: oy + (ry - 2.0) * sy,
        w: 100.0 * sx,
        h: 62.0 * sy,
    });
    // Shapeless marker: UV(0,64) 11×11 at (rx0+62, ry+2).
    if shapeless {
        bg_sprites.push(BgSprite {
            sheet: SpriteSheet::Crafting,
            u: 0.0,
            v: 64.0,
            uw: 11.0,
            uh: 11.0,
            x: ox + (rx0 + 62.0) * sx,
            y: oy + (ry + 2.0) * sy,
            w: 11.0 * sx,
            h: 11.0 * sy,
        });
    }

    let icon_s = 16.0 * sx.min(sy);
    let width = width.max(1);
    for (i, slot) in grid.iter().enumerate() {
        if let Some(item) = slot {
            overlays.push(OverlayCmd::McItem {
                item_id: item.clone(),
                x: ox + (rx0 + 3.0 + (i % width) as f32 * 19.0) * sx,
                y: oy + (ry + 3.0 + (i / width) as f32 * 19.0) * sy,
                w: icon_s,
                h: icon_s,
            });
        }
    }
    // Output item at (rx0+79, ry+22); stack count below it when > 1.
    overlays.push(OverlayCmd::McItem {
        item_id: output.to_owned(),
        x: ox + (rx0 + 79.0) * sx,
        y: oy + (ry + 22.0) * sy,
        w: icon_s,
        h: icon_s,
    });
    if count > 1 {
        overlays.push(OverlayCmd::McText {
            text: format!("x{}", count),
            x: ox + (rx0 + 81.0) * sx,
            y: oy + (ry + 40.0) * sy,
            color: theme.title,
        });
    }
    // Patchouli's "toast symbol" under the output: the crafting table.
    overlays.push(OverlayCmd::McItem {
        item_id: "minecraft:crafting_table".into(),
        x: ox + (rx0 + 79.0) * sx,
        y: oy + (ry + 41.0) * sy,
        w: icon_s,
        h: icon_s,
    });

    // Spacer from title end (10) to below the grid (ry+62+4).
    c = c.child(widget::spacer().h((ry + 62.0 + 4.0 - 10.0) * sy));
    *col = c;
}

// ── Page content builder ──────────────────────────────────────────────────────

/// Build the content widget for a single page.
/// `ox/oy` = screen top-left of the page body area.
/// `sx/sy` = book-local-to-screen scale factors.
fn build_page(
    page: Option<&BookPage>,
    page_num: usize,
    theme: &BookTheme,
    recipes: &HashMap<String, RecipeVis>,
    bg_sprites: &mut Vec<BgSprite>,
    overlays: &mut Vec<OverlayCmd>,
    ox: f32,
    oy: f32,
    sx: f32,
    sy: f32,
) -> widget::Widget {
    let mut col = widget::panel(FlexDir::Column).flex(1.0).gap(4.0);

    let Some(page) = page else {
        return col.child(lbl("No entries yet.").color(theme.nav));
    };

    match page {
        BookPage::Text { text, title } => {
            // On non-first pages, show an optional section title + separator.
            if page_num > 0 {
                if let Some(t) = title {
                    let sep_h_px = (SEP_H * sy).max(1.0);
                    let title_h = 9.0 * sy;
                    col = col.child(
                        lbl(t.as_str())
                            .color(theme.title)
                            .h(title_h)
                            .align(Align::Center)
                            .no_wrap(),
                    );
                    // sep at y=12 page-local, centered
                    bg_sprites.push(BgSprite {
                        sheet: SpriteSheet::Book,
                        u: SEP_U,
                        v: SEP_V,
                        uw: SEP_W,
                        uh: SEP_H,
                        x: ox + (PAGE_W / 2.0 - SEP_W / 2.0) * sx,
                        y: oy + 12.0 * sy,
                        w: SEP_W * sx,
                        h: sep_h_px,
                    });
                    col = col.child(widget::spacer().h((12.0 - 9.0) * sy));
                    col = col.child(widget::spacer().h(sep_h_px));
                    col = col.child(widget::spacer().h((22.0 - 12.0 - SEP_H) * sy));
                }
            }
            for para in text.split('\n') {
                col = col.child(lbl(para).color(theme.text));
            }
        }

        BookPage::Spotlight { item, title, text } => {
            // Patchouli PageSpotlight (page-body-local coordinates):
            //   title at y=0 centered, item frame at (PAGE_W/2-33, 10) 66×26
            //   (UV 0,102 on the 128×256 crafting sheet), item at (PAGE_W/2-8, 15),
            //   text at y=40.
            bg_sprites.push(BgSprite {
                sheet: SpriteSheet::Crafting,
                u: 0.0,
                v: 102.0,
                uw: 66.0,
                uh: 26.0,
                x: ox + (PAGE_W / 2.0 - 33.0) * sx,
                y: oy + 10.0 * sy,
                w: 66.0 * sx,
                h: 26.0 * sy,
            });

            let item_name = title
                .as_deref()
                .or(item.name.as_deref())
                .unwrap_or(item.id.as_str());
            col = col.child(
                lbl(item_name)
                    .color(theme.title)
                    .h(10.0 * sy)
                    .align(Align::Center),
            );

            let icon_s = 16.0 * sx.min(sy);
            overlays.push(OverlayCmd::McItem {
                item_id: item.id.clone(),
                x: ox + (PAGE_W / 2.0 - 8.0) * sx,
                y: oy + 15.0 * sy,
                w: icon_s,
                h: icon_s,
            });

            // Spacer from title end (y=10) to text start (y=40).
            col = col.child(widget::spacer().h(30.0 * sy));

            if let Some(t) = text {
                col = col.child(lbl(t.as_str()).color(theme.text));
            }
        }

        BookPage::Crafting { recipe_id, text } => {
            // Patchouli PageCrafting: recipe origin at page-local
            // (PAGE_W/2-49, 4); we place it in body-local coords below a title.
            let rx0 = PAGE_W / 2.0 - 49.0; // = 9
            let ry = 12.0; // body-local recipe y (title occupies 0..10)
            match recipes.get(recipe_id.as_str()) {
                Some(RecipeVis::Shaped {
                    grid,
                    width,
                    output,
                    count,
                }) => {
                    draw_crafting_grid(
                        theme, bg_sprites, overlays, &mut col, grid, *width, output, *count, false,
                        ox, oy, rx0, ry, sx, sy,
                    );
                }
                Some(RecipeVis::Shapeless {
                    ingredients,
                    output,
                    count,
                }) => {
                    let grid: Vec<Option<String>> = ingredients.iter().cloned().map(Some).collect();
                    draw_crafting_grid(
                        theme, bg_sprites, overlays, &mut col, &grid, 3, output, *count, true, ox,
                        oy, rx0, ry, sx, sy,
                    );
                }
                _ => {
                    col = col.child(lbl(format!("[Recipe: {}]", recipe_id)).color(theme.nav));
                }
            }
            if let Some(t) = text {
                col = col.child(lbl(t.as_str()).color(theme.text));
            }
        }

        BookPage::Smelting { recipe_id, text } => {
            // Patchouli PageSmelting: bg UV(11,71) 96×24, input at +4,+4,
            // output at +76,+4.
            let rx0 = PAGE_W / 2.0 - 49.0;
            let ry = 12.0;
            match recipes.get(recipe_id.as_str()) {
                Some(RecipeVis::Smelting { input, output }) => {
                    col = col.child(
                        lbl(pretty_item_name(output))
                            .color(theme.title)
                            .h(10.0 * sy)
                            .align(Align::Center),
                    );
                    bg_sprites.push(BgSprite {
                        sheet: SpriteSheet::Crafting,
                        u: 11.0,
                        v: 71.0,
                        uw: 96.0,
                        uh: 24.0,
                        x: ox + rx0 * sx,
                        y: oy + ry * sy,
                        w: 96.0 * sx,
                        h: 24.0 * sy,
                    });
                    let icon_s = 16.0 * sx.min(sy);
                    overlays.push(OverlayCmd::McItem {
                        item_id: input.clone(),
                        x: ox + (rx0 + 4.0) * sx,
                        y: oy + (ry + 4.0) * sy,
                        w: icon_s,
                        h: icon_s,
                    });
                    // Center: the furnace itself (Patchouli's "toast symbol").
                    overlays.push(OverlayCmd::McItem {
                        item_id: "minecraft:furnace".into(),
                        x: ox + (rx0 + 40.0) * sx,
                        y: oy + (ry + 4.0) * sy,
                        w: icon_s,
                        h: icon_s,
                    });
                    overlays.push(OverlayCmd::McItem {
                        item_id: output.clone(),
                        x: ox + (rx0 + 76.0) * sx,
                        y: oy + (ry + 4.0) * sy,
                        w: icon_s,
                        h: icon_s,
                    });
                    // Spacer from title end (10) to below the furnace strip (ry+24+4).
                    col = col.child(widget::spacer().h((ry + 24.0 + 4.0 - 10.0) * sy));
                }
                _ => {
                    col = col.child(lbl(format!("[Recipe: {}]", recipe_id)).color(theme.nav));
                }
            }
            if let Some(t) = text {
                col = col.child(lbl(t.as_str()).color(theme.text));
            }
        }

        BookPage::Image {
            texture,
            title,
            text,
            ..
        } => {
            if let Some(t) = title {
                col = col.child(lbl(t.as_str()).color(theme.title));
            }
            col = col.child(widget::mc_image(texture, 80.0, 80.0));
            if let Some(t) = text {
                col = col.child(lbl(t.as_str()).color(theme.text));
            }
        }

        BookPage::Svg { data, title, text } => {
            if let Some(t) = title {
                col = col.child(lbl(t.as_str()).color(theme.title));
            }
            overlays.push(OverlayCmd::Svg {
                data: data.clone(),
                x: ox,
                y: oy,
                w: 64.0,
                h: 64.0,
            });
            col = col.child(widget::spacer().h(68.0));
            if let Some(t) = text {
                col = col.child(lbl(t.as_str()).color(theme.text));
            }
        }

        BookPage::CustomText { text, font, color } => {
            overlays.push(OverlayCmd::Text {
                text: text.clone(),
                font: font.clone(),
                x: ox,
                y: oy,
                color: *color,
            });
            col = col.child(widget::spacer().h(font.size_px * 1.5));
        }

        BookPage::Relations { entries, text } => {
            if let Some(t) = text {
                col = col.child(lbl(t.as_str()).color(theme.text));
            }
            col = col.child(lbl("See also:").color(theme.title));
            for e in entries {
                col = col.child(lbl(format!("• {}", e)).color(theme.nav));
            }
        }

        BookPage::Entity {
            entity_type,
            name,
            text,
        } => {
            let display = name.as_deref().unwrap_or(entity_type.as_str());
            col = col.child(lbl(display).color(theme.title));
            if let Some(t) = text {
                col = col.child(lbl(t.as_str()).color(theme.text));
            }
        }

        BookPage::Pattern {
            op_id,
            input,
            output,
            text,
            ..
        } => {
            col = col.child(lbl(op_id.as_str()).color(theme.title));
            col = col.child(lbl(format!("{} → {}", input, output)).color(theme.nav));
            if !text.is_empty() {
                col = col.child(lbl(text.as_str()).color(theme.text));
            }
        }

        BookPage::Empty => {}
    }

    col
}
