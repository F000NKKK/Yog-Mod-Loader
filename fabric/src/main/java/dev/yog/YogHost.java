package dev.yog;

import net.fabricmc.api.ModInitializer;

/**
 * Fabric entry point. Boots the native Yog runtime so Rust mods can subscribe
 * to events. "The Gate and the Key."
 */
public class YogHost implements ModInitializer {
    @Override
    public void onInitialize() {
        NativeBridge.ensureLoaded();
        System.out.println("[yog] Fabric host initialised.");
    }
}
