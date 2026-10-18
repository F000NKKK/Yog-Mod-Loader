package dev.yog;

import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.renderer.RenderPipelines;
import net.minecraft.client.renderer.texture.MissingTextureAtlasSprite;
import net.minecraft.data.AtlasIds;
import net.minecraft.resources.Identifier;

/** The GUI sprite atlas (1.20.2+) for {@link NativeDraw#drawGuiSprite} — Minecraft 26.2. */
final class YogGuiSprites {
    private YogGuiSprites() {}

    static final boolean AVAILABLE = true;

    /** Whether the atlas has sprite `id` (the atlas answers the missing sprite otherwise). */
    static boolean has(Identifier id) {
        var sprite = Minecraft.getInstance().getAtlasManager().getAtlasOrThrow(AtlasIds.GUI).getSprite(id);
        return !sprite.contents().name().equals(MissingTextureAtlasSprite.getLocation());
    }

    /** Sprite `id` over (x, y, w, h), nine-sliced or tiled per its .mcmeta, colors times `tint` (0xAARRGGBB). */
    static void draw(GuiGraphicsExtractor ctx, Identifier id, int x, int y, int w, int h, int tint) {
        ctx.blitSprite(RenderPipelines.GUI_TEXTURED, id, x, y, w, h, tint);
    }
}
