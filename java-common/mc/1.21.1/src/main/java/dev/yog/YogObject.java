package dev.yog;

import dev.yog.data.YogContent;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.world.damagesource.DamageSource;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.entity.MoverType;
import net.minecraft.world.level.Level;

/**
 * A mod's object entity ({@code kind: object}): falls unless it mustn't,
 * can be hit and used (the mods' entity events), keeps its data (entity
 * data, saved by the loader); everything else is the mod's.
 */
public class YogObject extends Entity {
    public YogObject(EntityType<? extends YogObject> type, Level level) {
        super(type, level);
        YogContent.Entity def = YogEntityTypes.def(type);
        if (def != null) setNoGravity(def.noGravity());
    }

    @Override
    protected void defineSynchedData(net.minecraft.network.syncher.SynchedEntityData.Builder builder) {}

    @Override
    protected void readAdditionalSaveData(CompoundTag tag) {}

    @Override
    protected void addAdditionalSaveData(CompoundTag tag) {}

    @Override
    public void tick() {
        super.tick();
        if (!isNoGravity()) setDeltaMovement(getDeltaMovement().add(0.0, -dev.yog.util.YogGameNumbers.GRAVITY, 0.0));
        move(MoverType.SELF, getDeltaMovement());
        setDeltaMovement(getDeltaMovement().scale(onGround() ? dev.yog.util.YogGameNumbers.GROUND_DRAG : dev.yog.util.YogGameNumbers.AIR_DRAG));
        YogEntityTypes.tick(this);
    }

    @Override
    public boolean isPickable() {
        return !isRemoved();
    }

    @Override
    public boolean isPushable() {
        YogContent.Entity def = YogEntityTypes.def(getType());
        return def != null && Boolean.TRUE.equals(def.pushable());
    }

    @Override
    public boolean canBeCollidedWith() {
        YogContent.Entity def = YogEntityTypes.def(getType());
        return def != null && def.solid();
    }

    /** Hits go to the mods (entity events); the object itself takes none. */
    @Override
    public boolean hurt(DamageSource source, float amount) {
        return false;
    }
}
