//! Client-side rendering: FPS counter, world renderer, book UI.

use std::sync::{Arc, Mutex};
use std::time::Instant;

use yog_api::{
    GfxContext,
    gfx_core::{DataType, DrawMode, blend},
    gfx_gl::{Buffer, ShaderProgram, VertexArray},
    BookRenderer,
    Registry,
};
use yog_api::ui::{LayoutNode, layout};

// ── GLSL (world renderer) ────────────────────────────────────────────────────

const VERT: &str = "
#version 330 core
in vec3 aPos;
in vec3 aBary;
uniform mat4 uViewProj;
uniform vec3 uOffset;
uniform float uRotY;
out vec3 vBary;

void main() {
    float s = sin(uRotY); float c = cos(uRotY);
    vec3 p = vec3(aPos.x*c - aPos.z*s, aPos.y, aPos.x*s + aPos.z*c);
    gl_Position = uViewProj * vec4(p + uOffset, 1.0);
    vBary = aBary;
}";

const FRAG: &str = "
#version 330 core
in vec3 vBary;
out vec4 fragColor;
uniform vec4 uColor;
void main() {
    float d = min(min(vBary.x, vBary.y), vBary.z);
    float edge = 1.0 - d;           // 0=center, 1=edge
    float shade = 1.0 - edge * 0.7; // edges 30%, center 100%
    float glow = d * d * 0.6;       // center bloom
    float rim  = edge * 0.25;       // rim highlight
    vec3 col = uColor.rgb * (shade + glow + rim);
    fragColor = vec4(col, uColor.a * (0.4 + 0.6 * d));
}";

#[rustfmt::skip]
const PLUMBOB: &[f32] = {
    const T: f32 =  0.85;  // upper tip
    const B: f32 = -0.50;  // lower tip  
    const H: f32 =  0.45;  // base half-extent
    &[
        // Top pyramid — 4 faces
        0.0,T,0.0, 1.0,0.0,0.0,  -H,0.0, H, 0.0,1.0,0.0,   H,0.0, H, 0.0,0.0,1.0,
        0.0,T,0.0, 1.0,0.0,0.0,   H,0.0, H, 0.0,1.0,0.0,   H,0.0,-H, 0.0,0.0,1.0,
        0.0,T,0.0, 1.0,0.0,0.0,   H,0.0,-H, 0.0,1.0,0.0,  -H,0.0,-H, 0.0,0.0,1.0,
        0.0,T,0.0, 1.0,0.0,0.0,  -H,0.0,-H, 0.0,1.0,0.0,  -H,0.0, H, 0.0,0.0,1.0,
        // Bottom pyramid — 4 faces (winding reversed)
        0.0,B,0.0, 1.0,0.0,0.0,   H,0.0, H, 0.0,1.0,0.0,  -H,0.0, H, 0.0,0.0,1.0,
        0.0,B,0.0, 1.0,0.0,0.0,   H,0.0,-H, 0.0,1.0,0.0,   H,0.0, H, 0.0,0.0,1.0,
        0.0,B,0.0, 1.0,0.0,0.0,  -H,0.0,-H, 0.0,1.0,0.0,   H,0.0,-H, 0.0,0.0,1.0,
        0.0,B,0.0, 1.0,0.0,0.0,  -H,0.0, H, 0.0,1.0,0.0,  -H,0.0,-H, 0.0,0.0,1.0,
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
        let pa = ctx.create_vao();
        pa.attrib(ctx, &pv, 0, 3, DataType::F32, false, 24, 0);   // aPos
        pa.attrib(ctx, &pv, 1, 3, DataType::F32, false, 24, 12);  // aBary
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
        let off = [pw[0]-cam[0], pw[1]-cam[1], pw[2]-cam[2]-0.0];
        let rot = (t * ROT_SPEED) % std::f32::consts::TAU;
        let vp = ctx.view_proj();
        ctx.set_depth(true, false);
        ctx.set_blend(true, blend::SRC_ALPHA, blend::ONE_MINUS_SRC_ALPHA);
        prog.uniform_mat4(ctx, "uViewProj", &vp);
        if let Some(vao) = self.quad_vao.as_ref() {
            prog.uniform_1f(ctx, "uRotY", 0.0);
            prog.uniform_3f(ctx, "uOffset", 0.0-cam[0], 65.0-cam[1], 0.0-cam[2]-0.0);
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

struct BookState {
    renderer: BookRenderer,
    layout:   Option<LayoutNode>,
}

impl BookState {
    fn new() -> Self {
        let mut renderer = BookRenderer::new(crate::book::guide_book());
        // Feed recipes to the renderer so Crafting/Smelting pages draw the
        // actual grid instead of a placeholder.
        let (shaped, shapeless, furnace) = crate::content::recipes();
        for r in shaped    { renderer.add_recipe(r.id.clone(), &r.to_json()); }
        for r in shapeless { renderer.add_recipe(r.id.clone(), &r.to_json()); }
        for r in furnace   { renderer.add_recipe(r.id.clone(), &r.to_json()); }
        Self { renderer, layout: None }
    }
}

// ── Registration ──────────────────────────────────────────────────────────────

pub fn register(registry: &mut Registry) {
    // ── Book UI ───────────────────────────────────────────────────────────────
    let book = Arc::new(Mutex::new(None::<BookState>));
    let book_render = book.clone();
    let book_click  = book;

    registry.on_ui_render("yog:example_guide", move |ctx: &GfxContext| {
        let (sw, sh) = { let s = ctx.screen_size(); (s.0 as f32, s.1 as f32) };
        let mut lock = book_render.lock().unwrap();
        let state = lock.get_or_insert_with(BookState::new);
        state.renderer.render(ctx, sw, sh);
        state.layout = state.renderer.ui.as_ref()
            .map(|u| u.layout_root.clone());
    });

    registry.register_ui("yog:example_guide", move |_ui_id, event| {
        // "click:X:Y" — hit-test the layout, then dispatch the widget event.
        if let Some(rest) = event.strip_prefix("click:") {
            let mut parts = rest.splitn(2, ':');
            if let (Some(xs), Some(ys)) = (parts.next(), parts.next()) {
                if let (Ok(mx), Ok(my)) = (xs.parse::<f32>(), ys.parse::<f32>()) {
                    let click_ev = {
                        let lock = book_click.lock().unwrap();
                        lock.as_ref()
                            .and_then(|s| s.layout.as_ref())
                            .and_then(|l| layout::hit_test(l, mx, my))
                            .and_then(|hit| hit.on_click.clone())
                    };
                    if let Some(ev) = click_ev {
                        book_click.lock().unwrap()
                            .as_mut()
                            .map(|s| s.renderer.handle_event(&ev));
                    }
                    return;
                }
            }
        }
        // Direct event string (keyboard shortcuts, etc.)
        book_click.lock().unwrap()
            .as_mut()
            .map(|s| s.renderer.handle_event(event));
    });

    // ── HUD: FPS counter ──────────────────────────────────────────────────────
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
