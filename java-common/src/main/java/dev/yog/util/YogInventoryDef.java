package dev.yog.util;

import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

/**
 * Runtime-side mirror of {@code yog_inventory::InventoryDef} — parsed from
 * {@code NativeBridge.nativeInventoryDefs()} and shared by every host's block
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

    /** Parse the runtime's {@code id\tkey=value...} lines, keyed by id. {@code null} → empty. */
    public static Map<String, YogInventoryDef> parseAll(String raw) {
        Map<String, YogInventoryDef> defs = new HashMap<>();
        if (raw == null) return defs;
        for (String line : raw.split("\n")) {
            if (line.isBlank()) continue;
            String id = line.split("\t", 2)[0];
            Map<String, String> p = YogProps.parse(line);
            int slotCount = YogProps.parseInt(p, "slot_count", 0);
            List<float[]> layout = new ArrayList<>();
            List<String[]> accepts = new ArrayList<>();
            String layoutRaw = p.getOrDefault("layout", "");
            if (!layoutRaw.isEmpty()) {
                // "x:y" or "x:y:id|#tag|..." per slot.
                for (String entry : layoutRaw.split(",")) {
                    String[] parts = entry.split(":", 3);
                    float[] xy = parts.length >= 2 ? parsePair(parts[0] + ":" + parts[1]) : null;
                    if (xy == null) continue;
                    layout.add(xy);
                    accepts.add(parts.length == 3 && !parts[2].isEmpty() ? parts[2].split("\\|") : new String[0]);
                }
            }
            boolean includePlayerInv = !"0".equals(p.get("include_player_inventory"));
            float[] playerInv = parsePair(p.getOrDefault("player_inv", ""));
            if (playerInv == null) playerInv = new float[]{DEFAULT_PLAYER_INV_X, DEFAULT_PLAYER_INV_Y};
            float[] bgSize = parsePair(p.getOrDefault("background_size", ""));
            if (bgSize == null) bgSize = new float[]{0f, 0f};
            defs.put(id, new YogInventoryDef(id, slotCount, layout, accepts, includePlayerInv, playerInv[0], playerInv[1],
                    p.getOrDefault("background_texture", ""), p.getOrDefault("title", ""), bgSize[0], bgSize[1]));
        }
        return defs;
    }

    /** {@code "x:y"} → {@code {x, y}}, or {@code null} if malformed. */
    private static float[] parsePair(String s) {
        String[] xy = s.split(":", 2);
        if (xy.length != 2) return null;
        try {
            return new float[]{Float.parseFloat(xy[0]), Float.parseFloat(xy[1])};
        } catch (NumberFormatException e) {
            return null;
        }
    }
}
