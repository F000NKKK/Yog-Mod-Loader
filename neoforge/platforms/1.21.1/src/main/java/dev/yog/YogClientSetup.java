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
}
