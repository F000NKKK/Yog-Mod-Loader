package dev.yog.mixin;

import net.minecraft.world.entity.WalkAnimationState;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Accessor;

/** A walk's state, copied to a disguise's stand-in (yog-body). */
@Mixin(WalkAnimationState.class)
public interface WalkAnimationAccess {
    @Accessor("speedOld")
    float yog$speedOld();

    @Accessor("speedOld")
    void yog$setSpeedOld(float v);

    @Accessor("speed")
    float yog$speed();

    @Accessor("speed")
    void yog$setSpeed(float v);

    @Accessor("position")
    float yog$position();

    @Accessor("position")
    void yog$setPosition(float v);
}
