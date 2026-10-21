package dev.yog;

import net.minecraft.client.KeyMapping;
import net.minecraft.client.gui.screens.MenuScreens;
import net.minecraft.resources.Identifier;
import net.minecraft.world.level.block.Block;
import net.minecraftforge.client.event.AddGuiOverlayLayersEvent;
import net.minecraftforge.client.event.ClientPlayerNetworkEvent;
import net.minecraftforge.client.event.EntityRenderersEvent;
import net.minecraftforge.client.event.RegisterClientTooltipComponentFactoriesEvent;
import net.minecraftforge.client.event.RegisterColorHandlersEvent;
import net.minecraftforge.client.event.RegisterKeyMappingsEvent;
import net.minecraftforge.client.event.RegisterPictureInPictureRendererEvent;
import net.minecraftforge.client.event.ScreenEvent;
import net.minecraftforge.event.TickEvent;
import net.minecraftforge.event.entity.player.ItemTooltipEvent;
import net.minecraftforge.eventbus.api.bus.BusGroup;
import net.minecraftforge.fml.event.lifecycle.FMLClientSetupEvent;

/**
 * Forge's client: hooks Forge's client events up to {@link YogClientEvents}
 * (the level's drawing comes through {@code mixin.LevelMainPassMixin}:
 * Forge has no event there).
 */
public final class YogClient {
    private YogClient() {}

    /** The HUD layer Yog's mods draw in, over vanilla's. */
    private static final String HUD_LAYER = "hud";

    static void init(BusGroup mods) {
        FMLClientSetupEvent.getBus(mods).addListener(e -> e.enqueueWork(
                () -> MenuScreens.register(YogContentRegistry.INVENTORY_MENU_TYPE, YogInventoryScreen::new)));
        RegisterColorHandlersEvent.Block.BUS.addListener(e -> {
            var tinted = YogClientEvents.tintedBlocks();
            if (!tinted.isEmpty()) e.register(java.util.List.of(new YogClientApi.Tint()), tinted.toArray(new Block[0]));
        });
        RegisterPictureInPictureRendererEvent.BUS.addListener(e -> e.register(new YogBlocksPip()));
        RegisterClientTooltipComponentFactoriesEvent.BUS.addListener(e -> e.register(YogTooltipData.class, YogClientTooltip::new));
        RegisterKeyMappingsEvent.BUS.addListener(e -> {
            for (KeyMapping key : YogClientEvents.keyMappings(KeyMapping.Category::register)) e.register(key);
        });
        EntityRenderersEvent.RegisterRenderers.BUS.addListener(e -> YogClientEvents.renderers(e::registerEntityRenderer));
        AddGuiOverlayLayersEvent.BUS.addListener(e -> e.getLayeredDraw().add(Identifier.fromNamespaceAndPath(YogNames.Ns.YOG, HUD_LAYER),
                (ctx, delta) -> YogClientEvents.hud(ctx, delta.getGameTimeDeltaPartialTick(false))));

        TickEvent.ClientTickEvent.Post.BUS.addListener(e -> YogClientEvents.tick());
        ItemTooltipEvent.BUS.addListener(e -> YogClientEvents.tooltip(e.getItemStack(), e.getToolTip()::add));
        ClientPlayerNetworkEvent.LoggingOut.BUS.addListener(e -> YogClientEvents.disconnected());
        ScreenEvent.Opening.BUS.addListener(e -> {
            YogClientEvents.screenOpened(e.getScreen());
            return false;
        });
        ScreenEvent.Closing.BUS.addListener(e -> YogClientEvents.screenClosed(e.getScreen()));
        ScreenEvent.Init.Post.BUS.addListener(e -> YogClientEvents.menuButtons(e.getScreen()).forEach(e::addListener));
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
