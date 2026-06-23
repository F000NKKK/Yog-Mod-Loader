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

// ── Renderer state ────────────────────────────────────────────────────────────

struct WorldRenderer {
    vbo:  Option<Buffer>,
    vao:  Option<VertexArray>,
    prog: Option<ShaderProgram>,
}

impl WorldRenderer {
    const fn new() -> Self {
        Self { vbo: None, vao: None, prog: None }
    }

    fn init(&mut self, ctx: &GfxContext) {
        // A quad (two triangles) at Y=0, XZ -0.5..0.5, offset applied in shader.
        // Each vertex: x, y, z  (f32 × 3, stride 12)
        #[rustfmt::skip]
        let verts: &[f32] = &[
            -0.5, 0.0,  0.5,
             0.5, 0.0,  0.5,
             0.5, 0.0, -0.5,
            -0.5, 0.0,  0.5,
             0.5, 0.0, -0.5,
            -0.5, 0.0, -0.5,
        ];

        let vbo = ctx.create_buffer();
        unsafe { vbo.upload(ctx, verts, false) };

        let vao = ctx.create_vao();
        vao.attrib(ctx, &vbo, 0, 3, DataType::F32, false, 12, 0);

        let prog = match ctx.create_shader(VERT, FRAG) {
            Ok(p) => p,
            Err(()) => { return; }
        };

        self.vbo  = Some(vbo);
        self.vao  = Some(vao);
        self.prog = Some(prog);
    }

    fn render(&mut self, ctx: &GfxContext) {
        if self.vbo.is_none() { self.init(ctx); }
        let (Some(vao), Some(prog)) = (self.vao.as_ref(), self.prog.as_ref()) else { return };

        // Place the quad 1 block above the world origin, camera-relative.
        let cam = ctx.camera_pos();
        let offset = [0.0_f32 - cam[0], 65.0_f32 - cam[1], 0.0_f32 - cam[2]];

        prog.uniform_mat4(ctx, "uViewProj", &ctx.view_proj());
        prog.uniform_3f(ctx, "uOffset", offset[0], offset[1], offset[2]);
        prog.uniform_4f(ctx, "uColor", 1.0, 0.2, 0.2, 0.7); // red-ish, semi-transparent

        ctx.set_depth(true, false);                          // depth test, no depth write
        ctx.set_blend(true, blend::SRC_ALPHA, blend::ONE_MINUS_SRC_ALPHA);
        ctx.draw_arrays(vao, prog, DrawMode::Triangles, 0, 6);
        ctx.set_blend(false, 0, 0);
        ctx.set_depth(false, false);
    }
}

// ── Registration ──────────────────────────────────────────────────────────────

pub fn register(registry: &mut Registry) {
    // HUD overlay — draw2d helpers (text + rect)
    registry.on_hud_render(|ctx: &GfxContext| {
        let d = ctx.draw2d();
        // Semi-transparent background pill
        d.rect(4.0, 4.0, 80.0, 14.0, 0x88_00_00_00);
        d.text(
            &format!("yog {:.0}ms", ctx.delta_tick() * 50.0),
            6.0, 5.0, 0xFF_FF_FF_FF, true,
        );
    });

    // World geometry — a flat red quad hovering above (0, 65, 0)
    let renderer = Mutex::new(WorldRenderer::new());
    registry.on_world_render(move |ctx: &GfxContext| {
        renderer.lock().unwrap().render(ctx);
    });
}
