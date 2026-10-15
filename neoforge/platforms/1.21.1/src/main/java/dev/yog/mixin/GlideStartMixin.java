package dev.yog.mixin;

import dev.yog.util.YogGlide;
import net.minecraft.world.effect.MobEffects;
import net.minecraft.world.entity.player.Player;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** A glider ({@code entity_set_glider}) starts gliding as if it wore an elytra. */
@Mixin(Player.class)
public class GlideStartMixin {

    @Inject(method = "tryToStartFallFlying", at = @At("HEAD"), cancellable = true)
    private void yog$glide(CallbackInfoReturnable<Boolean> cir) {
        Player self = (Player) (Object) this;
        if (!YogGlide.can(self.getUUID(), self.level().isClientSide)) return;
        if (!self.onGround() && !self.isFallFlying() && !self.isInWater() && !self.hasEffect(MobEffects.LEVITATION)) {
            self.startFallFlying();
            cir.setReturnValue(true);
        }
    }
}
