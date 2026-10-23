package dev.yog.content.blocks;

import net.minecraft.core.BlockPos;
import net.minecraft.world.level.BlockGetter;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.phys.shapes.CollisionContext;
import net.minecraft.world.phys.shapes.VoxelShape;

/** A block with a custom collision/outline shape (pixel units, like vanilla). */
public class YogShapedBlock extends Block {
    private final VoxelShape shape;

    /** A block of {@code settings} shaped {@code box}: its corners' x, y, z, then the other's, in pixels. */
    public YogShapedBlock(Properties settings, float[] box) {
        super(settings);
        // box() is protected static on Block; accessible from this subclass.
        this.shape = box(box[0], box[1], box[2], box[3], box[4], box[5]);
    }

    @SuppressWarnings("deprecation") // overriding is fine; Mojang deprecates these only for callers
    @Override
    public VoxelShape getShape(BlockState state, BlockGetter world, BlockPos pos, CollisionContext ctx) {
        return shape;
    }

    @SuppressWarnings("deprecation") // overriding is fine; Mojang deprecates these only for callers
    @Override
    public VoxelShape getCollisionShape(BlockState state, BlockGetter world, BlockPos pos, CollisionContext ctx) {
        return shape;
    }
}
