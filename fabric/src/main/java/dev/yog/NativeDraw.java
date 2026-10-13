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

    /** True for "" and minecraft:default — the plain text path. */
    private static boolean isDefaultFont(String font) {
        return font == null || font.isEmpty() || font.equals("minecraft:default");
    }

    /** text as a literal in the font id font (unparsable ids: the default font). */
    private static net.minecraft.text.Text styled(String text, String font) {
        Identifier id = Identifier.tryParse(font);
        net.minecraft.text.MutableText t = net.minecraft.text.Text.literal(text);
        return id == null ? t : t.setStyle(net.minecraft.text.Style.EMPTY.withFont(id));
    }

    /** drawText in a font: "minecraft:uniform", a resource-pack font id, ... */
    public static void drawTextFont(String text, String font, float x, float y, int color, boolean shadow) {
        if (isDefaultFont(font)) {
            drawText(text, x, y, color, shadow);
            return;
        }
        DrawContext ctx = hudDrawContext;
        if (ctx == null) return;
        syncGlState(); // MC-pipeline draw may follow raw GL from Rust
        ctx.drawText(MinecraftClient.getInstance().textRenderer,
                styled(text, font), (int) x, (int) y, color, shadow);
    }

    /** Width in GUI pixels of text in font, as the text renderer measures it. */
    public static float textWidth(String text, String font) {
        var tr = MinecraftClient.getInstance().textRenderer;
        if (tr == null) return 0f;
        return isDefaultFont(font) ? tr.getWidth(text) : tr.getWidth(styled(text, font));
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

    // Two dummy GL textures used to force-desync-proof texture rebinding in
    // syncGlState(): binding A then B guarantees a real glBindTexture happens
    // regardless of what GlStateManager's cache currently holds.
    private static int dummyTexA, dummyTexB;

    /**
     * Re-synchronize GlStateManager's cached GL state with actual GL state
     * after raw OpenGL calls from the Rust side. Without this, MC draws that
     * rely on cached state (item rendering, texture blits) silently use stale
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

    // ── Yog image sources (resolved by the runtime, see images.rs) ──────────

    /** Whether this Minecraft has the GUI sprite atlas (1.20.2+). */
    public static boolean hasGuiSprites() {
        return YogGuiSprites.AVAILABLE;
    }

    private static void beginImage(int tint) {
        RenderSystem.enableBlend();
        RenderSystem.defaultBlendFunc();
        RenderSystem.setShaderColor(((tint >> 16) & 0xFF) / 255f, ((tint >> 8) & 0xFF) / 255f,
                (tint & 0xFF) / 255f, ((tint >>> 24) & 0xFF) / 255f);
    }

    private static void endImage() {
        RenderSystem.setShaderColor(1.0f, 1.0f, 1.0f, 1.0f);
        RenderSystem.disableBlend();
    }

    /** A GUI-atlas sprite over (x, y, w, h), nine-sliced/tiled per its .mcmeta. */
    public static void drawGuiSprite(String id, float x, float y, float w, float h, int tint) {
        DrawContext ctx = hudDrawContext;
        if (ctx == null || !hasGuiSprites()) return;
        Identifier ident = Identifier.tryParse(id);
        if (ident == null) return;
        syncGlState();
        beginImage(tint);
        int x0 = Math.round(x), y0 = Math.round(y);
        YogGuiSprites.draw(ctx, ident, x0, y0, Math.round(x + w) - x0, Math.round(y + h) - y0);
        endImage();
    }

    /**
     * Texel coordinates go to the blit as ints times this, so a region may
     * start or end between texels (an image cut by a clip rect).
     */
    private static final int SUBTEXEL = 64;

    /**
     * The texture region (u, v, uw, vh) (texels of a tw x th texture) scaled
     * into (x, y, w, h) GUI pixels, colors multiplied by tint (0xAARRGGBB).
     */
    public static void drawImage(String id, float x, float y, float w, float h,
                                 float u, float v, float uw, float vh,
                                 float tw, float th, int tint) {
        DrawContext ctx = hudDrawContext;
        if (ctx == null || w <= 0 || h <= 0 || uw <= 0 || vh <= 0) return;
        Identifier ident = Identifier.tryParse(id);
        if (ident == null) return;
        int rw = Math.max(1, Math.round(uw * SUBTEXEL));
        int rh = Math.max(1, Math.round(vh * SUBTEXEL));
        syncGlState();
        beginImage(tint);
        ctx.getMatrices().push();
        ctx.getMatrices().translate(x, y, 0f);
        ctx.getMatrices().scale(w / rw, h / rh, 1f);
        ctx.drawTexture(ident, 0, 0, u * SUBTEXEL, v * SUBTEXEL, rw, rh,
                Math.round(tw * SUBTEXEL), Math.round(th * SUBTEXEL));
        ctx.getMatrices().pop();
        endImage();
    }

    /**
     * An item (block = the id is a block's: its item form) through the item
     * renderer, scaled into (x, y, w, h) GUI pixels (16 x 16 = slot size).
     */
    public static void drawItemRect(String id, boolean block, float x, float y, float w, float h) {
        DrawContext ctx = hudDrawContext;
        if (ctx == null || w <= 0 || h <= 0) return;
        Identifier ident = Identifier.tryParse(id);
        if (ident == null) return;
        Item item = block ? Registries.BLOCK.get(ident).asItem() : Registries.ITEM.get(ident);
        if (item == Items.AIR) return;
        syncGlState();
        ctx.getMatrices().push();
        ctx.getMatrices().translate(x, y, 0f);
        ctx.getMatrices().scale(w / 16f, h / 16f, 1f);
        ctx.drawItem(new ItemStack(item), 0, 0);
        ctx.getMatrices().pop();
        // Items render with depth (3D models at z + 150): clear it so 2D UI
        // drawn next (stack counts, text) isn't hidden behind them.
        RenderSystem.clear(GL11.GL_DEPTH_BUFFER_BIT, MinecraftClient.IS_SYSTEM_MAC);
    }

    /** Pixel size of a PNG texture resource as (w << 16) | h; 0 if missing. */
    public static int textureSize(String id) {
        Identifier ident = Identifier.tryParse(id);
        if (ident == null) return 0;
        var res = MinecraftClient.getInstance().getResourceManager().getResource(ident);
        if (res.isEmpty()) return 0;
        try (java.io.InputStream in = res.get().getInputStream()) {
            byte[] head = in.readNBytes(24);
            // PNG signature, then the IHDR chunk: width and height, big-endian.
            if (head.length < 24 || head[0] != (byte) 0x89 || head[1] != 'P') return 0;
            int w = java.nio.ByteBuffer.wrap(head, 16, 4).getInt();
            int h = java.nio.ByteBuffer.wrap(head, 20, 4).getInt();
            if (w <= 0 || h <= 0 || w > 0xFFFF || h > 0xFFFF) return 0;
            return (w << 16) | h;
        } catch (java.io.IOException e) {
            return 0;
        }
    }
}
