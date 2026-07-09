package dev.yog;

import net.minecraft.entity.player.PlayerEntity;
import net.minecraft.entity.player.PlayerInventory;
import net.minecraft.inventory.Inventory;
import net.minecraft.inventory.SimpleInventory;
import net.minecraft.item.ItemStack;
import net.minecraft.screen.ScreenHandler;
import net.minecraft.screen.slot.Slot;

/**
 * Generic Container/Menu for any Yog inventory-backed block — see
 * rust/crates/yog-inventory/DESIGN.md. Slot layout (custom slots + optional
 * player-inventory grid underneath) comes from the matching `InventoryDef`
 * (cached in {@link YogHost#INVENTORY_DEFS}), shared identically by client
 * and server since mod registration code runs the same on both sides — only
 * slot *contents* need syncing, which vanilla's `ScreenHandler` does for us.
 */
public class YogInventoryMenu extends ScreenHandler {
    public final Inventory inventory;
    public final String defId;
    private final int customSlotCount;

    /** Server-side: opened against the block's real backing inventory. */
    public static YogInventoryMenu create(int syncId, PlayerInventory playerInv, Inventory inv, String defId) {
        return new YogInventoryMenu(syncId, playerInv, inv, defId);
    }

    /** Client-side factory for {@link YogHost#INVENTORY_SCREEN_HANDLER_TYPE} — builds a
     * throwaway inventory just to hold synced slot contents (same convention vanilla's
     * generic chest screen handler uses). */
    public static YogInventoryMenu createClient(int syncId, PlayerInventory playerInv, String defId) {
        YogHost.InventoryDefRt def = YogHost.INVENTORY_DEFS.get(defId);
        int slotCount = def != null ? def.slotCount : 0;
        return new YogInventoryMenu(syncId, playerInv, new SimpleInventory(slotCount), defId);
    }

    private YogInventoryMenu(int syncId, PlayerInventory playerInv, Inventory inv, String defId) {
        super(YogHost.INVENTORY_SCREEN_HANDLER_TYPE, syncId);
        this.inventory = inv;
        this.defId = defId;
        YogHost.InventoryDefRt def = YogHost.INVENTORY_DEFS.get(defId);
        this.customSlotCount = def != null ? def.slotCount : inv.size();
        inv.onOpen(playerInv.player);

        if (def != null) {
            for (int i = 0; i < def.slotCount; i++) {
                float[] xy = i < def.layout.size() ? def.layout.get(i) : new float[]{8f + (i % 9) * 18f, 18f + (i / 9) * 18f};
                this.addSlot(new Slot(inv, i, (int) xy[0], (int) xy[1]));
            }
            if (def.includePlayerInventory) {
                addPlayerInventorySlots(playerInv, def.playerInvX, def.playerInvY);
            }
        }
    }

    private void addPlayerInventorySlots(PlayerInventory playerInv, float ox, float oy) {
        for (int row = 0; row < 3; row++) {
            for (int col = 0; col < 9; col++) {
                addSlot(new Slot(playerInv, col + row * 9 + 9, (int) ox + col * 18, (int) oy + row * 18));
            }
        }
        for (int col = 0; col < 9; col++) {
            addSlot(new Slot(playerInv, col, (int) ox + col * 18, (int) oy + 58));
        }
    }

    @Override
    public ItemStack quickMove(PlayerEntity player, int index) {
        ItemStack result = ItemStack.EMPTY;
        Slot slot = this.slots.get(index);
        if (slot != null && slot.hasStack()) {
            ItemStack stack = slot.getStack();
            result = stack.copy();
            if (index < customSlotCount) {
                if (!this.insertItem(stack, customSlotCount, this.slots.size(), true)) {
                    return ItemStack.EMPTY;
                }
            } else {
                if (!this.insertItem(stack, 0, customSlotCount, false)) {
                    return ItemStack.EMPTY;
                }
            }
            if (stack.isEmpty()) slot.setStack(ItemStack.EMPTY);
            else slot.markDirty();
            if (stack.getCount() == result.getCount()) return ItemStack.EMPTY;
            slot.onTakeItem(player, stack);
        }
        return result;
    }

    @Override
    public boolean canUse(PlayerEntity player) {
        return inventory.canPlayerUse(player);
    }

    @Override
    public void onClosed(PlayerEntity player) {
        super.onClosed(player);
        inventory.onClose(player);
    }
}
