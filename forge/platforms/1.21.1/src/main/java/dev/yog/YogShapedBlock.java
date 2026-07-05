package dev.yog;

import net.minecraft.core.BlockPos;
import net.minecraft.world.level.BlockGetter;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.phys.shapes.CollisionContext;
import net.minecraft.world.phys.shapes.VoxelShape;

/** A block with a custom collision/outline shape (pixel units, like vanilla). */
public class YogShapedBlock extends Block {
    private final VoxelShape shape;

    public YogShapedBlock(Properties properties,
                          double x1, double y1, double z1,
                          double x2, double y2, double z2) {
        super(properties);
        // box() is protected static on Block; accessible from this subclass.
        this.shape = box(x1, y1, z1, x2, y2, z2);
    }

    @Override
    public VoxelShape getShape(BlockState state, BlockGetter level, BlockPos pos, CollisionContext ctx) {
        return shape;
    }

    @Override
    public VoxelShape getCollisionShape(BlockState state, BlockGetter level, BlockPos pos, CollisionContext ctx) {
        return shape;
    }
}
