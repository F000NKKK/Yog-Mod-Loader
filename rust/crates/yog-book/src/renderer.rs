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

// ── GL resource cache ─────────────────────────────────────────────────────────

struct BookGl {
    prog: gl::ShaderProgram,
    vao:  gl::VertexArray,
    vbo:  gl::Buffer,
    // SVG texture cache: hash → (Texture, pixel_w, pixel_h)
    svg_tex: HashMap<u64, (gl::Texture, u32, u32)>,
    // Font atlas cache: hash → (Texture, FontAtlas)
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

        Some(BookGl { prog, vao, vbo, svg_tex: HashMap::new(), font_atlas: HashMap::new() })
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
    dirty:     bool,
    last_sw:   f32,
    last_sh:   f32,
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

        // Rebuild widget tree if dirty or screen resized.
        if self.dirty || sw != self.last_sw || sh != self.last_sh {
            let (root, overlays) = build_ui(&self.book, &self.state, &self.theme, sw, sh);
            self.ui = Some(root);
            self.overlays = overlays;
            self.dirty = false;
            self.last_sw = sw;
            self.last_sh = sh;
        }

        // Layout + yog-ui render (backgrounds, text, buttons).
        if let Some(ui) = &mut self.ui {
            if ui.needs_layout { ui.layout(sw, sh); }
            ui.render(ctx);
        }

        // Custom GL overlays (SVG icons, custom font text).
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
fn build_ui(book: &Book, state: &BookViewState, theme: &BookTheme,
             sw: f32, sh: f32) -> (UiRoot, Vec<OverlayCmd>) {
    let mut overlays: Vec<OverlayCmd> = Vec::new();

    // Overall book proportions (centered, fixed size).
    let bw = (sw * 0.75).min(600.0);
    let bh = (sh * 0.80).min(400.0);
    let bx = (sw - bw) / 2.0;
    let by = (sh - bh) / 2.0;
    let sidebar_w = 130.0;
    let page_w    = bw - sidebar_w - 6.0;

    // ── Sidebar: categories ──────────────────────────────────────────────────
    let mut cats_col = widget::panel(FlexDir::Column).gap(1.0).padding(4.0, 4.0, 4.0, 4.0);
    for (i, cat) in book.categories.iter().enumerate() {
        let selected = i == state.cat;
        let color = if selected { theme.nav_selected } else { theme.nav };
        let bg    = if selected { 0x30_FFFFFF } else { 0 };
        let btn = widget::button(&cat.name)
            .color(color).bg(bg).h(16.0)
            .on_click(format!("cat:{}", i));

        // Schedule SVG icon overlay if category has one.
        if let Some(svg_data) = &cat.icon_svg {
            // Icon is drawn at indent position; we'll track it by cat index.
            // For layout, just add a small spacer to reserve horizontal space.
            overlays.push(OverlayCmd::Svg {
                data: svg_data.clone(),
                x: bx + 4.0,
                y: by + 20.0 + i as f32 * 18.0,
                w: 14.0, h: 14.0,
            });
        }
        cats_col = cats_col.child(btn);
    }

    // ── Sidebar: entry list ──────────────────────────────────────────────────
    let entries = state.entries_in_cat(book);
    let mut entries_col = widget::panel(FlexDir::Column).gap(1.0).padding(0.0, 4.0, 4.0, 4.0);
    entries_col = entries_col.child(
        widget::label("─────────").color(theme.divider).h(8.0)
    );
    for (i, entry) in entries.iter().enumerate() {
        let selected = i == state.entry;
        let color = if selected { theme.nav_selected } else { theme.nav };
        let bg    = if selected { 0x30_FFFFFF } else { 0 };
        let label = if entry.name.len() > 16 { &entry.name[..16] } else { &entry.name };
        let btn = widget::button(label)
            .color(color).bg(bg).h(14.0)
            .on_click(format!("entry:{}", i));
        entries_col = entries_col.child(btn);
    }

    let sidebar = widget::panel(FlexDir::Column)
        .w(sidebar_w).h(bh)
        .bg(theme.sidebar_bg)
        .child(
            widget::label(&book.name)
                .color(theme.nameplate).h(18.0)
                .padding(3.0, 4.0, 3.0, 4.0)
        )
        .child(cats_col)
        .child(entries_col);

    // ── Page area ────────────────────────────────────────────────────────────
    let entry = state.current_entry(book);
    let page  = entry.and_then(|e| e.pages.get(state.page));
    let page_count = state.page_count(book);

    let title_text = entry.map(|e| e.name.as_str()).unwrap_or("");
    let title_widget = widget::label(title_text)
        .color(theme.title).h(16.0)
        .padding(4.0, 4.0, 2.0, 6.0);

    let page_body = build_page(page, theme, &mut overlays,
                               bx + sidebar_w + 6.0, by + 32.0);

    // ── Nav buttons ──────────────────────────────────────────────────────────
    let page_label = format!("{}/{}", state.page + 1, page_count);
    let nav = widget::panel(FlexDir::Row).h(20.0).gap(4.0)
        .padding(2.0, 6.0, 2.0, 6.0)
        .child(widget::button("◀").w(22.0).h(16.0).color(theme.nav).on_click("prev_page"))
        .child(widget::label(&page_label).color(theme.nav).flex(1.0).align(Align::Center))
        .child(widget::button("▶").w(22.0).h(16.0).color(theme.nav).on_click("next_page"));

    let page_panel = widget::panel(FlexDir::Column)
        .w(page_w).h(bh)
        .bg(theme.page_bg)
        .child(title_widget)
        .child(
            widget::label("").h(1.0).bg(theme.border)  // divider
        )
        .child(page_body)
        .child(nav);

    // ── Root: outer book frame ───────────────────────────────────────────────
    let root_widget = widget::panel(FlexDir::Row)
        .w(bw).h(bh).gap(2.0)
        .bg(theme.bg)
        .padding(3.0, 3.0, 3.0, 3.0)
        .margin(by, 0.0, 0.0, bx)
        .child(sidebar)
        .child(page_panel);

    let ui = UiRoot::new(&book.id, root_widget);
    (ui, overlays)
}

/// Build the content widget for a single page. Appends overlay commands for
/// SVG images and custom-font text.
fn build_page(
    page: Option<&BookPage>,
    theme: &BookTheme,
    overlays: &mut Vec<OverlayCmd>,
    _content_x: f32,
    _content_y: f32,
) -> widget::Widget {
    let mut col = widget::panel(FlexDir::Column).flex(1.0).gap(4.0)
        .padding(6.0, 8.0, 6.0, 8.0);

    let Some(page) = page else {
        return col.child(
            widget::label("No entries yet.").color(theme.nav)
        );
    };

    match page {
        BookPage::Text { text } => {
            // Wrap long text into paragraphs at line breaks.
            for para in text.split('\n') {
                col = col.child(widget::label(para).color(theme.text));
            }
        }

        BookPage::Spotlight { item, title, text } => {
            if let Some(t) = title {
                col = col.child(widget::label(t).color(theme.title));
            }
            col = col.child(widget::item_slot(&item.id));
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
            // Use MC texture blitter via draw2d_mc_tex — rendered by yog-ui render pass.
            col = col.child(
                widget::mc_image(texture, 80.0, 80.0)
            );
            if let Some(t) = text {
                col = col.child(widget::label(t).color(theme.text));
            }
        }

        BookPage::Svg { data, title, text } => {
            if let Some(t) = title {
                col = col.child(widget::label(t).color(theme.title));
            }
            // Reserve space via a spacer; SVG drawn as overlay.
            // Overlay position is approximate — refined at render time.
            overlays.push(OverlayCmd::Svg {
                data:  data.clone(),
                x:     _content_x,
                y:     _content_y,
                w:     64.0, h: 64.0,
            });
            col = col.child(widget::spacer().h(68.0));
            if let Some(t) = text {
                col = col.child(widget::label(t).color(theme.text));
            }
        }

        BookPage::CustomText { text, font, color } => {
            overlays.push(OverlayCmd::Text {
                text:  text.clone(),
                font:  font.clone(),
                x:     _content_x,
                y:     _content_y,
                color: *color,
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
            if let Some(n) = name {
                col = col.child(widget::label(n).color(theme.title));
            } else {
                col = col.child(widget::label(entity_type).color(theme.title));
            }
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
