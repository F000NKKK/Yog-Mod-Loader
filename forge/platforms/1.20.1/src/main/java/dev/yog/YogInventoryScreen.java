package dev.yog;

import net.minecraft.client.gui.GuiGraphics;
import net.minecraft.network.chat.Component;
import net.minecraft.world.entity.player.Inventory;

public class YogInventoryScreen extends net.minecraft.client.gui.screens.inventory.AbstractContainerScreen<YogInventoryMenu> {
    private final String uiId;

    public YogInventoryScreen(YogInventoryMenu menu, Inventory inv, Component title) {
        super(menu, inv, title);
        this.uiId = "yog:inv/" + menu.defId;
        YogHost.InventoryDefRt def = YogHost.INVENTORY_DEFS.get(menu.defId);
        if (def != null) {
            this.imageWidth  = Math.max(176, this.imageWidth);
            this.imageHeight = Math.max(166, this.imageHeight);
            this.titleLabelX = 8; this.titleLabelY = 6;
            if (def.includePlayerInventory) {
                this.inventoryLabelX = (int) def.playerInvX;
                this.inventoryLabelY = (int) def.playerInvY - 10;
            }
        }
        NativeBridge.nativeUIShow(uiId, "", true, false, width, height);
    }

    @Override public void render(GuiGraphics ctx, int mx, int my, float delta) {
        NativeBridge.activeInventoryMenu = this.menu;
        NativeDraw.hudDrawContext = ctx;
        NativeBridge.nativeUIRender(uiId, this.width, this.height);
        NativeDraw.hudDrawContext = null;
        NativeDraw.syncGlState();
        this.renderTooltip(ctx, mx, my);
    }

    @Override protected void renderBg(GuiGraphics ctx, float d, int mx, int my) {}
    @Override protected void renderLabels(GuiGraphics ctx, int mx, int my) {}

    @Override public boolean mouseClicked(double mx, double my, int button) {
        NativeBridge.nativeUIClick(uiId, (float) mx, (float) my, button);
        return super.mouseClicked(mx, my, button);
    }
    @Override public boolean mouseReleased(double mx, double my, int button) {
        NativeBridge.nativeUIRelease(uiId, (float) mx, (float) my);
        return super.mouseReleased(mx, my, button);
    }
    @Override public boolean mouseDragged(double mx, double my, int b, double dx, double dy) {
        NativeBridge.nativeUIDrag(uiId, (float) mx, (float) my);
        return super.mouseDragged(mx, my, b, dx, dy);
    }
    public boolean mouseScrolled(double mx, double my, double h, double v) {
        NativeBridge.nativeUIScroll(uiId, (float) v);
        return true;
    }
    @Override public boolean keyPressed(int k, int s, int m) {
        NativeBridge.nativeUIKey(uiId, k, s, m, 1);
        return super.keyPressed(k, s, m);
    }
    @Override public void onClose() {
        NativeBridge.activeInventoryMenu = null;
        NativeBridge.nativeUIHide(uiId);
        super.onClose();
        if (this.menu != null) this.menu.removed(minecraft.player);
    }
}
