package dev.yog;

import com.mojang.blaze3d.systems.RenderSystem;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.render.BufferBuilder;
import net.minecraft.client.render.GameRenderer;
import net.minecraft.client.render.Tessellator;
import net.minecraft.client.render.VertexFormat;
import net.minecraft.client.render.VertexFormats;
import net.minecraft.client.util.Window;
import net.minecraft.util.Identifier;
import org.joml.Matrix4f;

/**
 * Client-only HUD draw helpers called from Rust via JNI during on_hud_render.
 *
 * {@link #hudDrawContext} is set by {@link YogClient} immediately before
 * {@code nativeOnHudRender} and cleared immediately after. All methods must
 * only be called on the render thread (they are, because the JNI call chain
 * originates from the Fabric HudRenderCallback).
 */
public final class NativeDraw {
    private NativeDraw() {}

    /** The DrawContext for the current HUD render frame. Render-thread only. */
    static DrawContext hudDrawContext;

    /** Whether a color mesh ({@code POSITION_COLOR}) is currently open. */
    private static boolean meshActive;
    /** Whether a textured mesh ({@code POSITION_TEXTURE_COLOR}) is currently open. */
    private static boolean texMeshActive;

    // ── 2-D primitives ───────────────────────────────────────────────────────

    public static void drawText(String text, int x, int y, int color, boolean shadow) {
        DrawContext ctx = hudDrawContext;
        if (ctx == null) return;
        ctx.drawText(MinecraftClient.getInstance().textRenderer, text, x, y, color, shadow);
    }

    public static void drawRect(int x1, int y1, int x2, int y2, int color) {
        DrawContext ctx = hudDrawContext;
        if (ctx == null) return;
        ctx.fill(x1, y1, x2, y2, color);
    }

    public static void drawGradientRect(int x1, int y1, int x2, int y2, int top, int bottom) {
        DrawContext ctx = hudDrawContext;
        if (ctx == null) return;
        ctx.fillGradient(x1, y1, x2, y2, top, bottom);
    }

    public static void drawTexture(String id, int x, int y, float u, float v, int w, int h, int texW, int texH) {
        DrawContext ctx = hudDrawContext;
        if (ctx == null) return;
        Identifier ident = Identifier.tryParse(id);
        if (ident == null) return;
        ctx.drawTexture(ident, x, y, u, v, w, h, texW, texH);
    }

    // ── matrix transform ─────────────────────────────────────────────────────

    public static void pushMatrix() {
        DrawContext ctx = hudDrawContext;
        if (ctx == null) return;
        ctx.getMatrices().push();
    }

    public static void popMatrix() {
        DrawContext ctx = hudDrawContext;
        if (ctx == null) return;
        ctx.getMatrices().pop();
    }

    public static void translate(float x, float y, float z) {
        DrawContext ctx = hudDrawContext;
        if (ctx == null) return;
        ctx.getMatrices().translate(x, y, z);
    }

    public static void scale(float sx, float sy, float sz) {
        DrawContext ctx = hudDrawContext;
        if (ctx == null) return;
        ctx.getMatrices().scale(sx, sy, sz);
    }

    // ── color vertex buffer (POSITION_COLOR) ─────────────────────────────────

    public static void beginMesh(int mode) {
        if (meshActive || texMeshActive) return;
        meshActive = true;
        RenderSystem.setShader(GameRenderer::getPositionColorProgram);
        RenderSystem.enableBlend();
        RenderSystem.defaultBlendFunc();
        Tessellator.getInstance().getBuffer().begin(drawMode(mode), VertexFormats.POSITION_COLOR);
    }

    public static void vertex(float x, float y, float z, int r, int g, int b, int a) {
        if (!meshActive) return;
        DrawContext ctx = hudDrawContext;
        if (ctx == null) return;
        Matrix4f matrix = ctx.getMatrices().peek().getPositionMatrix();
        Tessellator.getInstance().getBuffer()
                .vertex(matrix, x, y, z).color(r, g, b, a).next();
    }

    public static void endMesh() {
        if (!meshActive) return;
        meshActive = false;
        Tessellator.getInstance().draw();
        RenderSystem.disableBlend();
    }

    // ── textured vertex buffer (POSITION_TEXTURE_COLOR) ───────────────────────

    public static void beginTexturedMesh(int mode, String textureId) {
        if (meshActive || texMeshActive) return;
        Identifier ident = Identifier.tryParse(textureId);
        if (ident == null) return;
        texMeshActive = true;
        RenderSystem.setShaderTexture(0, ident);
        RenderSystem.setShader(GameRenderer::getPositionTexColorProgram);
        RenderSystem.enableBlend();
        RenderSystem.defaultBlendFunc();
        Tessellator.getInstance().getBuffer().begin(drawMode(mode), VertexFormats.POSITION_TEXTURE_COLOR);
    }

    public static void vertexUv(float x, float y, float z, float u, float v, int r, int g, int b, int a) {
        if (!texMeshActive) return;
        DrawContext ctx = hudDrawContext;
        if (ctx == null) return;
        Matrix4f matrix = ctx.getMatrices().peek().getPositionMatrix();
        Tessellator.getInstance().getBuffer()
                .vertex(matrix, x, y, z).texture(u, v).color(r, g, b, a).next();
    }

    public static void endTexturedMesh() {
        if (!texMeshActive) return;
        texMeshActive = false;
        Tessellator.getInstance().draw();
        RenderSystem.disableBlend();
    }

    // ── clip ─────────────────────────────────────────────────────────────────

    public static void scissor(int x, int y, int w, int h) {
        Window win = MinecraftClient.getInstance().getWindow();
        double scale = win.getScaleFactor();
        int physX = (int) Math.round(x * scale);
        int physY = (int) Math.round((win.getScaledHeight() - y - h) * scale);
        int physW = (int) Math.round(w * scale);
        int physH = (int) Math.round(h * scale);
        RenderSystem.enableScissor(physX, physY, Math.max(0, physW), Math.max(0, physH));
    }

    public static void clearScissor() {
        RenderSystem.disableScissor();
    }

    // ── screen info ──────────────────────────────────────────────────────────

    public static int screenWidth() {
        return MinecraftClient.getInstance().getWindow().getScaledWidth();
    }

    public static int screenHeight() {
        return MinecraftClient.getInstance().getWindow().getScaledHeight();
    }

    // ── helpers ──────────────────────────────────────────────────────────────

    private static VertexFormat.DrawMode drawMode(int mode) {
        return switch (mode) {
            case 1 -> VertexFormat.DrawMode.QUADS;
            case 2 -> VertexFormat.DrawMode.LINES;
            case 3 -> VertexFormat.DrawMode.LINE_STRIP;
            case 4 -> VertexFormat.DrawMode.TRIANGLE_STRIP;
            case 5 -> VertexFormat.DrawMode.TRIANGLE_FAN;
            default -> VertexFormat.DrawMode.TRIANGLES;
        };
    }
}
