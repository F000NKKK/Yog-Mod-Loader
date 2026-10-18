package dev.yog.mixin;

import dev.yog.util.YogDimensions;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.EntityDimensions;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** An entity's own hitbox (`entity_set_dimensions`), after vanilla worked out its size. */
@Mixin(Entity.class)
public abstract class DimensionsMixin {
    @Shadow private EntityDimensions dimensions;
    @Shadow private float eyeHeight;

    @Inject(method = "refreshDimensions", at = @At("TAIL"))
    private void yog$size(CallbackInfo ci) {
        Entity self = (Entity) (Object) this;
        float[] size = YogDimensions.get(self.level().isClientSide(), self.getUUID());
        if (size == null) return;
        dimensions = EntityDimensions.scalable(size[0], size[1]).withEyeHeight(size[2]);
        eyeHeight = size[2];
        self.setBoundingBox(dimensions.makeBoundingBox(self.position()));
    }
}
