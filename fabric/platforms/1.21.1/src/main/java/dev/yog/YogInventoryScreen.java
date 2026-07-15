package dev.yog;

import net.minecraft.client.gui.DrawContext;
import net.minecraft.entity.player.PlayerInventory;
import net.minecraft.text.Text;

public class YogInventoryScreen extends net.minecraft.client.gui.screen.ingame.HandledScreen<YogInventoryMenu> {
    private final String uiId;

    public YogInventoryScreen(YogInventoryMenu handler, PlayerInventory inv, Text title) {
        super(handler, inv, title);
        this.uiId = "yog:inv/" + handler.defId;
        YogHost.InventoryDefRt def = YogHost.INVENTORY_DEFS.get(handler.defId);
        if (def != null) {
            // A custom `on_ui_render` overlay may draw its decoration at a
            // different footprint than vanilla's 176x166 default — when it
            // does (`background_size` set), match it here so the real
            // vanilla `Slot`s (offset from this screen's `x`/`y`, computed
            // from backgroundWidth/Height in `init()`) land under that
            // decoration instead of the two disagreeing about where the
            // container's top-left corner is.
            this.backgroundWidth  = def.backgroundW > 0 ? (int) def.backgroundW : Math.max(176, this.backgroundWidth);
            this.backgroundHeight = def.backgroundH > 0 ? (int) def.backgroundH : Math.max(166, this.backgroundHeight);
            this.titleX = 8; this.titleY = 6;
            if (def.includePlayerInventory) {
                this.playerInventoryTitleX = (int) def.playerInvX;
                this.playerInventoryTitleY = (int) def.playerInvY - 10;
            }
        }
        NativeBridge.nativeUIShow(uiId, "", true, false, width, height);
    }

    @Override public void render(DrawContext ctx, int mx, int my, float delta) {
        NativeBridge.activeInventoryMenu = this.handler;
        NativeDraw.hudDrawContext = ctx;
        NativeBridge.nativeUIRender(uiId, this.width, this.height);
        NativeDraw.hudDrawContext = null;
        NativeDraw.syncGlState();
        this.drawMouseoverTooltip(ctx, mx, my);
    }

    @Override protected void drawBackground(DrawContext ctx, float d, int mx, int my) {}
    @Override protected void drawForeground(DrawContext ctx, int mx, int my) {}

    @Override public boolean mouseClicked(double mx, double my, int b) {
        if (NativeBridge.nativeUIClick(uiId, (float) mx, (float) my, b))
            return true;
        return super.mouseClicked(mx, my, b);
    }
    @Override public boolean mouseReleased(double mx, double my, int b) {
        NativeBridge.nativeUIRelease(uiId, (float) mx, (float) my);
        return super.mouseReleased(mx, my, b);
    }
    @Override public boolean mouseDragged(double mx, double my, int b, double dx, double dy) {
        NativeBridge.nativeUIDrag(uiId, (float) mx, (float) my);
        return super.mouseDragged(mx, my, b, dx, dy);
    }
    @Override public boolean mouseScrolled(double mx, double my, double h, double v) {
        NativeBridge.nativeUIScroll(uiId, (float) v);
        return true;
    }
    @Override public boolean keyPressed(int k, int s, int m) {
        NativeBridge.nativeUIKey(uiId, k, s, m, 1);
        return super.keyPressed(k, s, m);
    }
    @Override public void close() {
        NativeBridge.activeInventoryMenu = null;
        NativeBridge.nativeUIHide(uiId);
        super.close();
        if (this.handler != null) this.handler.onClosed(this.client.player);
    }
}
