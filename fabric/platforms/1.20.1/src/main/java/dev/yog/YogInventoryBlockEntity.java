package dev.yog;

import net.minecraft.block.entity.BlockEntity;
import net.minecraft.block.BlockState;
import net.minecraft.entity.player.PlayerEntity;
import net.minecraft.inventory.Inventories;
import net.minecraft.inventory.Inventory;
import net.minecraft.item.ItemStack;
import net.minecraft.nbt.NbtCompound;
import net.minecraft.network.PacketByteBuf;
import net.minecraft.screen.PropertyDelegate;
import net.minecraft.screen.ScreenHandler;
import net.minecraft.server.network.ServerPlayerEntity;
import net.minecraft.text.Text;
import net.minecraft.util.collection.DefaultedList;
import net.minecraft.util.math.BlockPos;

import net.fabricmc.fabric.api.screenhandler.v1.ExtendedScreenHandlerFactory;

/**
 * Generic inventory-backed block entity for any Yog block declaring
 * `.inventory(id)` — see rust/crates/yog-inventory/DESIGN.md. One instance
 * per block position; `defId` names the `InventoryDef` (slot count, layout,
 * player-inventory inclusion) it was built from, looked up from
 * {@link YogHost#INVENTORY_DEFS}.
 */
public class YogInventoryBlockEntity extends BlockEntity implements Inventory, ExtendedScreenHandlerFactory {
    private final String defId;
    private DefaultedList<ItemStack> items;

    public YogInventoryBlockEntity(BlockPos pos, BlockState state, String defId, int slotCount) {
        super(YogHost.INVENTORY_BLOCK_ENTITY_TYPE, pos, state);
        this.defId = defId;
        this.items = DefaultedList.ofSize(slotCount, ItemStack.EMPTY);
    }

    public String defId() { return defId; }

    @Override public int size() { return items.size(); }
    @Override public boolean isEmpty() {
        for (ItemStack s : items) if (!s.isEmpty()) return false;
        return true;
    }
    @Override public ItemStack getStack(int slot) { return items.get(slot); }
    @Override public ItemStack removeStack(int slot, int amount) {
        ItemStack r = Inventories.splitStack(items, slot, amount);
        if (!r.isEmpty()) markDirty();
        return r;
    }
    @Override public ItemStack removeStack(int slot) { return Inventories.removeStack(items, slot); }
    @Override public void setStack(int slot, ItemStack stack) {
        items.set(slot, stack);
        if (stack.getCount() > getMaxCountPerStack()) stack.setCount(getMaxCountPerStack());
    }
    @Override public void markDirty() { super.markDirty(); }
    @Override public void clear() {
        for (int i = 0; i < items.size(); i++) items.set(i, ItemStack.EMPTY);
    }
    @Override public boolean canPlayerUse(PlayerEntity player) {
        return world != null && world.getBlockEntity(pos) == this
                && player.squaredDistanceTo(pos.getX() + 0.5, pos.getY() + 0.5, pos.getZ() + 0.5) <= 64.0;
    }

    @Override
    public void readNbt(NbtCompound nbt) {
        super.readNbt(nbt);
        items = DefaultedList.ofSize(size(), ItemStack.EMPTY);
        Inventories.readNbt(nbt, items);
    }

    @Override
    protected void writeNbt(NbtCompound nbt) {
        super.writeNbt(nbt);
        Inventories.writeNbt(nbt, items);
    }

    @Override
    public Text getDisplayName() {
        YogHost.InventoryDefRt def = YogHost.INVENTORY_DEFS.get(defId);
        String title = def != null ? def.title : "";
        return title == null || title.isEmpty() ? Text.literal("Inventory") : Text.literal(title);
    }

    @Override
    public ScreenHandler createMenu(int syncId, net.minecraft.entity.player.PlayerInventory playerInv, PlayerEntity player) {
        return YogInventoryMenu.create(syncId, playerInv, this, defId);
    }

    @Override
    public void writeScreenOpeningData(ServerPlayerEntity player, PacketByteBuf buf) {
        buf.writeString(defId);
    }

    /** Unused — Yog inventories have no synced int properties (progress bars, etc.) yet. */
    public static final PropertyDelegate EMPTY_PROPERTIES = new PropertyDelegate() {
        @Override public int get(int index) { return 0; }
        @Override public void set(int index, int value) { }
        @Override public int size() { return 0; }
    };
}
