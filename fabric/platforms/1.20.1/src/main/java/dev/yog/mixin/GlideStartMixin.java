package dev.yog.mixin;

import dev.yog.util.YogGlide;
import net.minecraft.entity.effect.StatusEffects;
import net.minecraft.entity.player.PlayerEntity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** A glider ({@code entity_set_glider}) starts gliding as if it wore an elytra. */
@Mixin(PlayerEntity.class)
public class GlideStartMixin {

    @Inject(method = "checkFallFlying", at = @At("HEAD"), cancellable = true)
    private void yog$glide(CallbackInfoReturnable<Boolean> cir) {
        PlayerEntity self = (PlayerEntity) (Object) this;
        if (!YogGlide.can(self.getUuid(), self.getWorld().isClient)) return;
        if (!self.isOnGround() && !self.isFallFlying() && !self.isTouchingWater()
                && !self.hasStatusEffect(StatusEffects.LEVITATION)) {
            self.startFallFlying();
            cir.setReturnValue(true);
        }
    }
}
