package dev.yog;

import dev.yog.util.YogInventoryDef;
import net.minecraft.registry.Registries;
import net.minecraft.registry.RegistryKeys;
import net.minecraft.registry.tag.TagKey;
import net.minecraft.util.Identifier;
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
    /** A machine's made result: the menu's, never the block's (see {@link YogInventoryDef#machineResult}). */
    private final SimpleInventory machineOut = new SimpleInventory(1);
    /** Server-side, a machine's block entity; its current recipe and what it was found from. */
    private YogInventoryBlockEntity machine;
    private String machineRecipe;
    private String machineItems = "";
    private int machineTicks;

    /** Server-side: opened against the block's real backing inventory. */
    public static YogInventoryMenu create(int syncId, PlayerInventory playerInv, Inventory inv, String defId) {
        return new YogInventoryMenu(syncId, playerInv, inv, defId);
    }

    /** Client-side factory for {@link YogHost#INVENTORY_SCREEN_HANDLER_TYPE} — builds a
     * throwaway inventory just to hold synced slot contents (same convention vanilla's
     * generic chest screen handler uses). */
    public static YogInventoryMenu createClient(int syncId, PlayerInventory playerInv, String defId) {
        YogInventoryDef def = YogHost.INVENTORY_DEFS.get(defId);
        int slotCount = def != null ? def.slotCount : 0;
        return new YogInventoryMenu(syncId, playerInv, new SimpleInventory(slotCount), defId);
    }

    private YogInventoryMenu(int syncId, PlayerInventory playerInv, Inventory inv, String defId) {
        super(YogHost.INVENTORY_SCREEN_HANDLER_TYPE, syncId);
        this.inventory = inv;
        this.defId = defId;
        YogInventoryDef def = YogHost.INVENTORY_DEFS.get(defId);
        this.customSlotCount = def != null ? def.slotCount : inv.size();
        inv.onOpen(playerInv.player);
        if (def != null && def.isMachine() && inv instanceof YogInventoryBlockEntity be && be.getWorld() != null && !be.getWorld().isClient) {
            this.machine = be;
        }

        if (def != null) {
            for (int i = 0; i < def.slotCount; i++) {
                float[] xy = i < def.layout.size() ? def.layout.get(i) : new float[]{8f + (i % 9) * 18f, 18f + (i / 9) * 18f};
                String[] accepts = i < def.accepts.size() ? def.accepts.get(i) : new String[0];
                this.addSlot(def.isMachine() && i == def.machineResult
                        ? new MachineResultSlot((int) xy[0], (int) xy[1])
                        : accepts.length == 0
                        ? new Slot(inv, i, (int) xy[0], (int) xy[1])
                        : new FilteredSlot(inv, i, (int) xy[0], (int) xy[1], accepts));
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

    /** The machine's items by slot ({@code ""} empty), {@code \n}-joined, as the runtime takes them. */
    private String machineItems() {
        StringBuilder sb = new StringBuilder();
        for (int i = 0; i < inventory.size(); i++) {
            if (i > 0) sb.append('\n');
            ItemStack s = inventory.getStack(i);
            if (!s.isEmpty()) sb.append(Registries.ITEM.getId(s.getItem()));
        }
        return sb.toString();
    }

    /** Asks the runtime what the machine's slots make now and shows it. */
    private void refreshMachine() {
        if (machine == null) return;
        machineItems = machineItems();
        String found;
        try {
            found = NativeBridge.nativeMachineResult(defId, machine.getWorld().getRegistryKey().getValue().toString(),
                    machine.getPos().getX(), machine.getPos().getY(), machine.getPos().getZ(), machineItems);
        } catch (UnsatisfiedLinkError e) {
            found = null;
        }
        ItemStack out = ItemStack.EMPTY;
        machineRecipe = null;
        if (found != null) {
            String[] p = found.split("\n");
            Identifier id = Identifier.tryParse(p[1]);
            if (id != null && Registries.ITEM.containsId(id)) {
                machineRecipe = p[0];
                out = new ItemStack(Registries.ITEM.get(id), Integer.parseInt(p[2]));
            }
        }
        if (!ItemStack.areEqual(out, machineOut.getStack(0))) machineOut.setStack(0, out);
    }

    /** Makes the machine's recipe: the mods take their share, then one of each ingredient goes. */
    private void craftMachine(PlayerEntity player) {
        if (machine == null || machineRecipe == null) return;
        try {
            NativeBridge.nativeMachineCraft(defId, machineRecipe, machine.getWorld().getRegistryKey().getValue().toString(),
                    machine.getPos().getX(), machine.getPos().getY(), machine.getPos().getZ(),
                    machineItems(), player.getName().getString());
        } catch (UnsatisfiedLinkError ignored) {
        }
        YogInventoryDef def = YogHost.INVENTORY_DEFS.get(defId);
        for (int p : def.machinePlaces) {
            if (p >= inventory.size() || inventory.getStack(p).isEmpty()) continue;
            // What an ingredient leaves (a milk bucket's bucket): in its slot if that's empty, else the player's.
            net.minecraft.item.Item rem = inventory.getStack(p).getItem().getRecipeRemainder();
            ItemStack left = rem == null ? ItemStack.EMPTY : new ItemStack(rem);
            inventory.removeStack(p, 1);
            if (left.isEmpty()) continue;
            if (inventory.getStack(p).isEmpty()) inventory.setStack(p, left);
            else if (!player.getInventory().insertStack(left)) player.dropItem(left, false);
        }
        inventory.markDirty();
        refreshMachine();
    }

    @Override
    public void sendContentUpdates() {
        // What's in it changed, or now and then (a mod's share may have).
        if (machine != null && (++machineTicks % 10 == 0 || !machineItems.equals(machineItems()))) refreshMachine();
        super.sendContentUpdates();
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

    /** A machine's result, like a crafting table's: nothing goes in; taking it (all of it) makes it. */
    private final class MachineResultSlot extends Slot {
        MachineResultSlot(int x, int y) {
            super(machineOut, 0, x, y);
        }

        @Override
        public boolean canInsert(ItemStack stack) {
            return false;
        }

        @Override
        public boolean canTakeItems(PlayerEntity player) {
            // Still made of what's there and still allowed.
            refreshMachine();
            return hasStack();
        }

        @Override
        public ItemStack takeStack(int amount) {
            return super.takeStack(getStack().getCount());
        }

        @Override
        public void onTakeItem(PlayerEntity player, ItemStack stack) {
            craftMachine(player);
            super.onTakeItem(player, stack);
        }
    }

    /** A container slot that takes only the items and {@code #tags} its {@code <Slot accepts>} names. */
    private static final class FilteredSlot extends Slot {
        private final String[] accepts;

        FilteredSlot(Inventory inv, int index, int x, int y, String[] accepts) {
            super(inv, index, x, y);
            this.accepts = accepts;
        }

        @Override
        public boolean canInsert(ItemStack stack) {
            String item = Registries.ITEM.getId(stack.getItem()).toString();
            for (String a : accepts) {
                if (a.startsWith("#")) {
                    Identifier tag = Identifier.tryParse(a.substring(1));
                    if (tag != null && stack.isIn(TagKey.of(RegistryKeys.ITEM, tag))) return true;
                } else if (a.equals(item)) {
                    return true;
                }
            }
            return false;
        }
    }
}
