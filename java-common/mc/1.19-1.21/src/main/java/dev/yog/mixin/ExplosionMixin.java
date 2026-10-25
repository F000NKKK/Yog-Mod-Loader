package dev.yog.mixin;

import dev.yog.bus.YogEvents;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.level.Explosion;
import net.minecraft.world.level.Level;
import org.jetbrains.annotations.Nullable;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

@Mixin(Explosion.class)
public class ExplosionMixin {

    @Shadow @Final private Level level;
    @Shadow @Final private double x;
    @Shadow @Final private double y;
    @Shadow @Final private double z;
    @Shadow @Final private float radius;
    @Shadow @Final @Nullable private Entity source;

    @Inject(method = "explode()V", at = @At("HEAD"), cancellable = true)
    private void yog$onExplosion(CallbackInfo ci) {
        String dim       = level.dimension().location().toString();
        String causeUuid = source != null ? source.getStringUUID() : "";
        if (!new YogEvents.World.Explosion(dim, x, y, z, radius, causeUuid).emitPre()) {
            ci.cancel();
            return;
        }
        new YogEvents.World.Explosion(dim, x, y, z, radius, causeUuid).emit();
    }
}
