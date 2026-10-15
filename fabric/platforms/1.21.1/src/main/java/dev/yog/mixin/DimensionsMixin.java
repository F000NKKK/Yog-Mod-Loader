package dev.yog.mixin;

import dev.yog.util.YogDimensions;
import net.minecraft.entity.Entity;
import net.minecraft.entity.EntityDimensions;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** An entity's own hitbox (`entity_set_dimensions`), after vanilla worked out its size. */
@Mixin(Entity.class)
public abstract class DimensionsMixin {
    @Shadow private EntityDimensions dimensions;
    @Shadow private float standingEyeHeight;

    @Inject(method = "calculateDimensions", at = @At("TAIL"))
    private void yog$size(CallbackInfo ci) {
        Entity self = (Entity) (Object) this;
        float[] size = YogDimensions.get(self.getWorld().isClient, self.getUuid());
        if (size == null) return;
        dimensions = EntityDimensions.changing(size[0], size[1]).withEyeHeight(size[2]);
        standingEyeHeight = size[2];
        self.setBoundingBox(dimensions.getBoxAt(self.getPos()));
    }
}
