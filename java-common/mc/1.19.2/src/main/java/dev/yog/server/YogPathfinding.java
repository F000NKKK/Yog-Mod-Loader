package dev.yog.server;

import net.minecraft.core.BlockPos;
import net.minecraft.world.level.EmptyBlockGetter;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.pathfinder.PathComputationType;

/** The game's own pathfinding's view of a block — its call differs by version. */
public final class YogPathfinding {
    private YogPathfinding() {}

    /** Whether mobs walking on land may pass through {@code st}. */
    public static boolean land(BlockState st) {
        return st.isPathfindable(EmptyBlockGetter.INSTANCE, BlockPos.ZERO, PathComputationType.LAND);
    }
}
