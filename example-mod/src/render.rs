//! Client-side rendering demo: HUD overlay + world-space geometry.

use std::sync::Mutex;

use yog_api::{
    GfxContext,
    gfx_core::{DataType, DrawMode, blend},
    gfx_gl::{Buffer, ShaderProgram, VertexArray},
    Registry,
};

// ── GLSL sources ─────────────────────────────────────────────────────────────

const VERT: &str = r#"
#version 150 core
in vec3 aPos;
uniform mat4 uViewProj;
uniform vec3 uOffset;
void main() {
    gl_Position = uViewProj * vec4(aPos + uOffset, 1.0);
}
"#;

const FRAG: &str = r#"
#version 150 core
out vec4 fragColor;
uniform vec4 uColor;
void main() {
    fragColor = uColor;
}
"#;

// ── Plumbob geometry (double pyramid, like Sims) ──────────────────────────────
//
// Centered at origin; upper tip at (0, 0.5, 0), lower tip at (0, -0.25, 0).
// Base square at y=0: corners (±0.25, 0, ±0.25).

#[rustfmt::skip]
const PLUMBOB_VERTS: &[f32] = {
    const T: f32 =  0.5;   // top tip y
    const B: f32 = -0.25;  // bottom tip y
    const H: f32 =  0.25;  // half-width of base
    &[
        // ── upper pyramid ─────────────────────────────────────────────────
         0.0, T,  0.0,   H, 0.0,  H,  -H, 0.0,  H,   // +Z face
         0.0, T,  0.0,  -H, 0.0,  H,  -H, 0.0, -H,   // -X face
         0.0, T,  0.0,  -H, 0.0, -H,   H, 0.0, -H,   // -Z face
         0.0, T,  0.0,   H, 0.0, -H,   H, 0.0,  H,   // +X face
        // ── lower pyramid ─────────────────────────────────────────────────
         0.0, B,  0.0,  -H, 0.0,  H,   H, 0.0,  H,   // +Z face
         0.0, B,  0.0,  -H, 0.0, -H,  -H, 0.0,  H,   // -X face
         0.0, B,  0.0,   H, 0.0, -H,  -H, 0.0, -H,   // -Z face
         0.0, B,  0.0,   H, 0.0,  H,   H, 0.0, -H,   // +X face
    ]
};

// ── Renderer state ────────────────────────────────────────────────────────────

struct WorldRenderer {
    prog:        Option<ShaderProgram>,
    // flat quad at a fixed world position
    quad_vbo:    Option<Buffer>,
    quad_vao:    Option<VertexArray>,
    // plumbob above the player
    plumb_vbo:   Option<Buffer>,
    plumb_vao:   Option<VertexArray>,
}

impl WorldRenderer {
    const fn new() -> Self {
        Self {
            prog: None,
            quad_vbo: None, quad_vao: None,
            plumb_vbo: None, plumb_vao: None,
        }
    }

    fn init(&mut self, ctx: &GfxContext) {
        let prog = match ctx.create_shader(VERT, FRAG) {
            Ok(p) => p,
            Err(()) => return,
        };

        // Flat quad (two triangles) at Y=0, XZ -0.5..0.5.
        // offset will move it to (0, 65, 0) in world space.
        #[rustfmt::skip]
        let quad: &[f32] = &[
            -0.5, 0.0,  0.5,
             0.5, 0.0,  0.5,
             0.5, 0.0, -0.5,
            -0.5, 0.0,  0.5,
             0.5, 0.0, -0.5,
            -0.5, 0.0, -0.5,
        ];
        let quad_vbo = ctx.create_buffer();
        unsafe { quad_vbo.upload(ctx, quad, false) };
        let quad_vao = ctx.create_vao();
        quad_vao.attrib(ctx, &quad_vbo, 0, 3, DataType::F32, false, 12, 0);

        // Plumbob double-pyramid.
        let plumb_vbo = ctx.create_buffer();
        unsafe { plumb_vbo.upload(ctx, PLUMBOB_VERTS, false) };
        let plumb_vao = ctx.create_vao();
        plumb_vao.attrib(ctx, &plumb_vbo, 0, 3, DataType::F32, false, 12, 0);

        self.prog      = Some(prog);
        self.quad_vbo  = Some(quad_vbo);
        self.quad_vao  = Some(quad_vao);
        self.plumb_vbo = Some(plumb_vbo);
        self.plumb_vao = Some(plumb_vao);
    }

    fn render(&mut self, ctx: &GfxContext) {
        if self.prog.is_none() { self.init(ctx); }
        let Some(prog) = self.prog.as_ref() else { return };

        let cam = ctx.camera_pos();
        let vp  = ctx.view_proj();

        ctx.set_depth(true, false);
        ctx.set_blend(true, blend::SRC_ALPHA, blend::ONE_MINUS_SRC_ALPHA);

        prog.uniform_mat4(ctx, "uViewProj", &vp);

        // Red flat quad at world (0, 65, 0)
        if let Some(vao) = self.quad_vao.as_ref() {
            prog.uniform_3f(ctx, "uOffset",
                0.0 - cam[0], 65.0 - cam[1], 0.0 - cam[2]);
            prog.uniform_4f(ctx, "uColor", 1.0, 0.2, 0.2, 0.7);
            ctx.draw_arrays(vao, prog, DrawMode::Triangles, 0, 6);
        }

        // Green plumbob 2.5 blocks above the player's eye (camera position)
        if let Some(vao) = self.plumb_vao.as_ref() {
            prog.uniform_3f(ctx, "uOffset", 0.0, 2.5, 0.0);
            prog.uniform_4f(ctx, "uColor", 0.1, 0.9, 0.2, 0.85);
            ctx.draw_arrays(vao, prog, DrawMode::Triangles, 0,
                (PLUMBOB_VERTS.len() / 3) as u32);
        }

        ctx.set_blend(false, 0, 0);
        ctx.set_depth(false, false);
    }
}

// ── Rolling delta-tick average ────────────────────────────────────────────────

struct DtAvg {
    buf:    Box<[u32; 500]>,
    sum:    u64,
    idx:    usize,
    filled: bool,
}

impl DtAvg {
    fn new() -> Self {
        Self { buf: Box::new([0; 500]), sum: 0, idx: 0, filled: false }
    }

    fn push(&mut self, dt: f32) -> f32 {
        let v = (dt * 1_000_000.0) as u32;
        self.sum = self.sum.saturating_sub(self.buf[self.idx] as u64);
        self.buf[self.idx] = v;
        self.sum += v as u64;
        self.idx += 1;
        if self.idx >= 500 { self.idx = 0; self.filled = true; }
        let n = if self.filled { 500 } else { self.idx.max(1) } as u64;
        (self.sum as f32 / n as f32) / 1_000_000.0
    }
}

// ── Registration ──────────────────────────────────────────────────────────────

pub fn register(registry: &mut Registry) {
    let dt_avg = Mutex::new(DtAvg::new());
    registry.on_hud_render(move |ctx: &GfxContext| {
        let avg = dt_avg.lock().unwrap().push(ctx.delta_tick());
        let d = ctx.draw2d();
        d.rect(4.0, 4.0, 80.0, 14.0, 0x88_00_00_00);
        d.text(
            &format!("yog {:.1}ms", avg * 50.0),
            6.0, 5.0, 0xFF_FF_FF_FF, true,
        );
    });

    let renderer = Mutex::new(WorldRenderer::new());
    registry.on_world_render(move |ctx: &GfxContext| {
        renderer.lock().unwrap().render(ctx);
    });
}
