package dev.yog.mixin;

import dev.yog.bus.YogEvents;
import net.minecraft.client.Minecraft;
import net.minecraft.client.MouseHandler;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** The wheel turned in the world goes past the mods first; one may keep the hotbar from moving. */
@Mixin(MouseHandler.class)
public abstract class ScrollMixin {
    @Shadow @Final private Minecraft minecraft;

    @Inject(method = "onScroll", at = @At("HEAD"), cancellable = true)
    private void yog$onScroll(long window, double dx, double dy, CallbackInfo ci) {
        if (minecraft.gui.screen() != null || minecraft.player == null || dy == 0) return;
        if (!new YogEvents.Client.Input.Scroll(dy).emitPre()) ci.cancel();
    }
}
