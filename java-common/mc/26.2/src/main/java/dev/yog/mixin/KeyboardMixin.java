package dev.yog.mixin;

import dev.yog.bus.YogEvents;
import net.minecraft.client.KeyboardHandler;
import net.minecraft.client.input.KeyEvent;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Every key press goes past the mods first; one may swallow it. */
@Mixin(KeyboardHandler.class)
public abstract class KeyboardMixin {
    @Inject(method = "keyPress", at = @At("HEAD"), cancellable = true)
    private void yog$onKey(long window, int action, KeyEvent event, CallbackInfo ci) {
        if (!new YogEvents.Client.Input.Key(event.key(), event.scancode(), action, event.modifiers()).emitPre()) {
            ci.cancel();
        }
    }
}
