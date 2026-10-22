package dev.yog;

import net.minecraft.client.KeyMapping;
import net.minecraft.client.Minecraft;
import net.minecraft.resources.Identifier;
import net.minecraft.world.level.block.Block;
import net.neoforged.bus.api.IEventBus;
import net.neoforged.neoforge.client.event.ClientPlayerNetworkEvent;
import net.neoforged.neoforge.client.event.ClientTickEvent;
import net.neoforged.neoforge.client.event.EntityRenderersEvent;
import net.neoforged.neoforge.client.event.RegisterClientTooltipComponentFactoriesEvent;
import net.neoforged.neoforge.client.event.RegisterColorHandlersEvent;
import net.neoforged.neoforge.client.event.RegisterGuiLayersEvent;
import net.neoforged.neoforge.client.event.RegisterKeyMappingsEvent;
import net.neoforged.neoforge.client.event.RegisterMenuScreensEvent;
import net.neoforged.neoforge.client.event.RegisterPictureInPictureRenderersEvent;
import net.neoforged.neoforge.client.event.RenderLevelStageEvent;
import net.neoforged.neoforge.client.event.ScreenEvent;
import net.neoforged.neoforge.common.NeoForge;
import net.neoforged.neoforge.event.entity.player.ItemTooltipEvent;

/** NeoForge's client: hooks NeoForge's client events up to {@link YogClientEvents}. */
public final class YogClient {
    private YogClient() {}

    /** The HUD layer Yog's mods draw in, over vanilla's. */
    private static final String HUD_LAYER = "hud";

    static void init(IEventBus modBus) {
        modBus.addListener(RegisterColorHandlersEvent.BlockTintSources.class, e -> {
            var tinted = YogClientEvents.tintedBlocks();
            if (!tinted.isEmpty()) e.register(java.util.List.of(new YogClientApi.Tint()), tinted.toArray(new Block[0]));
        });
        modBus.addListener(RegisterPictureInPictureRenderersEvent.class, e -> {
            e.register(YogBlocksPip.State.class, YogBlocksPip::new);
            e.register(YogGlPip.Layer0.class, () -> new YogGlPip<>(YogGlPip.Layer0.class));
            e.register(YogGlPip.Layer1.class, () -> new YogGlPip<>(YogGlPip.Layer1.class));
            e.register(YogGlPip.Layer2.class, () -> new YogGlPip<>(YogGlPip.Layer2.class));
            e.register(YogGlPip.Layer3.class, () -> new YogGlPip<>(YogGlPip.Layer3.class));
        });
        modBus.addListener(RegisterClientTooltipComponentFactoriesEvent.class, e -> e.register(YogTooltipData.class, YogClientTooltip::new));
        modBus.addListener(RegisterMenuScreensEvent.class, e -> e.register(YogContentRegistry.INVENTORY_MENU_TYPE, YogInventoryScreen::new));
        modBus.addListener(RegisterKeyMappingsEvent.class, e -> {
            for (KeyMapping key : YogClientEvents.keyMappings(id -> {
                KeyMapping.Category category = new KeyMapping.Category(id);
                e.registerCategory(category);
                return category;
            })) {
                e.register(key);
            }
        });
        modBus.addListener(EntityRenderersEvent.RegisterRenderers.class, e -> YogClientEvents.renderers(e::registerEntityRenderer));
        modBus.addListener(RegisterGuiLayersEvent.class, e -> e.registerAboveAll(Identifier.fromNamespaceAndPath(YogNames.Ns.YOG, HUD_LAYER),
                (ctx, delta) -> YogClientEvents.hud(ctx, delta.getGameTimeDeltaPartialTick(false))));

        var bus = NeoForge.EVENT_BUS;
        bus.addListener(ClientTickEvent.Post.class, e -> YogClientEvents.tick());
        bus.addListener(ItemTooltipEvent.class, e -> YogClientEvents.tooltip(e.getItemStack(), e.getToolTip()::add));
        bus.addListener(ClientPlayerNetworkEvent.LoggingOut.class, e -> YogClientEvents.disconnected());
        bus.addListener(RenderLevelStageEvent.AfterLevel.class, e -> {
            var camera = e.getLevelRenderState().cameraRenderState;
            YogClientEvents.world(camera.viewRotationMatrix, camera.pos, Minecraft.getInstance().getDeltaTracker().getGameTimeDeltaPartialTick(false));
        });
        bus.addListener(ScreenEvent.Opening.class, e -> YogClientEvents.screenOpened(e.getScreen()));
        bus.addListener(ScreenEvent.Closing.class, e -> YogClientEvents.screenClosed(e.getScreen()));
        bus.addListener(ScreenEvent.Init.Post.class, e -> YogClientEvents.menuButtons(e.getScreen()).forEach(e::addListener));
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
