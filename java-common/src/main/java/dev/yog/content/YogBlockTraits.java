package dev.yog.content;

import dev.yog.server.YogPathfinding;

import dev.yog.bus.YogMirrors.BlockTrait;
import java.util.Set;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.tags.BlockTags;
import net.minecraft.tags.FluidTags;
import net.minecraft.world.level.BlockGetter;
import net.minecraft.world.level.EmptyBlockGetter;
import net.minecraft.world.level.block.AbstractCauldronBlock;
import net.minecraft.world.level.block.AbstractChestBlock;
import net.minecraft.world.level.block.AbstractSkullBlock;
import net.minecraft.world.level.block.AmethystClusterBlock;
import net.minecraft.world.level.block.AzaleaBlock;
import net.minecraft.world.level.block.BambooStalkBlock;
import net.minecraft.world.level.block.BaseFireBlock;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.CarpetBlock;
import net.minecraft.world.level.block.DoorBlock;
import net.minecraft.world.level.block.FallingBlock;
import net.minecraft.world.level.block.FenceBlock;
import net.minecraft.world.level.block.FenceGateBlock;
import net.minecraft.world.level.block.InfestedBlock;
import net.minecraft.world.level.block.LeavesBlock;
import net.minecraft.world.level.block.LiquidBlock;
import net.minecraft.world.level.block.PointedDripstoneBlock;
import net.minecraft.world.level.block.ScaffoldingBlock;
import net.minecraft.world.level.block.ShulkerBoxBlock;
import net.minecraft.world.level.block.SlabBlock;
import net.minecraft.world.level.block.SnowLayerBlock;
import net.minecraft.world.level.block.StairBlock;
import net.minecraft.world.level.block.TrapDoorBlock;
import net.minecraft.world.level.block.WallBlock;
import net.minecraft.world.level.block.piston.MovingPistonBlock;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.material.FluidState;

/**
 * What a block is to a body there and to whoever breaks or builds by it
 * ({@code yog_ids::block_trait}, a bit each): one place for the client's
 * area query and its block mirror.
 */
public final class YogBlockTraits {
    private YogBlockTraits() {}

    /** Blocks that hurt to touch, besides fire and lava. */
    private static final Set<Block> HARMFUL = Set.of(Blocks.CACTUS, Blocks.SWEET_BERRY_BUSH, Blocks.MAGMA_BLOCK, Blocks.WITHER_ROSE, Blocks.POWDER_SNOW);

    /** Can be stood on and can't be walked through. */
    public static boolean solid(BlockGetter w, BlockPos at, BlockState st) {
        return !st.getCollisionShape(w, at).isEmpty();
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

    /** Every trait of {@code st} at {@code at}, a bit each ({@link BlockTrait}). */
    public static long of(BlockGetter w, BlockPos at, BlockState st) {
        Block b = st.getBlock();
        FluidState fluid = st.getFluidState();
        long t = 0;
        t |= bit(BlockTrait.AIR, st.isAir());
        t |= bit(BlockTrait.SOLID, solid(w, at, st));
        t |= bit(BlockTrait.FULL, Block.isShapeFullBlock(st.getCollisionShape(w, at)));
        t |= bit(BlockTrait.LIQUID, !fluid.isEmpty());
        t |= bit(BlockTrait.WATER, fluid.is(FluidTags.WATER));
        t |= bit(BlockTrait.LAVA, fluid.is(FluidTags.LAVA));
        t |= bit(BlockTrait.SOURCE, fluid.isSource());
        t |= bit(BlockTrait.FLUID_BLOCK, b instanceof LiquidBlock);
        t |= bit(BlockTrait.CLIMBABLE, climbable(st));
        t |= bit(BlockTrait.HARMFUL, harmful(st));
        t |= bit(BlockTrait.FALLS, b instanceof FallingBlock);
        t |= bit(BlockTrait.PATHFINDABLE, YogPathfinding.land(st));
        t |= bit(BlockTrait.REPLACEABLE, st.canBeReplaced());
        t |= bit(BlockTrait.STURDY_TOP, st.isFaceSturdy(EmptyBlockGetter.INSTANCE, BlockPos.ZERO, Direction.UP));
        t |= bit(BlockTrait.FIRE, b instanceof BaseFireBlock);
        t |= bit(BlockTrait.SLAB, b instanceof SlabBlock);
        t |= bit(BlockTrait.STAIRS, b instanceof StairBlock);
        t |= bit(BlockTrait.DOOR, b instanceof DoorBlock);
        t |= bit(BlockTrait.WOODEN_DOOR, st.is(BlockTags.WOODEN_DOORS));
        t |= bit(BlockTrait.FENCE_GATE, b instanceof FenceGateBlock);
        t |= bit(BlockTrait.TRAPDOOR, b instanceof TrapDoorBlock);
        t |= bit(BlockTrait.FENCE, b instanceof FenceBlock);
        t |= bit(BlockTrait.WALL, b instanceof WallBlock);
        t |= bit(BlockTrait.CARPET, b instanceof CarpetBlock);
        t |= bit(BlockTrait.SNOW_LAYER, b instanceof SnowLayerBlock);
        t |= bit(BlockTrait.SKULL, b instanceof AbstractSkullBlock);
        t |= bit(BlockTrait.SHULKER_BOX, b instanceof ShulkerBoxBlock);
        t |= bit(BlockTrait.LEAVES, b instanceof LeavesBlock);
        t |= bit(BlockTrait.INFESTED, b instanceof InfestedBlock);
        t |= bit(BlockTrait.CAULDRON, b instanceof AbstractCauldronBlock);
        t |= bit(BlockTrait.AZALEA, b instanceof AzaleaBlock);
        t |= bit(BlockTrait.AMETHYST_CLUSTER, b instanceof AmethystClusterBlock);
        t |= bit(BlockTrait.POINTED_DRIPSTONE, b instanceof PointedDripstoneBlock);
        t |= bit(BlockTrait.BAMBOO, b instanceof BambooStalkBlock);
        t |= bit(BlockTrait.MOVING_PISTON, b instanceof MovingPistonBlock);
        t |= bit(BlockTrait.SCAFFOLDING, b instanceof ScaffoldingBlock);
        t |= bit(BlockTrait.CHEST, b instanceof AbstractChestBlock);
        t |= bit(BlockTrait.GLASS, st.is(BlockTags.IMPERMEABLE));
        return t;
    }

    private static long bit(int index, boolean on) {
        return on ? 1L << index : 0L;
    }

    /** How hard it is to break (negative: it can't be). */
    public static float hardness(BlockGetter w, BlockPos at, BlockState st) {
        return st.getDestroySpeed(w, at);
    }
}
