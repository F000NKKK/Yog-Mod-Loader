package dev.yog.client;

import net.minecraft.client.Minecraft;
import net.minecraft.client.renderer.texture.MissingTextureAtlasSprite;
import net.minecraft.data.AtlasIds;
import net.minecraft.resources.Identifier;

/** The GUI sprite atlas (1.20.2+) for {@link NativeDraw#drawGuiSprite} — Minecraft 26.2. */
public final class YogGuiSprites {
    private YogGuiSprites() {}

    public static final boolean AVAILABLE = true;

    /** Whether the atlas has sprite `id` (the atlas answers the missing sprite otherwise). */
    public static boolean has(Identifier id) {
        var sprite = Minecraft.getInstance().getAtlasManager().getAtlasOrThrow(AtlasIds.GUI).getSprite(id);
        return !sprite.contents().name().equals(MissingTextureAtlasSprite.getLocation());
    }
}
