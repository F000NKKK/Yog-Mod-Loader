package dev.yog.mixin;

import dev.yog.NativeBridge;
import net.minecraft.entity.Entity;
import net.minecraft.world.World;
import net.minecraft.world.explosion.Explosion;
import org.jetbrains.annotations.Nullable;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

@Mixin(Explosion.class)
public class ExplosionMixin {

    @Shadow private World world;
    @Shadow public double x;
    @Shadow public double y;
    @Shadow public double z;
    @Shadow public float power;
    @Shadow @Nullable public Entity entity;

    // Yarn: collectBlocksAndDamageEntities() -> void (method_8348)
    @Inject(method = "collectBlocksAndDamageEntities()V", at = @At("HEAD"), cancellable = true)
    private void yog$onExplosion(CallbackInfo ci) {
        String dim       = world.getRegistryKey().getValue().toString();
        String causeUuid = entity != null ? entity.getUuidAsString() : "";
        if (!NativeBridge.nativeOnExplosionPre(dim, x, y, z, power, causeUuid)) {
            ci.cancel();
            return;
        }
        NativeBridge.nativeOnExplosion(dim, x, y, z, power, causeUuid);
    }
}
