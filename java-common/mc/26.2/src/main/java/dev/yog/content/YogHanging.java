package dev.yog.content;

import dev.yog.NativeBridge;
import dev.yog.YogNames;

import dev.yog.data.YogContent;
import dev.yog.bus.YogTypes;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.network.protocol.Packet;
import net.minecraft.network.protocol.game.ClientGamePacketListener;
import net.minecraft.network.protocol.game.ClientboundAddEntityPacket;
import net.minecraft.network.syncher.EntityDataAccessor;
import net.minecraft.network.syncher.EntityDataSerializers;
import net.minecraft.network.syncher.SynchedEntityData;
import net.minecraft.resources.Identifier;
import net.minecraft.server.level.ServerEntity;
import net.minecraft.sounds.SoundEvent;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.entity.decoration.HangingEntity;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.level.gamerules.GameRules;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.storage.ValueInput;
import net.minecraft.world.level.storage.ValueOutput;
import net.minecraft.world.phys.AABB;
import net.minecraft.world.phys.Vec3;

/**
 * A mod's hanging entity ({@code kind: hanging}): on a block's side as a
 * painting, {@code width}×{@code height} blocks, keeping the item it was
 * placed from — dropped when it breaks or loses its wall. The mods read how
 * it hangs from its entity data {@code yog:hanging} ({@link YogNames.Yog#HANGING}) and draw it.
 */
public class YogHanging extends HangingEntity {
    private static final EntityDataAccessor<Integer> WIDTH = SynchedEntityData.defineId(YogHanging.class, EntityDataSerializers.INT);
    private static final EntityDataAccessor<Integer> HEIGHT = SynchedEntityData.defineId(YogHanging.class, EntityDataSerializers.INT);
    private static final EntityDataAccessor<ItemStack> ITEM = SynchedEntityData.defineId(YogHanging.class, EntityDataSerializers.ITEM_STACK);

    public YogHanging(EntityType<? extends YogHanging> type, Level world) {
        super(type, world);
    }

    /** A new one at the block before {@code pos}; {@link #hang} says how. */
    public YogHanging(EntityType<? extends YogHanging> type, Level world, BlockPos pos) {
        super(type, world, pos);
    }

    /** Hangs it on its block's {@code facing} side, {@code size} blocks wide and high, keeping {@code item}. */
    public YogHanging hang(Direction facing, YogTypes.Hangs size, ItemStack item) {
        entityData.set(WIDTH, size.width());
        entityData.set(HEIGHT, size.height());
        entityData.set(ITEM, item);
        setDirection(facing);
        return this;
    }

    private YogTypes.EntityDef def() {
        return YogEntityTypes.def(getType());
    }

    @Override
    protected void defineSynchedData(SynchedEntityData.Builder builder) {
        super.defineSynchedData(builder);
        YogTypes.EntityDef def = def();
        builder.define(WIDTH, def == null ? 1 : Math.max(1, Math.round(def.width())));
        builder.define(HEIGHT, def == null ? 1 : Math.max(1, Math.round(def.height())));
        builder.define(ITEM, ItemStack.EMPTY);
    }

    @Override
    public void onSyncedDataUpdated(EntityDataAccessor<?> data) {
        if (WIDTH.equals(data) || HEIGHT.equals(data)) recalculateBoundingBox();
    }

    /** The item it keeps. */
    public ItemStack item() {
        return entityData.get(ITEM);
    }

    /** Tells the mods how it hangs (server side). */
    public void share() {
        if (level().isClientSide() || getDirection() == null) return;
        ItemStack item = item();
        NativeBridge.nativeEntityDataSet(getStringUUID(), YogNames.Yog.HANGING, new YogTypes.Hanging(getDirection().getSerializedName(),
                entityData.get(WIDTH), entityData.get(HEIGHT), BuiltInRegistries.ITEM.getKey(item.getItem()).toString(),
                NativeBridge.stackSnbt(item)).toFrame());
    }

    /** As a painting's: on the wall, a pixel thick, centred on its blocks. */
    @Override
    protected AABB calculateBoundingBox(BlockPos pos, Direction side) {
        int w = entityData.get(WIDTH), h = entityData.get(HEIGHT);
        Vec3 c = Vec3.atCenterOf(pos).relative(side, -dev.yog.util.YogGameNumbers.HANGING_DEPTH)
                .relative(side.getCounterClockWise(), w % 2 == 0 ? 0.5 : 0.0)
                .relative(Direction.UP, h % 2 == 0 ? 0.5 : 0.0);
        Direction.Axis axis = side.getAxis();
        return AABB.ofSize(c, axis == Direction.Axis.X ? dev.yog.util.YogGameNumbers.HANGING_THICKNESS : w, h, axis == Direction.Axis.Z ? dev.yog.util.YogGameNumbers.HANGING_THICKNESS : w);
    }

    private void play(String sound) {
        Identifier id = Identifier.tryParse(sound);
        if (id != null) playSound(SoundEvent.createVariableRangeEvent(id), 1.0F, 1.0F);
    }

    @Override
    public void dropItem(net.minecraft.server.level.ServerLevel level, Entity breaker) {
        if (!level.getGameRules().get(GameRules.ENTITY_DROPS)) return;
        YogTypes.EntityDef def = def();
        if (def != null) play(def.breakSound());
        if (breaker instanceof Player p && p.getAbilities().instabuild) return;
        if (!item().isEmpty()) spawnAtLocation(level, item().copy());
    }

    @Override
    public void playPlacementSound() {
        YogTypes.EntityDef def = def();
        if (def != null) play(def.placeSound());
    }

    @Override
    public void tick() {
        super.tick();
        YogEntityTypes.tick(this);
    }

    @Override
    protected void addAdditionalSaveData(ValueOutput out) {
        out.putByte("facing", (byte) getDirection().get2DDataValue());
        out.putInt("width", entityData.get(WIDTH));
        out.putInt("height", entityData.get(HEIGHT));
        if (!item().isEmpty()) out.store(dev.yog.YogNames.Kind.ITEM, ItemStack.CODEC, item());
        super.addAdditionalSaveData(out);
    }

    @Override
    protected void readAdditionalSaveData(ValueInput in) {
        entityData.set(WIDTH, Math.max(1, in.getIntOr("width", 1)));
        entityData.set(HEIGHT, Math.max(1, in.getIntOr("height", 1)));
        entityData.set(ITEM, in.read(dev.yog.YogNames.Kind.ITEM, ItemStack.CODEC).orElse(ItemStack.EMPTY));
        Direction facing = Direction.from2DDataValue(in.getByteOr("facing", (byte) 0));
        super.readAdditionalSaveData(in);
        setDirection(facing);
        share();
    }

    // As a painting: it stays on its block, wherever the packets say it is.
    @Override
    public void snapTo(double x, double y, double z, float yaw, float pitch) {
        setPos(x, y, z);
    }

    @Override
    public Vec3 trackingPosition() {
        return Vec3.atLowerCornerOf(pos);
    }

    @Override
    public Packet<ClientGamePacketListener> getAddEntityPacket(ServerEntity entry) {
        return new ClientboundAddEntityPacket(this, getDirection().get3DDataValue(), getPos());
    }

    @Override
    public void recreateFromPacket(ClientboundAddEntityPacket packet) {
        super.recreateFromPacket(packet);
        setDirection(Direction.from3DDataValue(packet.getData()));
    }

    @Override
    public ItemStack getPickResult() {
        return item().isEmpty() ? null : item().copy();
    }
}
