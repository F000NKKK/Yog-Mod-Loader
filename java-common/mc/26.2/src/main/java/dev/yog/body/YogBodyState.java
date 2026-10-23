package dev.yog.body;

/**
 * What yog-body keeps on an entity's render state between extracting it and
 * drawing it: the mods' answer for the frame and a mod entity's body yaw.
 * Every {@code EntityRenderState} has it ({@code mixin.EntityRenderStateMixin}),
 * on every loader.
 */
public interface YogBodyState {
    Object yog$frame();

    void yog$setFrame(Object frame);

    float yog$bodyYaw();

    void yog$setBodyYaw(float yaw);
}
