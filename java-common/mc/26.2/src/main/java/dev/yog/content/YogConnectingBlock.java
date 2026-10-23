package dev.yog.content;

import dev.yog.YogNames;

import dev.yog.block.YogConnectingLogic;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.world.item.context.BlockPlaceContext;
import net.minecraft.world.level.BlockGetter;
import net.minecraft.world.level.LevelReader;
import net.minecraft.world.level.ScheduledTickAccess;
import net.minecraft.util.RandomSource;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.block.state.StateDefinition;
import net.minecraft.world.level.block.state.properties.BooleanProperty;
import net.minecraft.world.phys.shapes.CollisionContext;
import net.minecraft.world.phys.shapes.Shapes;
import net.minecraft.world.phys.shapes.VoxelShape;

/**
 * Fence/pipe-style block: dynamically connects to neighboring blocks that
 * share this exact registered id. This is the thin Yarn-mapped bridge — the
 * actual arm/shape geometry is shared (Yarn + Mojmap alike) via
 * `YogConnectingLogic` in java-common.
 */
public class YogConnectingBlock extends Block {
    public static final BooleanProperty NORTH = BooleanProperty.create(YogNames.Direction.NORTH);
    public static final BooleanProperty SOUTH = BooleanProperty.create(YogNames.Direction.SOUTH);
    public static final BooleanProperty EAST  = BooleanProperty.create(YogNames.Direction.EAST);
    public static final BooleanProperty WEST  = BooleanProperty.create(YogNames.Direction.WEST);
    public static final BooleanProperty UP    = BooleanProperty.create(YogNames.Direction.UP);
    public static final BooleanProperty DOWN  = BooleanProperty.create(YogNames.Direction.DOWN);

    private final double[] core;

    /** A block of {@code settings} whose core is {@code core}: its corners, in pixels (as {@link YogShapedBlock}). */
    public YogConnectingBlock(Properties settings, double[] core) {
        super(settings);
        this.core = core.clone();
        registerDefaultState(this.stateDefinition.any()
                .setValue(NORTH, false).setValue(SOUTH, false)
                .setValue(EAST, false).setValue(WEST, false)
                .setValue(UP, false).setValue(DOWN, false));
    }

    @Override
    protected void createBlockStateDefinition(StateDefinition.Builder<Block, BlockState> builder) {
        builder.add(NORTH, SOUTH, EAST, WEST, UP, DOWN);
    }

    private boolean connectsTo(BlockGetter world, BlockPos pos, Direction dir) {
        return YogConnectingLogic.compatible(this, world.getBlockState(pos.relative(dir)).getBlock());
    }

    @Override
    public BlockState getStateForPlacement(BlockPlaceContext ctx) {
        BlockGetter world = ctx.getLevel();
        BlockPos pos = ctx.getClickedPos();
        return defaultBlockState()
                .setValue(NORTH, connectsTo(world, pos, Direction.NORTH))
                .setValue(SOUTH, connectsTo(world, pos, Direction.SOUTH))
                .setValue(EAST,  connectsTo(world, pos, Direction.EAST))
                .setValue(WEST,  connectsTo(world, pos, Direction.WEST))
                .setValue(UP,    connectsTo(world, pos, Direction.UP))
                .setValue(DOWN,  connectsTo(world, pos, Direction.DOWN));
    }

    @Override
    protected BlockState updateShape(BlockState state, LevelReader world, ScheduledTickAccess ticks, BlockPos pos,
                                     Direction dir, BlockPos neighborPos, BlockState neighborState, RandomSource random) {
        BooleanProperty prop = propertyFor(dir);
        if (prop == null) return state;
        return state.setValue(prop, YogConnectingLogic.compatible(this, world.getBlockState(neighborPos).getBlock()));
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
    public VoxelShape getShape(BlockState state, BlockGetter world, BlockPos pos, CollisionContext ctx) {
        return computeShape(state);
    }

    @Override
    public VoxelShape getCollisionShape(BlockState state, BlockGetter world, BlockPos pos, CollisionContext ctx) {
        return computeShape(state);
    }
}
