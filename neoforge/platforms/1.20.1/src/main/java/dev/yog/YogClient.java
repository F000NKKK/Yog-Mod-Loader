package dev.yog;

import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.components.Button;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.network.FriendlyByteBuf;
import net.minecraft.network.chat.Component;
import net.minecraft.network.protocol.game.ServerboundCustomPayloadPacket;
import net.minecraft.resources.ResourceLocation;
import net.minecraftforge.api.distmarker.Dist;
import net.minecraftforge.client.event.RenderGuiEvent;
import net.minecraftforge.client.event.RenderLevelStageEvent;
import net.minecraftforge.client.event.ScreenEvent;
import net.minecraftforge.event.TickEvent;
import net.minecraftforge.eventbus.api.SubscribeEvent;
import net.minecraftforge.fml.common.Mod;
import org.joml.Matrix4f;

/** NeoForge client-side event handlers. Wired via @Mod.EventBusSubscriber. */
@Mod.EventBusSubscriber(modid = "yog", bus = Mod.EventBusSubscriber.Bus.FORGE, value = Dist.CLIENT)
public final class YogClient {
    private YogClient() {}

    // ── Client tick ──────────────────────────────────────────────────────

    @SubscribeEvent
    public static void onClientTick(TickEvent.ClientTickEvent event) {
        if (event.phase != TickEvent.Phase.END) return;
        NativeBridge.nativeOnClientTick();
    }

    // ── HUD render ───────────────────────────────────────────────────────

    @SubscribeEvent
    public static void onRenderGui(RenderGuiEvent.Post event) {
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
        NativeDraw.syncGlState(); // raw GL from Rust desyncs GlStateManager caches
    }

    // ── World render ─────────────────────────────────────────────────────

    @SubscribeEvent
    public static void onRenderLevel(RenderLevelStageEvent event) {
        if (event.getStage() != RenderLevelStageEvent.Stage.AFTER_TRANSLUCENT_BLOCKS) return;
        NativeBridge.nativeGlInit();
        Minecraft mc = Minecraft.getInstance();
        Matrix4f proj = event.getProjectionMatrix();
        Matrix4f view = event.getPoseStack().last().pose();
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
        NativeDraw.syncGlState(); // raw GL (e.g. demo world renderers) desyncs GL caches
    }

    // ── Screen open / close ──────────────────────────────────────────────

    @SubscribeEvent
    public static void onScreenOpen(ScreenEvent.Opening event) {
        String screenClass = event.getScreen().getClass().getSimpleName();
        NativeBridge.nativeOnScreenOpen(screenClass);
    }

    @SubscribeEvent
    public static void onScreenClose(ScreenEvent.Closing event) {
        String screenClass = event.getScreen().getClass().getSimpleName();
        NativeBridge.nativeOnScreenClose(screenClass);
    }

    // ── Menu entry injection ────────────────────────────────────────────────

    @SubscribeEvent
    public static void onScreenInitPost(ScreenEvent.Init.Post event) {
        Screen screen = event.getScreen();
        String cls = screen.getClass().getName();

        // NeoForge 1.20.1 ships under the "forge" package; 1.21+ uses "neoforged".
        boolean isModList = cls.equals("net.minecraftforge.client.gui.ModListScreen")
                         || cls.equals("net.neoforged.neoforge.client.gui.ModListScreen");
        boolean isTitle   = cls.equals("net.minecraft.client.gui.screens.TitleScreen");
        if (!isModList && !isTitle) return;

        String raw = NativeBridge.nativeMenuEntries();
        if (raw == null || raw.isEmpty()) return;

        String[] lines = raw.split("\\n");
        int x, y;
        if (isModList) {
            x = screen.width - 110;
            y = 10;
        } else {
            // Top-left corner — clear of the vanilla title button column.
            x = 4;
            y = 4;
        }

        for (String line : lines) {
            String[] parts = line.split("\\t", 2);
            if (parts.length < 2) continue;
            String label = parts[0];
            String uiId  = parts[1];

            event.addListener(
                Button.builder(Component.literal(label), btn -> {
                    YogUIScreen.open(uiId, false, false);
                }).pos(x, y).size(100, 20).build()
            );
            y += 24;
        }
    }

    /** Send a raw-byte packet to the server (client -> server). */
    public static boolean sendToServer(String channel, byte[] data) {
        ResourceLocation id = ResourceLocation.tryParse(channel);
        if (id == null) return false;
        try {
            var conn = Minecraft.getInstance().getConnection();
            if (conn == null) return false;
            FriendlyByteBuf buf = new FriendlyByteBuf(io.netty.buffer.Unpooled.buffer());
            buf.writeBytes(data);
            conn.send(new ServerboundCustomPayloadPacket(id, buf));
            return true;
        } catch (Throwable t) {
            return false;
        }
    }
}
