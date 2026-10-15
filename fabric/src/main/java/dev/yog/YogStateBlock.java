package dev.yog;

import dev.yog.block.YogStateLogic;
import dev.yog.data.YogContent;
import java.util.ArrayList;
import java.util.Collection;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.concurrent.ConcurrentHashMap;
import net.minecraft.block.Block;
import net.minecraft.block.BlockState;
import net.minecraft.block.ShapeContext;
import net.minecraft.block.Waterloggable;
import net.minecraft.block.WallMountedBlock;
import net.minecraft.fluid.FluidState;
import net.minecraft.fluid.Fluids;
import net.minecraft.item.ItemPlacementContext;
import net.minecraft.state.StateManager;
import net.minecraft.state.property.BooleanProperty;
import net.minecraft.state.property.DirectionProperty;
import net.minecraft.state.property.Properties;
import net.minecraft.state.property.Property;
import net.minecraft.util.BlockMirror;
import net.minecraft.util.BlockRotation;
import net.minecraft.util.math.BlockPos;
import net.minecraft.util.math.Direction;
import net.minecraft.util.shape.VoxelShape;
import net.minecraft.util.shape.VoxelShapes;
import net.minecraft.world.BlockView;
import net.minecraft.world.World;
import net.minecraft.world.WorldAccess;

/**
 * A Yog block with states of its own ({@code states} in its .yblock): it
 * turns when placed ({@code facing}, {@code placement}), hangs on a floor,
 * wall or ceiling ({@code attach}), stands in water ({@code waterlogged}),
 * and has the mod's own properties, set with {@code set_block_state}. A
 * yes/no property can follow redstone ({@code block.powered}) or feed a
 * comparator ({@code block.comparator}). Its shape turns with its model.
 */
public class YogStateBlock extends Block implements Waterloggable {
    /** The properties a block being made has: they're added in {@code Block}'s constructor, before its own fields. */
    private static final ThreadLocal<Spec> PENDING = new ThreadLocal<>();

    private final Spec spec;
    private final Map<BlockState, VoxelShape> shapes = new ConcurrentHashMap<>();

    /** A block's properties. */
    private static final class Spec {
        final YogContent.Block def;
        DirectionProperty facing;
        /** {@code floor}, {@code wall}, {@code ceiling} — named as a button's (the enum's name changed in 1.20.3). */
        Property<?> face;
        BooleanProperty waterlogged;
        final Map<String, Property<?>> own = new HashMap<>();
        final List<Property<?>> all = new ArrayList<>();

        Spec(YogContent.Block def) {
            this.def = def;
            if (def.attach()) {
                facing = Properties.HORIZONTAL_FACING;
                face = WallMountedBlock.FACE;
            } else if (def.facing().equals("all")) {
                facing = Properties.FACING;
            } else if (def.facing().equals("horizontal")) {
                facing = Properties.HORIZONTAL_FACING;
            }
            if (def.waterlogged()) waterlogged = Properties.WATERLOGGED;
            if (facing != null) all.add(facing);
            if (face != null) all.add(face);
            if (waterlogged != null) all.add(waterlogged);
            for (YogContent.Property p : def.properties()) {
                Property<?> made = p.yesNo() ? BooleanProperty.of(p.name()) : new Named(p.name(), p.values());
                own.put(p.name(), made);
                all.add(made);
            }
        }
    }

    public static YogStateBlock create(Settings settings, YogContent.Block def) {
        PENDING.set(new Spec(def));
        try {
            return new YogStateBlock(settings);
        } finally {
            PENDING.remove();
        }
    }

    private YogStateBlock(Settings settings) {
        super(settings);
        this.spec = PENDING.get();
        // The first value of each property, as the .yblock lists them.
        BlockState state = getStateManager().getDefaultState();
        if (spec.facing != null) state = state.with(spec.facing, Direction.NORTH);
        if (spec.face != null) state = withFirst(state, spec.face, "floor");
        if (spec.waterlogged != null) state = state.with(spec.waterlogged, false);
        for (YogContent.Property p : spec.def.properties()) state = withFirst(state, spec.own.get(p.name()), p.values().get(0));
        setDefaultState(state);
    }

    /** {@code state} with property {@code p} at the value named {@code value}. */
    private static <T extends Comparable<T>> BlockState withFirst(BlockState state, Property<T> p, String value) {
        return p.parse(value).map(v -> state.with(p, v)).orElse(state);
    }

    @Override
    protected void appendProperties(StateManager.Builder<Block, BlockState> builder) {
        for (Property<?> p : PENDING.get().all) builder.add(p);
    }

    @Override
    public BlockState getPlacementState(ItemPlacementContext ctx) {
        BlockState state = getDefaultState();
        boolean toward = !spec.def.placement().equals("away");
        if (spec.face != null) {
            Direction look = ctx.getPlacementDirections()[0];
            if (look.getAxis() == Direction.Axis.Y) {
                Direction horizontal = ctx.getHorizontalPlayerFacing();
                state = withFirst(state, spec.face, look == Direction.UP ? "ceiling" : "floor")
                        .with(spec.facing, toward ? horizontal.getOpposite() : horizontal);
            } else {
                state = withFirst(state, spec.face, "wall").with(spec.facing, look.getOpposite());
            }
        } else if (spec.facing == Properties.FACING) {
            Direction look = ctx.getPlayerLookDirection();
            state = state.with(spec.facing, toward ? look.getOpposite() : look);
        } else if (spec.facing != null) {
            Direction look = ctx.getHorizontalPlayerFacing();
            state = state.with(spec.facing, toward ? look.getOpposite() : look);
        }
        if (spec.waterlogged != null) {
            state = state.with(spec.waterlogged, ctx.getWorld().getFluidState(ctx.getBlockPos()).getFluid() == Fluids.WATER);
        }
        if (spec.def.powered() != null) {
            state = state.with(yesNo(spec.def.powered()), ctx.getWorld().isReceivingRedstonePower(ctx.getBlockPos()));
        }
        return state;
    }

    private static <T extends Comparable<T>> String named(BlockState state, Property<T> p) {
        return p.name(state.get(p));
    }

    private BooleanProperty yesNo(String name) {
        return (BooleanProperty) spec.own.get(name);
    }

    // ── Shape ─────────────────────────────────────────────────────────────────

    private VoxelShape shape(BlockState state) {
        float[] box = spec.def.shape();
        if (box == null) return VoxelShapes.fullCube();
        return shapes.computeIfAbsent(state, s -> {
            String facing = spec.facing == null ? null : s.get(spec.facing).asString();
            String face = spec.face == null ? null : named(s, spec.face);
            int[] turn = YogStateLogic.turn(facing, face);
            double[] b = YogStateLogic.turnBox(new double[] {box[0], box[1], box[2], box[3], box[4], box[5]}, turn[0], turn[1]);
            return createCuboidShape(b[0], b[1], b[2], b[3], b[4], b[5]);
        });
    }

    @SuppressWarnings("deprecation") // overriding is fine; Mojang deprecates these only for callers
    @Override
    public VoxelShape getOutlineShape(BlockState state, BlockView world, BlockPos pos, ShapeContext ctx) {
        return shape(state);
    }

    @SuppressWarnings("deprecation")
    @Override
    public VoxelShape getCollisionShape(BlockState state, BlockView world, BlockPos pos, ShapeContext ctx) {
        return spec.def.noCollision() ? VoxelShapes.empty() : shape(state);
    }

    // ── Water ─────────────────────────────────────────────────────────────────

    @SuppressWarnings("deprecation")
    @Override
    public FluidState getFluidState(BlockState state) {
        return spec.waterlogged != null && state.get(spec.waterlogged) ? Fluids.WATER.getStill(false) : super.getFluidState(state);
    }

    @SuppressWarnings("deprecation")
    @Override
    public BlockState getStateForNeighborUpdate(BlockState state, Direction direction, BlockState neighborState,
                                                WorldAccess world, BlockPos pos, BlockPos neighborPos) {
        if (spec.waterlogged != null && state.get(spec.waterlogged)) {
            world.scheduleFluidTick(pos, Fluids.WATER, Fluids.WATER.getTickRate(world));
        }
        return super.getStateForNeighborUpdate(state, direction, neighborState, world, pos, neighborPos);
    }

    // ── Redstone ──────────────────────────────────────────────────────────────

    @SuppressWarnings("deprecation")
    @Override
    public void neighborUpdate(BlockState state, World world, BlockPos pos, Block sourceBlock, BlockPos sourcePos, boolean notify) {
        super.neighborUpdate(state, world, pos, sourceBlock, sourcePos, notify);
        if (world.isClient || spec.def.powered() == null) return;
        BooleanProperty powered = yesNo(spec.def.powered());
        boolean now = world.isReceivingRedstonePower(pos);
        if (state.get(powered) != now) world.setBlockState(pos, state.with(powered, now), Block.NOTIFY_ALL);
    }

    @SuppressWarnings("deprecation")
    @Override
    public boolean hasComparatorOutput(BlockState state) {
        return spec.def.comparator() != null;
    }

    @SuppressWarnings("deprecation")
    @Override
    public int getComparatorOutput(BlockState state, World world, BlockPos pos) {
        return spec.def.comparator() != null && state.get(yesNo(spec.def.comparator())) ? 15 : 0;
    }

    // ── Structures turn and mirror it ─────────────────────────────────────────

    @SuppressWarnings("deprecation")
    @Override
    public BlockState rotate(BlockState state, BlockRotation rotation) {
        return spec.facing == null ? state : state.with(spec.facing, rotation.rotate(state.get(spec.facing)));
    }

    @SuppressWarnings("deprecation")
    @Override
    public BlockState mirror(BlockState state, BlockMirror mirror) {
        return spec.facing == null ? state : state.rotate(mirror.getRotation(state.get(spec.facing)));
    }

    /** A property of named values, in the order the .yblock lists them. */
    private static final class Named extends Property<String> {
        private final List<String> values;

        Named(String name, List<String> values) {
            super(name, String.class);
            this.values = List.copyOf(values);
        }

        @Override
        public Collection<String> getValues() {
            return values;
        }

        @Override
        public String name(String value) {
            return value;
        }

        @Override
        public Optional<String> parse(String name) {
            return values.contains(name) ? Optional.of(name) : Optional.empty();
        }
    }
}
