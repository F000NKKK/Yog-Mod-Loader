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
        NativeBridge.nativeGlInit();  // no-op after first call; GL is active here
        NativeDraw.hudDrawContext = ctx;
        Minecraft mc = Minecraft.getInstance();
        var playerPos = mc.player != null ? mc.player.getEyePosition() : net.minecraft.world.phys.Vec3.ZERO;
        NativeBridge.nativeOnHudRender(
            delta.getGameTimeDeltaPartialTick(false),
            mc.getWindow().getGuiScaledWidth(),
            mc.getWindow().getGuiScaledHeight(),
            (float) mc.getWindow().getGuiScale(),
            (float) playerPos.x, (float) playerPos.y, (float) playerPos.z);
        NativeDraw.hudDrawContext = null;
        NativeDraw.syncGlState(); // raw GL from Rust desyncs GlStateManager caches
    }
}
