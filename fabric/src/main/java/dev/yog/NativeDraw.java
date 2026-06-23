package dev.yog;

import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.texture.AbstractTexture;
import net.minecraft.util.Identifier;

/**
 * Client-only HUD draw helpers called from Rust via JNI during on_hud_render.
 *
 * {@link #hudDrawContext} is set by {@link YogClient} immediately before
 * {@code nativeOnHudRender} and cleared immediately after.
 */
public final class NativeDraw {
    private NativeDraw() {}

    /** The DrawContext for the current HUD render frame. Render-thread only. */
    static DrawContext hudDrawContext;

    // ── 2-D primitives ───────────────────────────────────────────────────────

    public static void drawText(String text, float x, float y, int color, boolean shadow) {
        DrawContext ctx = hudDrawContext;
        if (ctx == null) return;
        ctx.drawText(MinecraftClient.getInstance().textRenderer,
                text, (int) x, (int) y, color, shadow);
    }

    public static void drawRect(float x1, float y1, float x2, float y2, int color) {
        DrawContext ctx = hudDrawContext;
        if (ctx == null) return;
        ctx.fill((int) x1, (int) y1, (int) x2, (int) y2, color);
    }

    public static void drawGradientRect(float x1, float y1, float x2, float y2, int top, int bottom) {
        DrawContext ctx = hudDrawContext;
        if (ctx == null) return;
        ctx.fillGradient((int) x1, (int) y1, (int) x2, (int) y2, top, bottom);
    }

    /** All coordinates and sizes in GUI pixels; u0/v0 and tw/th in texels. */
    public static void drawTexture(String id, float x, float y,
                                   float u0, float v0, float w, float h,
                                   float tw, float th) {
        DrawContext ctx = hudDrawContext;
        if (ctx == null) return;
        Identifier ident = Identifier.tryParse(id);
        if (ident == null) return;
        ctx.drawTexture(ident, (int) x, (int) y, u0, v0,
                (int) w, (int) h, (int) tw, (int) th);
    }

    /**
     * Returns the OpenGL texture name for a Minecraft-managed texture, or 0 if
     * the texture has not been loaded yet.  Do NOT call glDeleteTextures on it.
     */
    public static int getMcTextureId(String id) {
        Identifier ident = Identifier.tryParse(id);
        if (ident == null) return 0;
        AbstractTexture tex = MinecraftClient.getInstance()
                .getTextureManager().getTexture(ident);
        return tex != null ? tex.getGlId() : 0;
    }
}
