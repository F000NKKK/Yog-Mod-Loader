package dev.yog.mixin;

import dev.yog.util.YogGlide;
import net.minecraft.world.effect.MobEffects;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.entity.player.Player;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** A glider keeps gliding without an elytra in its chest slot (and wears none down). */
@Mixin(LivingEntity.class)
public class GlideTickMixin {

    @Inject(method = "updateFallFlying", at = @At("HEAD"), cancellable = true)
    private void yog$glide(CallbackInfo ci) {
        if (!((Object) this instanceof Player self)) return;
        if (YogGlide.can(self.getUUID(), self.level().isClientSide) && self.isFallFlying() && !self.onGround()
                && !self.isPassenger() && !self.hasEffect(MobEffects.LEVITATION)) {
            ci.cancel();
        }
    }
}
