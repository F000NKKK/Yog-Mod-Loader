package dev.yog.mixin;

import dev.yog.YogBodyState;
import net.minecraft.client.renderer.entity.state.EntityRenderState;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;

/** yog-body's fields on every entity render state ({@link YogBodyState}). */
@Mixin(EntityRenderState.class)
public class EntityRenderStateMixin implements YogBodyState {
    @Unique
    private Object yog$frame;
    @Unique
    private float yog$bodyYaw;

    @Override
    public Object yog$frame() {
        return yog$frame;
    }

    @Override
    public void yog$setFrame(Object frame) {
        yog$frame = frame;
    }

    @Override
    public float yog$bodyYaw() {
        return yog$bodyYaw;
    }

    @Override
    public void yog$setBodyYaw(float yaw) {
        yog$bodyYaw = yaw;
    }
}
