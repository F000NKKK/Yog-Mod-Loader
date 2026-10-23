package dev.yog;

import net.fabricmc.api.ClientModInitializer;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.fabricmc.fabric.api.client.item.v1.ItemTooltipCallback;
import net.fabricmc.fabric.api.client.keymapping.v1.KeyMappingHelper;
import net.fabricmc.fabric.api.client.networking.v1.ClientPlayConnectionEvents;
import net.fabricmc.fabric.api.client.networking.v1.ClientPlayNetworking;
import net.fabricmc.fabric.api.client.rendering.v1.BlockColorRegistry;
import net.fabricmc.fabric.api.client.rendering.v1.ClientTooltipComponentCallback;
import net.fabricmc.fabric.api.client.rendering.v1.PictureInPictureRendererRegistry;
import net.fabricmc.fabric.api.client.rendering.v1.hud.HudElementRegistry;
import net.fabricmc.fabric.api.client.rendering.v1.level.LevelRenderEvents;
import net.fabricmc.fabric.api.client.screen.v1.ScreenEvents;
import net.fabricmc.fabric.api.client.screen.v1.Screens;
import net.minecraft.client.KeyMapping;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.screens.MenuScreens;
import net.minecraft.client.renderer.entity.EntityRenderers;
import net.minecraft.resources.Identifier;
import net.minecraft.world.level.block.Block;

/** Fabric's client entry point: hooks Fabric's client events up to {@link YogClientEvents}. */
public class YogClient implements ClientModInitializer {
    /** The HUD layer Yog's mods draw in, over vanilla's. */
    private static final String HUD_LAYER = "hud";

    @Override
    public void onInitializeClient() {
        NativeBridge.ensureLoaded();
        var tinted = YogClientEvents.tintedBlocks();
        if (!tinted.isEmpty()) {
            BlockColorRegistry.register(java.util.List.of(new YogClientApi.Tint()), tinted.toArray(new Block[0]));
        }
        // Blocks in 3D in a GUI box (NativeDraw.drawBlocks), drawn into a texture of their own.
        PictureInPictureRendererRegistry.register(context -> new YogBlocksPip());
        for (var layer : YogGlPip.LAYERS) PictureInPictureRendererRegistry.register(context -> YogGlPip.of(layer));
        // The mods' particles, drawn from their pictures.
        YogParticleClient.providers((type, make) ->
                net.fabricmc.fabric.api.client.particle.v1.ParticleProviderRegistry.getInstance().register(type, make::apply));
        ItemTooltipCallback.EVENT.register((stack, context, flag, lines) -> YogClientEvents.tooltip(stack, lines::add));
        // Pictures in item tooltips (ABI 2.2).
        ClientTooltipComponentCallback.EVENT.register(data -> data instanceof YogTooltipData d ? new YogClientTooltip(d) : null);
        MenuScreens.register(YogContentRegistry.INVENTORY_MENU_TYPE, YogInventoryScreen::new);

        ClientPlayNetworking.registerGlobalReceiver(YogPayload.TYPE, (payload, context) -> context.client().execute(payload::toClient));
        ClientPlayConnectionEvents.DISCONNECT.register((handler, client) -> YogClientEvents.disconnected());

        for (KeyMapping key : YogClientEvents.keyMappings(KeyMapping.Category::register)) KeyMappingHelper.registerKeyMapping(key);
        YogClientEvents.renderers(EntityRenderers::register);

        ClientTickEvents.END_CLIENT_TICK.register(client -> YogClientEvents.tick());
        HudElementRegistry.addLast(Identifier.fromNamespaceAndPath(YogNames.Ns.YOG, HUD_LAYER),
                (ctx, tickCounter) -> YogClientEvents.hud(ctx, tickCounter.getGameTimeDeltaPartialTick(false)));
        LevelRenderEvents.END_MAIN.register(ctx -> {
            var camera = ctx.levelState().cameraRenderState;
            YogClientEvents.world(camera.viewRotationMatrix, camera.pos, Minecraft.getInstance().getDeltaTracker().getGameTimeDeltaPartialTick(false));
        });
        ScreenEvents.AFTER_INIT.register((client, screen, w, h) -> {
            YogClientEvents.screenOpened(screen);
            ScreenEvents.remove(screen).register(YogClientEvents::screenClosed);
            Screens.getWidgets(screen).addAll(YogClientEvents.menuButtons(screen));
        });
    }

    /** Raw bytes to the server (client → server); the runtime calls it by name. */
    @YogServiceHost
    public static boolean sendToServer(String channel, byte[] data) {
        try {
            YogPlatform.sendToServer(channel, data);
            return true;
        } catch (Throwable t) {
            return false;
        }
    }
}
