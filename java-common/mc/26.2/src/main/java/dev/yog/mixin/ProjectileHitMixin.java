package dev.yog.mixin;

import dev.yog.bus.YogEvents;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.projectile.Projectile;
import net.minecraft.world.phys.EntityHitResult;
import net.minecraft.world.phys.HitResult;
import net.minecraft.world.phys.Vec3;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

@Mixin(Projectile.class)
public abstract class ProjectileHitMixin extends Entity {

    protected ProjectileHitMixin(net.minecraft.world.entity.EntityType<?> type, net.minecraft.world.level.Level world) {
        super(type, world);
    }

    @Inject(method = "onHit", at = @At("HEAD"), cancellable = true)
    private void yog$onProjectileHit(HitResult hitResult, CallbackInfo ci) {
        if (hitResult.getType() == HitResult.Type.MISS) return;
        if (level().isClientSide()) return;

        String projType = BuiltInRegistries.ENTITY_TYPE.getKey(getType()).toString();
        String projUuid = getStringUUID();
        Entity owner = ((Projectile) (Object) this).getOwner();
        String shooterUuid = owner != null ? owner.getStringUUID() : "";
        String dim = level().dimension().identifier().toString();

        String hitType;
        String hitEntityUuid;
        Vec3 pos = hitResult.getLocation();
        if (hitResult instanceof EntityHitResult ehr) {
            hitType       = "entity";
            hitEntityUuid = ehr.getEntity().getStringUUID();
        } else {
            hitType       = "block";
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
