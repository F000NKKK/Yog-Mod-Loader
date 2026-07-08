package dev.yog;

import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.world.item.context.BlockPlaceContext;
import net.minecraft.world.level.BlockGetter;
import net.minecraft.world.level.LevelAccessor;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.block.state.StateDefinition;
import net.minecraft.world.level.block.state.properties.BooleanProperty;
import net.minecraft.world.phys.shapes.CollisionContext;
import net.minecraft.world.phys.shapes.Shapes;
import net.minecraft.world.phys.shapes.VoxelShape;

/**
 * Fence/pipe-style block: dynamically connects to neighboring blocks that
 * share this exact registered id. This is the thin Mojang-mapped bridge —
 * the actual arm/shape geometry is shared (Yarn + Mojmap alike) via
 * `YogConnectingLogic` in java-common.
 */
public class YogConnectingBlock extends Block {
    public static final BooleanProperty NORTH = BooleanProperty.create("north");
    public static final BooleanProperty SOUTH = BooleanProperty.create("south");
    public static final BooleanProperty EAST  = BooleanProperty.create("east");
    public static final BooleanProperty WEST  = BooleanProperty.create("west");
    public static final BooleanProperty UP    = BooleanProperty.create("up");
    public static final BooleanProperty DOWN  = BooleanProperty.create("down");

    private final double[] core;

    public YogConnectingBlock(Properties properties, double x1, double y1, double z1, double x2, double y2, double z2) {
        super(properties);
        this.core = new double[]{x1, y1, z1, x2, y2, z2};
        registerDefaultState(this.stateDefinition.any()
                .setValue(NORTH, false).setValue(SOUTH, false)
                .setValue(EAST, false).setValue(WEST, false)
                .setValue(UP, false).setValue(DOWN, false));
    }

    @Override
    protected void createBlockStateDefinition(StateDefinition.Builder<Block, BlockState> builder) {
        builder.add(NORTH, SOUTH, EAST, WEST, UP, DOWN);
    }

    private boolean connectsTo(BlockGetter level, BlockPos pos, Direction dir) {
        return level.getBlockState(pos.relative(dir)).getBlock() == this;
    }

    @Override
    public BlockState getStateForPlacement(BlockPlaceContext ctx) {
        BlockGetter level = ctx.getLevel();
        BlockPos pos = ctx.getClickedPos();
        return defaultBlockState()
                .setValue(NORTH, connectsTo(level, pos, Direction.NORTH))
                .setValue(SOUTH, connectsTo(level, pos, Direction.SOUTH))
                .setValue(EAST,  connectsTo(level, pos, Direction.EAST))
                .setValue(WEST,  connectsTo(level, pos, Direction.WEST))
                .setValue(UP,    connectsTo(level, pos, Direction.UP))
                .setValue(DOWN,  connectsTo(level, pos, Direction.DOWN));
    }

    @Override
    public BlockState updateShape(BlockState state, Direction dir, BlockState neighborState,
                                   LevelAccessor level, BlockPos pos, BlockPos neighborPos) {
        BooleanProperty prop = propertyFor(dir);
        if (prop == null) return state;
        return state.setValue(prop, level.getBlockState(neighborPos).getBlock() == this);
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
            state.getValue(NORTH), state.getValue(SOUTH), state.getValue(EAST),
            state.getValue(WEST), state.getValue(UP), state.getValue(DOWN),
        };
        double[][] boxes = YogConnectingLogic.shapeBoxes(core, connected);
        VoxelShape shape = Shapes.empty();
        for (double[] b : boxes) {
            shape = Shapes.or(shape, box(b[0], b[1], b[2], b[3], b[4], b[5]));
        }
        return shape;
    }

    @Override
    public VoxelShape getShape(BlockState state, BlockGetter level, BlockPos pos, CollisionContext ctx) {
        return computeShape(state);
    }

    @Override
    public VoxelShape getCollisionShape(BlockState state, BlockGetter level, BlockPos pos, CollisionContext ctx) {
        return computeShape(state);
    }
}
