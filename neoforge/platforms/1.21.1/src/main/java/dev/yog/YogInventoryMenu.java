package dev.yog;

import net.minecraft.world.Container;
import net.minecraft.world.SimpleContainer;
import net.minecraft.world.entity.player.Inventory;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.inventory.AbstractContainerMenu;
import net.minecraft.world.inventory.Slot;
import net.minecraft.world.item.ItemStack;

/**
 * Generic Container/Menu for any Yog inventory-backed block — see
 * rust/crates/yog-inventory/DESIGN.md. Slot layout (custom slots + optional
 * player-inventory grid underneath) comes from the matching `InventoryDef`
 * (cached in {@link YogHost#INVENTORY_DEFS}), shared identically by client
 * and server since mod registration code runs the same on both sides — only
 * slot *contents* need syncing, which vanilla's `AbstractContainerMenu` does for us.
 */
public class YogInventoryMenu extends AbstractContainerMenu {
    public final Container inventory;
    public final String defId;
    private final int customSlotCount;

    /** Server-side: opened against the block's real backing inventory. */
    public static YogInventoryMenu create(int windowId, Inventory playerInv, Container inv, String defId) {
        return new YogInventoryMenu(windowId, playerInv, inv, defId);
    }

    /** Client-side factory registered via {@code IMenuTypeExtension.create(...)} — builds a
     * throwaway inventory just to hold synced slot contents (same convention vanilla's
     * generic chest menu uses). */
    public static YogInventoryMenu createClient(int windowId, Inventory playerInv, net.minecraft.network.RegistryFriendlyByteBuf buf) {
        String defId = buf.readUtf();
        YogHost.InventoryDefRt def = YogHost.INVENTORY_DEFS.get(defId);
        int slotCount = def != null ? def.slotCount : 0;
        return new YogInventoryMenu(windowId, playerInv, new SimpleContainer(slotCount), defId);
    }

    private YogInventoryMenu(int windowId, Inventory playerInv, Container inv, String defId) {
        super(YogHost.INVENTORY_MENU_TYPE, windowId);
        this.inventory = inv;
        this.defId = defId;
        YogHost.InventoryDefRt def = YogHost.INVENTORY_DEFS.get(defId);
        this.customSlotCount = def != null ? def.slotCount : inv.getContainerSize();
        inv.startOpen(playerInv.player);

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

    private void addPlayerInventorySlots(Inventory playerInv, float ox, float oy) {
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
    public ItemStack quickMoveStack(Player player, int index) {
        ItemStack result = ItemStack.EMPTY;
        Slot slot = this.slots.get(index);
        if (slot != null && slot.hasItem()) {
            ItemStack stack = slot.getItem();
            result = stack.copy();
            if (index < customSlotCount) {
                if (!this.moveItemStackTo(stack, customSlotCount, this.slots.size(), true)) {
                    return ItemStack.EMPTY;
                }
            } else {
                if (!this.moveItemStackTo(stack, 0, customSlotCount, false)) {
                    return ItemStack.EMPTY;
                }
            }
            if (stack.isEmpty()) slot.set(ItemStack.EMPTY);
            else slot.setChanged();
            if (stack.getCount() == result.getCount()) return ItemStack.EMPTY;
            slot.onTake(player, stack);
        }
        return result;
    }

    @Override
    public boolean stillValid(Player player) {
        return inventory.stillValid(player);
    }

    @Override
    public void removed(Player player) {
        super.removed(player);
        inventory.stopOpen(player);
    }
}
