package dev.yog;

import net.fabricmc.api.ClientModInitializer;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.fabricmc.fabric.api.client.networking.v1.ClientPlayNetworking;
import net.fabricmc.fabric.api.client.rendering.v1.HudRenderCallback;
import net.fabricmc.fabric.api.client.rendering.v1.WorldRenderEvents;
import net.fabricmc.fabric.api.client.screen.v1.ScreenEvents;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.client.gui.screen.TitleScreen;
import net.minecraft.client.gui.widget.ButtonWidget;
import net.minecraft.text.Text;
import net.minecraft.util.Identifier;
import org.joml.Matrix4f;

/** Client-side entry point: wires client packet receivers and client-side event hooks. */
public class YogClient implements ClientModInitializer {
    @Override
    public void onInitializeClient() {
        NativeBridge.ensureLoaded();

        // client packets — typed payloads since 1.20.5 (one codec per channel)
        String channels = NativeBridge.nativeClientPacketChannels();
        if (channels != null) {
            for (String channel : channels.split("\n")) {
                if (channel.isBlank()) continue;
                Identifier id = Identifier.tryParse(channel);
                if (id == null) continue;
                YogPayload.register(id);
                ClientPlayNetworking.registerGlobalReceiver(YogPayload.idFor(id), (payload, context) -> {
                    byte[] data = payload.data();
                    context.client().execute(() -> NativeBridge.nativeOnClientPacket(channel, data));
                });
            }
        }

        // client tick
        ClientTickEvents.END_CLIENT_TICK.register(client -> NativeBridge.nativeOnClientTick());

        // HUD render — store DrawContext for Rust draw calls, then clear it
        HudRenderCallback.EVENT.register((ctx, tickCounter) -> {
            NativeBridge.nativeGlInit();  // no-op after first call; deferred here so GL is active
            NativeDraw.hudDrawContext = ctx;
            MinecraftClient mc = MinecraftClient.getInstance();
            var playerPos = mc.player != null ? mc.player.getEyePos() : net.minecraft.util.math.Vec3d.ZERO;
            NativeBridge.nativeOnHudRender(
                tickCounter.getTickDelta(false),
                mc.getWindow().getScaledWidth(),
                mc.getWindow().getScaledHeight(),
                (float) mc.getWindow().getScaleFactor(),
                (float) playerPos.x, (float) playerPos.y, (float) playerPos.z);
            NativeDraw.hudDrawContext = null;
            NativeDraw.syncGlState(); // raw GL from Rust desyncs GlStateManager caches
        });

        // World render — fires at end of world render frame with camera matrices
        WorldRenderEvents.LAST.register(ctx -> {
            NativeBridge.nativeGlInit();  // no-op after first call
            MinecraftClient mc = MinecraftClient.getInstance();
            Matrix4f proj = ctx.projectionMatrix();
            Matrix4f view = ctx.matrixStack().peek().getPositionMatrix();
            float[] vp = new float[16];
            new Matrix4f(proj).mul(view).get(vp);
            var camPos = ctx.camera().getPos();
            var playerPos = mc.player != null ? mc.player.getEyePos() : camPos;
            NativeBridge.nativeOnWorldRender(
                ctx.tickCounter().getTickDelta(false),
                mc.getWindow().getScaledWidth(),
                mc.getWindow().getScaledHeight(),
                (float) mc.getWindow().getScaleFactor(),
                vp,
                (float) camPos.x, (float) camPos.y, (float) camPos.z,
                (float) playerPos.x, (float) playerPos.y, (float) playerPos.z);
            NativeDraw.syncGlState(); // raw GL (e.g. plumbob demo) desyncs GL caches
        });

        // screen open / close + menu entry injection
        ScreenEvents.AFTER_INIT.register((client, screen, scaledWidth, scaledHeight) -> {
            String screenClass = screen.getClass().getSimpleName();
            NativeBridge.nativeOnScreenOpen(screenClass);
            ScreenEvents.remove(screen).register(s -> NativeBridge.nativeOnScreenClose(screenClass));

            // Inject menu entry buttons on TitleScreen
            if (screen instanceof TitleScreen) {
                injectMenuButtons(screen);
            }
        });
    }

    private static void injectMenuButtons(Screen screen) {
        String raw = NativeBridge.nativeMenuEntries();
        if (raw == null || raw.isEmpty()) return;

        String[] lines = raw.split("\\n");
        int x = screen.width / 2 - 100;
        int y = screen.height / 4 + 120;

        for (String line : lines) {
            String[] parts = line.split("\\t", 2);
            if (parts.length < 2) continue;
            String label = parts[0];
            String uiId  = parts[1];

            screen.addDrawableChild(
                ButtonWidget.builder(Text.literal(label), btn -> {
                    YogUIScreen.open(uiId, false, false);
                }).position(x, y).size(100, 20).build()
            );
            y += 24;
        }
    }

    /** Send a raw-byte packet to the server (client -> server). */
    public static boolean sendToServer(String channel, byte[] data) {
        Identifier id = Identifier.tryParse(channel);
        if (id == null) {
            return false;
        }
        try {
            ClientPlayNetworking.send(new YogPayload(YogPayload.idFor(id), data));
            return true;
        } catch (Throwable t) {
            return false;
        }
    }
}
