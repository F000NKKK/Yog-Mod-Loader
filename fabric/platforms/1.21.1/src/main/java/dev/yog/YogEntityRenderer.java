package dev.yog;

import net.minecraft.client.render.VertexConsumerProvider;
import net.minecraft.client.render.entity.EntityRenderer;
import net.minecraft.client.render.entity.EntityRendererFactory;
import net.minecraft.client.texture.MissingSprite;
import net.minecraft.client.util.math.MatrixStack;
import net.minecraft.entity.Entity;
import net.minecraft.util.Identifier;

/** Draws a mod's entity: its body as the mods give it ({@link YogBody#drawEntity}), then its name. */
public class YogEntityRenderer extends EntityRenderer<Entity> {
    public YogEntityRenderer(EntityRendererFactory.Context ctx) {
        super(ctx);
        this.shadowRadius = 0.4F;
    }

    @Override
    public void render(Entity entity, float yaw, float partial, MatrixStack pose, VertexConsumerProvider buffers, int light) {
        YogBody.drawEntity(entity, partial, pose, buffers, light);
        super.render(entity, yaw, partial, pose, buffers, light);
    }

    @Override
    public Identifier getTexture(Entity entity) {
        return MissingSprite.getMissingSpriteId();
    }
}
