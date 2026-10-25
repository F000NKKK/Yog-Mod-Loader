package dev.yog;

import dev.yog.client.YogClientEvents;
import dev.yog.client.YogClientTooltip;
import dev.yog.client.YogInventoryScreen;
import dev.yog.client.YogParticleClient;
import dev.yog.client.YogTooltipData;
import dev.yog.content.YogContentRegistry;

import net.minecraft.client.KeyMapping;
import net.minecraft.client.gui.screens.MenuScreens;
import net.minecraft.world.level.block.Block;
import net.minecraftforge.client.event.ClientPlayerNetworkEvent;
import net.minecraftforge.client.event.EntityRenderersEvent;
import net.minecraftforge.client.event.RegisterClientTooltipComponentFactoriesEvent;
import net.minecraftforge.client.event.RegisterColorHandlersEvent;
import net.minecraftforge.client.event.RegisterKeyMappingsEvent;
import net.minecraftforge.client.event.RenderGuiEvent;
import net.minecraftforge.client.event.RenderLevelStageEvent;
import net.minecraftforge.client.event.ScreenEvent;
import net.minecraftforge.common.MinecraftForge;
import net.minecraftforge.event.TickEvent;
import net.minecraftforge.eventbus.api.IEventBus;
import net.minecraftforge.fml.event.lifecycle.FMLClientSetupEvent;

/** Forge's client: hooks its client events up to {@link YogClientEvents}. */
public final class YogClient {
    private YogClient() {}

    static void init(IEventBus modBus) {
        modBus.addListener((FMLClientSetupEvent e) -> e.enqueueWork(
                () -> MenuScreens.register(YogContentRegistry.INVENTORY_MENU_TYPE, YogInventoryScreen::new)));
        modBus.addListener((RegisterColorHandlersEvent.Block e) -> {
            var tinted = YogClientEvents.tintedBlocks();
            if (!tinted.isEmpty()) e.register(YogClientEvents.tint, tinted.toArray(new Block[0]));
        });
        modBus.addListener((RegisterClientTooltipComponentFactoriesEvent e) -> e.register(YogTooltipData.class, YogClientTooltip::new));
        modBus.addListener((net.minecraftforge.client.event.RegisterParticleProvidersEvent e) ->
                YogParticleClient.providers((type, make) -> e.register(type, make::apply)));
        modBus.addListener((RegisterKeyMappingsEvent e) -> {
            for (KeyMapping key : YogClientEvents.keyMappings()) e.register(key);
        });
        modBus.addListener((EntityRenderersEvent.RegisterRenderers e) -> YogClientEvents.renderers(e::registerEntityRenderer));

        var bus = MinecraftForge.EVENT_BUS;
        bus.addListener((TickEvent.ClientTickEvent e) -> {
            if (e.phase == TickEvent.Phase.END) YogClientEvents.tick();
        });
        bus.addListener((ClientPlayerNetworkEvent.LoggingOut e) -> YogClientEvents.disconnected());
        bus.addListener((RenderGuiEvent.Post e) -> YogClientEvents.hud(e.getPoseStack(), e.getPartialTick()));
        bus.addListener((RenderLevelStageEvent e) -> {
            if (e.getStage() != RenderLevelStageEvent.Stage.AFTER_TRANSLUCENT_BLOCKS) return;
            YogClientEvents.world(e.getProjectionMatrix(), e.getPoseStack().last().pose(), e.getCamera().getPosition(), e.getPartialTick());
        });
        bus.addListener((ScreenEvent.Opening e) -> YogClientEvents.screenOpened(e.getScreen()));
        bus.addListener((ScreenEvent.Closing e) -> YogClientEvents.screenClosed(e.getScreen()));
        bus.addListener((ScreenEvent.Init.Post e) -> YogClientEvents.menuButtons(e.getScreen()).forEach(e::addListener));
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
