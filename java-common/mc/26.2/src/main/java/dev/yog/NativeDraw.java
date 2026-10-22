package dev.yog;

import com.mojang.blaze3d.opengl.GlTexture;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.renderer.RenderPipelines;
import net.minecraft.client.renderer.texture.AbstractTexture;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.Identifier;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;

/**
 * Client-only HUD draw helpers called from Rust via JNI during on_hud_render.
 *
 * {@link #hudDrawContext} is set by {@link YogClientEvents} immediately before
 * {@code nativeOnHudRender} and cleared immediately after. Since 1.21.6 the
 * GUI is first extracted (these calls record what to draw) and drawn after.
 */
public final class NativeDraw {
    private NativeDraw() {}

    /** An inventory slot's item: 16 GUI pixels square. */
    private static final float SLOT = 16f;

    /** The DrawContext for the current HUD render frame. Render-thread only. */
    static GuiGraphicsExtractor hudDrawContext;

    /**
     * Draws Yog UI {@code uiId} ({@code w}×{@code h}) into {@code ctx}: its
     * 2-D draws recorded there, its surfaces' raw GL onto the main target.
     */
    static void ui(GuiGraphicsExtractor ctx, String uiId, int w, int h) {
        hudDrawContext = ctx;
        boolean gl = YogGl.begin();
        NativeBridge.nativeUIRender(uiId, w, h);
        if (gl) YogGl.end();
        hudDrawContext = null;
    }

    // ── 2-D primitives ───────────────────────────────────────────────────────

    public static void drawText(String text, float x, float y, int color, boolean shadow) {
        GuiGraphicsExtractor ctx = hudDrawContext;
        if (ctx == null) return;
        if (dev.yog.util.YogText.rich(text)) {
            ctx.text(Minecraft.getInstance().font, styled(text, ""), (int) x, (int) y, opaque(color), shadow);
        } else {
            ctx.text(Minecraft.getInstance().font, text, (int) x, (int) y, opaque(color), shadow);
        }
    }

    /** Alpha bits a color must have some of to be drawn (the font drew colors without any opaque, as before 1.21.6). */
    private static final int ALPHA_SET = 0xFC000000;
    private static final int OPAQUE = 0xFF000000;

    /** A text color as the font took it before 1.21.6: no alpha at all means opaque. */
    private static int opaque(int color) {
        return (color & ALPHA_SET) == 0 ? color | OPAQUE : color;
    }

    /** text — its Yog rich-text spans as styles (see {@link dev.yog.util.YogText}) — in the font id font. */
    private static net.minecraft.network.chat.Component styled(String text, String font) {
        Identifier id = YogIds.isDefaultFont(font) ? null : Identifier.tryParse(font);
        net.minecraft.network.chat.MutableComponent out = net.minecraft.network.chat.Component.empty();
        for (dev.yog.util.YogText.Run r : dev.yog.util.YogText.runs(text)) {
            net.minecraft.network.chat.Style st = net.minecraft.network.chat.Style.EMPTY;
            if (id != null) st = st.withFont(new net.minecraft.network.chat.FontDescription.Resource(id));
            if (r.rgb() >= 0) st = st.withColor(net.minecraft.network.chat.TextColor.fromRgb(r.rgb()));
            st = st.withBold(r.bold()).withItalic(r.italic()).withUnderlined(r.underline())
                    .withStrikethrough(r.strike());
            out.append(net.minecraft.network.chat.Component.literal(r.text()).setStyle(st));
        }
        return out;
    }

    /** drawText in a font: the game's id of it ("minecraft:uniform", a Yog font's) — the runtime resolved Yog's alias. */
    public static void drawTextFont(String text, String font, float x, float y, int color, boolean shadow) {
        if (YogIds.isDefaultFont(font)) {
            drawText(text, x, y, color, shadow);
            return;
        }
        GuiGraphicsExtractor ctx = hudDrawContext;
        if (ctx == null) return;
        ctx.text(Minecraft.getInstance().font, styled(text, font), (int) x, (int) y, opaque(color), shadow);
    }

    /** Width in GUI pixels of text in font, as the text renderer measures it. */
    public static float textWidth(String text, String font) {
        var tr = Minecraft.getInstance().font;
        if (tr == null) return 0f;
        return YogIds.isDefaultFont(font) && !dev.yog.util.YogText.rich(text) ? tr.width(text) : tr.width(styled(text, font));
    }

    public static void drawRect(float x1, float y1, float x2, float y2, int color) {
        GuiGraphicsExtractor ctx = hudDrawContext;
        if (ctx == null) return;
        ctx.fill((int) x1, (int) y1, (int) x2, (int) y2, color);
    }

    public static void drawGradientRect(float x1, float y1, float x2, float y2, int top, int bottom) {
        GuiGraphicsExtractor ctx = hudDrawContext;
        if (ctx == null) return;
        ctx.fillGradient((int) x1, (int) y1, (int) x2, (int) y2, top, bottom);
    }

    /**
     * Render an item stack at GUI position (x, y) with the given on-screen
     * size in GUI pixels (16 = standard inventory icon). Renders 3D block
     * models exactly like inventory slots / Patchouli.
     */
    public static void drawItem(String id, float x, float y, float size) {
        // Its name on this version, if the game renamed it (`minecraft:chain` is `iron_chain` from 1.21.9).
        id = dev.yog.data.YogVersions.itemOr(id);
        GuiGraphicsExtractor ctx = hudDrawContext;
        if (ctx == null) return;
        Identifier ident = Identifier.tryParse(id);
        if (ident == null) return;
        Item item = BuiltInRegistries.ITEM.getValue(ident);
        if (item == Items.AIR) return;
        float scale = size / SLOT;
        ctx.pose().pushMatrix();
        ctx.pose().scale(scale, scale);
        ctx.item(new ItemStack(item), (int) (x / scale), (int) (y / scale));
        ctx.pose().popMatrix();
    }

    /**
     * Returns the OpenGL texture name for a Minecraft-managed texture, or 0 if
     * the texture has not been loaded yet.  Do NOT call glDeleteTextures on it.
     */
    public static int getMcTextureId(String id) {
        Identifier ident = Identifier.tryParse(id);
        if (ident == null) return 0;
        Minecraft mc = Minecraft.getInstance();
        // Missing resources resolve to the checkerboard texture; report 0 instead
        // so callers can try alternative paths (e.g. item/ vs block/ textures).
        if (mc.getResourceManager().getResource(ident).isEmpty()) return 0;
        AbstractTexture tex = mc.getTextureManager().getTexture(ident);
        return tex != null && tex.getTexture() instanceof GlTexture gl ? gl.glId() : 0;
    }

    // ── Yog image sources (resolved by the runtime, see images.rs) ──────────

    /** Whether this Minecraft's GUI sprite atlas (1.20.2+) has sprite `id`. */
    public static boolean hasGuiSprite(String id) {
        Identifier ident = Identifier.tryParse(id);
        return YogGuiSprites.AVAILABLE && ident != null && YogGuiSprites.has(ident);
    }

    /** A GUI-atlas sprite over (x, y, w, h), nine-sliced/tiled per its .mcmeta. */
    public static void drawGuiSprite(String id, float x, float y, float w, float h, int tint) {
        GuiGraphicsExtractor ctx = hudDrawContext;
        if (ctx == null || !YogGuiSprites.AVAILABLE) return;
        Identifier ident = Identifier.tryParse(id);
        if (ident == null) return;
        int x0 = Math.round(x), y0 = Math.round(y);
        ctx.blitSprite(RenderPipelines.GUI_TEXTURED, ident, x0, y0, Math.round(x + w) - x0, Math.round(y + h) - y0, tint);
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
        GuiGraphicsExtractor ctx = hudDrawContext;
        if (ctx == null || w <= 0 || h <= 0 || uw <= 0 || vh <= 0) return;
        Identifier ident = Identifier.tryParse(id);
        if (ident == null) return;
        int rw = Math.max(1, Math.round(uw * SUBTEXEL));
        int rh = Math.max(1, Math.round(vh * SUBTEXEL));
        ctx.pose().pushMatrix();
        ctx.pose().translate(x, y);
        ctx.pose().scale(w / rw, h / rh);
        ctx.blit(RenderPipelines.GUI_TEXTURED, ident, 0, 0, u * SUBTEXEL, v * SUBTEXEL, rw, rh,
                Math.round(tw * SUBTEXEL), Math.round(th * SUBTEXEL), tint);
        ctx.pose().popMatrix();
    }

    /**
     * An item (block = the id is a block's: its item form) through the item
     * renderer, scaled into (x, y, w, h) GUI pixels (16 x 16 = slot size).
     */
    public static void drawItemRect(String id, boolean block, float x, float y, float w, float h) {
        // Its name on this version, if the game renamed it (`minecraft:chain` is `iron_chain` from 1.21.9).
        id = dev.yog.data.YogVersions.itemOr(id);
        GuiGraphicsExtractor ctx = hudDrawContext;
        if (ctx == null || w <= 0 || h <= 0) return;
        Identifier ident = Identifier.tryParse(id);
        if (ident == null) return;
        Item item = block ? BuiltInRegistries.BLOCK.getValue(ident).asItem() : BuiltInRegistries.ITEM.getValue(ident);
        if (item == Items.AIR) return;
        ctx.pose().pushMatrix();
        ctx.pose().translate(x, y);
        ctx.pose().scale(w / SLOT, h / SLOT);
        ctx.item(new ItemStack(item), 0, 0);
        ctx.pose().popMatrix();
    }

    /** Pixel size of a PNG texture resource as (w << 16) | h; 0 if missing. */
    public static int textureSize(String id) {
        Identifier ident = Identifier.tryParse(id);
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
        Identifier ident = Identifier.tryParse(id);
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
        Identifier ident = Identifier.tryParse(id);
        if (ident == null || w <= 0 || h <= 0 || rgba.length < w * h * 4) return false;
        com.mojang.blaze3d.platform.NativeImage img = new com.mojang.blaze3d.platform.NativeImage(w, h, false);
        for (int y = 0; y < h; y++) {
            for (int x = 0; x < w; x++) {
                int i = (y * w + x) * 4;
                int abgr = (rgba[i + 3] & 0xFF) << 24 | (rgba[i + 2] & 0xFF) << 16 | (rgba[i + 1] & 0xFF) << 8 | (rgba[i] & 0xFF);
                img.setPixelABGR(x, y, abgr);
            }
        }
        Minecraft.getInstance().getTextureManager()
                .register(ident, new net.minecraft.client.renderer.texture.DynamicTexture(ident::toString, img));
        return true;
    }

    /** Lets go of a texture {@link #registerTexture} made. */
    public static void releaseTexture(String id) {
        Identifier ident = Identifier.tryParse(id);
        if (ident != null) Minecraft.getInstance().getTextureManager().release(ident);
    }

    /** One entity per type, made once, to draw. */
    private static final java.util.Map<String, net.minecraft.world.entity.LivingEntity> PREVIEWS = new java.util.HashMap<>();

    static net.minecraft.world.entity.LivingEntity preview(String type) {
        var world = Minecraft.getInstance().level;
        if (world == null) return null;
        var cached = PREVIEWS.get(type);
        if (cached != null && cached.level() == world) return cached;
        Identifier ident = Identifier.tryParse(type);
        if (ident == null || !BuiltInRegistries.ENTITY_TYPE.containsKey(ident)) return null;
        var made = BuiltInRegistries.ENTITY_TYPE.getValue(ident).create(world, net.minecraft.world.entity.EntitySpawnReason.LOAD);
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
        GuiGraphicsExtractor ctx = hudDrawContext;
        if (ctx == null) return;
        var e = preview(type);
        if (e == null) return;
        float size = Math.min(h / Math.max(e.getBbHeight(), dev.yog.util.YogGameNumbers.ENTITY_MIN_SIZE), w / Math.max(e.getBbWidth() * dev.yog.util.YogGameNumbers.ENTITY_ARMS, dev.yog.util.YogGameNumbers.ENTITY_MIN_SIZE)) * dev.yog.util.YogGameNumbers.ENTITY_MARGIN;
        YogEntityPreview.draw(ctx, e, new YogEntityPreview.Shot(x, y, w, h, size, yaw, pitch));
    }

    // ── Scaled text, 3D blocks ────────────────────────────────────────────────

    /** How much room the blocks leave around them: their size times this fits the box. */
    private static final float BLOCKS_MARGIN = 1.7f;

    /** Text {@code scale} times its size, top-left at (x, y). */
    public static void drawTextScaled(String text, String font, float x, float y, float scale, int color, boolean shadow) {
        GuiGraphicsExtractor ctx = hudDrawContext;
        if (ctx == null) return;
        ctx.pose().pushMatrix();
        ctx.pose().translate(x, y);
        ctx.pose().scale(scale, scale);
        if (YogIds.isDefaultFont(font)) drawText(text, 0, 0, color, shadow);
        else drawTextFont(text, font, 0, 0, color, shadow);
        ctx.pose().popMatrix();
    }

    /**
     * Blocks in 3D ("x y z state" lines), as big as fits in (x, y, w, h),
     * turned {@code yaw} degrees about the vertical, tilted {@code pitch}.
     */
    public static void drawBlocks(String list, float x, float y, float w, float h, float yaw, float pitch) {
        GuiGraphicsExtractor ctx = hudDrawContext;
        if (ctx == null) return;
        java.util.List<YogBlocksPip.Placed> blocks = new java.util.ArrayList<>();
        int[] lo = {Integer.MAX_VALUE, Integer.MAX_VALUE, Integer.MAX_VALUE};
        int[] hi = {Integer.MIN_VALUE, Integer.MIN_VALUE, Integer.MIN_VALUE};
        for (String line : list.split("\n")) {
            String[] p = line.trim().split(" ", 4);
            if (p.length < 4) continue;
            try {
                int[] at = {Integer.parseInt(p[0]), Integer.parseInt(p[1]), Integer.parseInt(p[2])};
                var state = net.minecraft.commands.arguments.blocks.BlockStateParser
                        .parseForBlock(BuiltInRegistries.BLOCK, p[3], false).blockState();
                blocks.add(new YogBlocksPip.Placed(at[0], at[1], at[2], state));
                for (int i = 0; i < 3; i++) { lo[i] = Math.min(lo[i], at[i]); hi[i] = Math.max(hi[i], at[i]); }
            } catch (Exception e) {
                // An unknown block: left out.
            }
        }
        if (blocks.isEmpty()) return;
        float size = Math.max(hi[0] - lo[0], Math.max(hi[1] - lo[1], hi[2] - lo[2])) + 1;
        float s = Math.min(w, h) / (size * BLOCKS_MARGIN);
        int x0 = Math.round(x), y0 = Math.round(y), x1 = Math.round(x + w), y1 = Math.round(y + h);
        var scissor = ctx.scissorStack.peek();
        ctx.guiRenderState.addPicturesInPictureState(new YogBlocksPip.State(blocks,
                (lo[0] + hi[0] + 1) / 2f, (lo[1] + hi[1] + 1) / 2f, (lo[2] + hi[2] + 1) / 2f, yaw, pitch, s,
                x0, y0, x1, y1, scissor, net.minecraft.client.renderer.state.gui.pip.PictureInPictureRenderState.getBounds(x0, y0, x1, y1, scissor)));
    }
}
