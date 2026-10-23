package dev.yog.mixin;

import dev.yog.client.YogSounds;
import java.util.concurrent.CompletableFuture;
import net.minecraft.client.sounds.AudioStream;
import net.minecraft.client.sounds.SoundBufferLibrary;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** A streamed sound a mod started partway skips ahead ({@link YogSounds}). */
@Mixin(SoundBufferLibrary.class)
public class SoundStreamMixin {
    @Inject(method = "getStream", at = @At("RETURN"), cancellable = true)
    private void yog$skip(CallbackInfoReturnable<CompletableFuture<AudioStream>> cir) {
        cir.setReturnValue(YogSounds.skipped(cir.getReturnValue()));
    }
}
