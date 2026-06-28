//! Client-side rendering: FPS counter + world renderer + book UI via yog-ui.

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
use yog_api::ui::{UiRoot, widget, Align, FlexDir};

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

// ── Book UI via fluent API ────────────────────────────────────────────────────

static BOOK_OPEN: AtomicBool = AtomicBool::new(false);
static NAV: Mutex<(usize, usize, usize)> = Mutex::new((0, 0, 0)); // cat, entry, page

/// Build a `UiRoot` from the example-mod guide book at current navigation state.
/// Build the book UI, centered on screen with fixed size.
fn build_book_ui() -> UiRoot {
    use crate::book;
    let book = book::guide_book();
    let (cat_idx, ent_idx, pg_idx) = *NAV.lock().unwrap();

    // ── Left panel: categories + entries ──────────────────────────────────
    let mut left = widget::panel(FlexDir::Column).w(100.0).gap(2.0).padding(2.0,2.0,2.0,2.0);

    left = left.child(widget::label("Categories").color(0xFF_888888).font_scale(0.9));
    for (i, cat) in book.categories.iter().enumerate() {
        let c = if i == cat_idx { 0xFF_FFFF55 } else { 0xFF_AAAAAA };
        left = left.child(widget::button(&cat.name).color(c).on_click(format!("cat:{i}")).font_scale(0.85));
    }
    left = left.child(widget::spacer().h(4.0));

    if let Some(cat) = book.categories.get(cat_idx) {
        let entries: Vec<_> = book.entries.iter().filter(|e| e.category == cat.id).collect();
        left = left.child(widget::label("Entries").color(0xFF_888888).font_scale(0.9));
        for (i, e) in entries.iter().enumerate() {
            let c = if i == ent_idx { 0xFF_FFFF55 } else { 0xFF_CCCCCC };
            let label: String = e.name.chars().take(14).collect();
            left = left.child(widget::button(&label).color(c).on_click(format!("entry:{i}")).font_scale(0.85));
        }
    }

    // ── Right panel: page + nav ───────────────────────────────────────────
    let mut right = widget::panel(FlexDir::Column).flex(1.0).gap(4.0).padding(4.0,4.0,4.0,4.0);

    if let Some(cat) = book.categories.get(cat_idx) {
        let entries: Vec<_> = book.entries.iter().filter(|e| e.category == cat.id).collect();
        if let Some(entry) = entries.get(ent_idx) {
            // Entry title
            right = right.child(widget::label(&entry.name).color(0xFF_D4A84B).font_scale(1.1));
            right = right.child(widget::spacer().h(2.0));

            if let Some(page) = entry.pages.get(pg_idx) {
                match page {
                    BookPage::Text { text } => {
                        right = right.child(widget::label(text).color(0xFF_CCCCAA).flex(1.0));
                    }
                    BookPage::Spotlight { item, title, text } => {
                        if let Some(t) = title {
                            right = right.child(widget::label(t).color(0xFF_FFFF55));
                        }
                        right = right.child(widget::item_slot(&item.id));
                        if let Some(t) = text {
                            right = right.child(widget::label(t).color(0xFF_CCCCAA).flex(1.0));
                        }
                    }
                    _ => {
                        right = right.child(widget::label("(unsupported page)").color(0xFF_888888));
                    }
                }
            }

            // Nav bar
            let total = entry.pages.len().max(1);
            right = right.child(
                widget::panel(FlexDir::Row).gap(4.0)
                    .child(widget::button("<").w(24.0).on_click("prev_page"))
                    .child(widget::label(&format!("{}/{}", pg_idx + 1, total))
                        .color(0xFF_888888).align(Align::Center).flex(1.0))
                    .child(widget::button(">").w(24.0).on_click("next_page"))
            );
        }
    }

    // ── Root: centered book ───────────────────────────────────────────────
    UiRoot::new("yog:example_guide",
        // Wrapper: full screen, centers child
        widget::panel(FlexDir::Column).align(Align::Center)
            .child(
                widget::panel(FlexDir::Row).w(320.0).h(210.0).gap(2.0)
                    .padding(3.0,3.0,3.0,3.0).bg(0xFF_2A1A0E)
                    .child(left).child(right)
            )
    )
}

// ── Registration ──────────────────────────────────────────────────────────────

pub fn register(registry: &mut Registry) {
    // Track YogUIScreen open/close
    registry.on_screen_open(|ev| {
        if ev.screen_class.contains("YogUIScreen") { BOOK_OPEN.store(true, Ordering::Relaxed); }
    });
    registry.on_screen_close(|_| { BOOK_OPEN.store(false, Ordering::Relaxed); });

    // Handle book navigation events
    registry.register_ui("yog:example_guide", |_ui_id, event| {
        let mut nav = NAV.lock().unwrap();
        match event {
            "prev_page" => if nav.2 > 0 { nav.2 -= 1; }
            "next_page" => { nav.2 += 1; }
            e if e.starts_with("cat:") => {
                if let Ok(i) = e[4..].parse::<usize>() { nav.0 = i; nav.1 = 0; nav.2 = 0; }
            }
            e if e.starts_with("entry:") => {
                if let Ok(i) = e[6..].parse::<usize>() { nav.1 = i; nav.2 = 0; }
            }
            _ => {}
        }
    });

    // HUD: FPS + book
    let timer = Mutex::new(FrameTimer::new());
    registry.on_hud_render(move |ctx: &GfxContext| {
        let d = ctx.draw2d();
        let avg_ms = timer.lock().unwrap().tick() * 1000.0;
        d.text(&format!("{:.1} ms", avg_ms), 4.0, 4.0, 0xFF_00FF00, true);

        if BOOK_OPEN.load(Ordering::Relaxed) {
            let mut ui = build_book_ui();
            let sw = ctx.screen_size().0 as f32;
            let sh = ctx.screen_size().1 as f32;
            ui.layout(sw, sh);
            // Center the root rect
            let r = &ui.layout_root.rect;
            let dx = ((sw - r.w) / 2.0).max(0.0);
            let dy = ((sh - r.h) / 2.0).max(0.0);
            // Translate all layout coords to center
            // (layout engine positions from (0,0), we shift)
            // Workaround: just render at offset
            ui.render(ctx);  // yog-ui render uses layout coords directly
        }

    });

    // World renderer
    let renderer = Mutex::new(WorldRenderer::new());
    registry.on_world_render(move |ctx: &GfxContext| {
        renderer.lock().unwrap().render(ctx);
    });
}
