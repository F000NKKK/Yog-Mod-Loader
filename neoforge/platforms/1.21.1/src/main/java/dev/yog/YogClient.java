package dev.yog;

import net.minecraft.client.Minecraft;
import net.minecraft.resources.ResourceLocation;
import net.neoforged.neoforge.client.event.RenderGuiEvent;
import net.neoforged.neoforge.client.event.RenderLevelStageEvent;
import net.neoforged.neoforge.client.event.ScreenEvent;
import net.neoforged.neoforge.client.event.ClientTickEvent;
import net.neoforged.bus.api.SubscribeEvent;
import net.neoforged.fml.common.Mod;
import net.neoforged.fml.common.EventBusSubscriber;
import net.neoforged.api.distmarker.Dist;
import org.joml.Matrix4f;

@EventBusSubscriber(modid = "yog", bus = EventBusSubscriber.Bus.GAME, value = Dist.CLIENT)
public final class YogClient {
    private YogClient() {}

    @SubscribeEvent
    public static void onClientTick(ClientTickEvent.Post event) {
        NativeBridge.nativeOnClientTick();
    }

    @SubscribeEvent
    public static void onRenderGui(RenderGuiEvent.Post event) {
        NativeBridge.nativeGlInit();
        NativeDraw.hudDrawContext = event.getGuiGraphics();
        Minecraft mc = Minecraft.getInstance();
        var playerPos = mc.player != null ? mc.player.getEyePosition() : net.minecraft.world.phys.Vec3.ZERO;
        NativeBridge.nativeOnHudRender(
            event.getPartialTick().getGameTimeDeltaTicks(),
            mc.getWindow().getGuiScaledWidth(),
            mc.getWindow().getGuiScaledHeight(),
            (float) mc.getWindow().getGuiScale(),
            (float) playerPos.x, (float) playerPos.y, (float) playerPos.z);
        NativeDraw.hudDrawContext = null;
        NativeDraw.syncGlState();
    }

    @SubscribeEvent
    public static void onRenderLevel(RenderLevelStageEvent event) {
        if (event.getStage() != RenderLevelStageEvent.Stage.AFTER_TRANSLUCENT_BLOCKS) return;
        NativeBridge.nativeGlInit();
        Minecraft mc = Minecraft.getInstance();
        Matrix4f proj = event.getProjectionMatrix();
        Matrix4f view = event.getModelViewMatrix();
        float[] vp = new float[16];
        new Matrix4f(proj).mul(view).get(vp);
        var cam = event.getCamera();
        var playerPos = mc.player != null ? mc.player.getEyePosition() : cam.getPosition();
        NativeBridge.nativeOnWorldRender(
            event.getPartialTick().getGameTimeDeltaTicks(),
            mc.getWindow().getGuiScaledWidth(),
            mc.getWindow().getGuiScaledHeight(),
            (float) mc.getWindow().getGuiScale(),
            vp,
            (float) cam.getPosition().x, (float) cam.getPosition().y, (float) cam.getPosition().z,
            (float) playerPos.x, (float) playerPos.y, (float) playerPos.z);
        NativeDraw.syncGlState();
    }

    @SubscribeEvent
    public static void onScreenOpen(ScreenEvent.Opening event) {
        if (event.getScreen() instanceof YogUIScreen) return;
        NativeBridge.nativeOnScreenOpen(event.getScreen().getClass().getSimpleName());
    }

    @SubscribeEvent
    public static void onScreenClose(ScreenEvent.Closing event) {
        if (event.getScreen() instanceof YogUIScreen) return;
        NativeBridge.nativeOnScreenClose(event.getScreen().getClass().getSimpleName());
    }

    /** Send a raw-byte packet to the server (client -> server) via YogPayload. */
    public static boolean sendToServer(String channel, byte[] data) {
        ResourceLocation id = ResourceLocation.tryParse(channel);
        if (id == null) return false;
        try {
            var conn = Minecraft.getInstance().getConnection();
            if (conn == null) return false;
            conn.send(new net.minecraft.network.protocol.common.ServerboundCustomPayloadPacket(
                    new YogPayload(YogPayload.typeFor(id), data)));
            return true;
        } catch (Throwable t) {
            return false;
        }
    }
}
