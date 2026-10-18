package dev.yog;

import net.fabricmc.api.ClientModInitializer;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.fabricmc.fabric.api.client.keybinding.v1.KeyBindingHelper;
import net.fabricmc.fabric.api.client.networking.v1.ClientPlayConnectionEvents;
import net.fabricmc.fabric.api.client.networking.v1.ClientPlayNetworking;
import net.fabricmc.fabric.api.client.rendering.v1.ColorProviderRegistry;
import net.fabricmc.fabric.api.client.rendering.v1.EntityRendererRegistry;
import net.fabricmc.fabric.api.client.rendering.v1.HudRenderCallback;
import net.fabricmc.fabric.api.client.rendering.v1.TooltipComponentCallback;
import net.fabricmc.fabric.api.client.rendering.v1.WorldRenderEvents;
import net.fabricmc.fabric.api.client.screen.v1.ScreenEvents;
import net.fabricmc.fabric.api.client.screen.v1.Screens;
import net.minecraft.client.KeyMapping;
import net.minecraft.client.gui.screens.MenuScreens;
import net.minecraft.world.level.block.Block;

/** Fabric's client entry point: hooks Fabric's client events up to {@link YogClientEvents}. */
public class YogClient implements ClientModInitializer {
    @Override
    public void onInitializeClient() {
        NativeBridge.ensureLoaded();
        var tinted = YogClientEvents.tintedBlocks();
        if (!tinted.isEmpty()) ColorProviderRegistry.BLOCK.register(YogClientEvents.tint, tinted.toArray(new Block[0]));
        // Pictures in item tooltips (ABI 2.2).
        TooltipComponentCallback.EVENT.register(data -> data instanceof YogTooltipData d ? new YogClientTooltip(d) : null);
        MenuScreens.register(YogContentRegistry.INVENTORY_MENU_TYPE, YogInventoryScreen::new);

        ClientPlayNetworking.registerGlobalReceiver(YogPayload.CHANNEL, (client, handler, buf, sender) -> {
            YogPayload payload = YogPayload.read(buf);
            client.execute(payload::toClient);
        });
        ClientPlayConnectionEvents.DISCONNECT.register((handler, client) -> YogClientEvents.disconnected());

        for (KeyMapping key : YogClientEvents.keyMappings()) KeyBindingHelper.registerKeyBinding(key);
        YogClientEvents.renderers(EntityRendererRegistry::register);

        ClientTickEvents.END_CLIENT_TICK.register(client -> YogClientEvents.tick());
        HudRenderCallback.EVENT.register(YogClientEvents::hud);
        WorldRenderEvents.LAST.register(ctx -> YogClientEvents.world(ctx.projectionMatrix(), ctx.matrixStack().last().pose(),
                ctx.camera().getPosition(), ctx.tickDelta()));
        ScreenEvents.AFTER_INIT.register((client, screen, w, h) -> {
            YogClientEvents.screenOpened(screen);
            ScreenEvents.remove(screen).register(YogClientEvents::screenClosed);
            Screens.getButtons(screen).addAll(YogClientEvents.menuButtons(screen));
        });
    }

    /** Raw bytes to the server (client → server); the runtime calls it by name. */
    public static boolean sendToServer(String channel, byte[] data) {
        try {
            YogPlatform.sendToServer(channel, data);
            return true;
        } catch (Throwable t) {
            return false;
        }
    }
}
