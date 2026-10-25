package dev.yog.mixin;

import net.minecraft.core.Registry;
import dev.yog.bus.YogEvents;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.entity.projectile.Projectile;
import net.minecraft.world.level.Level;
import net.minecraft.world.phys.EntityHitResult;
import net.minecraft.world.phys.HitResult;
import net.minecraft.world.phys.Vec3;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

@Mixin(Projectile.class)
@SuppressWarnings("deprecation") // BuiltInRegistries is the vanilla registry; Forge only nudges towards its wrappers
public abstract class ProjectileHitMixin extends Entity {

    protected ProjectileHitMixin(EntityType<?> type, Level level) {
        super(type, level);
    }

    @Inject(method = "onHit", at = @At("HEAD"), cancellable = true)
    private void yog$onProjectileHit(HitResult hitResult, CallbackInfo ci) {
        if (hitResult.getType() == HitResult.Type.MISS) return;
        if (getLevel().isClientSide()) return;

        Projectile self = (Projectile)(Object)this;
        String projType = Registry.ENTITY_TYPE.getKey(getType()).toString();
        String projUuid = getStringUUID();
        Entity owner = self.getOwner();
        String shooterUuid = owner != null ? owner.getStringUUID() : "";
        String dim = getLevel().dimension().location().toString();

        String hitType;
        String hitEntityUuid;
        Vec3 pos = hitResult.getLocation();
        if (hitResult instanceof EntityHitResult ehr) {
            hitType       = dev.yog.YogNames.Kind.ENTITY;
            hitEntityUuid = ehr.getEntity().getStringUUID();
        } else {
            hitType       = dev.yog.YogNames.Kind.BLOCK;
            hitEntityUuid = "";
        }

        boolean allow = new YogEvents.Entity.Combat.ProjectileHit(projType, projUuid, shooterUuid, hitType, hitEntityUuid, pos.x, pos.y, pos.z, dim).emitPre();
        if (!allow) {
            ci.cancel();
            return;
        }
        new YogEvents.Entity.Combat.ProjectileHit(projType, projUuid, shooterUuid, hitType, hitEntityUuid, pos.x, pos.y, pos.z, dim).emit();
    }
}
