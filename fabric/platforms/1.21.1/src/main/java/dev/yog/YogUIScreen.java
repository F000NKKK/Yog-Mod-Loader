package dev.yog;

import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.text.Text;

/** Minecraft Screen hosting a Rust Yog UI. Supports modal, pause, layering. */
public class YogUIScreen extends Screen {
    private final String uiId;
    private final boolean modal;
    private final boolean pauseGame;

    public YogUIScreen(String uiId, boolean modal, boolean pauseGame) {
        super(Text.literal(uiId));
        this.uiId = uiId;
        this.modal = modal;
        this.pauseGame = pauseGame;
        NativeBridge.nativeUIShow(uiId, "", modal, pauseGame, width, height);
    }

    public YogUIScreen(String uiId) { this(uiId, true, false); }

    @Override public void render(DrawContext ctx, int mx, int my, float delta) {
        renderBackground(ctx, mx, my, delta);
        ctx.draw();
        NativeDraw.hudDrawContext = ctx;
        NativeBridge.nativeUIRender(uiId, this.width, this.height);
        NativeDraw.hudDrawContext = null;
        NativeDraw.syncGlState(); // raw GL from Rust desyncs GlStateManager caches
        super.render(ctx, mx, my, delta);
    }

    @Override public boolean mouseClicked(double mx, double my, int button) {
        NativeBridge.nativeUIClick(uiId, (float) mx, (float) my, button);
        return true; // always consume — prevent game from processing the click
    }

    @Override public boolean keyPressed(int keyCode, int scanCode, int modifiers) {
        NativeBridge.nativeUIKey(uiId, keyCode, scanCode, modifiers, 1);
        return super.keyPressed(keyCode, scanCode, modifiers);
    }

    @Override public boolean shouldPause() { return pauseGame; }

    @Override public void close() {
        NativeBridge.nativeUIHide(uiId);
        super.close();
    }

    public static void open(String uiId, boolean modal, boolean pause) {
        MinecraftClient.getInstance().setScreen(new YogUIScreen(uiId, modal, pause));
    }
}
