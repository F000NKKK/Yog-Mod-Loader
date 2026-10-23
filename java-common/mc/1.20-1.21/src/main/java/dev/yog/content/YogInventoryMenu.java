package dev.yog.content;

import dev.yog.NativeBridge;

import dev.yog.util.YogInventoryDef;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.registries.Registries;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.tags.TagKey;
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
 * (cached in {@link YogContentRegistry#INVENTORY_DEFS}), shared identically by client
 * and server since mod registration code runs the same on both sides — only
 * slot *contents* need syncing, which vanilla's `AbstractContainerMenu` does for us.
 */
public class YogInventoryMenu extends AbstractContainerMenu {
    public final Container inventory;
    public final String defId;
    private final int customSlotCount;
    /** A machine's made result: the menu's, never the block's (see {@link YogInventoryDef#machineResult}). */
    private final SimpleContainer machineOut = new SimpleContainer(1);
    /** Server-side, a machine's block entity; its current recipe and what it was found from. */
    private YogInventoryBlockEntity machine;
    private String machineRecipe;
    private String machineItems = "";
    private int machineTicks;

    /** Server-side: opened against the block's real backing inventory. */
    public static YogInventoryMenu create(int windowId, Inventory playerInv, Container inv, String defId) {
        return new YogInventoryMenu(windowId, playerInv, inv, defId);
    }

    /** Client-side factory registered via the loader's menu type ({@code YogHost}) — builds a
     * throwaway inventory just to hold synced slot contents (same convention vanilla's
     * generic chest menu uses). */
    public static YogInventoryMenu createClient(int windowId, Inventory playerInv, net.minecraft.network.FriendlyByteBuf buf) {
        return createClient(windowId, playerInv, buf.readUtf());
    }

    /** The client's menu of inventory {@code defId} (a loader that decodes the id itself). */
    public static YogInventoryMenu createClient(int windowId, Inventory playerInv, String defId) {
        YogInventoryDef def = YogContentRegistry.INVENTORY_DEFS.get(defId);
        int slotCount = def != null ? def.slotCount() : 0;
        return new YogInventoryMenu(windowId, playerInv, new SimpleContainer(slotCount), defId);
    }

    private YogInventoryMenu(int windowId, Inventory playerInv, Container inv, String defId) {
        super(YogContentRegistry.INVENTORY_MENU_TYPE, windowId);
        this.inventory = inv;
        this.defId = defId;
        YogInventoryDef def = YogContentRegistry.INVENTORY_DEFS.get(defId);
        this.customSlotCount = def != null ? def.slotCount() : inv.getContainerSize();
        inv.startOpen(playerInv.player);
        if (def != null && def.isMachine() && inv instanceof YogInventoryBlockEntity be && be.getLevel() != null && !be.getLevel().isClientSide) {
            this.machine = be;
        }

        if (def != null) {
            for (int i = 0; i < def.slotCount(); i++) {
                float[] xy = i < def.layout().size() ? def.layout().get(i) : new float[]{8f + (i % 9) * 18f, 18f + (i / 9) * 18f};
                String[] accepts = i < def.accepts().size() ? def.accepts().get(i) : new String[0];
                this.addSlot(def.isMachine() && i == def.machineResult()
                        ? new MachineResultSlot((int) xy[0], (int) xy[1])
                        : accepts.length == 0
                        ? new Slot(inv, i, (int) xy[0], (int) xy[1])
                        : new FilteredSlot(inv, i, (int) xy[0], (int) xy[1], accepts));
            }
            if (def.includePlayerInventory()) {
                addPlayerInventorySlots(playerInv, def.playerInvX(), def.playerInvY());
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

    /** The machine's items by slot ({@code ""} empty), {@code \n}-joined, as the runtime takes them. */
    private String machineItems() {
        StringBuilder sb = new StringBuilder();
        for (int i = 0; i < inventory.getContainerSize(); i++) {
            if (i > 0) sb.append('\n');
            ItemStack s = inventory.getItem(i);
            if (!s.isEmpty()) sb.append(BuiltInRegistries.ITEM.getKey(s.getItem()));
        }
        return sb.toString();
    }

    /** Asks the runtime what the machine's slots make now and shows it. */
    private void refreshMachine() {
        if (machine == null) return;
        machineItems = machineItems();
        String found;
        try {
            found = NativeBridge.nativeMachineResult(defId, machine.getLevel().dimension().location().toString(),
                    machine.getBlockPos().getX(), machine.getBlockPos().getY(), machine.getBlockPos().getZ(), machineItems);
        } catch (UnsatisfiedLinkError e) {
            found = null;
        }
        ItemStack out = ItemStack.EMPTY;
        machineRecipe = null;
        if (found != null) {
            String[] p = found.split("\n");
            ResourceLocation id = ResourceLocation.tryParse(p[1]);
            if (id != null && BuiltInRegistries.ITEM.containsKey(id)) {
                machineRecipe = p[0];
                out = new ItemStack(BuiltInRegistries.ITEM.get(id), Integer.parseInt(p[2]));
            }
        }
        if (!ItemStack.matches(out, machineOut.getItem(0))) machineOut.setItem(0, out);
    }

    /** Makes the machine's recipe: the mods take their share, then one of each ingredient goes. */
    private void craftMachine(Player player) {
        if (machine == null || machineRecipe == null) return;
        NativeBridge.nativeMachineCraft(defId, machineRecipe, machine.getLevel().dimension().location().toString(),
                machine.getBlockPos().getX(), machine.getBlockPos().getY(), machine.getBlockPos().getZ(),
                machineItems(), player.getName().getString());
        YogInventoryDef def = YogContentRegistry.INVENTORY_DEFS.get(defId);
        for (int p : def.machinePlaces()) {
            if (p >= inventory.getContainerSize() || inventory.getItem(p).isEmpty()) continue;
            // What an ingredient leaves (a milk bucket's bucket): in its slot if that's empty, else the player's.
            ItemStack left = new ItemStack(inventory.getItem(p).getItem().getCraftingRemainingItem());
            inventory.removeItem(p, 1);
            if (left.isEmpty()) continue;
            if (inventory.getItem(p).isEmpty()) inventory.setItem(p, left);
            else if (!player.getInventory().add(left)) player.drop(left, false);
        }
        inventory.setChanged();
        refreshMachine();
    }

    @Override
    public void broadcastChanges() {
        // What's in it changed, or now and then (a mod's share may have).
        if (machine != null && (++machineTicks % 10 == 0 || !machineItems.equals(machineItems()))) refreshMachine();
        super.broadcastChanges();
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

    /** A machine's result, like a crafting table's: nothing goes in; taking it (all of it) makes it. */
    private final class MachineResultSlot extends Slot {
        MachineResultSlot(int x, int y) {
            super(machineOut, 0, x, y);
        }

        @Override
        public boolean mayPlace(ItemStack stack) {
            return false;
        }

        @Override
        public boolean mayPickup(Player player) {
            // Still made of what's there and still allowed.
            refreshMachine();
            return hasItem();
        }

        @Override
        public ItemStack remove(int amount) {
            return super.remove(getItem().getCount());
        }

        @Override
        public void onTake(Player player, ItemStack stack) {
            craftMachine(player);
            super.onTake(player, stack);
        }
    }

    /** A container slot that takes only the items and {@code #tags} its {@code <Slot accepts>} names. */
    private static final class FilteredSlot extends Slot {
        private final String[] accepts;

        FilteredSlot(Container inv, int index, int x, int y, String[] accepts) {
            super(inv, index, x, y);
            this.accepts = accepts;
        }

        @Override
        public boolean mayPlace(ItemStack stack) {
            String item = BuiltInRegistries.ITEM.getKey(stack.getItem()).toString();
            for (String a : accepts) {
                if (a.startsWith("#")) {
                    ResourceLocation tag = ResourceLocation.tryParse(a.substring(1));
                    if (tag != null && stack.is(TagKey.create(Registries.ITEM, tag))) return true;
                } else if (a.equals(item)) {
                    return true;
                }
            }
            return false;
        }
    }
}
