package dev.yog.util;

import dev.yog.bus.YogTypes;
import java.util.ArrayList;
import java.util.List;

/**
 * Runtime-side mirror of {@code yog_inventory::InventoryDef} — read from
 * the runtime's content blob ({@code YogContent}) and shared by every host's block
 * entity, menu and screen (see rust/crates/yog-inventory/DESIGN.md).
 */
public record YogInventoryDef(String id, int slotCount, List<float[]> layout,
        /** Per slot (by index, parallel to {@link #layout}): the item ids and {@code #tags} it takes; empty = anything. */
        List<String[]> accepts, boolean includePlayerInventory, float playerInvX, float playerInvY,
        String backgroundTexture, String title, float backgroundW, float backgroundH,
        /** A machine's ingredient slots ({@code .yui <RecipeArea>}): the runtime finds the recipe they make. */
        int[] machinePlaces,
        /** A machine's result slot, or -1: not a machine. Its stack lives in the menu, never in the block. */
        int machineResult) {
    public boolean isMachine() {
        return machineResult >= 0;
    }

    /** One inventory of the runtime's content ({@code dev.yog.data.YogContent}). */
    public static YogInventoryDef of(YogTypes.Inventory inv) {
        List<float[]> layout = new ArrayList<>();
        List<String[]> accepts = new ArrayList<>();
        for (YogTypes.InventorySlot slot : inv.slots()) {
            layout.add(slot.at());
            accepts.add(slot.accepts().toArray(String[]::new));
        }
        float[] bg = inv.backgroundSize() != null ? inv.backgroundSize() : new float[] {0f, 0f};
        YogTypes.Machine m = inv.machine();
        int[] places = m == null ? new int[0] : m.places();
        int result = m == null ? NOT_A_MACHINE : m.result();
        String background = inv.backgroundTexture() != null ? inv.backgroundTexture() : "";
        return new YogInventoryDef(inv.id(), inv.slotCount(), layout, accepts,
                inv.includePlayerInventory(), inv.playerInv()[0], inv.playerInv()[1], background, inv.title(),
                bg[0], bg[1], places, result);
    }

    /** {@link #machineResult} of an inventory that isn't a machine. */
    private static final int NOT_A_MACHINE = -1;
}
