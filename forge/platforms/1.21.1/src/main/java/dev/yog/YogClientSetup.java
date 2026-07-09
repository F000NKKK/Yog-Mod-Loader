package dev.yog;

import net.minecraftforge.api.distmarker.Dist;
import net.minecraftforge.client.event.RegisterMenuScreensEvent;
import net.minecraftforge.eventbus.api.SubscribeEvent;
import net.minecraftforge.fml.common.Mod;

/**
 * One-shot client setup on the MOD bus — kept separate from {@link YogClient}
 * (which subscribes to the FORGE bus) since `@Mod.EventBusSubscriber` picks
 * one bus per class. Registers the yog-inventory generic screen (see
 * rust/crates/yog-inventory/DESIGN.md).
 */
@Mod.EventBusSubscriber(modid = "yog", bus = Mod.EventBusSubscriber.Bus.MOD, value = Dist.CLIENT)
public final class YogClientSetup {
    private YogClientSetup() {}

    @SubscribeEvent
    public static void onRegisterMenuScreens(RegisterMenuScreensEvent event) {
        event.register(YogHost.INVENTORY_MENU_TYPE, YogInventoryScreen::new);
    }
}
