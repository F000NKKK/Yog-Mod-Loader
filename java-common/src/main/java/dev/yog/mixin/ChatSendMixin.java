package dev.yog.mixin;

import dev.yog.bus.YogEvents;
import net.minecraft.client.multiplayer.ClientPacketListener;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** What the player sends to the chat goes past the mods first; one may keep it from the server. */
@Mixin(ClientPacketListener.class)
public abstract class ChatSendMixin {
    @Inject(method = "sendChat", at = @At("HEAD"), cancellable = true)
    private void yog$onChat(String text, CallbackInfo ci) {
        if (!new YogEvents.Client.Chat.Send(text).emitPre()) ci.cancel();
    }
}
