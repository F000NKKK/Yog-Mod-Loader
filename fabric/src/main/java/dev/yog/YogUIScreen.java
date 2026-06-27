package dev.yog;

import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.text.Text;

/**
 * A Minecraft Screen that hosts a Rust-side Yog UI.
 * The Rust mod renders via yog-gfx draw2d; this screen just
 * captures input and forwards it.
 */
public class YogUIScreen extends Screen {
    private final String uiId;

    public YogUIScreen(String uiId) {
        super(Text.literal(uiId));
        this.uiId = uiId;
        // Tell Rust to show this UI
        NativeBridge.nativeUIShow(uiId, width, height);
    }

    @Override
    public void render(DrawContext ctx, int mx, int my, float delta) {
        // Background
        renderBackground(ctx);
        NativeBridge.nativeUIRender(uiId);
        super.render(ctx, mx, my, delta);
    }

    @Override
    public boolean mouseClicked(double mx, double my, int button) {
        NativeBridge.nativeUIClick(uiId, (float) mx, (float) my, button);
        return true;
    }

    @Override
    public boolean keyPressed(int keyCode, int scanCode, int modifiers) {
        NativeBridge.nativeUIKey(uiId, keyCode, scanCode, modifiers, 1);
        return super.keyPressed(keyCode, scanCode, modifiers);
    }

    @Override
    public boolean keyReleased(int keyCode, int scanCode, int modifiers) {
        NativeBridge.nativeUIKey(uiId, keyCode, scanCode, modifiers, 0);
        return super.keyReleased(keyCode, scanCode, modifiers);
    }

    @Override
    public boolean shouldPause() { return false; }

    @Override
    public void close() {
        NativeBridge.nativeUIHide(uiId);
        super.close();
    }

    /** Open a UI screen by its id. Called from Rust via JNI. */
    public static void open(String uiId) {
        MinecraftClient.getInstance().setScreen(new YogUIScreen(uiId));
    }
}
