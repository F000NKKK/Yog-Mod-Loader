package dev.yog;

import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.GuiGraphics;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.network.chat.Component;

/** Minecraft Screen hosting a Rust Yog UI. Supports modal, pause, layering. */
public class YogUIScreen extends Screen {
    private final String uiId;
    private final boolean modal;
    private final boolean pauseGame;
    /** No dimmed/blurred background: the world stays visible (`ui_flags::NO_BACKGROUND`). */
    private boolean noBackground;
    /** Last cursor position sent as a `move:` event. */
    private int lastMx = Integer.MIN_VALUE, lastMy = Integer.MIN_VALUE;

    public YogUIScreen(String uiId, boolean modal, boolean pauseGame) {
        super(Component.literal(uiId));
        this.uiId = uiId;
        this.modal = modal;
        this.pauseGame = pauseGame;
    }

    /** Shown once the screen is current (after the previous one's `removed()`), with its real size. */
    private boolean shown;

    @Override protected void init() {
        super.init();
        if (!shown) {
            shown = true;
            NativeBridge.nativeUIShow(uiId, "", modal, pauseGame, width, height);
        }
    }

    public YogUIScreen(String uiId) { this(uiId, true, false); }

    @Override public void render(GuiGraphics ctx, int mx, int my, float delta) {
        if (mx != lastMx || my != lastMy) {
            lastMx = mx;
            lastMy = my;
            NativeBridge.nativeUIMove(uiId, mx, my);
        }
        if (!noBackground) super.renderBackground(ctx, mx, my, delta);
        ctx.flush();
        NativeDraw.hudDrawContext = ctx;
        NativeBridge.nativeUIRender(uiId, this.width, this.height);
        NativeDraw.hudDrawContext = null;
        NativeDraw.syncGlState(); // raw GL from Rust desyncs GlStateManager caches
        super.render(ctx, mx, my, delta);
    }

    @Override
    public void renderBackground(GuiGraphics guiGraphics, int mouseX, int mouseY, float partialTicks) {
        // No-op: the blurred background is rendered explicitly via
        // super.renderBackground() above, before the book.
    }

    @Override public boolean mouseClicked(double mx, double my, int button) {
        NativeBridge.nativeUIClick(uiId, (float) mx, (float) my, button);
        return true; // always consume — prevent game from processing the click
    }

    @Override public boolean mouseScrolled(double mx, double my, double horizontal, double vertical) {
        NativeBridge.nativeUIScroll(uiId, (float) vertical);
        return true;
    }

    @Override public boolean mouseDragged(double mx, double my, int button, double dx, double dy) {
        NativeBridge.nativeUIDrag(uiId, (float) mx, (float) my);
        return true;
    }

    @Override public boolean mouseReleased(double mx, double my, int button) {
        NativeBridge.nativeUIRelease(uiId, (float) mx, (float) my);
        return true;
    }

    @Override public boolean keyPressed(int keyCode, int scanCode, int modifiers) {
        // The mod may use the key (a focused text box takes Esc to unfocus).
        if (NativeBridge.nativeUIKey(uiId, keyCode, scanCode, modifiers, 1)) return true;
        return super.keyPressed(keyCode, scanCode, modifiers);
    }

    @Override public boolean charTyped(char chr, int modifiers) {
        if (NativeBridge.nativeUIChar(uiId, chr, modifiers)) return true;
        return super.charTyped(chr, modifiers);
    }

    @Override public boolean isPauseScreen() { return pauseGame; }

    @Override public void onClose() {
        super.onClose();
    }

    /** Leaving the screen any way (Esc, a button, another screen opening): the mod gets `close`. */
    @Override public void removed() {
        NativeBridge.nativeUIHide(uiId);
        super.removed();
    }

    /** Closes the open screen if it's the Yog UI uiId ("" = any Yog UI). */
    public static void closeIfOpen(String uiId) {
        if (Minecraft.getInstance().screen instanceof YogUIScreen s
                && (uiId.isEmpty() || s.uiId.equals(uiId))) {
            s.onClose();
        }
    }

    public static void open(String uiId, boolean modal, boolean pause) {
        Minecraft.getInstance().setScreen(new YogUIScreen(uiId, modal, pause));
    }

    /** Opens a Yog UI with `ui_flags` bits (1 = modal, 2 = pause, 4 = no background). */
    public static void openFlags(String uiId, int flags) {
        YogUIScreen s = new YogUIScreen(uiId, (flags & 1) != 0, (flags & 2) != 0);
        s.noBackground = (flags & 4) != 0;
        Minecraft.getInstance().setScreen(s);
    }
}
