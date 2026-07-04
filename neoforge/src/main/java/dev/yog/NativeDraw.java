package dev.yog;

import com.mojang.blaze3d.platform.GlStateManager;
import com.mojang.blaze3d.systems.RenderSystem;

import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.GuiGraphics;
import net.minecraft.client.renderer.texture.AbstractTexture;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.ResourceLocation;

import org.lwjgl.opengl.GL11;
import org.lwjgl.opengl.GL13;

/**
 * Client-only HUD draw helpers called from Rust via JNI during on_hud_render.
 */
public final class NativeDraw {
    private NativeDraw() {}

    /** The GuiGraphics for the current HUD render frame. Render-thread only. */
    static GuiGraphics hudDrawContext;

    // ── 2-D primitives ───────────────────────────────────────────────────────

    public static void drawText(String text, float x, float y, int color, boolean shadow) {
        GuiGraphics ctx = hudDrawContext;
        if (ctx == null) return;
        ctx.drawString(Minecraft.getInstance().font, text, (int) x, (int) y, color);
        if (shadow) ctx.drawString(Minecraft.getInstance().font, text, (int) x + 1, (int) y + 1, 0x44000000);
    }

    public static void drawRect(float x1, float y1, float x2, float y2, int color) {
        GuiGraphics ctx = hudDrawContext;
        if (ctx == null) return;
        ctx.fill((int) x1, (int) y1, (int) x2, (int) y2, color);
    }

    public static void drawGradientRect(float x1, float y1, float x2, float y2, int top, int bottom) {
        GuiGraphics ctx = hudDrawContext;
        if (ctx == null) return;
        ctx.fillGradient((int) x1, (int) y1, (int) x2, (int) y2, top, bottom);
    }

    /** All coordinates and sizes in GUI pixels; u0/v0 and tw/th in texels. */
    public static void drawTexture(String id, float x, float y,
                                   float u0, float v0, float w, float h,
                                   float tw, float th) {
        GuiGraphics ctx = hudDrawContext;
        if (ctx == null) return;
        ResourceLocation ident = ResourceLocation.tryParse(id);
        if (ident == null) return;
        ctx.blit(ident, (int) x, (int) y, u0, v0, (int) w, (int) h, (int) tw, (int) th);
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
        GuiGraphics ctx = hudDrawContext;
        if (ctx == null) return;
        ResourceLocation ident = ResourceLocation.tryParse(id);
        if (ident == null) return;
        Item item = BuiltInRegistries.ITEM.get(ident);
        if (item == Items.AIR) return;
        syncGlState();
        float scale = size / 16.0f;
        ctx.pose().pushPose();
        ctx.pose().scale(scale, scale, 1.0f);
        ctx.renderItem(new ItemStack(item), (int) (x / scale), (int) (y / scale));
        ctx.pose().popPose();
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
}