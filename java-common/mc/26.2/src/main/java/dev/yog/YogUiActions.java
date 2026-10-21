package dev.yog;

/** The host's ui actions: the methods {@code yog_event::ui} names. */
public final class YogUiActions {
    private YogUiActions() {}

    /** Open a registered Yog UI (called from Rust; schedules onto the render thread). */
    @YogServiceHost
    public static void openUI(String uiId, boolean modal, boolean pause) {
        try {
            net.minecraft.client.Minecraft mc = net.minecraft.client.Minecraft.getInstance();
            mc.execute(() -> YogUIScreen.open(uiId, modal, pause));
        } catch (Throwable t) {
            // dedicated server — no client classes; ignore
        }
    }

    /**
     * Close the Yog UI screen showing uiId ("" = any Yog UI screen) — called
     * from Rust. Always queued, so the mod's event handler finishes first.
     */
    @YogServiceHost
    public static void closeUI(String uiId) {
        try {
            net.minecraft.client.Minecraft mc = net.minecraft.client.Minecraft.getInstance();
            mc.execute(() -> YogUIScreen.closeIfOpen(uiId));
        } catch (Throwable t) {
            // dedicated server — no client classes; ignore
        }
    }
}
