package dev.yog.mixin;

import dev.yog.util.YogGlide;
import net.minecraft.client.network.ClientPlayerEntity;
import net.minecraft.network.packet.c2s.play.ClientCommandC2SPacket;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * The local player, if a glider, starts gliding when they press jump in the
 * air — the check vanilla makes only for an elytra in the chest slot.
 */
@Mixin(ClientPlayerEntity.class)
public class GlideClientMixin {
    @Unique private boolean yog$wasJumping;
    @Unique private boolean yog$wasOnGround;

    @Inject(method = "tickMovement", at = @At("HEAD"))
    private void yog$before(CallbackInfo ci) {
        ClientPlayerEntity self = (ClientPlayerEntity) (Object) this;
        yog$wasJumping = self.input.jumping;
        yog$wasOnGround = self.isOnGround();
    }

    @Inject(method = "tickMovement", at = @At("RETURN"))
    private void yog$glide(CallbackInfo ci) {
        ClientPlayerEntity self = (ClientPlayerEntity) (Object) this;
        if (!self.input.jumping || yog$wasJumping || yog$wasOnGround) return;
        if (self.isFallFlying() || self.hasVehicle() || self.isClimbing() || self.getAbilities().flying) return;
        if (!YogGlide.can(self.getUuid(), true)) return;
        if (self.checkFallFlying()) {
            self.networkHandler.sendPacket(new ClientCommandC2SPacket(self, ClientCommandC2SPacket.Mode.START_FALL_FLYING));
        }
    }
}
