package dev.yog.mixin;

import dev.yog.NativeBridge;
import net.minecraft.entity.Entity;
import net.minecraft.entity.projectile.ProjectileEntity;
import net.minecraft.registry.Registries;
import net.minecraft.util.hit.EntityHitResult;
import net.minecraft.util.hit.HitResult;
import net.minecraft.util.math.Vec3d;
import org.jetbrains.annotations.Nullable;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

@Mixin(ProjectileEntity.class)
public abstract class ProjectileHitMixin extends Entity {

    // field_33399 in ProjectileEntity
    @Shadow @Nullable protected Entity owner;

    protected ProjectileHitMixin(net.minecraft.entity.EntityType<?> type, net.minecraft.world.World world) {
        super(type, world);
    }

    @Inject(method = "onCollision", at = @At("HEAD"), cancellable = true)
    private void yog$onProjectileHit(HitResult hitResult, CallbackInfo ci) {
        if (hitResult.getType() == HitResult.Type.MISS) return;
        if (getWorld().isClient()) return;

        String projType = Registries.ENTITY_TYPE.getId(getType()).toString();
        String projUuid = getUuidAsString();
        String shooterUuid = owner != null ? owner.getUuidAsString() : "";
        String dim = getWorld().getRegistryKey().getValue().toString();

        String hitType;
        String hitEntityUuid;
        Vec3d pos = hitResult.getPos();
        if (hitResult instanceof EntityHitResult ehr) {
            hitType       = "entity";
            hitEntityUuid = ehr.getEntity().getUuidAsString();
        } else {
            hitType       = "block";
            hitEntityUuid = "";
        }

        boolean allow = NativeBridge.nativeOnProjectileHitPre(
                projType, projUuid, shooterUuid,
                hitType, hitEntityUuid,
                pos.x, pos.y, pos.z, dim);
        if (!allow) {
            ci.cancel();
            return;
        }
        NativeBridge.nativeOnProjectileHit(
                projType, projUuid, shooterUuid,
                hitType, hitEntityUuid,
                pos.x, pos.y, pos.z, dim);
    }
}
