package dev.yog;

import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.renderer.entity.state.LivingEntityRenderState;
import net.minecraft.world.entity.LivingEntity;
import org.joml.Quaternionf;
import org.joml.Vector3f;

/** Draws an entity in a GUI — {@link NativeDraw#drawEntity}'s part that differs by version. */
final class YogEntityPreview {
    private YogEntityPreview() {}

    /** Facing the viewer: a body turned half round. */
    private static final float FACING_VIEWER = 180f;

    /**
     * {@code e} standing in the box (x, y, w, h), {@code size} px per block,
     * turned {@code yaw} degrees (0 = facing the viewer) and tilted {@code pitch}.
     */
    static void draw(GuiGraphicsExtractor ctx, LivingEntity e, float x, float y, float w, float h, float size, float yaw, float pitch) {
        Quaternionf tilt = new Quaternionf().rotateX((float) Math.toRadians(pitch));
        Quaternionf rotation = new Quaternionf().rotateZ((float) Math.PI).mul(tilt);
        var renderer = Minecraft.getInstance().getEntityRenderDispatcher().getRenderer(e);
        var state = renderer.createRenderState(e, 1.0F);
        state.shadowPieces.clear();
        state.outlineColor = 0;
        if (state instanceof LivingEntityRenderState living) {
            living.bodyRot = FACING_VIEWER + yaw;
            living.yRot = 0f;
            living.xRot = 0f;
        }
        Vector3f translation = new Vector3f(0f, state.boundingBoxHeight / 2f, 0f);
        ctx.entity(state, size, translation, rotation, tilt, Math.round(x), Math.round(y), Math.round(x + w), Math.round(y + h));
    }
}
