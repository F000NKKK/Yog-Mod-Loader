package dev.yog;

import dev.yog.data.YogContent;
import dev.yog.util.YogHangingWire;
import net.minecraft.entity.Entity;
import net.minecraft.entity.EntityType;
import net.minecraft.entity.data.DataTracker;
import net.minecraft.entity.data.TrackedData;
import net.minecraft.entity.data.TrackedDataHandlerRegistry;
import net.minecraft.entity.decoration.AbstractDecorationEntity;
import net.minecraft.entity.player.PlayerEntity;
import net.minecraft.item.ItemStack;
import net.minecraft.nbt.NbtCompound;
import net.minecraft.network.listener.ClientPlayPacketListener;
import net.minecraft.network.packet.Packet;
import net.minecraft.network.packet.s2c.play.EntitySpawnS2CPacket;
import net.minecraft.registry.Registries;
import net.minecraft.sound.SoundEvent;
import net.minecraft.util.Identifier;
import net.minecraft.util.math.BlockPos;
import net.minecraft.util.math.Direction;
import net.minecraft.util.math.Vec3d;
import net.minecraft.world.GameRules;
import net.minecraft.world.World;

/**
 * A mod's hanging entity ({@code kind: hanging}): on a block's side as a
 * painting, {@code width}×{@code height} blocks, keeping the item it was
 * placed from — dropped when it breaks or loses its wall. The mods read how
 * it hangs from its entity data {@link YogHangingWire#KEY} and draw it.
 */
public class YogHanging extends AbstractDecorationEntity {
    private static final TrackedData<Integer> WIDTH = DataTracker.registerData(YogHanging.class, TrackedDataHandlerRegistry.INTEGER);
    private static final TrackedData<Integer> HEIGHT = DataTracker.registerData(YogHanging.class, TrackedDataHandlerRegistry.INTEGER);
    private static final TrackedData<ItemStack> ITEM = DataTracker.registerData(YogHanging.class, TrackedDataHandlerRegistry.ITEM_STACK);

    public YogHanging(EntityType<? extends YogHanging> type, World world) {
        super(type, world);
    }

    /** A new one on the {@code facing} side of the block before {@code pos}, keeping {@code item}. */
    public YogHanging(EntityType<? extends YogHanging> type, World world, BlockPos pos, Direction facing, int width, int height, ItemStack item) {
        super(type, world, pos);
        dataTracker.set(WIDTH, width);
        dataTracker.set(HEIGHT, height);
        dataTracker.set(ITEM, item);
        setFacing(facing);
    }

    private YogContent.Entity def() {
        return YogEntityTypes.def(getType());
    }

    @Override
    protected void initDataTracker() {
        YogContent.Entity def = def();
        dataTracker.startTracking(WIDTH, def == null ? 1 : Math.max(1, Math.round(def.width())));
        dataTracker.startTracking(HEIGHT, def == null ? 1 : Math.max(1, Math.round(def.height())));
        dataTracker.startTracking(ITEM, ItemStack.EMPTY);
    }

    @Override
    public void onTrackedDataSet(TrackedData<?> data) {
        if (WIDTH.equals(data) || HEIGHT.equals(data)) updateAttachmentPosition();
    }

    /** The item it keeps. */
    public ItemStack item() {
        return dataTracker.get(ITEM);
    }

    /** Tells the mods how it hangs (server side). */
    public void share() {
        if (getWorld().isClient || facing == null) return;
        ItemStack item = item();
        NativeBridge.nativeEntityDataSet(getUuidAsString(), YogHangingWire.KEY, YogHangingWire.encode(facing.asString(),
                dataTracker.get(WIDTH), dataTracker.get(HEIGHT), Registries.ITEM.getId(item.getItem()).toString(),
                NativeBridge.stackSnbt(item)));
    }

    @Override
    public int getWidthPixels() {
        return 16 * dataTracker.get(WIDTH);
    }

    @Override
    public int getHeightPixels() {
        return 16 * dataTracker.get(HEIGHT);
    }

    private void play(String sound) {
        Identifier id = Identifier.tryParse(sound);
        if (id != null) playSound(SoundEvent.of(id), 1.0F, 1.0F);
    }

    @Override
    public void onBreak(Entity breaker) {
        if (!getWorld().getGameRules().getBoolean(GameRules.DO_ENTITY_DROPS)) return;
        YogContent.Entity def = def();
        if (def != null) play(def.breakSound());
        if (breaker instanceof PlayerEntity p && p.getAbilities().creativeMode) return;
        if (!item().isEmpty()) dropStack(item().copy());
    }

    @Override
    public void onPlace() {
        YogContent.Entity def = def();
        if (def != null) play(def.placeSound());
    }

    @Override
    public void tick() {
        super.tick();
        YogEntityTypes.tick(this);
    }

    @Override
    public void writeCustomDataToNbt(NbtCompound nbt) {
        nbt.putByte("facing", (byte) facing.getHorizontal());
        nbt.putInt("width", dataTracker.get(WIDTH));
        nbt.putInt("height", dataTracker.get(HEIGHT));
        nbt.put("item", item().writeNbt(new NbtCompound()));
        super.writeCustomDataToNbt(nbt);
    }

    @Override
    public void readCustomDataFromNbt(NbtCompound nbt) {
        if (nbt.contains("width")) dataTracker.set(WIDTH, Math.max(1, nbt.getInt("width")));
        if (nbt.contains("height")) dataTracker.set(HEIGHT, Math.max(1, nbt.getInt("height")));
        dataTracker.set(ITEM, ItemStack.fromNbt(nbt.getCompound("item")));
        Direction facing = Direction.fromHorizontal(nbt.getByte("facing"));
        super.readCustomDataFromNbt(nbt);
        setFacing(facing);
        share();
    }

    // As a painting: it stays on its block, wherever the packets say it is.
    @Override
    public void refreshPositionAndAngles(double x, double y, double z, float yaw, float pitch) {
        setPosition(x, y, z);
    }

    @Override
    public void updateTrackedPositionAndAngles(double x, double y, double z, float yaw, float pitch, int steps, boolean interpolate) {
        setPosition(x, y, z);
    }

    @Override
    public Vec3d getSyncedPos() {
        return Vec3d.of(attachmentPos);
    }

    @Override
    public Packet<ClientPlayPacketListener> createSpawnPacket() {
        return new EntitySpawnS2CPacket(this, facing.getId(), getDecorationBlockPos());
    }

    @Override
    public void onSpawnPacket(EntitySpawnS2CPacket packet) {
        super.onSpawnPacket(packet);
        setFacing(Direction.byId(packet.getEntityData()));
    }

    @Override
    public ItemStack getPickBlockStack() {
        return item().isEmpty() ? null : item().copy();
    }
}
