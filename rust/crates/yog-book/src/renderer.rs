//! Book renderer — ties together yog-ui layout, yog-gfx GPU pipeline,
//! SVG icon rasterization, and custom font rendering.

use std::collections::HashMap;
use yog_gfx::{GfxContext, gl, core::{DrawMode, DataType, blend}};
use yog_ui::{widget, FlexDir, Align, UiRoot};

use crate::{Book, BookPage};
use crate::state::BookViewState;
use crate::theme::BookTheme;
use crate::font::{BookFontRegistry, FontAtlas};
use crate::svg;

// Patchouli-compatible book texture layout (512×256 sprite sheet).
// UV coordinates are in pixels on a 512×256 sheet.
// The full open-book background occupies 272×180 at UV (0,0).
pub const BOOK_TEX_W: f32   = 512.0;
pub const BOOK_TEX_H: f32   = 256.0;
// Open-book dimensions in texture-space
pub const BK_W: f32         = 272.0;
pub const BK_H: f32         = 180.0;
// Per-page dimensions and offsets (in book-local coords)
pub const PAGE_W: f32       = 116.0;
pub const PAGE_H: f32       = 156.0;
pub const TOP_PAD: f32      = 18.0;   // vertical space above page text area
pub const LEFT_X: f32       = 15.0;   // left page X inside book
pub const RIGHT_X: f32      = 141.0;  // right page X inside book
// Separator strip UV origin on the sprite sheet
pub const SEP_U: f32        = 140.0;
pub const SEP_V: f32        = 180.0;
pub const SEP_W: f32        = 110.0;
pub const SEP_H: f32        = 3.0;

// ── Vertex for the custom 2D shader (pos + uv + color) ───────────────────────

#[repr(C)]
#[derive(Copy, Clone)]
struct Vert { x: f32, y: f32, u: f32, v: f32, r: f32, g: f32, b: f32, a: f32 }

fn rgba_f(c: u32) -> (f32, f32, f32, f32) {
    let a = ((c >> 24) & 0xFF) as f32 / 255.0;
    let r = ((c >> 16) & 0xFF) as f32 / 255.0;
    let g = ((c >>  8) & 0xFF) as f32 / 255.0;
    let b = ( c        & 0xFF) as f32 / 255.0;
    (r, g, b, a)
}

fn quad(verts: &mut Vec<Vert>, x: f32, y: f32, w: f32, h: f32,
        u0: f32, v0: f32, u1: f32, v1: f32, color: u32) {
    let (r, g, b, a) = rgba_f(color);
    let p = [
        Vert { x,        y,        u: u0, v: v0, r, g, b, a },
        Vert { x: x+w,   y,        u: u1, v: v0, r, g, b, a },
        Vert { x,        y: y+h,   u: u0, v: v1, r, g, b, a },
        Vert { x: x+w,   y: y+h,   u: u1, v: v1, r, g, b, a },
    ];
    // two triangles: 0,1,2  1,3,2
    verts.extend_from_slice(&[p[0], p[1], p[2], p[1], p[3], p[2]]);
}

// ── GLSL shaders ─────────────────────────────────────────────────────────────

const VERT: &str = r#"
#version 150 core
in vec2 aPos;
in vec2 aUv;
in vec4 aCol;
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
#version 150 core
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

// ── Embedded book background texture ─────────────────────────────────────────

static BOOK_PNG: &[u8] = include_bytes!("../assets/book_brown.png");

fn decode_book_png() -> Option<(Vec<u8>, u32, u32)> {
    use png::Decoder;
    let decoder = Decoder::new(std::io::Cursor::new(BOOK_PNG));
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).ok()?;
    // Convert to RGBA if needed
    let rgba = match info.color_type {
        png::ColorType::Rgba => buf[..info.buffer_size()].to_vec(),
        png::ColorType::Rgb  => {
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
    prog:     gl::ShaderProgram,
    vao:      gl::VertexArray,
    vbo:      gl::Buffer,
    book_tex: Option<(gl::Texture, u32, u32)>,  // embedded book background
    svg_tex:  HashMap<u64, (gl::Texture, u32, u32)>,
    font_atlas: HashMap<u64, (gl::Texture, FontAtlas)>,
}

impl BookGl {
    fn init(ctx: &GfxContext) -> Option<Self> {
        let prog = ctx.create_shader(VERT, FRAG).ok()?;
        let vbo  = ctx.create_buffer();
        let vao  = ctx.create_vao();

        const STRIDE: u32 = 32; // 8 × f32
        vao.attrib(ctx, &vbo, 0, 2, DataType::F32, false, STRIDE, 0);  // pos
        vao.attrib(ctx, &vbo, 1, 2, DataType::F32, false, STRIDE, 8);  // uv
        vao.attrib(ctx, &vbo, 2, 4, DataType::F32, false, STRIDE, 16); // col

        let book_tex = decode_book_png().map(|(rgba, w, h)| {
            let tex = ctx.create_texture_rgba(w, h, &rgba, true);
            (tex, w, h)
        });

        Some(BookGl { prog, vao, vbo, book_tex, svg_tex: HashMap::new(), font_atlas: HashMap::new() })
    }

    fn svg_tex(&mut self, ctx: &GfxContext, hash: u64, data: &str, w: u32, h: u32)
        -> Option<&(gl::Texture, u32, u32)>
    {
        if !self.svg_tex.contains_key(&hash) {
            let pixels = svg::rasterize(data, w, h)?;
            let tex = ctx.create_texture_rgba(w, h, &pixels, true);
            self.svg_tex.insert(hash, (tex, w, h));
        }
        self.svg_tex.get(&hash)
    }

    fn flush(&self, ctx: &GfxContext, verts: &[Vert]) {
        if verts.is_empty() { return; }
        unsafe { self.vbo.upload(ctx, verts, true); }
        ctx.draw_arrays(&self.vao, &self.prog, DrawMode::Triangles, 0, verts.len() as u32);
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

    /// Blit the open-book background from the embedded book texture.
    fn draw_book_bg(&self, ctx: &GfxContext, bx: f32, by: f32, bw: f32, bh: f32) {
        let Some((tex, tw, th)) = &self.book_tex else { return };
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
        let tw = w as u32; let th = h as u32;
        if let Some(&(tex, _, _)) = self.svg_tex(ctx, hash, data, tw, th) {
            self.prog.uniform_1i(ctx, "uMode", 1);
            ctx.bind_texture(0, &tex);
            let mut v = Vec::with_capacity(6);
            quad(&mut v, x, y, w, h, 0.0, 0.0, 1.0, 1.0, 0xFF_FFFFFF);
            self.flush(ctx, &v);
        }
    }

    fn draw_text_custom(&mut self, ctx: &GfxContext, ttf: &[u8], size_px: f32,
                        text: &str, mut x: f32, y: f32, color: u32) {
        let hash = font_hash(ttf);
        if !self.font_atlas.contains_key(&hash) {
            if let Some(atlas) = FontAtlas::build(ttf, size_px) {
                let tex = ctx.create_texture_rgba(
                    atlas.atlas_size, atlas.atlas_size, &atlas.pixels, true);
                self.font_atlas.insert(hash, (tex, atlas));
            }
        }
        // Get raw pointers so the borrow of self.font_atlas ends before we call
        // self.prog / self.flush, which also borrow fields of self.
        let ptrs = self.font_atlas.get(&hash)
            .map(|(t, a)| (t as *const gl::Texture, a as *const FontAtlas));
        if let Some((tex_ptr, atlas_ptr)) = ptrs {
            // SAFETY: we hold &mut self; font_atlas is not mutated below.
            let tex   = unsafe { &*tex_ptr   };
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
                        quad(&mut verts, gx, gy, g.width as f32, g.height as f32,
                             g.u0, g.v0, g.u1, g.v1, color);
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

// ── Pending overlay draw commands ─────────────────────────────────────────────

#[derive(Clone)]
pub(crate) enum OverlayCmd {
    Svg  { data: String, x: f32, y: f32, w: f32, h: f32 },
    Text { text: String, font: crate::font::BookFont, x: f32, y: f32, color: u32 },
}

// ── BookRenderer ──────────────────────────────────────────────────────────────

pub struct BookRenderer {
    pub book:  Book,
    pub state: BookViewState,
    pub theme: BookTheme,
    gl:         Option<BookGl>,
    pub ui:     Option<UiRoot>,
    overlays:   Vec<OverlayCmd>,
    dirty:   bool,
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
            gl: None,
            ui: None,
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

    pub fn render(&mut self, ctx: &GfxContext, sw: f32, sh: f32, fonts: &BookFontRegistry) {
        // Lazy GL init.
        if self.gl.is_none() {
            self.gl = BookGl::init(ctx);
        }

        // Compute book rect (same formula as build_ui).
        let bw = (sw * 0.72).min(BK_W * 2.5);
        let bh = (sh * 0.80).min(BK_H * 2.5);
        let bx = (sw - bw) / 2.0;
        let by = (sh - bh) / 2.0;

        // Rebuild widget tree if dirty or screen resized.
        if self.dirty || sw != self.last_sw || sh != self.last_sh {
            let (root, overlays) = build_ui(&self.book, &self.state, &self.theme, sw, sh, bx, by, bw, bh);
            self.ui = Some(root);
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

        // 2. yog-ui: text, buttons, icons (transparent BG — texture is already there).
        if let Some(ui) = &mut self.ui {
            if ui.needs_layout { ui.layout(sw, sh); }
            ui.render(ctx);
        }

        // 3. Custom GL overlays (SVG icons, custom font text).
        if let Some(gl) = &mut self.gl {
            gl.begin_frame(ctx, sw, sh);
            for ov in self.overlays.clone() {
                match ov {
                    OverlayCmd::Svg  { data, x, y, w, h } =>
                        gl.draw_svg(ctx, &data, x, y, w, h),
                    OverlayCmd::Text { text, font, x, y, color } => {
                        if let Some(ttf) = fonts.get(&font.font_id) {
                            gl.draw_text_custom(ctx, ttf, font.size_px, &text, x, y, color);
                        }
                    }
                }
            }
        }
    }
}

// ── UI builder ────────────────────────────────────────────────────────────────

/// Build the yog-ui widget tree + overlay commands for the current book state.
/// `bx/by/bw/bh` are the screen-space book rect (same values used to blit the bg texture).
fn build_ui(book: &Book, state: &BookViewState, theme: &BookTheme,
             sw: f32, sh: f32,
             bx: f32, by: f32, bw: f32, bh: f32) -> (UiRoot, Vec<OverlayCmd>) {
    let _ = (sw, sh);
    let mut overlays: Vec<OverlayCmd> = Vec::new();

    // Scale from Patchouli's fixed BK_W×BK_H coordinate space → actual bw×bh.
    let sx = bw / BK_W;
    let sy = bh / BK_H;

    // In book-local: left page starts at x=LEFT_X, right at RIGHT_X, both y=TOP_PAD.
    let pw = PAGE_W * sx;
    let ph = PAGE_H * sy;
    let spine_gap = (RIGHT_X - LEFT_X - PAGE_W) * sx; // ≈10px scaled

    // Content x/y in screen space (for SVG overlay positioning).
    let lx = bx + LEFT_X  * sx;
    let rx = bx + RIGHT_X * sx;
    let py = by + TOP_PAD  * sy;

    let (left_page, right_page) = if state.at_home {
        let l = build_landing_left(book, theme, pw, ph, lx, py);
        let r = build_categories_right(book, state, theme, pw, ph, rx, py, &mut overlays);
        (l, r)
    } else {
        let l = build_entry_left(book, state, theme, pw, ph, lx, py, &mut overlays);
        let r = build_entries_right(book, state, theme, pw, ph, rx, py);
        (l, r)
    };

    // Root row spans the full book width.
    // padding-left = LEFT_X*sx offsets both pages from the left cover edge.
    // padding-top  = TOP_PAD*sy offsets from the header banner.
    // spine_gap spacer sits between the two pages.
    let root_widget = widget::panel(FlexDir::Row)
        .w(bw).h(bh)
        .padding(TOP_PAD * sy, 0.0, 0.0, LEFT_X * sx)
        .child(left_page)
        .child(widget::spacer().w(spine_gap))  // spine gap
        .child(right_page);

    // Outer shell: positions the book on screen via a full-screen transparent Row.
    // We use a Column + Row arrangement to apply top/left offsets without margin support.
    let outer = widget::panel(FlexDir::Column)
        .w(bw).h(bh)
        .child(root_widget);

    // Screen-level container: positions book at (bx, by).
    // bx offset via padding, by offset via padding-top.
    // Note: layout computes from (0,0), so we pass bx/by into the UiRoot manually
    // by wrapping in a full-screen panel with the right padding.
    let screen_root = widget::panel(FlexDir::Row)
        .padding(by, 0.0, 0.0, bx)
        .child(outer);

    let ui = UiRoot::new(&book.id, screen_root);
    (ui, overlays)
}

// ── Left page: landing (home view) ───────────────────────────────────────────

fn build_landing_left(book: &Book, theme: &BookTheme,
                       page_w: f32, page_h: f32, _lx: f32, _py: f32)
    -> widget::Widget
{
    let mut col = widget::panel(FlexDir::Column)
        .w(page_w).h(page_h)
        .padding(4.0, 6.0, 4.0, 4.0)
        .gap(4.0);

    // Landing text
    for para in book.landing_text.split('\n') {
        col = col.child(widget::label(para).color(theme.text));
    }
    col
}

// ── Right page: category list (home view) ────────────────────────────────────

fn build_categories_right(
    book: &Book, state: &BookViewState, theme: &BookTheme,
    page_w: f32, page_h: f32,
    _rx: f32, _py: f32,
    overlays: &mut Vec<OverlayCmd>,
) -> widget::Widget {
    let _ = overlays;
    let mut col = widget::panel(FlexDir::Column)
        .w(page_w).h(page_h)
        .padding(4.0, 6.0, 4.0, 4.0)
        .gap(1.0);

    col = col.child(widget::label("Categories").color(theme.divider).h(11.0));
    col = col.child(widget::spacer().h(2.0));

    for (i, cat) in book.categories.iter().enumerate() {
        let selected = !state.at_home && i == state.cat;
        let bg    = if selected { theme.nav_selected_bg } else { 0 };
        let color = if selected { theme.nav_selected } else { theme.nav };

        let mut row = widget::panel(FlexDir::Row)
            .h(20.0).gap(4.0).bg(bg)
            .on_click(format!("cat:{}", i))
            .id(format!("book_cat_{}", i))
            .padding(2.0, 2.0, 2.0, 2.0);

        if let Some(icon_id) = &cat.icon {
            row = row.child(item_icon_widget(icon_id));
        } else {
            row = row.child(widget::spacer().w(16.0));
        }
        row = row.child(widget::label(&cat.name).color(color).flex(1.0));
        col = col.child(row);
    }
    col
}

// ── Left page: entry content (entry view) ────────────────────────────────────

fn build_entry_left(
    book: &Book, state: &BookViewState, theme: &BookTheme,
    page_w: f32, page_h: f32,
    ox: f32, oy: f32,
    overlays: &mut Vec<OverlayCmd>,
) -> widget::Widget {
    let entry      = state.current_entry(book);
    let page       = entry.and_then(|e| e.pages.get(state.page));
    let page_count = state.page_count(book);
    let title_text = entry.map(|e| e.name.as_str()).unwrap_or("");

    let page_label = format!("{}/{}", state.page + 1, page_count);
    let nav = widget::panel(FlexDir::Row)
        .h(16.0).gap(4.0)
        .padding(0.0, 2.0, 0.0, 2.0)
        .child(widget::button("◀").w(16.0).h(12.0).color(theme.nav)
            .on_click("prev_page").id("prev_page"))
        .child(widget::label(&page_label).color(theme.nav).flex(1.0).align(Align::Center))
        .child(widget::button("⌂").w(16.0).h(12.0).color(theme.nav)
            .on_click("home").id("book_home"))
        .child(widget::button("▶").w(16.0).h(12.0).color(theme.nav)
            .on_click("next_page").id("next_page"));

    let page_body = build_page(page, theme, overlays, ox, oy);

    widget::panel(FlexDir::Column)
        .w(page_w).h(page_h)
        .padding(4.0, 6.0, 4.0, 4.0)
        .gap(3.0)
        .child(widget::label(title_text).color(theme.title).h(14.0))
        .child(widget::spacer().h(1.0).bg(theme.divider))
        .child(page_body)
        .child(nav)
}

// ── Right page: entry list for selected category (entry view) ─────────────────

fn build_entries_right(
    book: &Book, state: &BookViewState, theme: &BookTheme,
    page_w: f32, page_h: f32, _rx: f32, _py: f32,
) -> widget::Widget {
    let entries  = state.entries_in_cat(book);
    let cat_name = book.categories.get(state.cat).map(|c| c.name.as_str()).unwrap_or("Entries");

    let mut col = widget::panel(FlexDir::Column)
        .w(page_w).h(page_h)
        .padding(4.0, 6.0, 4.0, 4.0)
        .gap(1.0);

    col = col.child(widget::label(cat_name).color(theme.divider).h(11.0));
    col = col.child(widget::spacer().h(2.0));

    for (i, entry) in entries.iter().enumerate() {
        let selected = i == state.entry;
        let bg    = if selected { theme.nav_selected_bg } else { 0 };
        let color = if selected { theme.nav_selected } else { theme.nav };

        let mut row = widget::panel(FlexDir::Row)
            .h(16.0).gap(4.0).bg(bg)
            .on_click(format!("entry:{}", i))
            .id(format!("book_entry_{}", i))
            .padding(1.0, 2.0, 1.0, 2.0);

        if let Some(icon_id) = &entry.icon {
            row = row.child(item_icon_widget(icon_id));
        } else {
            row = row.child(widget::spacer().w(16.0));
        }
        row = row.child(widget::label(&entry.name).color(color).flex(1.0));
        col = col.child(row);
    }
    col
}

/// Bare 16×16 item icon widget from an item ID ("ns:item_name" or "ns:item/name").
fn item_icon_widget(item_id: &str) -> widget::Widget {
    // Normalize "ns:item/name" or "ns:block/name" → "ns:textures/item/name.png"
    // Normalize "ns:name" → "ns:textures/item/name.png"
    let tex = if let Some((ns, path)) = item_id.split_once(':') {
        if path.starts_with("item/") || path.starts_with("block/") {
            format!("{ns}:textures/{path}.png")
        } else {
            format!("{ns}:textures/item/{path}.png")
        }
    } else {
        format!("minecraft:textures/item/{item_id}.png")
    };
    widget::mc_image(&tex, 16.0, 16.0)
}

// ── Page content builder ──────────────────────────────────────────────────────

/// Build the content widget for a single page.
fn build_page(
    page: Option<&BookPage>,
    theme: &BookTheme,
    overlays: &mut Vec<OverlayCmd>,
    _ox: f32,
    _oy: f32,
) -> widget::Widget {
    let mut col = widget::panel(FlexDir::Column).flex(1.0).gap(4.0);

    let Some(page) = page else {
        return col.child(widget::label("No entries yet.").color(theme.nav));
    };

    match page {
        BookPage::Text { text } => {
            for para in text.split('\n') {
                col = col.child(widget::label(para).color(theme.text));
            }
        }

        BookPage::Spotlight { item, title, text } => {
            // Icon row: bare item icon + item name side by side
            let item_name = title.as_deref()
                .or(item.name.as_deref())
                .unwrap_or(item.id.as_str());
            let row = widget::panel(FlexDir::Row).h(24.0).gap(6.0)
                .child(item_icon_widget(&item.id))
                .child(widget::label(item_name).color(theme.title).flex(1.0));
            col = col.child(row);
            if let Some(t) = text {
                col = col.child(widget::label(t).color(theme.text));
            }
        }

        BookPage::Crafting { recipe_id, text } => {
            col = col.child(
                widget::label(format!("[Crafting: {}]", recipe_id)).color(theme.nav)
            );
            if let Some(t) = text {
                col = col.child(widget::label(t).color(theme.text));
            }
        }

        BookPage::Smelting { recipe_id, text } => {
            col = col.child(
                widget::label(format!("[Smelting: {}]", recipe_id)).color(theme.nav)
            );
            if let Some(t) = text {
                col = col.child(widget::label(t).color(theme.text));
            }
        }

        BookPage::Image { texture, title, text, .. } => {
            if let Some(t) = title {
                col = col.child(widget::label(t).color(theme.title));
            }
            col = col.child(widget::mc_image(texture, 80.0, 80.0));
            if let Some(t) = text {
                col = col.child(widget::label(t).color(theme.text));
            }
        }

        BookPage::Svg { data, title, text } => {
            if let Some(t) = title {
                col = col.child(widget::label(t).color(theme.title));
            }
            overlays.push(OverlayCmd::Svg { data: data.clone(), x: _ox, y: _oy, w: 64.0, h: 64.0 });
            col = col.child(widget::spacer().h(68.0));
            if let Some(t) = text {
                col = col.child(widget::label(t).color(theme.text));
            }
        }

        BookPage::CustomText { text, font, color } => {
            overlays.push(OverlayCmd::Text {
                text: text.clone(), font: font.clone(), x: _ox, y: _oy, color: *color,
            });
            col = col.child(widget::spacer().h(font.size_px * 1.5));
        }

        BookPage::Relations { entries, text } => {
            if let Some(t) = text {
                col = col.child(widget::label(t).color(theme.text));
            }
            col = col.child(widget::label("See also:").color(theme.title));
            for e in entries {
                col = col.child(widget::label(format!("• {}", e)).color(theme.nav));
            }
        }

        BookPage::Entity { entity_type, name, text } => {
            let display = name.as_deref().unwrap_or(entity_type.as_str());
            col = col.child(widget::label(display).color(theme.title));
            if let Some(t) = text {
                col = col.child(widget::label(t).color(theme.text));
            }
        }

        BookPage::Pattern { op_id, input, output, text, .. } => {
            col = col.child(widget::label(op_id).color(theme.title));
            col = col.child(
                widget::label(format!("{} → {}", input, output)).color(theme.nav)
            );
            if !text.is_empty() {
                col = col.child(widget::label(text).color(theme.text));
            }
        }

        BookPage::Empty => {}
    }

    col
}
