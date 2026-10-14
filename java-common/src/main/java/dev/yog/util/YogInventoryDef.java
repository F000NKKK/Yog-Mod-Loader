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
public final class YogInventoryDef {
    /** Vanilla container screens put the player inventory at (8, 84). */
    private static final float DEFAULT_PLAYER_INV_X = 8f;
    private static final float DEFAULT_PLAYER_INV_Y = 84f;

    public final String id;
    public final int slotCount;
    public final List<float[]> layout;
    /** Per slot (by index, parallel to {@link #layout}): the item ids and {@code #tags} it takes; empty = anything. */
    public final List<String[]> accepts;
    public final boolean includePlayerInventory;
    public final float playerInvX, playerInvY;
    public final String backgroundTexture;
    public final String title;
    public final float backgroundW, backgroundH;

    YogInventoryDef(String id, int slotCount, List<float[]> layout, List<String[]> accepts, boolean includePlayerInventory,
                    float playerInvX, float playerInvY, String backgroundTexture, String title,
                    float backgroundW, float backgroundH) {
        this.id = id;
        this.slotCount = slotCount;
        this.layout = layout;
        this.accepts = accepts;
        this.includePlayerInventory = includePlayerInventory;
        this.playerInvX = playerInvX;
        this.playerInvY = playerInvY;
        this.backgroundTexture = backgroundTexture;
        this.title = title;
        this.backgroundW = backgroundW;
        this.backgroundH = backgroundH;
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
        return new YogInventoryDef(YogData.str(m.get("id"), ""), (int) YogData.integer(m.get("slot_count"), 0), layout, accepts,
                YogData.bool(m.get("include_player_inventory"), true), playerInv[0], playerInv[1],
                YogData.str(m.get("background_texture"), ""), YogData.str(m.get("title"), ""), bg[0], bg[1]);
    }
}
