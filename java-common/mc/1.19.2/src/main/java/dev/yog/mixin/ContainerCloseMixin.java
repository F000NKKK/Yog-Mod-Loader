package dev.yog.mixin;

import dev.yog.bus.YogEvents;
import net.minecraft.network.protocol.game.ServerboundContainerClosePacket;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.server.network.ServerGamePacketListenerImpl;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

@Mixin(ServerGamePacketListenerImpl.class)
public class ContainerCloseMixin {

    @Shadow public ServerPlayer player;

    @Inject(method = "handleContainerClose", at = @At("HEAD"))
    private void yog$onContainerClose(ServerboundContainerClosePacket packet, CallbackInfo ci) {
        new YogEvents.Container.Close(player.getName().getString(), player.getStringUUID(), player.getLevel().dimension().location().toString()).emit();
    }
}
