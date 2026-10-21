package dev.yog.util;

import dev.yog.data.YogContent;
import dev.yog.data.YogData;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;

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
    /** Vanilla container screens put the player inventory at (8, 84). */
    private static final float DEFAULT_PLAYER_INV_X = 8f;
    private static final float DEFAULT_PLAYER_INV_Y = 84f;

    public boolean isMachine() {
        return machineResult >= 0;
    }

    /** One inventory of the runtime's content blob (see {@code dev.yog.data.YogContent}). */
    public static YogInventoryDef fromData(Map<String, Object> m) {
        List<float[]> layout = new ArrayList<>();
        List<String[]> accepts = new ArrayList<>();
        for (Object o : YogData.list(m.get("slots"))) {
            Map<String, Object> slot = YogData.map(o);
            layout.add(YogContent.floats(slot.get("at")));
            List<Object> acc = YogData.list(slot.get("accepts"));
            String[] a = new String[acc.size()];
            for (int i = 0; i < a.length; i++) a[i] = YogData.str(acc.get(i), "");
            accepts.add(a);
        }
        float[] playerInv = m.get("player_inv") != null ? YogContent.floats(m.get("player_inv")) : new float[]{DEFAULT_PLAYER_INV_X, DEFAULT_PLAYER_INV_Y};
        float[] bg = m.get("background_size") != null ? YogContent.floats(m.get("background_size")) : new float[]{0f, 0f};
        int[] places = new int[0];
        int result = -1;
        if (m.get("machine") != null) {
            Map<String, Object> machine = YogData.map(m.get("machine"));
            List<Object> p = YogData.list(machine.get("places"));
            places = new int[p.size()];
            for (int i = 0; i < places.length; i++) places[i] = (int) YogData.integer(p.get(i), 0);
            result = (int) YogData.integer(machine.get("result"), -1);
        }
        return new YogInventoryDef(YogData.str(m.get("id"), ""), (int) YogData.integer(m.get("slot_count"), 0), layout, accepts,
                YogData.bool(m.get("include_player_inventory"), true), playerInv[0], playerInv[1],
                YogData.str(m.get("background_texture"), ""), YogData.str(m.get("title"), ""), bg[0], bg[1], places, result);
    }
}
