//! Client-side rendering demo: HUD overlay + animated plumbob above the player.

use std::sync::Mutex;
use std::time::Instant;

use yog_api::{
    GfxContext,
    gfx_core::{DataType, DrawMode, blend},
    gfx_gl::{Buffer, ShaderProgram, VertexArray},
    Registry,
};

// ── GLSL ─────────────────────────────────────────────────────────────────────

const VERT: &str = r#"
#version 150 core
in vec3 aPos;
uniform mat4 uViewProj;
uniform vec3 uOffset;
uniform float uRotY;
void main() {
    float s = sin(uRotY);
    float c = cos(uRotY);
    // Rotate around world Y axis, then translate to camera-relative offset.
    vec3 p = vec3(aPos.x * c - aPos.z * s,
                  aPos.y,
                  aPos.x * s + aPos.z * c);
    gl_Position = uViewProj * vec4(p + uOffset, 1.0);
}
"#;

const FRAG: &str = r#"
#version 150 core
out vec4 fragColor;
uniform vec4 uColor;
void main() { fragColor = uColor; }
"#;

// ── Plumbob geometry (double pyramid, Sims-style) ─────────────────────────────
//
// Base square at y=0, corners A/B/C/D going CCW from above:
//   A=(+H,0,+H)  B=(-H,0,+H)  C=(-H,0,-H)  D=(+H,0,-H)
//
// Winding: CCW when viewed from outside (standard GL front-face).
// Derived per face by ensuring cross-product of edge vectors points outward.
#[rustfmt::skip]
const PLUMBOB: &[f32] = {
    const T: f32 =  0.70;   // upper tip y
    const B: f32 = -0.35;   // lower tip y
    const H: f32 =  0.35;   // base half-extent
    &[
        // ── upper pyramid ─────────────────────────────────────
        // +Z face: tip, B_corner, A_corner
        0.0,T,0.0,  -H,0.0, H,   H,0.0, H,
        // +X face: tip, A_corner, D_corner
        0.0,T,0.0,   H,0.0, H,   H,0.0,-H,
        // -Z face: tip, D_corner, C_corner
        0.0,T,0.0,   H,0.0,-H,  -H,0.0,-H,
        // -X face: tip, C_corner, B_corner
        0.0,T,0.0,  -H,0.0,-H,  -H,0.0, H,
        // ── lower pyramid ─────────────────────────────────────
        // +Z face: bot, A_corner, B_corner  (winding reversed vs upper)
        0.0,B,0.0,   H,0.0, H,  -H,0.0, H,
        // +X face: bot, D_corner, A_corner
        0.0,B,0.0,   H,0.0,-H,   H,0.0, H,
        // -Z face: bot, C_corner, D_corner
        0.0,B,0.0,  -H,0.0,-H,   H,0.0,-H,
        // -X face: bot, B_corner, C_corner
        0.0,B,0.0,  -H,0.0, H,  -H,0.0,-H,
    ]
};

// ── Spring constants ──────────────────────────────────────────────────────────
//
// Critically damped: D ≈ 2√K.  K=12 → D_crit≈6.9.
// Using D=7 (just barely overdamped) → no bounce, fast settle.
const SPRING_K: f32 = 12.0;
const SPRING_D: f32 = 7.0;
const ROT_SPEED: f32 = 1.2; // radians / second

// ── Renderer ─────────────────────────────────────────────────────────────────

struct WorldRenderer {
    prog:      Option<ShaderProgram>,
    quad_vbo:  Option<Buffer>,
    quad_vao:  Option<VertexArray>,
    plumb_vbo: Option<Buffer>,
    plumb_vao: Option<VertexArray>,

    start:     Option<Instant>,
    last:      Option<Instant>,

    // Plumbob spring state (world-space position and velocity).
    // None until first frame (teleport to initial position).
    plumb_pos: Option<[f32; 3]>,
    plumb_vel: [f32; 3],
}

impl WorldRenderer {
    const fn new() -> Self {
        Self {
            prog: None, quad_vbo: None, quad_vao: None,
            plumb_vbo: None, plumb_vao: None,
            start: None, last: None,
            plumb_pos: None, plumb_vel: [0.0; 3],
        }
    }

    fn init(&mut self, ctx: &GfxContext) {
        let prog = match ctx.create_shader(VERT, FRAG) { Ok(p) => p, Err(()) => return };

        // Flat quad at y=0 local space (placed at world 0,65,0 via offset)
        #[rustfmt::skip]
        let quad: &[f32] = &[
            -0.5,0.0, 0.5,   0.5,0.0, 0.5,   0.5,0.0,-0.5,
            -0.5,0.0, 0.5,   0.5,0.0,-0.5,  -0.5,0.0,-0.5,
        ];
        let qv = ctx.create_buffer();
        unsafe { qv.upload(ctx, quad, false) };
        let qa = ctx.create_vao();
        qa.attrib(ctx, &qv, 0, 3, DataType::F32, false, 12, 0);

        let pv = ctx.create_buffer();
        unsafe { pv.upload(ctx, PLUMBOB, false) };
        let pa = ctx.create_vao();
        pa.attrib(ctx, &pv, 0, 3, DataType::F32, false, 12, 0);

        let now = Instant::now();
        self.prog      = Some(prog);
        self.quad_vbo  = Some(qv);  self.quad_vao  = Some(qa);
        self.plumb_vbo = Some(pv);  self.plumb_vao = Some(pa);
        self.start     = Some(now); self.last      = Some(now);
    }

    fn render(&mut self, ctx: &GfxContext) {
        if self.prog.is_none() { self.init(ctx); }
        let Some(prog) = self.prog.as_ref() else { return };

        let now   = Instant::now();
        let dt    = self.last.map_or(0.0, |t| t.elapsed().as_secs_f32().min(0.1));
        let t     = self.start.map_or(0.0, |s| s.elapsed().as_secs_f32());
        self.last = Some(now);

        let cam = ctx.camera_pos();
        let p   = ctx.player_pos();

        // ── Spring physics (world-space) ──────────────────────────────────────
        // Target: 1.8 blocks above player eye.
        let target = [p[0], p[1] + 1.8, p[2]];
        // Teleport on first frame to avoid flying in from (0,0,0).
        let pos = self.plumb_pos.get_or_insert(target);
        for i in 0..3 {
            let force = (target[i] - pos[i]) * SPRING_K - self.plumb_vel[i] * SPRING_D;
            self.plumb_vel[i] += force * dt;
            pos[i]            += self.plumb_vel[i] * dt;
        }
        let plumb_world = *pos;

        // Camera-relative offset.  Subtract tiny Z so vertices are never at
        // view-space Z=0 when looking at horizon (prevents w=0 in perspective).
        let offset = [
            plumb_world[0] - cam[0],
            plumb_world[1] - cam[1],
            plumb_world[2] - cam[2] - 0.25,
        ];

        let rot_y = (t * ROT_SPEED) % std::f32::consts::TAU;
        let vp    = ctx.view_proj();

        ctx.set_depth(true, false);
        ctx.set_blend(true, blend::SRC_ALPHA, blend::ONE_MINUS_SRC_ALPHA);

        prog.uniform_mat4(ctx, "uViewProj", &vp);
        prog.uniform_1f(ctx, "uRotY", 0.0); // quad doesn't rotate

        // Red flat quad at world (0, 65, 0)
        if let Some(vao) = self.quad_vao.as_ref() {
            prog.uniform_3f(ctx, "uOffset",
                0.0 - cam[0], 65.0 - cam[1], 0.0 - cam[2] - 0.25);
            prog.uniform_4f(ctx, "uColor", 1.0, 0.2, 0.2, 0.7);
            ctx.draw_arrays(vao, prog, DrawMode::Triangles, 0, 6);
        }

        // Green rotating plumbob above the player
        if let Some(vao) = self.plumb_vao.as_ref() {
            prog.uniform_3f(ctx, "uOffset", offset[0], offset[1], offset[2]);
            prog.uniform_1f(ctx, "uRotY", rot_y);
            prog.uniform_4f(ctx, "uColor", 0.1, 0.9, 0.2, 0.92);
            ctx.draw_arrays(vao, prog, DrawMode::Triangles, 0,
                (PLUMBOB.len() / 3) as u32);
        }

        ctx.set_blend(false, 0, 0);
        ctx.set_depth(false, false);
    }
}

// ── Frame-time rolling average ────────────────────────────────────────────────

struct FrameTimer {
    buf:    [f32; 500],
    sum:    f64,
    idx:    usize,
    filled: bool,
    last:   Option<Instant>,
}

impl FrameTimer {
    const fn new() -> Self {
        Self { buf: [0.0; 500], sum: 0.0, idx: 0, filled: false, last: None }
    }

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

// ── Registration ──────────────────────────────────────────────────────────────

pub fn register(registry: &mut Registry) {
    let timer = Mutex::new(FrameTimer::new());
    registry.on_hud_render(move |ctx: &GfxContext| {
        let avg_ms = timer.lock().unwrap().tick() * 1000.0;
        let d = ctx.draw2d();
        d.rect(4.0, 4.0, 84.0, 14.0, 0x88_00_00_00);
        d.text(&format!("yog {avg_ms:.1}ms"), 6.0, 5.0, 0xFF_FF_FF_FF, true);
    });

    let renderer = Mutex::new(WorldRenderer::new());
    registry.on_world_render(move |ctx: &GfxContext| {
        renderer.lock().unwrap().render(ctx);
    });
}
