package dev.yog.mixin;

import dev.yog.bus.YogEvents;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.level.ServerExplosion;
import net.minecraft.world.phys.Vec3;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** An explosion goes past the mods before it breaks or hurts anything. */
@Mixin(ServerExplosion.class)
public class ExplosionMixin {
    @Inject(method = "explode", at = @At("HEAD"), cancellable = true)
    private void yog$onExplosion(CallbackInfoReturnable<Integer> cir) {
        ServerExplosion self = (ServerExplosion) (Object) this;
        String dim = self.level().dimension().identifier().toString();
        Vec3 at = self.center();
        Entity cause = self.getDirectSourceEntity();
        String causeUuid = cause != null ? cause.getStringUUID() : "";
        if (!new YogEvents.Explosion(dim, at.x, at.y, at.z, self.radius(), causeUuid).emitPre()) {
            cir.setReturnValue(0);
            return;
        }
        new YogEvents.Explosion(dim, at.x, at.y, at.z, self.radius(), causeUuid).emit();
    }
}
