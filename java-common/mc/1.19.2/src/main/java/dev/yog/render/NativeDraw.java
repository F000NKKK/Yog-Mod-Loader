package dev.yog.render;

import net.minecraft.core.Registry;
import dev.yog.YogIds;

import com.mojang.blaze3d.platform.GlStateManager;
import com.mojang.blaze3d.systems.RenderSystem;
import com.mojang.blaze3d.vertex.BufferUploader;

import net.minecraft.client.Minecraft;
import com.mojang.blaze3d.vertex.PoseStack;
import com.mojang.math.Vector3f;
import net.minecraft.client.gui.GuiComponent;
import net.minecraft.client.renderer.GameRenderer;
import net.minecraft.client.renderer.texture.AbstractTexture;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.resources.ResourceLocation;

import org.lwjgl.opengl.GL11;
import org.lwjgl.opengl.GL13;

/**
 * Client-only HUD draw helpers called from Rust via JNI during on_hud_render.
 */
@SuppressWarnings("deprecation") // BuiltInRegistries is the vanilla registry; Forge only nudges towards its wrappers
public final class NativeDraw {
    private NativeDraw() {}

    /** The pose of the current HUD render frame. Render-thread only. */
    public static PoseStack hudDrawContext;

    /** GuiComponent's gradient, which it keeps to its subclasses. */
    private static final class Gui extends GuiComponent {
        static void gradient(PoseStack pose, int x1, int y1, int x2, int y2, int top, int bottom) {
            fillGradient(pose, x1, y1, x2, y2, top, bottom, 0);
        }
    }

    // ── 2-D primitives ───────────────────────────────────────────────────────

    public static void drawText(String text, float x, float y, int color, boolean shadow) {
        PoseStack ctx = hudDrawContext;
        if (ctx == null) return;
        syncGlState(); // MC-pipeline draw may follow raw GL from Rust
        var font = Minecraft.getInstance().font;
        if (dev.yog.util.YogText.rich(text)) {
            var styled = styled(text, "");
            if (shadow) font.drawShadow(ctx, styled, (int) x, (int) y, color);
            else font.draw(ctx, styled, (int) x, (int) y, color);
        } else if (shadow) {
            font.drawShadow(ctx, text, (int) x, (int) y, color);
        } else {
            font.draw(ctx, text, (int) x, (int) y, color);
        }
    }

    /** text — its Yog rich-text spans as styles (see {@link dev.yog.util.YogText}) — in the font id font. */
    private static net.minecraft.network.chat.Component styled(String text, String font) {
        ResourceLocation id = YogIds.isDefaultFont(font) ? null : ResourceLocation.tryParse(font);
        net.minecraft.network.chat.MutableComponent out = net.minecraft.network.chat.Component.empty();
        for (dev.yog.util.YogText.Run r : dev.yog.util.YogText.runs(text)) {
            net.minecraft.network.chat.Style st = net.minecraft.network.chat.Style.EMPTY;
            if (id != null) st = st.withFont(id);
            if (r.rgb() >= 0) st = st.withColor(net.minecraft.network.chat.TextColor.fromRgb(r.rgb()));
            st = st.withBold(r.bold()).withItalic(r.italic()).withUnderlined(r.underline())
                    .withStrikethrough(r.strike());
            out.append(net.minecraft.network.chat.Component.literal(r.text()).withStyle(st));
        }
        return out;
    }

    /** drawText in a font: the game's id of it ("minecraft:uniform", a Yog font's) — the runtime resolved Yog's alias. */
    public static void drawTextFont(String text, String font, float x, float y, int color, boolean shadow) {
        if (YogIds.isDefaultFont(font)) {
            drawText(text, x, y, color, shadow);
            return;
        }
        PoseStack ctx = hudDrawContext;
        if (ctx == null) return;
        syncGlState(); // MC-pipeline draw may follow raw GL from Rust
        var styled = styled(text, font);
        if (shadow) Minecraft.getInstance().font.drawShadow(ctx, styled, (int) x, (int) y, color);
        else Minecraft.getInstance().font.draw(ctx, styled, (int) x, (int) y, color);
    }

    /** Width in GUI pixels of text in font, as the text renderer measures it. */
    public static float textWidth(String text, String font) {
        var f = Minecraft.getInstance().font;
        if (f == null) return 0f;
        return YogIds.isDefaultFont(font) && !dev.yog.util.YogText.rich(text) ? f.width(text) : f.width(styled(text, font));
    }

    public static void drawRect(float x1, float y1, float x2, float y2, int color) {
        PoseStack ctx = hudDrawContext;
        if (ctx == null) return;
        syncGlState(); // MC-pipeline draw may follow raw GL from Rust
        GuiComponent.fill(ctx, (int) x1, (int) y1, (int) x2, (int) y2, color);
    }

    public static void drawGradientRect(float x1, float y1, float x2, float y2, int top, int bottom) {
        PoseStack ctx = hudDrawContext;
        if (ctx == null) return;
        syncGlState(); // MC-pipeline draw may follow raw GL from Rust
        Gui.gradient(ctx, (int) x1, (int) y1, (int) x2, (int) y2, top, bottom);
    }

    private static int dummyTexA, dummyTexB;

    /**
     * Re-synchronize GlStateManager's cached GL state with actual GL state
     * after raw OpenGL calls from the Rust side.
     */
    public static void syncGlState() {
        if (dummyTexA == 0) {
            dummyTexA = GL11.glGenTextures();
            dummyTexB = GL11.glGenTextures();
        }
        // Raw GL from Rust binds its own VAOs; BufferUploader caches the last
        // bound VAO/VBO and skips the real glBindVertexArray when it thinks
        // nothing changed — the next MC draw then hits a foreign VAO and dies
        // with GL_INVALID_OPERATION in glDrawElements. Drop the cache.
        BufferUploader.reset();
        GlStateManager._activeTexture(GL13.GL_TEXTURE0);
        GlStateManager._bindTexture(dummyTexA);
        GlStateManager._bindTexture(dummyTexB);
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
     * size in GUI pixels (16 = standard inventory icon).
     */
    public static void drawItem(String id, float x, float y, float size) {
        // Its name on this version, if the game renamed it (`minecraft:chain` is `iron_chain` from 1.21.9).
        id = dev.yog.data.YogVersions.itemOr(id);
        PoseStack ctx = hudDrawContext;
        if (ctx == null) return;
        ResourceLocation ident = ResourceLocation.tryParse(id);
        if (ident == null) return;
        Item item = Registry.ITEM.get(ident);
        if (item == Items.AIR) return;
        syncGlState();
        float scale = size / dev.yog.util.YogGameNumbers.ITEM_PIXELS;
        ctx.pushPose();
        ctx.scale(scale, scale, 1.0f);
        renderItem(ctx, new ItemStack(item), (int) (x / scale), (int) (y / scale));
        ctx.popPose();
    }

    /**
     * Returns the OpenGL texture name for a Minecraft-managed texture, or 0.
     */
    public static int getMcTextureId(String id) {
        ResourceLocation ident = ResourceLocation.tryParse(id);
        if (ident == null) return 0;
        Minecraft mc = Minecraft.getInstance();
        if (mc.getResourceManager().getResource(ident).isEmpty()) return 0;
        AbstractTexture tex = mc.getTextureManager().getTexture(ident);
        return tex != null ? tex.getId() : 0;
    }

    // ── Yog image sources (resolved by the runtime, see images.rs) ──────────

    /** Whether this Minecraft has the GUI sprite atlas (1.20.2+). */
    /** Whether this Minecraft's GUI sprite atlas (1.20.2+) has sprite `id`: there's none before 1.20.2. */
    public static boolean hasGuiSprite(String id) {
        return false;
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
        PoseStack ctx = hudDrawContext;
        if (ctx == null) return;
        ResourceLocation ident = ResourceLocation.tryParse(id);
        if (ident == null) return;
        syncGlState();
        beginImage(tint);
        int x0 = Math.round(x), y0 = Math.round(y);
        // No GUI sprite atlas before 1.20.2.
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
        PoseStack ctx = hudDrawContext;
        if (ctx == null || w <= 0 || h <= 0 || uw <= 0 || vh <= 0) return;
        ResourceLocation ident = ResourceLocation.tryParse(id);
        if (ident == null) return;
        int rw = Math.max(1, Math.round(uw * SUBTEXEL));
        int rh = Math.max(1, Math.round(vh * SUBTEXEL));
        syncGlState();
        beginImage(tint);
        ctx.pushPose();
        ctx.translate(x, y, 0f);
        ctx.scale(w / rw, h / rh, 1f);
        RenderSystem.setShader(GameRenderer::getPositionTexShader);
        RenderSystem.setShaderTexture(0, ident);
        GuiComponent.blit(ctx, 0, 0, u * SUBTEXEL, v * SUBTEXEL, rw, rh,
                Math.round(tw * SUBTEXEL), Math.round(th * SUBTEXEL));
        ctx.popPose();
        endImage();
    }

    /**
     * An item (block = the id is a block's: its item form) through the item
     * renderer, scaled into (x, y, w, h) GUI pixels (16 x 16 = slot size).
     */
    public static void drawItemRect(String id, boolean block, float x, float y, float w, float h) {
        // Its name on this version, if the game renamed it (`minecraft:chain` is `iron_chain` from 1.21.9).
        id = dev.yog.data.YogVersions.itemOr(id);
        PoseStack ctx = hudDrawContext;
        if (ctx == null || w <= 0 || h <= 0) return;
        ResourceLocation ident = ResourceLocation.tryParse(id);
        if (ident == null) return;
        Item item = block ? Registry.BLOCK.get(ident).asItem() : Registry.ITEM.get(ident);
        if (item == Items.AIR) return;
        syncGlState();
        ctx.pushPose();
        ctx.translate(x, y, 0f);
        ctx.scale(w / 16f, h / 16f, 1f);
        renderItem(ctx, new ItemStack(item), 0, 0);
        ctx.popPose();
        // Items render with depth (3D models at z + 150): clear it so 2D UI
        // drawn next (stack counts, text) isn't hidden behind them.
        RenderSystem.clear(GL11.GL_DEPTH_BUFFER_BIT, Minecraft.ON_OSX);
    }

    /**
     * An item's GUI icon at (x, y) under pose: the item renderer draws on the
     * model-view stack, so pose is carried onto it for the draw.
     */
    private static void renderItem(PoseStack pose, ItemStack stack, int x, int y) {
        PoseStack view = RenderSystem.getModelViewStack();
        view.pushPose();
        view.mulPoseMatrix(pose.last().pose());
        RenderSystem.applyModelViewMatrix();
        Minecraft.getInstance().getItemRenderer().renderGuiItem(stack, x, y);
        view.popPose();
        RenderSystem.applyModelViewMatrix();
    }

    /** Pixel size of a PNG texture resource as (w << 16) | h; 0 if missing. */
    public static int textureSize(String id) {
        ResourceLocation ident = ResourceLocation.tryParse(id);
        if (ident == null) return 0;
        var res = Minecraft.getInstance().getResourceManager().getResource(ident);
        if (res.isEmpty()) return 0;
        try (java.io.InputStream in = res.get().open()) {
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
        ResourceLocation ident = ResourceLocation.tryParse(id);
        if (ident == null) return "";
        var res = Minecraft.getInstance().getResourceManager().getResource(ident);
        if (res.isEmpty()) return "";
        try (java.io.InputStream in = res.get().open()) {
            return new String(in.readAllBytes(), java.nio.charset.StandardCharsets.UTF_8);
        } catch (java.io.IOException e) {
            return "";
        }
    }

    /** Registers (or replaces) the texture {@code id} made of {@code w}×{@code h} RGBA pixels. */
    public static boolean registerTexture(String id, int w, int h, byte[] rgba) {
        ResourceLocation ident = ResourceLocation.tryParse(id);
        if (ident == null || !YogPixels.fits(w, h, rgba)) return false;
        com.mojang.blaze3d.platform.NativeImage img = new com.mojang.blaze3d.platform.NativeImage(w, h, false);
        YogPixels.fill(img, rgba);
        var tex = new net.minecraft.client.renderer.texture.DynamicTexture(img);
        Minecraft.getInstance().getTextureManager().register(ident, tex);
        DYNAMIC.put(id, tex);
        return true;
    }

    /** The textures {@link #registerTexture} made, by id. */
    private static final java.util.Map<String, net.minecraft.client.renderer.texture.DynamicTexture> DYNAMIC = new java.util.HashMap<>();

    /**
     * Texture {@code id}'s pixels replaced (a video's frame): kept and uploaded again
     * when its size is the same, else made anew.
     */
    public static boolean updateTexture(String id, int w, int h, byte[] rgba) {
        var tex = DYNAMIC.get(id);
        var img = tex == null ? null : tex.getPixels();
        if (img == null || img.getWidth() != w || img.getHeight() != h) return registerTexture(id, w, h, rgba);
        if (!YogPixels.fill(img, rgba)) return false;
        tex.upload();
        return true;
    }

    /** Lets go of a texture {@link #registerTexture} made. */
    public static void releaseTexture(String id) {
        DYNAMIC.remove(id);
        ResourceLocation ident = ResourceLocation.tryParse(id);
        if (ident != null) Minecraft.getInstance().getTextureManager().release(ident);
    }

    /** One entity per type, made once, to draw. */
    private static final java.util.Map<String, net.minecraft.world.entity.LivingEntity> PREVIEWS = new java.util.HashMap<>();

    static net.minecraft.world.entity.LivingEntity preview(String type) {
        var level = Minecraft.getInstance().level;
        if (level == null) return null;
        var cached = PREVIEWS.get(type);
        if (cached != null && cached.getLevel() == level) return cached;
        ResourceLocation ident = ResourceLocation.tryParse(type);
        if (ident == null || !Registry.ENTITY_TYPE.containsKey(ident)) return null;
        var made = Registry.ENTITY_TYPE.get(ident).create(level);
        if (!(made instanceof net.minecraft.world.entity.LivingEntity living)) return null;
        PREVIEWS.put(type, living);
        return living;
    }

    /**
     * A living entity of type {@code type} standing in (x, y, w, h) GUI
     * pixels, as big as fits, turned {@code yaw} degrees (0 = facing the
     * viewer) and tilted {@code pitch}.
     */
    public static void drawEntity(String type, float x, float y, float w, float h, float yaw, float pitch) {
        PoseStack ctx = hudDrawContext;
        if (ctx == null) return;
        var e = preview(type);
        if (e == null) return;
        float size = Math.min(h / Math.max(e.getBbHeight(), dev.yog.util.YogGameNumbers.ENTITY_MIN_SIZE), w / Math.max(e.getBbWidth() * dev.yog.util.YogGameNumbers.ENTITY_ARMS, dev.yog.util.YogGameNumbers.ENTITY_MIN_SIZE)) * dev.yog.util.YogGameNumbers.ENTITY_MARGIN;
        e.yBodyRot = 180f + yaw;
        e.setYRot(180f + yaw);
        e.yHeadRot = 180f + yaw;
        e.yHeadRotO = 180f + yaw;
        e.setXRot(0f);
        syncGlState();
        YogEntityPreview.render(ctx, x + w / 2f, y + h / 2f + e.getBbHeight() * size / 2f, size, pitch, e);
    }

    // ── Scaled text, 3D blocks ────────────────────────────────────────────────

    /** Text {@code scale} times its size, top-left at (x, y). */
    public static void drawTextScaled(String text, String font, float x, float y, float scale, int color, boolean shadow) {
        PoseStack ctx = hudDrawContext;
        if (ctx == null) return;
        ctx.pushPose();
        ctx.translate(x, y, 0f);
        ctx.scale(scale, scale, 1f);
        if (YogIds.isDefaultFont(font)) drawText(text, 0, 0, color, shadow);
        else drawTextFont(text, font, 0, 0, color, shadow);
        ctx.popPose();
    }

    /**
     * Blocks in 3D ("x y z state" lines), as big as fits in (x, y, w, h),
     * turned {@code yaw} degrees about the vertical, tilted {@code pitch}.
     */
    public static void drawBlocks(String list, float x, float y, float w, float h, float yaw, float pitch) {
        PoseStack ctx = hudDrawContext;
        if (ctx == null) return;
        Minecraft mc = Minecraft.getInstance();
        java.util.List<Object[]> blocks = new java.util.ArrayList<>();
        int[] lo = {Integer.MAX_VALUE, Integer.MAX_VALUE, Integer.MAX_VALUE};
        int[] hi = {Integer.MIN_VALUE, Integer.MIN_VALUE, Integer.MIN_VALUE};
        for (String line : list.split("\n")) {
            String[] p = line.trim().split(" ", 4);
            if (p.length < 4) continue;
            try {
                int[] at = {Integer.parseInt(p[0]), Integer.parseInt(p[1]), Integer.parseInt(p[2])};
                var state = net.minecraft.commands.arguments.blocks.BlockStateParser
                        .parseForBlock(Registry.BLOCK, p[3], false).blockState();
                blocks.add(new Object[]{at, state});
                for (int i = 0; i < 3; i++) { lo[i] = Math.min(lo[i], at[i]); hi[i] = Math.max(hi[i], at[i]); }
            } catch (Exception e) {
                // An unknown block: left out.
            }
        }
        if (blocks.isEmpty()) return;
        float size = Math.max(hi[0] - lo[0], Math.max(hi[1] - lo[1], hi[2] - lo[2])) + 1;
        float s = Math.min(w, h) / (size * dev.yog.util.YogGameNumbers.BLOCK_CORNERS);
        syncGlState();
        var m = ctx;
        m.pushPose();
        m.translate(x + w / 2f, y + h / 2f, 150f);
        m.scale(s, -s, s);
        m.mulPose(Vector3f.XP.rotationDegrees(pitch));
        m.mulPose(Vector3f.YP.rotationDegrees(yaw));
        m.translate(-(lo[0] + hi[0] + 1) / 2f, -(lo[1] + hi[1] + 1) / 2f, -(lo[2] + hi[2] + 1) / 2f);
        var buffers = mc.renderBuffers().bufferSource();
        var renderer = mc.getBlockRenderer();
        com.mojang.blaze3d.platform.Lighting.setupForFlatItems();
        for (Object[] b : blocks) {
            int[] at = (int[]) b[0];
            m.pushPose();
            m.translate(at[0], at[1], at[2]);
            renderer.renderSingleBlock((net.minecraft.world.level.block.state.BlockState) b[1], m, buffers,
                    net.minecraft.client.renderer.LightTexture.FULL_BRIGHT,
                    net.minecraft.client.renderer.texture.OverlayTexture.NO_OVERLAY);
            m.popPose();
        }
        buffers.endBatch();
        com.mojang.blaze3d.platform.Lighting.setupFor3DItems();
        m.popPose();
    }
}
