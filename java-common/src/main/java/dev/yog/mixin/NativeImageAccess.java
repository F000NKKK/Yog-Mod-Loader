package dev.yog.mixin;

import com.mojang.blaze3d.platform.NativeImage;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Accessor;

/** An image's pixels' address (dynamic textures are filled with one copy). */
@Mixin(NativeImage.class)
public interface NativeImageAccess {
    @Accessor("pixels")
    long yog$pixels();
}
