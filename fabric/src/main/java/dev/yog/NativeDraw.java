package dev.yog;

import com.mojang.blaze3d.platform.GlStateManager;
import com.mojang.blaze3d.systems.RenderSystem;

import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.render.BufferRenderer;
import net.minecraft.client.texture.AbstractTexture;
import net.minecraft.item.Item;
import net.minecraft.item.ItemStack;
import net.minecraft.item.Items;
import net.minecraft.registry.Registries;
import net.minecraft.util.Identifier;

import org.lwjgl.opengl.GL11;
import org.lwjgl.opengl.GL13;

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
        syncGlState(); // MC-pipeline draw may follow raw GL from Rust
        ctx.drawText(MinecraftClient.getInstance().textRenderer,
                text, (int) x, (int) y, color, shadow);
    }

    public static void drawRect(float x1, float y1, float x2, float y2, int color) {
        DrawContext ctx = hudDrawContext;
        if (ctx == null) return;
        syncGlState(); // MC-pipeline draw may follow raw GL from Rust
        ctx.fill((int) x1, (int) y1, (int) x2, (int) y2, color);
    }

    public static void drawGradientRect(float x1, float y1, float x2, float y2, int top, int bottom) {
        DrawContext ctx = hudDrawContext;
        if (ctx == null) return;
        syncGlState(); // MC-pipeline draw may follow raw GL from Rust
        ctx.fillGradient((int) x1, (int) y1, (int) x2, (int) y2, top, bottom);
    }

    /** All coordinates and sizes in GUI pixels; u0/v0 and tw/th in texels. */
    public static void drawTexture(String id, float x, float y,
                                   float u0, float v0, float w, float h,
                                   float tw, float th) {
        DrawContext ctx = hudDrawContext;
        if (ctx == null) return;
        syncGlState(); // MC-pipeline draw may follow raw GL from Rust
        Identifier ident = Identifier.tryParse(id);
        if (ident == null) return;
        // Standalone icon textures (e.g. vanilla mob-effect icons) are drawn
        // outside MC's own icon-rendering code, which normally resets the
        // shader color itself before blitting; without this reset the blit
        // inherits whatever tint an earlier draw call left behind and can
        // render fully invisible. Patchouli's own TextureIcon does the same.
        RenderSystem.setShaderColor(1.0f, 1.0f, 1.0f, 1.0f);
        ctx.drawTexture(ident, (int) x, (int) y, u0, v0,
                (int) w, (int) h, (int) tw, (int) th);
    }

    // Two dummy GL textures used to force-desync-proof texture rebinding in
    // syncGlState(): binding A then B guarantees a real glBindTexture happens
    // regardless of what GlStateManager's cache currently holds.
    private static int dummyTexA, dummyTexB;

    /**
     * Re-synchronize GlStateManager's cached GL state with actual GL state
     * after raw OpenGL calls from the Rust side. Without this, MC draws that
     * rely on cached state (item rendering, drawTexture) silently use stale
     * bindings and render nothing.
     */
    public static void syncGlState() {
        if (dummyTexA == 0) {
            dummyTexA = GL11.glGenTextures();
            dummyTexB = GL11.glGenTextures();
        }
        // Raw GL from Rust binds its own VAOs; BufferRenderer caches the last
        // bound VAO/VBO and skips the real glBindVertexArray when it thinks
        // nothing changed — the next MC draw then hits a foreign VAO and dies
        // with GL_INVALID_OPERATION in glDrawElements. Drop the cache.
        BufferRenderer.reset();
        // Texture unit 0: bind two distinct ids so the second bind is real.
        GlStateManager._activeTexture(GL13.GL_TEXTURE0);
        GlStateManager._bindTexture(dummyTexA);
        GlStateManager._bindTexture(dummyTexB);
        // Toggle boolean states both ways — ends real and cached in a known
        // state regardless of what the cache held before.
        RenderSystem.enableBlend();
        RenderSystem.disableBlend();
        RenderSystem.enableDepthTest();
        RenderSystem.disableDepthTest();
        RenderSystem.depthMask(false);
        RenderSystem.depthMask(true);
        RenderSystem.disableCull();
        RenderSystem.enableCull();
        RenderSystem.defaultBlendFunc();
    }

    /**
     * Render an item stack at GUI position (x, y) with the given on-screen
     * size in GUI pixels (16 = standard inventory icon). Renders 3D block
     * models exactly like inventory slots / Patchouli.
     */
    public static void drawItem(String id, float x, float y, float size) {
        DrawContext ctx = hudDrawContext;
        if (ctx == null) return;
        Identifier ident = Identifier.tryParse(id);
        if (ident == null) return;
        Item item = Registries.ITEM.get(ident);
        if (item == Items.AIR) return;
        syncGlState();
        float scale = size / 16.0f;
        ctx.getMatrices().push();
        ctx.getMatrices().scale(scale, scale, 1.0f);
        ctx.drawItem(new ItemStack(item), (int) (x / scale), (int) (y / scale));
        ctx.getMatrices().pop();
    }

    /**
     * Returns the OpenGL texture name for a Minecraft-managed texture, or 0 if
     * the texture has not been loaded yet.  Do NOT call glDeleteTextures on it.
     */
    public static int getMcTextureId(String id) {
        Identifier ident = Identifier.tryParse(id);
        if (ident == null) return 0;
        MinecraftClient mc = MinecraftClient.getInstance();
        // Missing resources resolve to the checkerboard texture; report 0 instead
        // so callers can try alternative paths (e.g. item/ vs block/ textures).
        if (mc.getResourceManager().getResource(ident).isEmpty()) return 0;
        AbstractTexture tex = mc.getTextureManager().getTexture(ident);
        return tex != null ? tex.getGlId() : 0;
    }
}
