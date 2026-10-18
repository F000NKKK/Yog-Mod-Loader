package dev.yog;

import net.minecraft.client.KeyMapping;
import net.minecraft.world.level.block.Block;
import net.neoforged.bus.api.IEventBus;
import net.neoforged.neoforge.client.event.ClientPlayerNetworkEvent;
import net.neoforged.neoforge.client.event.ClientTickEvent;
import net.neoforged.neoforge.client.event.EntityRenderersEvent;
import net.neoforged.neoforge.client.event.RegisterClientTooltipComponentFactoriesEvent;
import net.neoforged.neoforge.client.event.RegisterColorHandlersEvent;
import net.neoforged.neoforge.client.event.RegisterKeyMappingsEvent;
import net.neoforged.neoforge.client.event.RegisterMenuScreensEvent;
import net.neoforged.neoforge.client.event.RenderGuiEvent;
import net.neoforged.neoforge.client.event.RenderLevelStageEvent;
import net.neoforged.neoforge.client.event.ScreenEvent;
import net.neoforged.neoforge.common.NeoForge;

/** NeoForge's client: hooks NeoForge's client events up to {@link YogClientEvents}. */
public final class YogClient {
    private YogClient() {}

    static void init(IEventBus modBus) {
        modBus.addListener(RegisterColorHandlersEvent.Block.class, e -> {
            var tinted = YogClientEvents.tintedBlocks();
            if (!tinted.isEmpty()) e.register(YogClientEvents.tint, tinted.toArray(new Block[0]));
        });
        modBus.addListener(RegisterClientTooltipComponentFactoriesEvent.class, e -> e.register(YogTooltipData.class, YogClientTooltip::new));
        modBus.addListener(RegisterMenuScreensEvent.class, e -> e.register(YogContentRegistry.INVENTORY_MENU_TYPE, YogInventoryScreen::new));
        modBus.addListener(RegisterKeyMappingsEvent.class, e -> {
            for (KeyMapping key : YogClientEvents.keyMappings()) e.register(key);
        });
        modBus.addListener(EntityRenderersEvent.RegisterRenderers.class, e -> YogClientEvents.renderers(e::registerEntityRenderer));

        var bus = NeoForge.EVENT_BUS;
        bus.addListener(ClientTickEvent.Post.class, e -> YogClientEvents.tick());
        bus.addListener(ClientPlayerNetworkEvent.LoggingOut.class, e -> YogClientEvents.disconnected());
        bus.addListener(RenderGuiEvent.Post.class, e -> YogClientEvents.hud(e.getGuiGraphics(), e.getPartialTick().getGameTimeDeltaTicks()));
        bus.addListener(RenderLevelStageEvent.class, e -> {
            if (e.getStage() != RenderLevelStageEvent.Stage.AFTER_TRANSLUCENT_BLOCKS) return;
            YogClientEvents.world(e.getProjectionMatrix(), e.getModelViewMatrix(), e.getCamera().getPosition(),
                    e.getPartialTick().getGameTimeDeltaTicks());
        });
        bus.addListener(ScreenEvent.Opening.class, e -> YogClientEvents.screenOpened(e.getScreen()));
        bus.addListener(ScreenEvent.Closing.class, e -> YogClientEvents.screenClosed(e.getScreen()));
        bus.addListener(ScreenEvent.Init.Post.class, e -> YogClientEvents.menuButtons(e.getScreen()).forEach(e::addListener));
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
