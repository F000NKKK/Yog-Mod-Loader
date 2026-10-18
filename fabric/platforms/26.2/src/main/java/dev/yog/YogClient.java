package dev.yog;

import net.fabricmc.api.ClientModInitializer;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.fabricmc.fabric.api.client.networking.v1.ClientPlayNetworking;
import net.fabricmc.fabric.api.client.rendering.v1.hud.HudElementRegistry;
import net.fabricmc.fabric.api.client.rendering.v1.level.LevelRenderEvents;
import net.fabricmc.fabric.api.client.screen.v1.ScreenEvents;
import net.fabricmc.fabric.api.client.screen.v1.Screens;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.components.Button;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.gui.screens.TitleScreen;
import net.minecraft.network.chat.Component;
import net.minecraft.resources.Identifier;
import net.minecraft.world.phys.Vec3;
import org.joml.Matrix4f;

/** Client-side entry point: wires client packet receivers and client-side event hooks. */
public class YogClient implements ClientModInitializer {
    /** The HUD layer Yog's mods draw in, over vanilla's. */
    private static final String HUD_LAYER = "hud";

    @Override
    public void onInitializeClient() {
        // Tints a mod sets per position (client::set_block_tint), on faces with a tint index.
        NativeBridge.ensureLoaded();
        java.util.List<net.minecraft.world.level.block.Block> yogBlocks = new java.util.ArrayList<>();
        for (dev.yog.data.YogContent.Block b : dev.yog.data.YogContent.cached(NativeBridge::nativeContent).blocks) {
            net.minecraft.resources.Identifier id = net.minecraft.resources.Identifier.tryParse(b.id());
            if (id != null && net.minecraft.core.registries.BuiltInRegistries.BLOCK.containsKey(id)) {
                yogBlocks.add(net.minecraft.core.registries.BuiltInRegistries.BLOCK.getValue(id));
            }
        }
        if (!yogBlocks.isEmpty()) {
            net.fabricmc.fabric.api.client.rendering.v1.BlockColorRegistry.register(
                    java.util.List.of(new YogClientApi.Tint()),
                    yogBlocks.toArray(new net.minecraft.world.level.block.Block[0]));
        }
        // Blocks in 3D in a GUI box (NativeDraw.drawBlocks), drawn into a texture of their own.
        net.fabricmc.fabric.api.client.rendering.v1.PictureInPictureRendererRegistry.register(context -> new YogBlocksPip());
        // The mods' tooltip lines.
        net.fabricmc.fabric.api.client.item.v1.ItemTooltipCallback.EVENT.register((stack, context, flag, lines) -> {
            if (stack.getItem() instanceof YogTooltipLines own) own.tooltipLines(stack, lines::add);
        });
        // Pictures in item tooltips (ABI 2.2).
        net.fabricmc.fabric.api.client.rendering.v1.ClientTooltipComponentCallback.EVENT.register(
                data -> data instanceof YogTooltipData d ? new YogClientTooltip(d) : null);
        NativeBridge.ensureLoaded();

        // yog-inventory: register the generic Container/Menu screen (see
        // rust/crates/yog-inventory/DESIGN.md). registerContent() (common init,
        // runs before onInitializeClient) has already set the type.
        net.minecraft.client.gui.screens.MenuScreens.register(
                YogHost.INVENTORY_SCREEN_HANDLER_TYPE, YogInventoryScreen::new);

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

        // A world left: its entity data is forgotten.
        net.fabricmc.fabric.api.client.networking.v1.ClientPlayConnectionEvents.DISCONNECT.register(
                (handler, client) -> NativeBridge.nativeEntityDataClientReset());

        // Mods' entities: drawn by what the mods give (yog-body).
        for (net.minecraft.world.entity.EntityType<?> type : YogEntityTypes.types()) {
            registerRenderer(type);
        }

        // client tick
        ClientTickEvents.END_CLIENT_TICK.register(client -> NativeBridge.nativeOnClientTick());

        // HUD — the GUI being extracted for this frame: Rust's 2-D draws go into it, its raw GL straight onto the main target.
        HudElementRegistry.addLast(Identifier.fromNamespaceAndPath(YogNames.Ns.YOG, HUD_LAYER), (ctx, tickCounter) -> {
            NativeBridge.nativeGlInit();  // no-op after first call; deferred here so GL is active
            Minecraft mc = Minecraft.getInstance();
            var playerPos = mc.player != null ? mc.player.getEyePosition() : net.minecraft.world.phys.Vec3.ZERO;
            NativeDraw.hudDrawContext = ctx;
            boolean gl = YogGl.begin();
            NativeBridge.nativeOnHudRender(
                tickCounter.getGameTimeDeltaPartialTick(false),
                mc.getWindow().getGuiScaledWidth(),
                mc.getWindow().getGuiScaledHeight(),
                (float) mc.getWindow().getGuiScale(),
                (float) playerPos.x, (float) playerPos.y, (float) playerPos.z);
            if (gl) YogGl.end();
            NativeDraw.hudDrawContext = null;
        });

        // World render — at the end of the level's main pass, with the camera's matrices.
        LevelRenderEvents.END_MAIN.register(ctx -> {
            NativeBridge.nativeGlInit();  // no-op after first call
            Minecraft mc = Minecraft.getInstance();
            var camera = ctx.levelState().cameraRenderState;
            // The camera's rotation is the view matrix; geometry is given relative to the camera's position.
            float[] vp = new float[16];
            new Matrix4f(camera.projectionMatrix).mul(camera.viewRotationMatrix).get(vp);
            Vec3 camPos = camera.pos;
            var playerPos = mc.player != null ? mc.player.getEyePosition() : camPos;
            if (!YogGl.begin()) return;
            NativeBridge.nativeOnWorldRender(
                mc.getDeltaTracker().getGameTimeDeltaPartialTick(false),
                mc.getWindow().getGuiScaledWidth(),
                mc.getWindow().getGuiScaledHeight(),
                (float) mc.getWindow().getGuiScale(),
                vp,
                (float) camPos.x, (float) camPos.y, (float) camPos.z,
                (float) playerPos.x, (float) playerPos.y, (float) playerPos.z);
            YogGl.end();
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
        // Top-left corner — the one spot vanilla title/pause layouts leave
        // free, so we never collide with (or shift) existing buttons.
        int x = 4;
        int y = 4;

        for (String line : lines) {
            String[] parts = line.split("\\t", 2);
            if (parts.length < 2) continue;
            String label = parts[0];
            String uiId  = parts[1];

            // Screen.addDrawableChild is protected — Fabric API exposes the
            // screen's button list for exactly this kind of injection.
            Screens.getWidgets(screen).add(
                Button.builder(Component.translatable(label), btn -> {
                    YogUIScreen.open(uiId, false, false);
                }).pos(x, y).size(100, 20).build()
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

    @SuppressWarnings("unchecked")
    private static void registerRenderer(net.minecraft.world.entity.EntityType<?> type) {
        net.minecraft.client.renderer.entity.EntityRenderers.register(
                (net.minecraft.world.entity.EntityType<net.minecraft.world.entity.Entity>) type, ctx -> new YogEntityRenderer(ctx, YogEntityTypes.def(type)));
    }
}
