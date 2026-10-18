package dev.yog;

import com.mojang.blaze3d.vertex.PoseStack;
import net.minecraft.client.renderer.SubmitNodeCollector;
import net.minecraft.client.renderer.entity.EntityRenderer;
import net.minecraft.client.renderer.entity.EntityRendererProvider;
import net.minecraft.client.renderer.entity.state.EntityRenderState;
import net.minecraft.client.renderer.state.level.CameraRenderState;
import net.minecraft.world.entity.Entity;

/** Draws a mod's entity: its body as the mods give it ({@link YogBody#drawEntity}), then its name. */
public class YogEntityRenderer extends EntityRenderer<Entity, EntityRenderState> {
    /** A shadow's radius under a mod's entity; a hanging thing casts none, as a painting. */
    private static final float SHADOW = 0.4F;

    public YogEntityRenderer(EntityRendererProvider.Context ctx, dev.yog.data.YogContent.Entity def) {
        super(ctx);
        this.shadowRadius = def != null && def.hanging() ? 0.0F : SHADOW;
    }

    @Override
    public EntityRenderState createRenderState() {
        return new EntityRenderState();
    }

    @Override
    public void extractRenderState(Entity entity, EntityRenderState state, float partial) {
        super.extractRenderState(entity, state, partial);
        YogBody.extractEntity(entity, state, partial);
    }

    @Override
    public void submit(EntityRenderState state, PoseStack pose, SubmitNodeCollector out, CameraRenderState camera) {
        YogBody.drawEntity(state, pose, out);
        super.submit(state, pose, out, camera);
    }
}
