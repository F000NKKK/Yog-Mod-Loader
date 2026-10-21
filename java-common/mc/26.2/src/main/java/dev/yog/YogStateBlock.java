package dev.yog;

import dev.yog.bus.YogTypes;
import dev.yog.block.YogStateLogic;
import dev.yog.data.YogContent;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.concurrent.ConcurrentHashMap;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.world.WorldlyContainer;
import net.minecraft.world.WorldlyContainerHolder;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.context.BlockPlaceContext;
import net.minecraft.world.level.BlockGetter;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.LevelAccessor;
import net.minecraft.world.level.LevelReader;
import net.minecraft.world.level.ScheduledTickAccess;
import net.minecraft.util.RandomSource;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.FaceAttachedHorizontalDirectionalBlock;
import net.minecraft.world.level.block.Mirror;
import net.minecraft.world.level.block.Rotation;
import net.minecraft.world.level.block.SimpleWaterloggedBlock;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.block.state.StateDefinition;
import net.minecraft.world.level.block.state.properties.BlockStateProperties;
import net.minecraft.world.level.block.state.properties.BooleanProperty;
import net.minecraft.world.level.block.state.properties.EnumProperty;
import net.minecraft.world.level.block.state.properties.Property;
import net.minecraft.world.level.material.FluidState;
import net.minecraft.world.level.redstone.Orientation;
import net.minecraft.world.level.material.Fluids;
import net.minecraft.world.phys.shapes.CollisionContext;
import net.minecraft.world.phys.shapes.Shapes;
import net.minecraft.world.phys.shapes.VoxelShape;

/**
 * A Yog block with states of its own ({@code states} in its .yblock): it
 * turns when placed ({@code facing}, {@code placement}), hangs on a floor,
 * wall or ceiling ({@code attach}), stands in water ({@code waterlogged}),
 * and has the mod's own properties, set with {@code set_block_state}. A
 * yes/no property can follow redstone ({@code block.powered}) or feed a
 * comparator ({@code block.comparator}). Its shape turns with its model.
 */
public class YogStateBlock extends Block implements SimpleWaterloggedBlock, WorldlyContainerHolder {
    /** The properties a block being made has: they're added in {@code Block}'s constructor, before its own fields. */
    private static final ThreadLocal<Spec> PENDING = new ThreadLocal<>();

    private final Spec spec;
    private final Map<BlockState, VoxelShape> shapes = new ConcurrentHashMap<>();

    /** A block's properties. */
    private static final class Spec {
        final YogTypes.Block def;
        EnumProperty<Direction> facing;
        /** {@code floor}, {@code wall}, {@code ceiling} — named as a button's (the enum's name changed in 1.20.3). */
        Property<?> face;
        BooleanProperty waterlogged;
        final Map<String, Property<?>> own = new HashMap<>();
        final List<Property<?>> all = new ArrayList<>();

        Spec(YogTypes.Block def) {
            this.def = def;
            if (def.attach()) {
                facing = BlockStateProperties.HORIZONTAL_FACING;
                face = FaceAttachedHorizontalDirectionalBlock.FACE;
            } else if (YogTypes.Facing.ALL.equals(def.facing())) {
                facing = BlockStateProperties.FACING;
            } else if (YogTypes.Facing.HORIZONTAL.equals(def.facing())) {
                facing = BlockStateProperties.HORIZONTAL_FACING;
            }
            if (def.waterlogged()) waterlogged = BlockStateProperties.WATERLOGGED;
            if (facing != null) all.add(facing);
            if (face != null) all.add(face);
            if (waterlogged != null) all.add(waterlogged);
            for (YogTypes.BlockProperty p : def.properties()) {
                Property<?> made = YogContent.yesNo(p) ? BooleanProperty.create(p.name()) : new Named(p.name(), p.values());
                own.put(p.name(), made);
                all.add(made);
            }
        }
    }

    public static YogStateBlock create(Properties settings, YogTypes.Block def) {
        PENDING.set(new Spec(def));
        try {
            return new YogStateBlock(settings);
        } finally {
            PENDING.remove();
        }
    }

    private YogStateBlock(Properties settings) {
        super(settings);
        this.spec = PENDING.get();
        // The first value of each property, as the .yblock lists them.
        BlockState state = getStateDefinition().any();
        if (spec.facing != null) state = state.setValue(spec.facing, Direction.NORTH);
        if (spec.face != null) state = withFirst(state, spec.face, YogNames.BlockState.FLOOR);
        if (spec.waterlogged != null) state = state.setValue(spec.waterlogged, false);
        for (YogTypes.BlockProperty p : spec.def.properties()) state = withFirst(state, spec.own.get(p.name()), p.values().get(0));
        registerDefaultState(state);
    }

    /** {@code state} with property {@code p} at the value named {@code value}. */
    private static <T extends Comparable<T>> BlockState withFirst(BlockState state, Property<T> p, String value) {
        return p.getValue(value).map(v -> state.setValue(p, v)).orElse(state);
    }

    @Override
    protected void createBlockStateDefinition(StateDefinition.Builder<Block, BlockState> builder) {
        for (Property<?> p : PENDING.get().all) builder.add(p);
    }

    @Override
    public BlockState getStateForPlacement(BlockPlaceContext ctx) {
        BlockState state = defaultBlockState();
        boolean toward = !YogTypes.Placement.AWAY.equals(spec.def.placement());
        if (spec.face != null) {
            Direction look = ctx.getNearestLookingDirections()[0];
            if (look.getAxis() == Direction.Axis.Y) {
                Direction horizontal = ctx.getHorizontalDirection();
                state = withFirst(state, spec.face, look == Direction.UP ? YogNames.BlockState.CEILING : YogNames.BlockState.FLOOR)
                        .setValue(spec.facing, toward ? horizontal.getOpposite() : horizontal);
            } else {
                state = withFirst(state, spec.face, YogNames.BlockState.WALL).setValue(spec.facing, look.getOpposite());
            }
        } else if (spec.facing == BlockStateProperties.FACING) {
            Direction look = ctx.getNearestLookingDirection();
            state = state.setValue(spec.facing, toward ? look.getOpposite() : look);
        } else if (spec.facing != null) {
            Direction look = ctx.getHorizontalDirection();
            state = state.setValue(spec.facing, toward ? look.getOpposite() : look);
        }
        if (spec.waterlogged != null) {
            state = state.setValue(spec.waterlogged, ctx.getLevel().getFluidState(ctx.getClickedPos()).getType() == Fluids.WATER);
        }
        if (spec.def.powered() != null) {
            state = state.setValue(yesNo(spec.def.powered()), ctx.getLevel().hasNeighborSignal(ctx.getClickedPos()));
        }
        return state;
    }

    private static <T extends Comparable<T>> String named(BlockState state, Property<T> p) {
        return p.getName(state.getValue(p));
    }

    private BooleanProperty yesNo(String name) {
        return (BooleanProperty) spec.own.get(name);
    }

    // ── Shape ─────────────────────────────────────────────────────────────────

    private VoxelShape shape(BlockState state) {
        float[] box = spec.def.shape();
        if (box == null) return Shapes.block();
        return shapes.computeIfAbsent(state, s -> {
            String facing = spec.facing == null ? null : s.getValue(spec.facing).getSerializedName();
            String face = spec.face == null ? null : named(s, spec.face);
            int[] turn = YogStateLogic.turn(facing, face);
            double[] b = YogStateLogic.turnBox(new double[] {box[0], box[1], box[2], box[3], box[4], box[5]}, turn[0], turn[1]);
            return box(b[0], b[1], b[2], b[3], b[4], b[5]);
        });
    }

    @SuppressWarnings("deprecation") // overriding is fine; Mojang deprecates these only for callers
    @Override
    public VoxelShape getShape(BlockState state, BlockGetter world, BlockPos pos, CollisionContext ctx) {
        return shape(state);
    }

    @SuppressWarnings("deprecation") // overriding is fine; Mojang deprecates these only for callers
    @Override
    public VoxelShape getCollisionShape(BlockState state, BlockGetter world, BlockPos pos, CollisionContext ctx) {
        return spec.def.noCollision() ? Shapes.empty() : shape(state);
    }

    // ── Water ─────────────────────────────────────────────────────────────────

    @SuppressWarnings("deprecation") // overriding is fine; Mojang deprecates these only for callers
    @Override
    public FluidState getFluidState(BlockState state) {
        return spec.waterlogged != null && state.getValue(spec.waterlogged) ? Fluids.WATER.getSource(false) : super.getFluidState(state);
    }

    @SuppressWarnings("deprecation") // overriding is fine; Mojang deprecates these only for callers
    @Override
    protected BlockState updateShape(BlockState state, LevelReader world, ScheduledTickAccess ticks, BlockPos pos,
                                     Direction direction, BlockPos neighborPos, BlockState neighborState, RandomSource random) {
        if (spec.waterlogged != null && state.getValue(spec.waterlogged)) {
            ticks.scheduleTick(pos, Fluids.WATER, Fluids.WATER.getTickDelay(world));
        }
        return super.updateShape(state, world, ticks, pos, direction, neighborPos, neighborState, random);
    }

    // ── Redstone ──────────────────────────────────────────────────────────────

    @SuppressWarnings("deprecation") // overriding is fine; Mojang deprecates these only for callers
    @Override
    protected void neighborChanged(BlockState state, Level world, BlockPos pos, Block sourceBlock, Orientation orientation, boolean notify) {
        super.neighborChanged(state, world, pos, sourceBlock, orientation, notify);
        if (world.isClientSide() || spec.def.powered() == null) return;
        BooleanProperty powered = yesNo(spec.def.powered());
        boolean now = world.hasNeighborSignal(pos);
        if (state.getValue(powered) != now) world.setBlock(pos, state.setValue(powered, now), Block.UPDATE_ALL);
    }

    @SuppressWarnings("deprecation") // overriding is fine; Mojang deprecates these only for callers
    @Override
    public boolean hasAnalogOutputSignal(BlockState state) {
        return spec.def.comparator() != null;
    }

    @SuppressWarnings("deprecation") // overriding is fine; Mojang deprecates these only for callers
    @Override
    protected int getAnalogOutputSignal(BlockState state, Level world, BlockPos pos, Direction direction) {
        return spec.def.comparator() != null && state.getValue(yesNo(spec.def.comparator())) ? 15 : 0;
    }

    // ── Structures turn and mirror it ─────────────────────────────────────────

    @SuppressWarnings("deprecation") // overriding is fine; Mojang deprecates these only for callers
    @Override
    public BlockState rotate(BlockState state, Rotation rotation) {
        return spec.facing == null ? state : state.setValue(spec.facing, rotation.rotate(state.getValue(spec.facing)));
    }

    @SuppressWarnings("deprecation") // overriding is fine; Mojang deprecates these only for callers
    @Override
    public BlockState mirror(BlockState state, Mirror mirror) {
        return spec.facing == null ? state : state.rotate(mirror.getRotation(state.getValue(spec.facing)));
    }

    // ── Hoppers and droppers put items into it ({@code block.accepts_items}) ──

    @Override
    public WorldlyContainer getContainer(BlockState state, LevelAccessor world, BlockPos pos) {
        return spec.def.acceptsItems() && world instanceof Level w && !w.isClientSide() ? new Input(w, pos) : null;
    }

    /** One slot that's always empty: what's put in goes to the mod ({@code on_item_insert}). */
    private record Input(Level world, BlockPos pos) implements WorldlyContainer {
        private static final int[] SLOTS = {0};

        @Override
        public int[] getSlotsForFace(Direction side) {
            return SLOTS;
        }

        @Override
        public boolean canPlaceItem(int slot, ItemStack stack) {
            return !stack.isEmpty() && NativeBridge.itemInsert(world, pos, stack, true);
        }

        @Override
        public boolean canPlaceItemThroughFace(int slot, ItemStack stack, Direction side) {
            return canPlaceItem(slot, stack);
        }

        @Override
        public boolean canTakeItemThroughFace(int slot, ItemStack stack, Direction side) {
            return false;
        }

        @Override
        public void setItem(int slot, ItemStack stack) {
            if (!stack.isEmpty()) NativeBridge.itemInsert(world, pos, stack, false);
        }

        @Override
        public int getContainerSize() {
            return 1;
        }

        @Override
        public boolean isEmpty() {
            return true;
        }

        @Override
        public ItemStack getItem(int slot) {
            return ItemStack.EMPTY;
        }

        @Override
        public ItemStack removeItem(int slot, int amount) {
            return ItemStack.EMPTY;
        }

        @Override
        public ItemStack removeItemNoUpdate(int slot) {
            return ItemStack.EMPTY;
        }

        @Override
        public void setChanged() {
        }

        @Override
        public boolean stillValid(Player player) {
            return false;
        }

        @Override
        public void clearContent() {
        }
    }

    /** A property of named values, in the order the .yblock lists them. */
    private static final class Named extends Property<String> {
        private final List<String> values;

        Named(String name, List<String> values) {
            super(name, String.class);
            this.values = List.copyOf(values);
        }

        @Override
        public List<String> getPossibleValues() {
            return values;
        }

        @Override
        public String getName(String value) {
            return value;
        }

        @Override
        public int getInternalIndex(String value) {
            return values.indexOf(value);
        }

        @Override
        public Optional<String> getValue(String name) {
            return values.contains(name) ? Optional.of(name) : Optional.empty();
        }
    }
}
