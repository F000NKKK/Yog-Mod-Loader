package dev.yog.content;

import net.minecraft.core.Registry;
import dev.yog.NativeBridge;
import dev.yog.YogNames;

import dev.yog.data.YogContent;
import dev.yog.bus.YogTypes;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.network.protocol.Packet;
import net.minecraft.network.protocol.game.ClientGamePacketListener;
import net.minecraft.network.protocol.game.ClientboundAddEntityPacket;
import net.minecraft.network.syncher.EntityDataAccessor;
import net.minecraft.network.syncher.EntityDataSerializers;
import net.minecraft.network.syncher.SynchedEntityData;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.sounds.SoundEvent;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.entity.decoration.HangingEntity;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.level.GameRules;
import net.minecraft.world.level.Level;
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

    public YogHanging(EntityType<? extends YogHanging> type, Level level) {
        super(type, level);
    }

    /** A new one at the block before {@code pos}; {@link #hang} says how. */
    public YogHanging(EntityType<? extends YogHanging> type, Level level, BlockPos pos) {
        super(type, level, pos);
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
    protected void defineSynchedData() {
        YogTypes.EntityDef def = def();
        entityData.define(WIDTH, def == null ? 1 : Math.max(1, Math.round(def.width())));
        entityData.define(HEIGHT, def == null ? 1 : Math.max(1, Math.round(def.height())));
        entityData.define(ITEM, ItemStack.EMPTY);
    }

    @Override
    public void onSyncedDataUpdated(EntityDataAccessor<?> key) {
        if (WIDTH.equals(key) || HEIGHT.equals(key)) recalculateBoundingBox();
    }

    /** The item it keeps. */
    public ItemStack item() {
        return entityData.get(ITEM);
    }

    /** Tells the mods how it hangs (server side). */
    public void share() {
        if (getLevel().isClientSide || direction == null) return;
        ItemStack item = item();
        NativeBridge.nativeEntityDataSet(getStringUUID(), YogNames.Yog.HANGING, new YogTypes.Hanging(direction.getSerializedName(),
                entityData.get(WIDTH), entityData.get(HEIGHT), Registry.ITEM.getKey(item.getItem()).toString(),
                NativeBridge.stackSnbt(item)).toFrame());
    }

    @Override
    public int getWidth() {
        return 16 * entityData.get(WIDTH);
    }

    @Override
    public int getHeight() {
        return 16 * entityData.get(HEIGHT);
    }

    private void play(String sound) {
        ResourceLocation id = ResourceLocation.tryParse(sound);
        if (id != null) playSound(SoundEvent.createVariableRangeEvent(id), 1.0F, 1.0F);
    }

    @Override
    public void dropItem(Entity breaker) {
        if (!getLevel().getGameRules().getBoolean(GameRules.RULE_DOENTITYDROPS)) return;
        YogTypes.EntityDef def = def();
        if (def != null) play(def.breakSound());
        if (breaker instanceof Player p && p.getAbilities().instabuild) return;
        if (!item().isEmpty()) spawnAtLocation(item().copy());
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
    public void addAdditionalSaveData(CompoundTag tag) {
        tag.putByte("facing", (byte) direction.get2DDataValue());
        tag.putInt("width", entityData.get(WIDTH));
        tag.putInt("height", entityData.get(HEIGHT));
        tag.put(dev.yog.YogNames.Kind.ITEM, item().save(new CompoundTag()));
        super.addAdditionalSaveData(tag);
    }

    @Override
    public void readAdditionalSaveData(CompoundTag tag) {
        if (tag.contains("width")) entityData.set(WIDTH, Math.max(1, tag.getInt("width")));
        if (tag.contains("height")) entityData.set(HEIGHT, Math.max(1, tag.getInt("height")));
        entityData.set(ITEM, ItemStack.of(tag.getCompound(dev.yog.YogNames.Kind.ITEM)));
        Direction facing = Direction.from2DDataValue(tag.getByte("facing"));
        super.readAdditionalSaveData(tag);
        setDirection(facing);
        share();
    }

    // As a painting: it stays on its block, wherever the packets say it is.
    @Override
    public void moveTo(double x, double y, double z, float yaw, float pitch) {
        setPos(x, y, z);
    }

    @Override
    public void lerpTo(double x, double y, double z, float yaw, float pitch, int steps, boolean teleport) {
        setPos(x, y, z);
    }

    @Override
    public Vec3 trackingPosition() {
        return Vec3.atLowerCornerOf(pos);
    }

    @Override
    public Packet<ClientGamePacketListener> getAddEntityPacket() {
        return new ClientboundAddEntityPacket(this, direction.get3DDataValue(), getPos());
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
