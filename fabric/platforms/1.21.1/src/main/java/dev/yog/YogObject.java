package dev.yog;

import dev.yog.data.YogContent;
import net.minecraft.entity.Entity;
import net.minecraft.entity.EntityType;
import net.minecraft.entity.MovementType;
import net.minecraft.entity.damage.DamageSource;
import net.minecraft.nbt.NbtCompound;
import net.minecraft.world.World;

/**
 * A mod's object entity ({@code kind: object}): falls unless it mustn't,
 * can be hit and used (the mods' entity events), keeps its data (entity
 * data, saved by the loader); everything else is the mod's.
 */
public class YogObject extends Entity {
    public YogObject(EntityType<? extends YogObject> type, World world) {
        super(type, world);
        YogContent.Entity def = YogEntityTypes.def(type);
        if (def != null) setNoGravity(def.noGravity());
    }

    @Override
    protected void initDataTracker(net.minecraft.entity.data.DataTracker.Builder builder) {}

    @Override
    protected void readCustomDataFromNbt(NbtCompound nbt) {}

    @Override
    protected void writeCustomDataToNbt(NbtCompound nbt) {}

    @Override
    public void tick() {
        super.tick();
        if (!hasNoGravity()) setVelocity(getVelocity().add(0.0, -0.04, 0.0));
        move(MovementType.SELF, getVelocity());
        setVelocity(getVelocity().multiply(isOnGround() ? 0.5 : 0.98));
        YogEntityTypes.tick(this);
    }

    @Override
    public boolean canHit() {
        return !isRemoved();
    }

    @Override
    public boolean isPushable() {
        YogContent.Entity def = YogEntityTypes.def(getType());
        return def != null && Boolean.TRUE.equals(def.pushable());
    }

    @Override
    public boolean isCollidable() {
        YogContent.Entity def = YogEntityTypes.def(getType());
        return def != null && def.solid();
    }

    /** Hits go to the mods (entity events); the object itself takes none. */
    @Override
    public boolean damage(DamageSource source, float amount) {
        return false;
    }
}
