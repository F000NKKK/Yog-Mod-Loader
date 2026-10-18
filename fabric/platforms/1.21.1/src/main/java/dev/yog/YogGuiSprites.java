package dev.yog;

import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.texture.MissingSprite;
import net.minecraft.util.Identifier;

/** The GUI sprite atlas (1.20.2+) for {@link NativeDraw#drawGuiSprite} — Minecraft 1.21.1. */
final class YogGuiSprites {
    private YogGuiSprites() {}

    static final boolean AVAILABLE = true;

    /** Whether the atlas has sprite `id` (the atlas answers the missing sprite otherwise). */
    static boolean has(Identifier id) {
        var sprite = MinecraftClient.getInstance().getGuiAtlasManager().getSprite(id);
        return !sprite.getContents().getId().equals(MissingSprite.getMissingSpriteId());
    }

    static void draw(DrawContext ctx, Identifier id, int x, int y, int w, int h) {
        ctx.drawGuiTexture(id, x, y, w, h);
    }
}
