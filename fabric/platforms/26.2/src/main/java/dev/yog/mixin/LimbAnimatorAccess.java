package dev.yog.mixin;

import net.minecraft.world.entity.WalkAnimationState;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Accessor;

/** A walk's state, copied to a disguise's stand-in (yog-body). */
@Mixin(WalkAnimationState.class)
public interface LimbAnimatorAccess {
    @Accessor("speedOld")
    float yog$prevSpeed();

    @Accessor("speedOld")
    void yog$setPrevSpeed(float v);

    @Accessor("speed")
    float yog$speed();

    @Accessor("speed")
    void yog$setSpeed(float v);

    @Accessor("position")
    float yog$pos();

    @Accessor("position")
    void yog$setPos(float v);
}
