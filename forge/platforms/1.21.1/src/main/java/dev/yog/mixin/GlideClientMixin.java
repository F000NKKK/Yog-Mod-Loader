package dev.yog.mixin;

import dev.yog.util.YogGlide;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.network.protocol.game.ServerboundPlayerCommandPacket;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * The local player, if a glider, starts gliding when they press jump in the
 * air — the check vanilla makes only for an elytra in the chest slot.
 */
@Mixin(LocalPlayer.class)
public class GlideClientMixin {
    @Unique private boolean yog$wasJumping;
    @Unique private boolean yog$wasOnGround;

    @Inject(method = "aiStep", at = @At("HEAD"))
    private void yog$before(CallbackInfo ci) {
        LocalPlayer self = (LocalPlayer) (Object) this;
        yog$wasJumping = self.input.jumping;
        yog$wasOnGround = self.onGround();
    }

    @Inject(method = "aiStep", at = @At("RETURN"))
    private void yog$glide(CallbackInfo ci) {
        LocalPlayer self = (LocalPlayer) (Object) this;
        if (!self.input.jumping || yog$wasJumping || yog$wasOnGround) return;
        if (self.isFallFlying() || self.isPassenger() || self.onClimbable() || self.getAbilities().flying) return;
        if (!YogGlide.can(self.getUUID(), true)) return;
        if (self.tryToStartFallFlying()) {
            self.connection.send(new ServerboundPlayerCommandPacket(self, ServerboundPlayerCommandPacket.Action.START_FALL_FLYING));
        }
    }
}
