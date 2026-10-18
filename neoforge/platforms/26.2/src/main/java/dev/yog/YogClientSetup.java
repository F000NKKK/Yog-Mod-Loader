package dev.yog;

import net.neoforged.api.distmarker.Dist;
import net.neoforged.bus.api.SubscribeEvent;
import net.neoforged.fml.common.EventBusSubscriber;
import net.neoforged.neoforge.client.event.RegisterMenuScreensEvent;

/**
 * One-shot client setup on the MOD bus — kept separate from {@link YogClient}
 * (which subscribes to the GAME bus) since `@EventBusSubscriber` picks
 * one bus per class. Registers the yog-inventory generic screen (see
 * rust/crates/yog-inventory/DESIGN.md).
 */
@EventBusSubscriber(modid = "yog", bus = EventBusSubscriber.Bus.MOD, value = Dist.CLIENT)
public final class YogClientSetup {
    private YogClientSetup() {}

    @SubscribeEvent
    public static void onRegisterMenuScreens(RegisterMenuScreensEvent event) {
        event.register(YogHost.INVENTORY_MENU_TYPE, YogInventoryScreen::new);
    }

    /** Pictures in item tooltips (ABI 2.2): the mod's code draws them. */
    @SubscribeEvent
    public static void onTooltipFactories(net.neoforged.neoforge.client.event.RegisterClientTooltipComponentFactoriesEvent event) {
        event.register(YogTooltipData.class, YogClientTooltip::new);
    }

    /** The mods' key bindings (ABI 3.10): each a game key under its mod's category; its presses go to the mods each tick. */
    @SubscribeEvent
    public static void onKeyMappings(net.neoforged.neoforge.client.event.RegisterKeyMappingsEvent event) {
        NativeBridge.ensureLoaded();
        for (dev.yog.util.YogKeyBindings.Binding b : dev.yog.util.YogKeyBindings.declared(NativeBridge.nativeKeyBindings())) {
            net.minecraft.client.KeyMapping key = new net.minecraft.client.KeyMapping(
                    b.name(), com.mojang.blaze3d.platform.InputConstants.Type.KEYSYM, b.defaultKey(), b.categoryKey());
            event.register(key);
            dev.yog.util.YogKeyBindings.track(b.id(), key::consumeClick);
        }
    }
}
