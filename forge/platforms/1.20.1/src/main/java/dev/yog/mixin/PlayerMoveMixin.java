package dev.yog.mixin;

import dev.yog.NativeBridge;
import net.minecraft.network.protocol.game.ServerboundMovePlayerPacket;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.server.network.ServerGamePacketListenerImpl;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

@Mixin(ServerGamePacketListenerImpl.class)
public class PlayerMoveMixin {

    @Shadow public ServerPlayer player;

    @Inject(method = "handleMovePlayer", at = @At("HEAD"))
    private void yog$onPlayerMove(ServerboundMovePlayerPacket packet, CallbackInfo ci) {
        double x     = packet.getX(player.getX());
        double y     = packet.getY(player.getY());
        double z     = packet.getZ(player.getZ());
        float  yaw   = packet.getYRot(player.getYRot());
        float  pitch = packet.getXRot(player.getXRot());
        NativeBridge.nativeOnPlayerMove(
                player.getName().getString(), player.getStringUUID(),
                x, y, z, yaw, pitch);
    }
}
