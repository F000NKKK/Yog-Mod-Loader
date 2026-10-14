package dev.yog;

import net.minecraft.client.gui.DrawContext;
import net.minecraft.entity.LivingEntity;

/** Draws an entity in a GUI — {@link NativeDraw#drawEntity}'s part that differs by version. */
final class YogEntityPreview {
    private YogEntityPreview() {}

    /** {@code e} centred on {@code cx} in the box from {@code y}, {@code h} high, {@code size} px per block. */
    static void draw(DrawContext ctx, LivingEntity e, float cx, float y, float h, float size, float pitch) {
        org.joml.Quaternionf pose = new org.joml.Quaternionf().rotateZ((float) Math.PI);
        pose.mul(new org.joml.Quaternionf().rotateX(pitch * (float) (Math.PI / 180.0)));
        net.minecraft.client.gui.screen.ingame.InventoryScreen.drawEntity(
                ctx, cx, y + h / 2f, size, new org.joml.Vector3f(0f, e.getHeight() / 2f, 0f), pose, null, e);
    }
}
