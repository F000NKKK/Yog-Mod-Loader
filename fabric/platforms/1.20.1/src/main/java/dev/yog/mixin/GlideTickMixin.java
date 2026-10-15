package dev.yog.mixin;

import dev.yog.util.YogGlide;
import net.minecraft.entity.LivingEntity;
import net.minecraft.entity.effect.StatusEffects;
import net.minecraft.entity.player.PlayerEntity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** A glider keeps gliding without an elytra in its chest slot (and wears none down). */
@Mixin(LivingEntity.class)
public class GlideTickMixin {

    @Inject(method = "tickFallFlying", at = @At("HEAD"), cancellable = true)
    private void yog$glide(CallbackInfo ci) {
        if (!((Object) this instanceof PlayerEntity self)) return;
        if (YogGlide.can(self.getUuid(), self.getWorld().isClient) && self.isFallFlying() && !self.isOnGround()
                && !self.hasVehicle() && !self.hasStatusEffect(StatusEffects.LEVITATION)) {
            ci.cancel();
        }
    }
}
