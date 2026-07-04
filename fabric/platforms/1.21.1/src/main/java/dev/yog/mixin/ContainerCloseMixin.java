package dev.yog.mixin;

import dev.yog.NativeBridge;
import net.minecraft.network.packet.c2s.play.CloseHandledScreenC2SPacket;
import net.minecraft.server.network.ServerPlayNetworkHandler;
import net.minecraft.server.network.ServerPlayerEntity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

@Mixin(ServerPlayNetworkHandler.class)
public class ContainerCloseMixin {

    @Shadow public ServerPlayerEntity player;

    @Inject(method = "onCloseHandledScreen", at = @At("HEAD"))
    private void yog$onContainerClose(CloseHandledScreenC2SPacket packet, CallbackInfo ci) {
        NativeBridge.nativeOnContainerClose(
                player.getName().getString(), player.getUuidAsString());
    }
}
