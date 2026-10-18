package dev.yog.mixin;

import dev.yog.NativeBridge;
import dev.yog.NativeDraw;
import net.minecraft.client.DeltaTracker;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.Gui;
import net.minecraft.client.gui.GuiGraphics;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Forge 1.21 dropped RenderGuiEvent, so the HUD hook (which also performs the
 * deferred nativeGlInit on the render thread) is injected straight into
 * Gui.render instead.
 */
@Mixin(Gui.class)
public abstract class HudRenderMixin {

    @Inject(method = "render", at = @At("TAIL"))
    private void yog$onHudRender(GuiGraphics ctx, DeltaTracker delta, CallbackInfo ci) {
        dev.yog.YogClientEvents.hud(ctx, delta.getGameTimeDeltaPartialTick(false));
    }
}
