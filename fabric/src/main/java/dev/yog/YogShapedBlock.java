package dev.yog;

import net.minecraft.block.Block;
import net.minecraft.block.BlockState;
import net.minecraft.block.ShapeContext;
import net.minecraft.util.math.BlockPos;
import net.minecraft.util.shape.VoxelShape;
import net.minecraft.world.BlockView;

/** A block with a custom collision/outline shape (pixel units, like vanilla). */
public class YogShapedBlock extends Block {
    private final VoxelShape shape;

    public YogShapedBlock(Settings settings,
                          double x1, double y1, double z1,
                          double x2, double y2, double z2) {
        super(settings);
        // createCuboidShape is protected static on Block; accessible from this subclass.
        this.shape = createCuboidShape(x1, y1, z1, x2, y2, z2);
    }

    @Override
    public VoxelShape getOutlineShape(BlockState state, BlockView world, BlockPos pos, ShapeContext ctx) {
        return shape;
    }

    @Override
    public VoxelShape getCollisionShape(BlockState state, BlockView world, BlockPos pos, ShapeContext ctx) {
        return shape;
    }
}
