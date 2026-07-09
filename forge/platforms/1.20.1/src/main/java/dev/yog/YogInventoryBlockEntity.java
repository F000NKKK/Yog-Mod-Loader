package dev.yog;

import net.minecraft.core.BlockPos;
import net.minecraft.core.NonNullList;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.network.chat.Component;
import net.minecraft.world.Container;
import net.minecraft.world.ContainerHelper;
import net.minecraft.world.MenuProvider;
import net.minecraft.world.entity.player.Inventory;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.inventory.AbstractContainerMenu;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.level.block.entity.BlockEntity;
import net.minecraft.world.level.block.state.BlockState;

/**
 * Generic inventory-backed block entity for any Yog block declaring
 * `.inventory(id)` — see rust/crates/yog-inventory/DESIGN.md. One instance
 * per block position; `defId` names the `InventoryDef` (slot count, layout,
 * player-inventory inclusion) it was built from, looked up from
 * {@link YogHost#INVENTORY_DEFS}.
 */
public class YogInventoryBlockEntity extends BlockEntity implements Container, MenuProvider {
    private final String defId;
    private NonNullList<ItemStack> items;

    public YogInventoryBlockEntity(BlockPos pos, BlockState state, String defId, int slotCount) {
        super(YogHost.INVENTORY_BLOCK_ENTITY_TYPE, pos, state);
        this.defId = defId;
        this.items = NonNullList.withSize(slotCount, ItemStack.EMPTY);
    }

    public String defId() { return defId; }

    @Override public int getContainerSize() { return items.size(); }
    @Override public boolean isEmpty() {
        for (ItemStack s : items) if (!s.isEmpty()) return false;
        return true;
    }
    @Override public ItemStack getItem(int slot) { return items.get(slot); }
    @Override public ItemStack removeItem(int slot, int amount) {
        ItemStack r = ContainerHelper.removeItem(items, slot, amount);
        if (!r.isEmpty()) setChanged();
        return r;
    }
    @Override public ItemStack removeItemNoUpdate(int slot) { return ContainerHelper.takeItem(items, slot); }
    @Override public void setItem(int slot, ItemStack stack) {
        items.set(slot, stack);
        if (stack.getCount() > getMaxStackSize()) stack.setCount(getMaxStackSize());
        setChanged();
    }
    @Override public void clearContent() { items.clear(); }
    @Override public boolean stillValid(Player player) {
        return level != null && level.getBlockEntity(worldPosition) == this
                && player.distanceToSqr(worldPosition.getX() + 0.5, worldPosition.getY() + 0.5, worldPosition.getZ() + 0.5) <= 64.0;
    }

    @Override
    public void load(CompoundTag tag) {
        super.load(tag);
        items = NonNullList.withSize(getContainerSize(), ItemStack.EMPTY);
        ContainerHelper.loadAllItems(tag, items);
    }

    @Override
    protected void saveAdditional(CompoundTag tag) {
        super.saveAdditional(tag);
        ContainerHelper.saveAllItems(tag, items);
    }

    @Override
    public Component getDisplayName() {
        YogHost.InventoryDefRt def = YogHost.INVENTORY_DEFS.get(defId);
        String title = def != null ? def.title : "";
        return title == null || title.isEmpty() ? Component.literal("Inventory") : Component.literal(title);
    }

    @Override
    public AbstractContainerMenu createMenu(int windowId, Inventory playerInv, Player player) {
        return YogInventoryMenu.create(windowId, playerInv, this, defId);
    }

    /** Save this block entity's items to an {@link ItemStack} so the inventory
     *  survives block breaking — the shulker-box pattern (phase 6). */
    public void saveToItemStack(ItemStack stack) {
        if (isEmpty()) return;
        CompoundTag tag = new CompoundTag();
        saveAdditional(tag);
        tag.remove("id");
        tag.remove("x");
        tag.remove("y");
        tag.remove("z");
        net.minecraft.world.item.BlockItem.setBlockEntityData(stack,
                YogHost.INVENTORY_BLOCK_ENTITY_TYPE, tag);
    }
}
