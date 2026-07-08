package dev.yog;

import net.minecraft.block.Block;
import net.minecraft.block.BlockState;
import net.minecraft.block.ShapeContext;
import net.minecraft.item.ItemPlacementContext;
import net.minecraft.state.StateManager;
import net.minecraft.state.property.BooleanProperty;
import net.minecraft.util.math.BlockPos;
import net.minecraft.util.math.Direction;
import net.minecraft.util.shape.VoxelShape;
import net.minecraft.util.shape.VoxelShapes;
import net.minecraft.world.BlockView;
import net.minecraft.world.WorldAccess;

/**
 * Fence/pipe-style block: dynamically connects to neighboring blocks that
 * share this exact registered id. This is the thin Yarn-mapped bridge — the
 * actual arm/shape geometry is shared (Yarn + Mojmap alike) via
 * `YogConnectingLogic` in java-common.
 */
public class YogConnectingBlock extends Block {
    public static final BooleanProperty NORTH = BooleanProperty.of("north");
    public static final BooleanProperty SOUTH = BooleanProperty.of("south");
    public static final BooleanProperty EAST  = BooleanProperty.of("east");
    public static final BooleanProperty WEST  = BooleanProperty.of("west");
    public static final BooleanProperty UP    = BooleanProperty.of("up");
    public static final BooleanProperty DOWN  = BooleanProperty.of("down");

    private final double[] core;

    public YogConnectingBlock(Settings settings, double x1, double y1, double z1, double x2, double y2, double z2) {
        super(settings);
        this.core = new double[]{x1, y1, z1, x2, y2, z2};
        setDefaultState(this.stateManager.getDefaultState()
                .with(NORTH, false).with(SOUTH, false)
                .with(EAST, false).with(WEST, false)
                .with(UP, false).with(DOWN, false));
    }

    @Override
    protected void appendProperties(StateManager.Builder<Block, BlockState> builder) {
        builder.add(NORTH, SOUTH, EAST, WEST, UP, DOWN);
    }

    private boolean connectsTo(BlockView world, BlockPos pos, Direction dir) {
        return YogConnectingLogic.compatible(this, world.getBlockState(pos.offset(dir)).getBlock());
    }

    @Override
    public BlockState getPlacementState(ItemPlacementContext ctx) {
        BlockView world = ctx.getWorld();
        BlockPos pos = ctx.getBlockPos();
        return getDefaultState()
                .with(NORTH, connectsTo(world, pos, Direction.NORTH))
                .with(SOUTH, connectsTo(world, pos, Direction.SOUTH))
                .with(EAST,  connectsTo(world, pos, Direction.EAST))
                .with(WEST,  connectsTo(world, pos, Direction.WEST))
                .with(UP,    connectsTo(world, pos, Direction.UP))
                .with(DOWN,  connectsTo(world, pos, Direction.DOWN));
    }

    @Override
    public BlockState getStateForNeighborUpdate(BlockState state, Direction dir, BlockState neighborState,
                                                 WorldAccess world, BlockPos pos, BlockPos neighborPos) {
        BooleanProperty prop = propertyFor(dir);
        if (prop == null) return state;
        return state.with(prop, YogConnectingLogic.compatible(this, world.getBlockState(neighborPos).getBlock()));
    }

    private static BooleanProperty propertyFor(Direction dir) {
        switch (dir) {
            case NORTH: return NORTH;
            case SOUTH: return SOUTH;
            case EAST:  return EAST;
            case WEST:  return WEST;
            case UP:    return UP;
            case DOWN:  return DOWN;
            default:    return null;
        }
    }

    private VoxelShape computeShape(BlockState state) {
        boolean[] connected = {
            state.get(NORTH), state.get(SOUTH), state.get(EAST),
            state.get(WEST), state.get(UP), state.get(DOWN),
        };
        double[][] boxes = YogConnectingLogic.shapeBoxes(core, connected);
        VoxelShape shape = VoxelShapes.empty();
        for (double[] b : boxes) {
            shape = VoxelShapes.union(shape, createCuboidShape(b[0], b[1], b[2], b[3], b[4], b[5]));
        }
        return shape;
    }

    @Override
    public VoxelShape getOutlineShape(BlockState state, BlockView world, BlockPos pos, ShapeContext ctx) {
        return computeShape(state);
    }

    @Override
    public VoxelShape getCollisionShape(BlockState state, BlockView world, BlockPos pos, ShapeContext ctx) {
        return computeShape(state);
    }
}
