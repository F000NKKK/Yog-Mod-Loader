package dev.yog.block;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;

/**
 * Pure geometry + connection-compatibility bookkeeping for fence/pipe-style
 * blocks — no Minecraft types at all, so it compiles identically under Yarn
 * (Fabric) and Mojang mappings (Forge/NeoForge). Each platform's
 * `YogConnectingBlock` is a thin bridge: it resolves neighbor blocks (a
 * one-line, mapping-specific lookup) and hands them here, keyed as opaque
 * `Object`s (every registered Block instance, connecting or not, is a valid
 * key) to decide compatibility and compute the resulting shape.
 */
public final class YogConnectingLogic {
    public static final int NORTH = 0, SOUTH = 1, EAST = 2, WEST = 3, UP = 4, DOWN = 5;

    /** Connection-compatibility tags per registered block instance. */
    private static final Map<Object, Set<String>> GROUPS = new HashMap<>();

    private YogConnectingLogic() {}

    /** Called once per block at registration time, for every block that carries `connect_groups`. */
    public static void registerGroups(Object block, String[] groups) {
        GROUPS.put(block, new HashSet<>(Arrays.asList(groups)));
    }

    /** Two blocks are compatible (should connect) when their tag sets share at least one entry. */
    public static boolean compatible(Object a, Object b) {
        Set<String> ga = GROUPS.getOrDefault(a, Collections.emptySet());
        Set<String> gb = GROUPS.getOrDefault(b, Collections.emptySet());
        if (ga.isEmpty() || gb.isEmpty()) return false;
        for (String g : ga) {
            if (gb.contains(g)) return true;
        }
        return false;
    }

    /**
     * @param core      the block's core box, pixel units 0-16: [x1,y1,z1,x2,y2,z2]
     * @param connected 6 flags indexed by NORTH..DOWN
     * @return the core box plus one arm box per connected direction, each
     *         extending from the core's face out to the block boundary (0 or
     *         16) along that axis, at the core's own thickness/height range.
     */
    public static double[][] shapeBoxes(double[] core, boolean[] connected) {
        List<double[]> boxes = new ArrayList<>();
        boxes.add(core.clone());
        double x1 = core[0], y1 = core[1], z1 = core[2], x2 = core[3], y2 = core[4], z2 = core[5];
        if (connected[NORTH]) boxes.add(new double[]{x1, y1, 0,  x2, y2, z1});
        if (connected[SOUTH]) boxes.add(new double[]{x1, y1, z2, x2, y2, 16});
        if (connected[WEST])  boxes.add(new double[]{0,  y1, z1, x1, y2, z2});
        if (connected[EAST])  boxes.add(new double[]{x2, y1, z1, 16, y2, z2});
        if (connected[DOWN])  boxes.add(new double[]{x1, 0,  z1, x2, y1, z2});
        if (connected[UP])    boxes.add(new double[]{x1, y2, z1, x2, 16, z2});
        return boxes.toArray(new double[0][]);
    }
}
