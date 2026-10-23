package dev.yog.client;

import dev.yog.NativeBridge;
import dev.yog.render.NativeDraw;

import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.input.CharacterEvent;
import net.minecraft.client.input.KeyEvent;
import net.minecraft.client.input.MouseButtonEvent;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.network.chat.Component;

/** Minecraft Screen hosting a Rust Yog UI. Supports modal, pause, layering. */
public class YogUIScreen extends Screen {
    private final String uiId;
    private final boolean modal;
    private final boolean pauseGame;
    /** No dimmed/blurred background: the world stays visible (`ui_flags::NO_BACKGROUND`). */
    private boolean noBackground;
    /** A key's press, as GLFW reports it (not its release or repeat). */
    private static final int KEY_PRESS = 1;
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

    @Override public void extractRenderState(GuiGraphicsExtractor ctx, int mx, int my, float delta) {
        if (mx != lastMx || my != lastMy) {
            lastMx = mx;
            lastMy = my;
            NativeBridge.nativeUIMove(uiId, mx, my);
        }
        NativeDraw.ui(ctx, uiId, this.width, this.height);
        super.extractRenderState(ctx, mx, my, delta);
    }

    /** The dimmed, blurred world behind the UI, unless it asked for none. */
    @Override
    public void extractBackground(GuiGraphicsExtractor ctx, int mouseX, int mouseY, float delta) {
        if (!noBackground) super.extractBackground(ctx, mouseX, mouseY, delta);
    }

    @Override public boolean mouseClicked(MouseButtonEvent e, boolean doubleClick) {
        NativeBridge.nativeUIClick(uiId, (float) e.x(), (float) e.y(), e.button());
        return true; // always consume — prevent game from processing the click
    }

    @Override public boolean mouseScrolled(double mx, double my, double horizontal, double vertical) {
        NativeBridge.nativeUIScroll(uiId, (float) vertical);
        return true;
    }

    @Override public boolean mouseDragged(MouseButtonEvent e, double dx, double dy) {
        NativeBridge.nativeUIDrag(uiId, (float) e.x(), (float) e.y());
        return true;
    }

    @Override public boolean mouseReleased(MouseButtonEvent e) {
        NativeBridge.nativeUIRelease(uiId, (float) e.x(), (float) e.y());
        return true;
    }

    @Override public boolean keyPressed(KeyEvent e) {
        // The mod may use the key (a focused text box takes Esc to unfocus).
        if (NativeBridge.nativeUIKey(uiId, e.key(), e.scancode(), e.modifiers(), KEY_PRESS)) return true;
        return super.keyPressed(e);
    }

    @Override public boolean charTyped(CharacterEvent e) {
        boolean used = false;
        for (char c : Character.toChars(e.codepoint())) used |= NativeBridge.nativeUIChar(uiId, c, 0);
        return used || super.charTyped(e);
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

    /** Opens Yog UI uiId on the render thread, from any thread ({@code YogUiActions}). */
    public static void openLater(String uiId, boolean modal, boolean pause) {
        Minecraft.getInstance().execute(() -> open(uiId, modal, pause));
    }

    /** Closes Yog UI uiId ("" = any) — queued, so the caller's handler finishes first. */
    public static void closeLater(String uiId) {
        Minecraft.getInstance().execute(() -> closeIfOpen(uiId));
    }

    /** Closes the open screen if it's the Yog UI uiId ("" = any Yog UI). */
    public static void closeIfOpen(String uiId) {
        if (Minecraft.getInstance().gui.screen() instanceof YogUIScreen s
                && (uiId.isEmpty() || s.uiId.equals(uiId))) {
            s.onClose();
        }
        if (Minecraft.getInstance().gui.screen() instanceof YogInventoryScreen s
                && (uiId.isEmpty() || s.uiId.equals(uiId))) {
            s.onClose();
        }
    }

    public static void open(String uiId, boolean modal, boolean pause) {
        Minecraft.getInstance().gui.setScreen(new YogUIScreen(uiId, modal, pause));
    }

    /** Opens a Yog UI with `ui_flags` bits (1 = modal, 2 = pause, 4 = no background). */
    public static void openFlags(String uiId, int flags) {
        YogUIScreen s = new YogUIScreen(uiId, (flags & 1) != 0, (flags & 2) != 0);
        s.noBackground = (flags & 4) != 0;
        Minecraft.getInstance().gui.setScreen(s);
    }
}
