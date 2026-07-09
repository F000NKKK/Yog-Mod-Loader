package dev.yog;

import net.minecraft.client.gui.GuiGraphics;
import net.minecraft.network.chat.Component;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.world.entity.player.Inventory;
import net.minecraft.world.inventory.Slot;

/**
 * Generic Container/Menu screen for {@link YogInventoryMenu} — draws a
 * native-looking dark panel (or a mod-supplied custom texture) sized to the
 * def's slot layout, with a light slot-shaped backdrop behind each slot.
 * See rust/crates/yog-inventory/DESIGN.md.
 */
public class YogInventoryScreen extends net.minecraft.client.gui.screens.inventory.AbstractContainerScreen<YogInventoryMenu> {
    private static final int PANEL_BG    = 0xFF_C6C6C6;
    private static final int PANEL_BORDER = 0xFF_373737;
    private static final int SLOT_BG     = 0xFF_8B8B8B;

    private final YogHost.InventoryDefRt def;

    public YogInventoryScreen(YogInventoryMenu menu, Inventory inv, Component title) {
        super(menu, inv, title);
        this.def = YogHost.INVENTORY_DEFS.get(menu.defId);

        float maxX = 176f, maxY = 18f;
        if (def != null) {
            for (float[] xy : def.layout) {
                maxX = Math.max(maxX, xy[0] + 18f);
                maxY = Math.max(maxY, xy[1] + 18f);
            }
            if (def.includePlayerInventory) {
                maxY = Math.max(maxY, def.playerInvY + 58f + 18f);
                maxX = Math.max(maxX, def.playerInvX + 9 * 18f);
            }
        }
        this.imageWidth = (int) maxX + 8;
        this.imageHeight = (int) maxY + 8;
        this.titleLabelX = 8;
        this.titleLabelY = 6;
        if (def != null && def.includePlayerInventory) {
            this.inventoryLabelX = (int) def.playerInvX;
            this.inventoryLabelY = (int) def.playerInvY - 10;
        }
    }

    @Override
    protected void renderBg(GuiGraphics gfx, float partialTick, int mouseX, int mouseY) {
        int x = this.leftPos;
        int y = this.topPos;

        if (def != null && def.backgroundTexture != null && !def.backgroundTexture.isEmpty()) {
            ResourceLocation id = ResourceLocation.tryParse(def.backgroundTexture);
            if (id != null) {
                gfx.blit(id, x, y, 0, 0f, 0f, this.imageWidth, this.imageHeight, this.imageWidth, this.imageHeight);
                return;
            }
        }

        gfx.fill(x, y, x + imageWidth, y + imageHeight, PANEL_BG);
        gfx.fill(x, y, x + imageWidth, y + 1, PANEL_BORDER);
        gfx.fill(x, y + imageHeight - 1, x + imageWidth, y + imageHeight, PANEL_BORDER);
        gfx.fill(x, y, x + 1, y + imageHeight, PANEL_BORDER);
        gfx.fill(x + imageWidth - 1, y, x + imageWidth, y + imageHeight, PANEL_BORDER);

        for (Slot slot : this.menu.slots) {
            int sx = x + slot.x - 1;
            int sy = y + slot.y - 1;
            gfx.fill(sx, sy, sx + 18, sy + 18, SLOT_BG);
        }
    }

    @Override
    public void render(GuiGraphics gfx, int mouseX, int mouseY, float partialTick) {
        this.renderBackground(gfx);
        super.render(gfx, mouseX, mouseY, partialTick);
        this.renderTooltip(gfx, mouseX, mouseY);
    }
}
