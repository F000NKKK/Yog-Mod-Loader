package dev.yog;

import net.minecraft.client.gui.GuiGraphics;
import net.minecraft.network.chat.Component;
import net.minecraft.world.entity.player.Inventory;

/**
 * Inventory screen powered by yog-ui — renders everything (background, slot
 * frames, slot items, custom widgets) through Rust's flexbox layout engine.
 * Vanilla still handles slot *interaction* (drag-and-drop, quick-move,
 * tooltips), but all pixel output comes from {@code NativeBridge.nativeUIRender}.
 *
 * See rust/crates/yog-inventory/DESIGN.md and rust/crates/yog-ui/.
 */
public class YogInventoryScreen extends net.minecraft.client.gui.screens.inventory.AbstractContainerScreen<YogInventoryMenu> {
    private final String uiId;

    public YogInventoryScreen(YogInventoryMenu menu, Inventory inv, Component title) {
        super(menu, inv, title);
        this.uiId = "yog:inv/" + menu.defId;
        YogHost.InventoryDefRt def = YogHost.INVENTORY_DEFS.get(menu.defId);
        if (def != null) {
            this.imageWidth  = Math.max(176, this.imageWidth);
            this.imageHeight = Math.max(166, this.imageHeight);
            this.titleLabelX = 8;
            this.titleLabelY = 6;
            if (def.includePlayerInventory) {
                this.inventoryLabelX = (int) def.playerInvX;
                this.inventoryLabelY = (int) def.playerInvY - 10;
            }
        }
        NativeBridge.nativeUIShow(uiId, "", true, false, width, height);
    }

    @Override
    public void render(GuiGraphics ctx, int mx, int my, float delta) {
        // Let yog-ui draw the entire screen: background, slot frames, items, widgets.
        NativeBridge.activeInventoryMenu = this.menu;
        NativeDraw.hudDrawContext = ctx;
        NativeBridge.nativeUIRender(uiId, this.width, this.height);
        NativeDraw.hudDrawContext = null;
        NativeDraw.syncGlState();

        // Vanilla still draws tooltips on top.
        this.renderTooltip(ctx, mx, my);
    }

    @Override
    protected void renderBg(GuiGraphics ctx, float delta, int mx, int my) {
        // no-op — yog-ui handles the background.
    }

    @Override
    protected void renderLabels(GuiGraphics ctx, int mx, int my) {
        // no-op — yog-ui renders text.
    }

    @Override
    public boolean mouseClicked(double mx, double my, int button) {
        // Try yog-ui first (buttons, scroll areas). If consumed, done.
        if (NativeBridge.nativeUIClick(uiId, (float) mx, (float) my, button))
            return true;
        // Not a yog-ui widget — let vanilla handle slot interaction.
        return super.mouseClicked(mx, my, button);
    }

    @Override
    public boolean mouseReleased(double mx, double my, int button) {
        if (NativeBridge.nativeUIRelease(uiId, (float) mx, (float) my))
            return true;
        return super.mouseReleased(mx, my, button);
    }

    @Override
    public boolean mouseDragged(double mx, double my, int button, double dx, double dy) {
        NativeBridge.nativeUIDrag(uiId, (float) mx, (float) my);
        return super.mouseDragged(mx, my, button, dx, dy);
    }

    @Override
    public boolean mouseScrolled(double mx, double my, double horiz, double vert) {
        NativeBridge.nativeUIScroll(uiId, (float) vert);
        return true;
    }

    @Override
    public boolean keyPressed(int keyCode, int scanCode, int modifiers) {
        NativeBridge.nativeUIKey(uiId, keyCode, scanCode, modifiers, 1);
        return super.keyPressed(keyCode, scanCode, modifiers);
    }

    @Override
    public void onClose() {
        NativeBridge.activeInventoryMenu = null;
        NativeBridge.nativeUIHide(uiId);
        super.onClose();
        // Propagate close to the block entity
        if (this.menu != null) this.menu.removed(minecraft.player);
    }
}
