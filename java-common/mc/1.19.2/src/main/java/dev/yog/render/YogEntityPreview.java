package dev.yog.render;

import com.mojang.blaze3d.platform.Lighting;
import com.mojang.blaze3d.systems.RenderSystem;
import com.mojang.blaze3d.vertex.PoseStack;
import com.mojang.math.Quaternion;
import com.mojang.math.Vector3f;

import net.minecraft.client.Minecraft;
import net.minecraft.client.renderer.LightTexture;
import net.minecraft.world.entity.LivingEntity;

/**
 * A living entity drawn in the GUI, as the inventory screen draws the
 * player — but tilted by any pitch, and placed under the HUD's pose (this
 * version's inventory renderer takes neither).
 */
final class YogEntityPreview {
    private YogEntityPreview() {}

    /** How far towards the viewer the model-view puts the entity, as the inventory screen does. */
    private static final float VIEW_DEPTH = 1050f;
    /** How far back the entity's own pose puts it, as the inventory screen does. */
    private static final float POSE_DEPTH = 1000f;
    /** The turn that stands a model upright on screen. */
    private static final float UPRIGHT = 180f;

    /** e at (x, y) under pose (its feet), size GUI pixels a block, tilted pitch degrees. */
    static void render(PoseStack pose, float x, float y, float size, float pitch, LivingEntity e) {
        PoseStack view = RenderSystem.getModelViewStack();
        view.pushPose();
        view.mulPoseMatrix(pose.last().pose());
        view.translate(x, y, VIEW_DEPTH);
        view.scale(1f, 1f, -1f);
        RenderSystem.applyModelViewMatrix();
        PoseStack own = new PoseStack();
        own.translate(0f, 0f, POSE_DEPTH);
        own.scale(size, size, size);
        Quaternion upright = Vector3f.ZP.rotationDegrees(UPRIGHT);
        Quaternion tilt = Vector3f.XP.rotationDegrees(pitch);
        upright.mul(tilt);
        own.mulPose(upright);
        Lighting.setupForEntityInInventory();
        var dispatcher = Minecraft.getInstance().getEntityRenderDispatcher();
        tilt.conj();
        dispatcher.overrideCameraOrientation(tilt);
        dispatcher.setRenderShadow(false);
        var buffers = Minecraft.getInstance().renderBuffers().bufferSource();
        RenderSystem.runAsFancy(() -> dispatcher.render(e, 0, 0, 0, 0f, 1f, own, buffers, LightTexture.FULL_BRIGHT));
        buffers.endBatch();
        dispatcher.setRenderShadow(true);
        view.popPose();
        RenderSystem.applyModelViewMatrix();
        Lighting.setupFor3DItems();
    }
}
