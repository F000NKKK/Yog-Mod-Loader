package dev.yog;

import net.minecraft.client.gui.DrawContext;
import net.minecraft.entity.player.PlayerInventory;
import net.minecraft.text.Text;
import net.minecraft.util.Identifier;

/**
 * Generic Container/Menu screen for {@link YogInventoryMenu} — draws a
 * native-looking dark panel (or a mod-supplied custom texture) sized to the
 * def's slot layout, with a light slot-shaped backdrop behind each slot.
 * See rust/crates/yog-inventory/DESIGN.md.
 */
public class YogInventoryScreen extends net.minecraft.client.gui.screen.ingame.HandledScreen<YogInventoryMenu> {
    private static final int PANEL_BG    = 0xFF_C6C6C6;
    private static final int PANEL_BORDER = 0xFF_373737;
    private static final int SLOT_BG     = 0xFF_8B8B8B;
    private static final int TITLE_COLOR = 0xFF_404040;

    private final YogHost.InventoryDefRt def;

    public YogInventoryScreen(YogInventoryMenu handler, PlayerInventory inv, Text title) {
        super(handler, inv, title);
        this.def = YogHost.INVENTORY_DEFS.get(handler.defId);

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
        this.backgroundWidth = (int) maxX + 8;
        this.backgroundHeight = (int) maxY + 8;
        this.titleX = 8;
        this.titleY = 6;
        if (def != null && def.includePlayerInventory) {
            this.playerInventoryTitleX = (int) def.playerInvX;
            this.playerInventoryTitleY = (int) def.playerInvY - 10;
        }
    }

    @Override
    protected void drawBackground(DrawContext ctx, float delta, int mouseX, int mouseY) {
        int x = (this.width - this.backgroundWidth) / 2;
        int y = (this.height - this.backgroundHeight) / 2;

        if (def != null && def.backgroundTexture != null && !def.backgroundTexture.isEmpty()) {
            Identifier id = Identifier.tryParse(def.backgroundTexture);
            if (id != null) {
                ctx.drawTexture(id, x, y, 0f, 0f, this.backgroundWidth, this.backgroundHeight,
                        this.backgroundWidth, this.backgroundHeight);
                return;
            }
        }

        ctx.fill(x, y, x + backgroundWidth, y + backgroundHeight, PANEL_BG);
        ctx.fill(x, y, x + backgroundWidth, y + 1, PANEL_BORDER);
        ctx.fill(x, y + backgroundHeight - 1, x + backgroundWidth, y + backgroundHeight, PANEL_BORDER);
        ctx.fill(x, y, x + 1, y + backgroundHeight, PANEL_BORDER);
        ctx.fill(x + backgroundWidth - 1, y, x + backgroundWidth, y + backgroundHeight, PANEL_BORDER);

        for (var slot : this.handler.slots) {
            int sx = x + slot.x - 1;
            int sy = y + slot.y - 1;
            ctx.fill(sx, sy, sx + 18, sy + 18, SLOT_BG);
        }
    }

    @Override
    public void render(DrawContext ctx, int mouseX, int mouseY, float delta) {
        this.renderBackground(ctx);
        super.render(ctx, mouseX, mouseY, delta);
        this.drawMouseoverTooltip(ctx, mouseX, mouseY);
    }
}
