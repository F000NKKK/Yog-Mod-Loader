//! Client-side rendering: FPS counter + custom book UI + world renderer.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use yog_api::{
    GfxContext,
    gfx_core::{DataType, DrawMode, blend},
    gfx_gl::{Buffer, ShaderProgram, VertexArray},
    Book, BookCategory, BookEntry, BookPage,
    Registry,
};
use yog_api::ui::{UiRoot, LayoutNode, widget, Align, FlexDir};

// ── GLSL (world renderer) ────────────────────────────────────────────────────

const VERT: &str = r#"#version 150 core
in vec3 aPos;
uniform mat4 uViewProj;
uniform vec3 uOffset;
uniform float uRotY;
void main() {
    float s = sin(uRotY); float c = cos(uRotY);
    vec3 p = vec3(aPos.x*c - aPos.z*s, aPos.y, aPos.x*s + aPos.z*c);
    gl_Position = uViewProj * vec4(p + uOffset, 1.0);
}"#;

const FRAG: &str = r#"#version 150 core
out vec4 fragColor;
uniform vec4 uColor;
void main() { fragColor = uColor; }"#;

#[rustfmt::skip]
const PLUMBOB: &[f32] = {
    const T: f32 = 0.70; const B: f32 = -0.35; const H: f32 = 0.35;
    &[
        0.0,T,0.0, -H,0.0,H,  H,0.0,H,   0.0,T,0.0,  H,0.0,H,  H,0.0,-H,
        0.0,T,0.0,  H,0.0,-H, -H,0.0,-H,  0.0,T,0.0, -H,0.0,-H, -H,0.0,H,
        0.0,B,0.0,  H,0.0,H,  -H,0.0,H,   0.0,B,0.0,  H,0.0,-H,  H,0.0,H,
        0.0,B,0.0, -H,0.0,-H,  H,0.0,-H,  0.0,B,0.0, -H,0.0,H,  -H,0.0,-H,
    ]
};

const SPRING_K: f32 = 200.0;
const SPRING_D: f32 = 28.0;
const ROT_SPEED: f32 = 1.2;

// ── WorldRenderer ─────────────────────────────────────────────────────────────

struct WorldRenderer {
    prog: Option<ShaderProgram>, quad_vbo: Option<Buffer>, quad_vao: Option<VertexArray>,
    plumb_vbo: Option<Buffer>, plumb_vao: Option<VertexArray>,
    start: Option<Instant>, last: Option<Instant>,
    plumb_pos: Option<[f32; 3]>, plumb_vel: [f32; 3],
}

impl WorldRenderer {
    const fn new() -> Self { Self {
        prog: None, quad_vbo: None, quad_vao: None,
        plumb_vbo: None, plumb_vao: None,
        start: None, last: None, plumb_pos: None, plumb_vel: [0.0; 3],
    }}

    fn init(&mut self, ctx: &GfxContext) {
        let prog = match ctx.create_shader(VERT, FRAG) { Ok(p) => p, Err(()) => return };
        let quad: &[f32] = &[-0.5,0.0,0.5, 0.5,0.0,0.5, 0.5,0.0,-0.5, -0.5,0.0,0.5, 0.5,0.0,-0.5, -0.5,0.0,-0.5];
        let qv = ctx.create_buffer(); unsafe { qv.upload(ctx, quad, false) };
        let qa = ctx.create_vao(); qa.attrib(ctx, &qv, 0, 3, DataType::F32, false, 12, 0);
        let pv = ctx.create_buffer(); unsafe { pv.upload(ctx, PLUMBOB, false) };
        let pa = ctx.create_vao(); pa.attrib(ctx, &pv, 0, 3, DataType::F32, false, 12, 0);
        let now = Instant::now();
        self.prog = Some(prog); self.quad_vbo = Some(qv); self.quad_vao = Some(qa);
        self.plumb_vbo = Some(pv); self.plumb_vao = Some(pa);
        self.start = Some(now); self.last = Some(now);
    }

    fn render(&mut self, ctx: &GfxContext) {
        if self.prog.is_none() { self.init(ctx); }
        let Some(prog) = self.prog.as_ref() else { return };
        let now = Instant::now();
        let dt = self.last.map_or(0.0, |t| t.elapsed().as_secs_f32().min(0.1));
        let t = self.start.map_or(0.0, |s| s.elapsed().as_secs_f32());
        self.last = Some(now);
        let cam = ctx.camera_pos(); let p = ctx.player_pos();
        let target = [p[0], p[1] + 1.8, p[2]];
        let pos = self.plumb_pos.get_or_insert(target);
        for i in 0..3 {
            let force = (target[i] - pos[i]) * SPRING_K - self.plumb_vel[i] * SPRING_D;
            self.plumb_vel[i] += force * dt; pos[i] += self.plumb_vel[i] * dt;
        }
        let pw = *pos;
        let off = [pw[0]-cam[0], pw[1]-cam[1], pw[2]-cam[2]-0.25];
        let rot = (t * ROT_SPEED) % std::f32::consts::TAU;
        let vp = ctx.view_proj();
        ctx.set_depth(true, false);
        ctx.set_blend(true, blend::SRC_ALPHA, blend::ONE_MINUS_SRC_ALPHA);
        prog.uniform_mat4(ctx, "uViewProj", &vp);
        if let Some(vao) = self.quad_vao.as_ref() {
            prog.uniform_1f(ctx, "uRotY", 0.0);
            prog.uniform_3f(ctx, "uOffset", 0.0-cam[0], 65.0-cam[1], 0.0-cam[2]-0.25);
            prog.uniform_4f(ctx, "uColor", 1.0, 0.2, 0.2, 0.7);
            ctx.draw_arrays(vao, prog, DrawMode::Triangles, 0, 6);
        }
        if let Some(vao) = self.plumb_vao.as_ref() {
            prog.uniform_1f(ctx, "uRotY", rot);
            prog.uniform_3f(ctx, "uOffset", off[0], off[1], off[2]);
            prog.uniform_4f(ctx, "uColor", 0.1, 0.9, 0.2, 0.92);
            ctx.draw_arrays(vao, prog, DrawMode::Triangles, 0, (PLUMBOB.len()/3) as u32);
        }
        ctx.set_blend(false, 0, 0); ctx.set_depth(false, false);
    }
}

// ── FrameTimer ────────────────────────────────────────────────────────────────

struct FrameTimer { buf: [f32; 500], sum: f64, idx: usize, filled: bool, last: Option<Instant> }
impl FrameTimer {
    const fn new() -> Self { Self { buf: [0.0; 500], sum: 0.0, idx: 0, filled: false, last: None } }
    fn tick(&mut self) -> f32 {
        let now = Instant::now();
        let dt = self.last.map_or(0.016, |t| t.elapsed().as_secs_f32().min(0.5));
        self.last = Some(now);
        self.sum -= self.buf[self.idx] as f64;
        self.buf[self.idx] = dt;
        self.sum += dt as f64;
        self.idx = (self.idx + 1) % 500;
        if self.idx == 0 { self.filled = true; }
        let n = if self.filled { 500 } else { self.idx.max(1) };
        (self.sum / n as f64) as f32
    }
}

// ── Book UI state ─────────────────────────────────────────────────────────────

// (cat, entry, page)
static NAV: Mutex<(usize, usize, usize)> = Mutex::new((0, 0, 0));
// Last computed layout root — used for mod-side click hit-testing.
static LAST_LAYOUT: Mutex<Option<LayoutNode>> = Mutex::new(None);

fn handle_nav(event: &str) {
    let mut nav = NAV.lock().unwrap();
    match event {
        "prev_page" => { if nav.2 > 0 { nav.2 -= 1; } }
        "next_page" => { nav.2 += 1; }
        e if e.starts_with("cat:") => {
            if let Ok(i) = e[4..].parse::<usize>() { *nav = (i, 0, 0); }
        }
        e if e.starts_with("entry:") => {
            if let Ok(i) = e[6..].parse::<usize>() { nav.1 = i; nav.2 = 0; }
        }
        _ => {}
    }
}

// ── Book UI builder ───────────────────────────────────────────────────────────

/// Build the book UiRoot centered on screen using flex spacers for both axes.
fn build_book_ui(sw: f32, sh: f32) -> UiRoot {
    use crate::book;
    let book = book::guide_book();
    let (cat_idx, ent_idx, pg_idx) = *NAV.lock().unwrap();

    // ── Sidebar: categories ──────────────────────────────────────────────────
    // Sidebar background colour: dark parchment
    const SIDEBAR_BG: u32 = 0xFF_1A1008;
    const PAGE_BG:    u32 = 0xFF_2A1E10;
    const BORDER:     u32 = 0xFF_5C3A1A;
    const TITLE_COL:  u32 = 0xFF_E8C070;
    const SEL_COL:    u32 = 0xFF_FFE080;
    const DIM_COL:    u32 = 0xFF_9A7850;
    const TEXT_COL:   u32 = 0xFF_D4C8A0;
    const NAV_COL:    u32 = 0xFF_C0A060;

    let mut cats_col = widget::panel(FlexDir::Column).gap(1.0).padding(4.0, 4.0, 2.0, 4.0);
    cats_col = cats_col.child(
        widget::label("── Разделы ──").color(DIM_COL).font_scale(0.8)
    );
    for (i, cat) in book.categories.iter().enumerate() {
        let selected = i == cat_idx;
        let color = if selected { SEL_COL } else { DIM_COL };
        let bg    = if selected { 0x40_FFFFFF } else { 0 };
        cats_col = cats_col.child(
            widget::button(&cat.name).color(color).bg(bg)
                .padding(2.0, 6.0, 2.0, 6.0).font_scale(0.85)
                .on_click(format!("cat:{i}"))
        );
    }

    // ── Sidebar: entries ─────────────────────────────────────────────────────
    let entries: Vec<_> = book.entries.iter()
        .filter(|e| book.categories.get(cat_idx).map_or(false, |c| e.category == c.id))
        .collect();
    let mut entries_col = widget::panel(FlexDir::Column).gap(1.0).padding(2.0, 4.0, 4.0, 4.0);
    entries_col = entries_col.child(
        widget::label("── Записи ──").color(DIM_COL).font_scale(0.8)
    );
    for (i, e) in entries.iter().enumerate() {
        let selected = i == ent_idx;
        let color = if selected { SEL_COL } else { NAV_COL };
        let bg    = if selected { 0x30_FFFFFF } else { 0 };
        let label: String = e.name.chars().take(14).collect();
        entries_col = entries_col.child(
            widget::button(&label).color(color).bg(bg)
                .padding(1.0, 6.0, 1.0, 6.0).font_scale(0.85)
                .on_click(format!("entry:{i}"))
        );
    }

    let sidebar = widget::panel(FlexDir::Column)
        .w(110.0).bg(SIDEBAR_BG)
        .padding(0.0, 0.0, 0.0, 0.0)
        .child(
            widget::label(&book.name).color(TITLE_COL)
                .padding(4.0, 6.0, 4.0, 6.0).font_scale(1.0)
        )
        .child(widget::label("").h(1.0).bg(BORDER))
        .child(cats_col)
        .child(widget::label("").h(1.0).bg(BORDER))
        .child(entries_col);

    // ── Page area ────────────────────────────────────────────────────────────
    let entry = entries.get(ent_idx).copied();
    let page  = entry.and_then(|e| e.pages.get(pg_idx));
    let page_count = entry.map_or(1, |e| e.pages.len().max(1));

    let mut page_col = widget::panel(FlexDir::Column).flex(1.0).gap(3.0)
        .padding(6.0, 8.0, 6.0, 8.0);

    if let Some(e) = entry {
        page_col = page_col.child(
            widget::label(&e.name).color(TITLE_COL).font_scale(1.05)
        );
        page_col = page_col.child(widget::label("").h(1.0).bg(BORDER));
    }

    if let Some(p) = page {
        match p {
            BookPage::Text { text } => {
                for para in text.split('\n') {
                    page_col = page_col.child(
                        widget::label(para).color(TEXT_COL).font_scale(0.9)
                    );
                }
            }
            BookPage::Spotlight { item, title, text } => {
                if let Some(t) = title {
                    page_col = page_col.child(widget::label(t).color(TITLE_COL));
                }
                page_col = page_col.child(widget::item_slot(&item.id));
                if let Some(t) = text {
                    page_col = page_col.child(widget::label(t).color(TEXT_COL).font_scale(0.9));
                }
            }
            _ => {
                page_col = page_col.child(
                    widget::label("(unsupported page type)").color(DIM_COL).font_scale(0.85)
                );
            }
        }
    } else if entry.is_none() {
        page_col = page_col.child(
            widget::label("Выберите запись слева.").color(DIM_COL).font_scale(0.9)
        );
    }

    // push nav bar to bottom
    page_col = page_col.child(widget::spacer().flex(1.0));
    page_col = page_col.child(widget::label("").h(1.0).bg(BORDER));
    let pg_label = format!("{}/{}", pg_idx + 1, page_count);
    page_col = page_col.child(
        widget::panel(FlexDir::Row).h(18.0).gap(4.0).padding(2.0, 4.0, 2.0, 4.0)
            .child(widget::button("◀").w(20.0).h(14.0).color(NAV_COL).on_click("prev_page"))
            .child(widget::label(&pg_label).color(DIM_COL).flex(1.0).align(Align::Center).font_scale(0.85))
            .child(widget::button("▶").w(20.0).h(14.0).color(NAV_COL).on_click("next_page"))
    );

    // ── Book frame ───────────────────────────────────────────────────────────
    let book_panel = widget::panel(FlexDir::Row)
        .w(340.0).h(220.0)
        .bg(PAGE_BG)
        .padding(1.0, 1.0, 1.0, 1.0)
        .child(sidebar)
        .child(widget::label("").w(1.0).bg(BORDER))
        .child(page_col);

    // ── Full-screen wrapper with flex spacers — centers the book both axes ──
    // Column: spacer + book + spacer  →  vertical center
    // Align::Center on the Column     →  horizontal center
    UiRoot::new("yog:example_guide",
        widget::panel(FlexDir::Column).w(sw).h(sh).align(Align::Center)
            .child(widget::spacer().flex(1.0))
            .child(book_panel)
            .child(widget::spacer().flex(1.0))
    )
}

// ── Registration ──────────────────────────────────────────────────────────────

pub fn register(registry: &mut Registry) {
    // ── Screen open/close tracking ────────────────────────────────────────────
    registry.on_screen_open(|ev| {
        if ev.screen_class.contains("YogUIScreen") {
            // clear stale layout when a new screen opens
            *LAST_LAYOUT.lock().unwrap() = None;
        }
    });

    // ── Book UI: render (on_ui_render fires AFTER screen darkening) ───────────
    registry.on_ui_render("yog:example_guide", |ctx: &GfxContext| {
        let (sw, sh) = {
            let s = ctx.screen_size();
            (s.0 as f32, s.1 as f32)
        };
        let mut ui = build_book_ui(sw, sh);
        ui.layout(sw, sh);
        // Store layout for mod-side click hit-testing.
        *LAST_LAYOUT.lock().unwrap() = Some(ui.layout_root.clone());
        ui.render(ctx);
    });

    // ── Book UI: click handling (receives "click:X:Y" from runtime) ───────────
    registry.register_ui("yog:example_guide", |_ui_id, event| {
        // Hit-test against the last rendered layout.
        if let Some(rest) = event.strip_prefix("click:") {
            let mut parts = rest.splitn(2, ':');
            if let (Some(xs), Some(ys)) = (parts.next(), parts.next()) {
                if let (Ok(mx), Ok(my)) = (xs.parse::<f32>(), ys.parse::<f32>()) {
                    let lock = LAST_LAYOUT.lock().unwrap();
                    if let Some(layout) = lock.as_ref() {
                        if let Some(hit) = yog_api::ui::layout::hit_test(layout, mx, my) {
                            if let Some(click_ev) = &hit.on_click {
                                handle_nav(click_ev);
                            }
                        }
                    }
                    return;
                }
            }
        }
        handle_nav(event);
    });

    // ── HUD: FPS counter only (book renders via on_ui_render above) ───────────
    let timer = Mutex::new(FrameTimer::new());
    registry.on_hud_render(move |ctx: &GfxContext| {
        let avg_ms = timer.lock().unwrap().tick() * 1000.0;
        ctx.draw2d().text(&format!("{:.1} ms", avg_ms), 4.0, 4.0, 0xFF_00FF00, true);
    });

    // ── World renderer ────────────────────────────────────────────────────────
    let renderer = Mutex::new(WorldRenderer::new());
    registry.on_world_render(move |ctx: &GfxContext| {
        renderer.lock().unwrap().render(ctx);
    });
}
