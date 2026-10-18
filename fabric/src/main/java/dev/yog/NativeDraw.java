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
        if (dev.yog.util.YogText.rich(text)) {
            ctx.drawText(MinecraftClient.getInstance().textRenderer, styled(text, ""), (int) x, (int) y, color, shadow);
        } else {
            ctx.drawText(MinecraftClient.getInstance().textRenderer, text, (int) x, (int) y, color, shadow);
        }
    }


    /** text — its Yog rich-text spans as styles (see {@link dev.yog.util.YogText}) — in the font id font. */
    private static net.minecraft.text.Text styled(String text, String font) {
        Identifier id = YogIds.isDefaultFont(font) ? null : Identifier.tryParse(font);
        net.minecraft.text.MutableText out = net.minecraft.text.Text.empty();
        for (dev.yog.util.YogText.Run r : dev.yog.util.YogText.runs(text)) {
            net.minecraft.text.Style st = net.minecraft.text.Style.EMPTY;
            if (id != null) st = st.withFont(id);
            if (r.rgb() >= 0) st = st.withColor(net.minecraft.text.TextColor.fromRgb(r.rgb()));
            st = st.withBold(r.bold()).withItalic(r.italic()).withUnderline(r.underline())
                    .withStrikethrough(r.strike());
            out.append(net.minecraft.text.Text.literal(r.text()).setStyle(st));
        }
        return out;
    }

    /** drawText in a font: "minecraft:uniform", a resource-pack font id, ... */
    public static void drawTextFont(String text, String font, float x, float y, int color, boolean shadow) {
        if (YogIds.isDefaultFont(font)) {
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
        return YogIds.isDefaultFont(font) && !dev.yog.util.YogText.rich(text) ? tr.getWidth(text) : tr.getWidth(styled(text, font));
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

    // ── Dynamic textures, resource text, entities ─────────────────────────────

    /** A text resource (an `.svg`) as UTF-8, or "" when it's missing. */
    public static String readResourceText(String id) {
        Identifier ident = Identifier.tryParse(id);
        if (ident == null) return "";
        var res = MinecraftClient.getInstance().getResourceManager().getResource(ident);
        if (res.isEmpty()) return "";
        try (java.io.InputStream in = res.get().getInputStream()) {
            return new String(in.readAllBytes(), java.nio.charset.StandardCharsets.UTF_8);
        } catch (java.io.IOException e) {
            return "";
        }
    }

    /** Registers (or replaces) the texture {@code id} made of {@code w}×{@code h} RGBA pixels. */
    public static boolean registerTexture(String id, int w, int h, byte[] rgba) {
        Identifier ident = Identifier.tryParse(id);
        if (ident == null || w <= 0 || h <= 0 || rgba.length < w * h * 4) return false;
        net.minecraft.client.texture.NativeImage img = new net.minecraft.client.texture.NativeImage(w, h, false);
        for (int y = 0; y < h; y++) {
            for (int x = 0; x < w; x++) {
                int i = (y * w + x) * 4;
                int abgr = (rgba[i + 3] & 0xFF) << 24 | (rgba[i + 2] & 0xFF) << 16 | (rgba[i + 1] & 0xFF) << 8 | (rgba[i] & 0xFF);
                img.setColor(x, y, abgr);
            }
        }
        MinecraftClient.getInstance().getTextureManager()
                .registerTexture(ident, new net.minecraft.client.texture.NativeImageBackedTexture(img));
        return true;
    }

    /** Lets go of a texture {@link #registerTexture} made. */
    public static void releaseTexture(String id) {
        Identifier ident = Identifier.tryParse(id);
        if (ident != null) MinecraftClient.getInstance().getTextureManager().destroyTexture(ident);
    }

    /** One entity per type, made once, to draw. */
    private static final java.util.Map<String, net.minecraft.entity.LivingEntity> PREVIEWS = new java.util.HashMap<>();

    static net.minecraft.entity.LivingEntity preview(String type) {
        var world = MinecraftClient.getInstance().world;
        if (world == null) return null;
        var cached = PREVIEWS.get(type);
        if (cached != null && cached.getWorld() == world) return cached;
        Identifier ident = Identifier.tryParse(type);
        if (ident == null || !Registries.ENTITY_TYPE.containsId(ident)) return null;
        var made = Registries.ENTITY_TYPE.get(ident).create(world);
        if (!(made instanceof net.minecraft.entity.LivingEntity living)) return null;
        PREVIEWS.put(type, living);
        return living;
    }

    /**
     * A living entity of type {@code type} standing in (x, y, w, h) GUI
     * pixels, as big as fits, turned {@code yaw} degrees (0 = facing the
     * viewer) and tilted {@code pitch}.
     */
    public static void drawEntity(String type, float x, float y, float w, float h, float yaw, float pitch) {
        DrawContext ctx = hudDrawContext;
        if (ctx == null) return;
        var e = preview(type);
        if (e == null) return;
        float size = Math.min(h / Math.max(e.getHeight(), 0.1f), w / Math.max(e.getWidth() * 1.4f, 0.1f)) * 0.9f;
        e.bodyYaw = 180f + yaw;
        e.setYaw(180f + yaw);
        e.headYaw = 180f + yaw;
        e.prevHeadYaw = 180f + yaw;
        e.setPitch(0f);
        syncGlState();
        YogEntityPreview.draw(ctx, e, x + w / 2f, y, h, size, pitch);
    }

    // ── Scaled text, 3D blocks ────────────────────────────────────────────────

    /** Text {@code scale} times its size, top-left at (x, y). */
    public static void drawTextScaled(String text, String font, float x, float y, float scale, int color, boolean shadow) {
        DrawContext ctx = hudDrawContext;
        if (ctx == null) return;
        ctx.getMatrices().push();
        ctx.getMatrices().translate(x, y, 0f);
        ctx.getMatrices().scale(scale, scale, 1f);
        if (YogIds.isDefaultFont(font)) drawText(text, 0, 0, color, shadow);
        else drawTextFont(text, font, 0, 0, color, shadow);
        ctx.getMatrices().pop();
    }

    /**
     * Blocks in 3D ("x y z state" lines), as big as fits in (x, y, w, h),
     * turned {@code yaw} degrees about the vertical, tilted {@code pitch}.
     */
    public static void drawBlocks(String list, float x, float y, float w, float h, float yaw, float pitch) {
        DrawContext ctx = hudDrawContext;
        if (ctx == null) return;
        MinecraftClient mc = MinecraftClient.getInstance();
        java.util.List<Object[]> blocks = new java.util.ArrayList<>();
        int[] lo = {Integer.MAX_VALUE, Integer.MAX_VALUE, Integer.MAX_VALUE};
        int[] hi = {Integer.MIN_VALUE, Integer.MIN_VALUE, Integer.MIN_VALUE};
        for (String line : list.split("\n")) {
            String[] p = line.trim().split(" ", 4);
            if (p.length < 4) continue;
            try {
                int[] at = {Integer.parseInt(p[0]), Integer.parseInt(p[1]), Integer.parseInt(p[2])};
                var state = net.minecraft.command.argument.BlockArgumentParser
                        .block(Registries.BLOCK.getReadOnlyWrapper(), p[3], false).blockState();
                blocks.add(new Object[]{at, state});
                for (int i = 0; i < 3; i++) { lo[i] = Math.min(lo[i], at[i]); hi[i] = Math.max(hi[i], at[i]); }
            } catch (Exception e) {
                // An unknown block: left out.
            }
        }
        if (blocks.isEmpty()) return;
        float size = Math.max(hi[0] - lo[0], Math.max(hi[1] - lo[1], hi[2] - lo[2])) + 1;
        float s = Math.min(w, h) / (size * 1.7f);
        syncGlState();
        var m = ctx.getMatrices();
        m.push();
        m.translate(x + w / 2f, y + h / 2f, 150f);
        m.scale(s, -s, s);
        m.multiply(net.minecraft.util.math.RotationAxis.POSITIVE_X.rotationDegrees(pitch));
        m.multiply(net.minecraft.util.math.RotationAxis.POSITIVE_Y.rotationDegrees(yaw));
        m.translate(-(lo[0] + hi[0] + 1) / 2f, -(lo[1] + hi[1] + 1) / 2f, -(lo[2] + hi[2] + 1) / 2f);
        var buffers = mc.getBufferBuilders().getEntityVertexConsumers();
        var renderer = mc.getBlockRenderManager();
        net.minecraft.client.render.DiffuseLighting.disableGuiDepthLighting();
        for (Object[] b : blocks) {
            int[] at = (int[]) b[0];
            m.push();
            m.translate(at[0], at[1], at[2]);
            renderer.renderBlockAsEntity((net.minecraft.block.BlockState) b[1], m, buffers,
                    net.minecraft.client.render.LightmapTextureManager.MAX_LIGHT_COORDINATE,
                    net.minecraft.client.render.OverlayTexture.DEFAULT_UV);
            m.pop();
        }
        buffers.draw();
        net.minecraft.client.render.DiffuseLighting.enableGuiDepthLighting();
        m.pop();
    }
}
