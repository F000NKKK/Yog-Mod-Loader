package dev.yog.mixin;

import net.minecraft.entity.LimbAnimator;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Accessor;

/** A walk's state, copied to a disguise's stand-in (yog-body). */
@Mixin(LimbAnimator.class)
public interface LimbAnimatorAccess {
    @Accessor("prevSpeed")
    float yog$prevSpeed();

    @Accessor("prevSpeed")
    void yog$setPrevSpeed(float v);

    @Accessor("speed")
    float yog$speed();

    @Accessor("speed")
    void yog$setSpeed(float v);

    @Accessor("pos")
    float yog$pos();

    @Accessor("pos")
    void yog$setPos(float v);
}
