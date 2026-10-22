package dev.yog;

import dev.yog.util.YogInventoryDef;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.input.CharacterEvent;
import net.minecraft.client.input.KeyEvent;
import net.minecraft.client.input.MouseButtonEvent;
import net.minecraft.network.chat.Component;
import net.minecraft.world.entity.player.Inventory;

public class YogInventoryScreen extends net.minecraft.client.gui.screens.inventory.AbstractContainerScreen<YogInventoryMenu> {
    /** Vanilla's container footprint, the least a Yog inventory screen takes. */
    private static final int VANILLA_W = 176, VANILLA_H = 166;
    /** Where vanilla puts the title label, and how far above the player's inventory its label sits. */
    private static final int TITLE_X = 8, TITLE_Y = 6, INVENTORY_LABEL_GAP = 10;
    /** A key's press, as GLFW reports it (not its release or repeat). */
    private static final int KEY_PRESS = 1;

    /** The inventory screen's UI id — its `InventoryDef` id. */
    final String uiId;
    private int lastMx = Integer.MIN_VALUE, lastMy = Integer.MIN_VALUE;

    public YogInventoryScreen(YogInventoryMenu handler, Inventory inv, Component title) {
        // A custom `on_ui_render` overlay may draw its decoration at a
        // different footprint than vanilla's 176x166 default — when it
        // does (`background_size` set), match it here so the real
        // vanilla `Slot`s (offset from this screen's `x`/`y`, computed
        // from imageWidth/Height in `init()`) land under that
        // decoration instead of the two disagreeing about where the
        // container's top-left corner is.
        super(handler, inv, title, width(YogContentRegistry.INVENTORY_DEFS.get(handler.defId)), height(YogContentRegistry.INVENTORY_DEFS.get(handler.defId)));
        this.uiId = handler.defId;
        YogInventoryDef def = YogContentRegistry.INVENTORY_DEFS.get(handler.defId);
        if (def != null) {
            this.titleLabelX = TITLE_X;
            this.titleLabelY = TITLE_Y;
            if (def.includePlayerInventory()) {
                this.inventoryLabelX = (int) def.playerInvX();
                this.inventoryLabelY = (int) def.playerInvY() - INVENTORY_LABEL_GAP;
            }
        }
        NativeBridge.nativeUIShow(uiId, "", true, false, width, height);
    }

    private static int width(YogInventoryDef def) {
        return def != null && def.backgroundW() > 0 ? (int) def.backgroundW() : VANILLA_W;
    }

    private static int height(YogInventoryDef def) {
        return def != null && def.backgroundH() > 0 ? (int) def.backgroundH() : VANILLA_H;
    }

    /** The screen's markup is the background; the game draws the real slots' items over it. */
    @Override public void extractBackground(GuiGraphicsExtractor ctx, int mx, int my, float delta) {
        super.extractBackground(ctx, mx, my, delta);
        NativeDraw.ui(ctx, uiId, this.width, this.height);
    }

    @Override public void extractRenderState(GuiGraphicsExtractor ctx, int mx, int my, float delta) {
        NativeBridge.activeInventoryMenu = this.menu;
        if (mx != lastMx || my != lastMy) {
            lastMx = mx;
            lastMy = my;
            NativeBridge.nativeUIMove(uiId, mx, my);
        }
        super.extractRenderState(ctx, mx, my, delta);
        // What the screen draws over the game's items (its own tooltips), a layer above them.
        ctx.nextStratum();
        NativeDraw.ui(ctx, uiId + "#overlay", this.width, this.height);
    }

    @Override protected void extractLabels(GuiGraphicsExtractor ctx, int mx, int my) {}

    @Override public boolean mouseClicked(MouseButtonEvent e, boolean doubleClick) {
        if (NativeBridge.nativeUIClick(uiId, (float) e.x(), (float) e.y(), e.button())) return true;
        return super.mouseClicked(e, doubleClick);
    }
    @Override public boolean mouseReleased(MouseButtonEvent e) {
        NativeBridge.nativeUIRelease(uiId, (float) e.x(), (float) e.y());
        return super.mouseReleased(e);
    }
    @Override public boolean mouseDragged(MouseButtonEvent e, double dx, double dy) {
        NativeBridge.nativeUIDrag(uiId, (float) e.x(), (float) e.y());
        return super.mouseDragged(e, dx, dy);
    }
    @Override public boolean mouseScrolled(double mx, double my, double h, double v) {
        NativeBridge.nativeUIScroll(uiId, (float) v);
        return true;
    }
    @Override public boolean charTyped(CharacterEvent e) {
        boolean used = false;
        for (char c : Character.toChars(e.codepoint())) used |= NativeBridge.nativeUIChar(uiId, c, 0);
        return used || super.charTyped(e);
    }
    @Override public boolean keyPressed(KeyEvent e) {
        // A key the screen used (typing in a text box) doesn't close it.
        if (NativeBridge.nativeUIKey(uiId, e.key(), e.scancode(), e.modifiers(), KEY_PRESS)) return true;
        return super.keyPressed(e);
    }
    @Override public void onClose() {
        NativeBridge.activeInventoryMenu = null;
        NativeBridge.nativeUIHide(uiId);
        super.onClose();
        if (this.menu != null) this.menu.removed(this.minecraft.player);
    }
}
