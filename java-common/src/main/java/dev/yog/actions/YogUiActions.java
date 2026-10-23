package dev.yog.actions;

import dev.yog.YogPlatform;
import dev.yog.YogServiceHost;
import dev.yog.client.YogUIScreen;

/**
 * The host's ui actions: the methods {@code yog_event::ui} names. Screens
 * are a client's: on a server these do nothing (the mod's own handler on
 * the client opens it there), and the client's work goes through the
 * client's screen class, reached only on a client.
 */
public final class YogUiActions {
    private YogUiActions() {}

    /** Open a registered Yog UI (called from Rust; on the render thread). */
    @YogServiceHost
    public static void openUI(String uiId, boolean modal, boolean pause) {
        if (YogPlatform.isClient()) YogUIScreen.openLater(uiId, modal, pause);
    }

    /**
     * Close the Yog UI screen showing uiId ("" = any Yog UI screen) — called
     * from Rust. Always queued, so the mod's event handler finishes first.
     */
    @YogServiceHost
    public static void closeUI(String uiId) {
        if (YogPlatform.isClient()) YogUIScreen.closeLater(uiId);
    }
}
