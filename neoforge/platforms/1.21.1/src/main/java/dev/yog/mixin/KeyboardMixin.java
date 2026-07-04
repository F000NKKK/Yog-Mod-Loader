package dev.yog.mixin;

import dev.yog.NativeBridge;
import net.minecraft.client.KeyboardHandler;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

@Mixin(KeyboardHandler.class)
public abstract class KeyboardMixin {
    @Inject(method = "keyPress(JIIII)V", at = @At("HEAD"), cancellable = true)
    private void yog$onKey(long window, int keyCode, int scanCode, int action, int modifiers, CallbackInfo ci) {
        if (!NativeBridge.nativeOnKeyPress(keyCode, scanCode, action, modifiers)) {
            ci.cancel();
        }
    }
}
