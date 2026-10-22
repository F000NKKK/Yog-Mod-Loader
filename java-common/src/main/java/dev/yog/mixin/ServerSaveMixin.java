package dev.yog.mixin;

import dev.yog.bus.YogEvents;
import net.minecraft.server.MinecraftServer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * The world being saved — an autosave, {@code /save-all}, the server stopping
 * (each goes through {@code saveAllChunks}): mods write what's kept with the
 * world first, so it's saved with it.
 */
@Mixin(MinecraftServer.class)
public abstract class ServerSaveMixin {
    @Inject(method = "saveAllChunks", at = @At("HEAD"))
    private void yog$saving(boolean suppressLog, boolean flush, boolean force, CallbackInfoReturnable<Boolean> cir) {
        new YogEvents.Server.Life.Saving().emit();
    }
}
