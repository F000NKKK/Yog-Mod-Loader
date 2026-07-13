package dev.yog.mixin;

import dev.yog.NativeBridge;
import net.minecraft.network.packet.c2s.play.PlayerMoveC2SPacket;
import net.minecraft.server.network.ServerPlayNetworkHandler;
import net.minecraft.server.network.ServerPlayerEntity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

@Mixin(ServerPlayNetworkHandler.class)
public class PlayerMoveMixin {

    @Shadow public ServerPlayerEntity player;

    @Inject(method = "onPlayerMove", at = @At("HEAD"))
    private void yog$onPlayerMove(PlayerMoveC2SPacket packet, CallbackInfo ci) {
        double x     = packet.getX(player.getX());
        double y     = packet.getY(player.getY());
        double z     = packet.getZ(player.getZ());
        float  yaw   = packet.getYaw(player.getYaw());
        float  pitch = packet.getPitch(player.getPitch());
        NativeBridge.nativeOnPlayerMove(
                player.getName().getString(), player.getUuidAsString(),
                x, y, z, yaw, pitch,
                player.getWorld().getRegistryKey().getValue().toString());
    }
}
