package dev.yog;

import java.util.Set;
import net.minecraft.core.BlockPos;
import net.minecraft.tags.BlockTags;
import net.minecraft.tags.FluidTags;
import net.minecraft.world.level.BlockGetter;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.FallingBlock;
import net.minecraft.world.level.block.state.BlockState;

/** What a block is to a body there: one place for the client's area and its block mirror. */
public final class YogBlockFlags {
    private YogBlockFlags() {}

    /** Blocks that hurt to touch, besides fire and lava. */
    private static final Set<Block> HARMFUL = Set.of(Blocks.CACTUS, Blocks.SWEET_BERRY_BUSH, Blocks.MAGMA_BLOCK, Blocks.WITHER_ROSE, Blocks.POWDER_SNOW);

    /** Can be stood on and can't be walked through. */
    public static boolean solid(BlockGetter w, BlockPos at, BlockState st) {
        return !st.getCollisionShape(w, at).isEmpty();
    }

    /** Its collision is the whole cube. */
    public static boolean full(BlockGetter w, BlockPos at, BlockState st) {
        return Block.isShapeFullBlock(st.getCollisionShape(w, at));
    }

    /** Water, lava… */
    public static boolean liquid(BlockState st) {
        return !st.getFluidState().isEmpty();
    }

    /** Ladders, vines. */
    public static boolean climbable(BlockState st) {
        return st.is(BlockTags.CLIMBABLE);
    }

    /** Hurts to touch: fire, lava, cactus… */
    public static boolean harmful(BlockState st) {
        return st.is(BlockTags.FIRE) || st.getFluidState().is(FluidTags.LAVA) || HARMFUL.contains(st.getBlock());
    }

    /** Falls when nothing holds it. */
    public static boolean falls(BlockState st) {
        return st.getBlock() instanceof FallingBlock;
    }

    /** How hard it is to break (negative: it can't be). */
    public static float hardness(BlockGetter w, BlockPos at, BlockState st) {
        return st.getDestroySpeed(w, at);
    }
}
