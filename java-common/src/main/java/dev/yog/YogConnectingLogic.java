package dev.yog;

import java.util.ArrayList;
import java.util.List;

/**
 * Pure geometry for fence/pipe-style "connects to same-id neighbors" blocks —
 * no Minecraft types at all, so it compiles identically under Yarn (Fabric)
 * and Mojang mappings (Forge/NeoForge). Each platform's `YogConnectingBlock`
 * is a thin bridge: it resolves which of the 6 neighbors are the same block
 * (a one-line, mapping-specific check) and hands the boolean results here to
 * get back the list of boxes (core + one per connected side) to union into a
 * VoxelShape using whatever that platform's shape API looks like.
 */
public final class YogConnectingLogic {
    public static final int NORTH = 0, SOUTH = 1, EAST = 2, WEST = 3, UP = 4, DOWN = 5;

    private YogConnectingLogic() {}

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
