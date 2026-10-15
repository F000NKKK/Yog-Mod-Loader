package dev.yog;

import com.mojang.blaze3d.vertex.PoseStack;
import net.minecraft.client.renderer.MultiBufferSource;
import net.minecraft.client.renderer.entity.EntityRenderer;
import net.minecraft.client.renderer.entity.EntityRendererProvider;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.world.entity.Entity;

/** Draws a mod's entity: its body as the mods give it ({@link YogBody#drawEntity}), then its name. */
public class YogEntityRenderer extends EntityRenderer<Entity> {
    public YogEntityRenderer(EntityRendererProvider.Context ctx) {
        super(ctx);
        this.shadowRadius = 0.4F;
    }

    @Override
    public void render(Entity entity, float yaw, float partial, PoseStack pose, MultiBufferSource buffers, int light) {
        YogBody.drawEntity(entity, partial, pose, buffers, light);
        super.render(entity, yaw, partial, pose, buffers, light);
    }

    @Override
    public ResourceLocation getTextureLocation(Entity entity) {
        return net.minecraft.client.renderer.texture.MissingTextureAtlasSprite.getLocation();
    }
}
