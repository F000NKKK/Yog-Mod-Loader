package dev.yog;

import net.neoforged.api.distmarker.Dist;
import net.neoforged.fml.common.Mod;
import net.neoforged.bus.api.IEventBus;
import net.neoforged.bus.api.SubscribeEvent;
import net.neoforged.neoforge.client.event.ClientTickEvent;
import net.neoforged.neoforge.client.event.RenderGuiEvent;
import net.neoforged.neoforge.client.event.RenderLevelStageEvent;
import net.neoforged.neoforge.client.event.ScreenEvent;
import net.neoforged.neoforge.network.PacketDistributor;
import net.minecraft.client.Minecraft;
import net.minecraft.network.FriendlyByteBuf;
import net.minecraft.resources.ResourceLocation;
import org.joml.Matrix4f;

/** NeoForge client-side event handlers. Wired via @Mod.EventBusSubscriber. */
@Mod.EventBusSubscriber(modid = "yog", bus = Mod.EventBusSubscriber.Bus.FORGE, value = Dist.CLIENT)
public class YogClient {
    private static boolean initialised = false;

    private static void ensureInit() {
        if (!initialised) {
            NativeBridge.ensureLoaded();
            initialised = true;
        }
    }

    // ── Client tick ──────────────────────────────────────────────────────

    @SubscribeEvent
    public void onClientTick(ClientTickEvent.Post event) {
        NativeBridge.nativeOnClientTick();
    }

    // ── HUD render ───────────────────────────────────────────────────────

    @SubscribeEvent
    public void onRenderGui(RenderGuiEvent.Post event) {
        NativeBridge.nativeGlInit();
        NativeDraw.hudDrawContext = event.getGuiGraphics();
        Minecraft mc = Minecraft.getInstance();
        var playerPos = mc.player != null ? mc.player.getEyePosition() : net.minecraft.world.phys.Vec3.ZERO;
        NativeBridge.nativeOnHudRender(
            event.getPartialTick(),
            mc.getWindow().getGuiScaledWidth(),
            mc.getWindow().getGuiScaledHeight(),
            (float) mc.getWindow().getGuiScale(),
            (float) playerPos.x, (float) playerPos.y, (float) playerPos.z);
        NativeDraw.hudDrawContext = null;
    }

    // ── World render ─────────────────────────────────────────────────────

    @SubscribeEvent
    public void onRenderLevel(RenderLevelStageEvent event) {
        if (event.getStage() != RenderLevelStageEvent.Stage.AFTER_TRANSLUCENT) return;
        NativeBridge.nativeGlInit();
        Minecraft mc = Minecraft.getInstance();
        Matrix4f proj = event.getProjectionMatrix();
        Matrix4f view = event.getModelViewMatrix();
        float[] vp = new float[16];
        new Matrix4f(proj).mul(view).get(vp);
        var cam = event.getCamera();
        var playerPos = mc.player != null ? mc.player.getEyePosition() : cam.getPosition();
        NativeBridge.nativeOnWorldRender(
            event.getPartialTick(),
            mc.getWindow().getGuiScaledWidth(),
            mc.getWindow().getGuiScaledHeight(),
            (float) mc.getWindow().getGuiScale(),
            vp,
            (float) cam.getPosition().x, (float) cam.getPosition().y, (float) cam.getPosition().z,
            (float) playerPos.x, (float) playerPos.y, (float) playerPos.z);
    }

    // ── Screen open / close ──────────────────────────────────────────────

    @SubscribeEvent
    public void onScreenOpen(ScreenEvent.Opening event) {
        String screenClass = event.getScreen().getClass().getSimpleName();
        NativeBridge.nativeOnScreenOpen(screenClass);
    }

    @SubscribeEvent
    public void onScreenClose(ScreenEvent.Closing event) {
        String screenClass = event.getScreen().getClass().getSimpleName();
        NativeBridge.nativeOnScreenClose(screenClass);
    }

    /** Send a raw-byte packet to the server (client -> server). */
    public static boolean sendToServer(String channel, byte[] data) {
        ResourceLocation id = ResourceLocation.tryParse(channel);
        if (id == null) return false;
        try {
            FriendlyByteBuf buf = new FriendlyByteBuf(io.netty.buffer.Unpooled.buffer());
            buf.writeBytes(data);
            PacketDistributor.SERVER.noArg().send(new net.neoforged.neoforge.network.handling.SimplePayload(id, buf));
            return true;
        } catch (Throwable t) {
            return false;
        }
    }
}